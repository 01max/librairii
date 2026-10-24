"""Keep native units, sustained idle windows and incomplete runs honest."""
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import platform
import signal
import subprocess
import tarfile
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location('resources', pathlib.Path(__file__).parents[1] / 'measure-resources.py')
resources = importlib.util.module_from_spec(spec)
spec.loader.exec_module(resources)


class ResourceMeasurement(unittest.TestCase):
    def fixture(self, system='Darwin'):
        timing = 'real 12.10\nuser 0.30\nsys 0.20\n104857600 maximum resident set size\n'
        if system == 'Linux':
            timing = 'qualification_time; wall=12.10; user=0.30; system=0.20; maxrss_kib=102400\n'
        return {'returncode': 0, 'timed_out': False, 'errors': [], 'backend_request': 'winit-skia-software',
                'output': timing + 'Slint: Backend: Skia renderer (skia backend software; surface: 24 bpp)\n'
                          'thumbnail_smoke=passed;\nresource_probe_complete; workers_joined=true; fixture_retired=true\n',
                'ready_markers': 1, 'idle_end_markers': 1, 'idle_end_seconds': 10,
                'launch_to_ready_marker_ms': 150,
                'samples': [{'monotonic_seconds': 100 + n / 4, 'since_ready_seconds': n / 4,
                             'cpu_seconds': n / 40, 'rss_bytes': 90_000_000, 'collection_seconds': 0.002}
                            for n in range(0, 49)]}

    def test_native_peak_units_are_converted_to_bytes(self):
        for system in ('Darwin', 'Linux'):
            report = resources.analyze(self.fixture(system), system)
            self.assertEqual(report['measurement'], 'pass')
            self.assertEqual(report['native_time']['peak_rss_bytes'], 104_857_600)
            self.assertAlmostEqual(report['idle']['cpu_percent_one_core'], 10)
            self.assertEqual(report['idle']['wall_seconds'], 7.5)

    def test_settling_and_workload_outliers_do_not_enter_idle_delta(self):
        run = self.fixture()
        for sample in run['samples']:
            if sample['since_ready_seconds'] < 2:
                sample['cpu_seconds'] = 0
            if sample['since_ready_seconds'] > 9.5:
                sample['cpu_seconds'] = 1000
        report = resources.analyze(run, 'Darwin')
        self.assertEqual(report['measurement'], 'pass')
        self.assertAlmostEqual(report['idle']['cpu_percent_one_core'], 10)

    def test_short_slow_or_regressing_cpu_samples_fail(self):
        for defect in ('short', 'slow', 'backwards'):
            with self.subTest(defect=defect):
                run = self.fixture()
                if defect == 'short':
                    run['samples'] = run['samples'][:12]
                if defect == 'slow':
                    for sample in run['samples']:
                        sample['collection_seconds'] = 0.2
                if defect == 'backwards':
                    run['samples'][20]['cpu_seconds'] = 0
                self.assertEqual(resources.analyze(run, 'Darwin')['measurement'], 'fail')

    def test_partial_runs_cannot_pass(self):
        for key, value in [('returncode', 1), ('timed_out', True), ('errors', ['unowned PID']),
                           ('ready_markers', 0), ('ready_markers', 2), ('idle_end_markers', 0),
                           ('idle_end_seconds', None)]:
            with self.subTest(key=key, value=value):
                run = self.fixture()
                run[key] = value
                self.assertEqual(resources.analyze(run, 'Darwin')['measurement'], 'fail')
        for removed in ('maximum resident set size', 'thumbnail_smoke=passed;', 'resource_probe_complete;',
                        'Skia renderer (skia backend software;'):
            run = self.fixture()
            run['output'] = run['output'].replace(removed, 'missing')
            self.assertEqual(resources.analyze(run, 'Darwin')['measurement'], 'fail')

    def test_wrong_surface_is_not_a_successful_explicit_cpu_measurement(self):
        run = self.fixture()
        run['output'] = run['output'].replace('backend software;', 'backend opengl;')
        self.assertFalse(resources.analyze(run, 'Darwin')['checks']['requested_cpu_surface_selected'])

    def test_cpu_time_formats_include_fractional_minutes_hours_and_days(self):
        self.assertAlmostEqual(resources.cpu_seconds('01:02.34'), 62.34)
        self.assertEqual(resources.cpu_seconds('01:02:03'), 3723)
        self.assertEqual(resources.cpu_seconds('2-01:02:03'), 176523)

    def test_archive_source_mismatch_and_payload_tampering_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            package = root / 'probe'
            package.mkdir()
            (package / 'binary').write_bytes(b'authored inert fixture')
            manifest = {'source': 'expected', 'files': {'binary': hashlib.sha256(b'authored inert fixture').hexdigest()}}
            (package / 'manifest.json').write_text(json.dumps(manifest))
            expected = sum(path.stat().st_size for path in package.iterdir())
            self.assertEqual(resources.package_bytes(package, manifest), expected)
            archive = root / 'probe.tar.gz'
            with tarfile.open(archive, 'x:gz') as stream:
                data = json.dumps({'source': 'other'}).encode()
                member = tarfile.TarInfo('probe/manifest.json')
                member.size = len(data)
                stream.addfile(member, io.BytesIO(data))
            with self.assertRaisesRegex(ValueError, 'does not match'):
                resources.archive_identity(archive, package, manifest)
            (package / 'binary').write_bytes(b'tampered')
            with self.assertRaisesRegex(ValueError, 'bytes changed'):
                resources.package_bytes(package, manifest)

    def test_hung_probe_is_killed_and_retains_partial_output(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = pathlib.Path(directory) / 'authored-probe'
            binary.write_text('#!/usr/bin/env python3\nimport os,time\n'
                              'print(f"resource_probe_ready; pid={os.getpid()}; scale=1; size=1180x760", flush=True)\n'
                              'time.sleep(30)\n')
            binary.chmod(0o755)
            run = resources.run_probe(binary, 'winit-skia-software', platform.system(), timeout=0.3)
            self.assertTrue(run['timed_out'])
            self.assertIn('resource_probe_ready;', run['output'])
            self.assertEqual(run['analysis']['measurement'], 'fail')

    def test_distribution_requires_every_payload_member_once_with_exact_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            package = root / 'probe'
            package.mkdir()
            payload = b'authored inert executable bytes'
            manifest = {'source': 'expected', 'files': {'binary': hashlib.sha256(payload).hexdigest()}}
            manifest_bytes = json.dumps(manifest).encode()
            (package / 'manifest.json').write_bytes(manifest_bytes)
            (package / 'binary').write_bytes(payload)
            for variant in ('complete', 'missing', 'changed', 'duplicate', 'alias'):
                with self.subTest(variant=variant):
                    archive = root / (variant + '.tar.gz')
                    with tarfile.open(archive, 'x:gz') as stream:
                        entries = [('probe/manifest.json', manifest_bytes)]
                        if variant != 'missing':
                            entries.append(('probe/binary', payload if variant != 'changed' else b'wrong'))
                        if variant == 'duplicate':
                            entries.append(('probe/binary', payload))
                        for name, data in entries:
                            member = tarfile.TarInfo(name)
                            member.size = len(data)
                            stream.addfile(member, io.BytesIO(data))
                        if variant == 'alias':
                            member = tarfile.TarInfo('probe/alias')
                            member.type, member.linkname = tarfile.SYMTYPE, 'binary'
                            stream.addfile(member)
                    if variant == 'complete':
                        self.assertEqual(resources.archive_identity(archive, package, manifest)['bytes'], archive.stat().st_size)
                    else:
                        with self.assertRaises(ValueError):
                            resources.archive_identity(archive, package, manifest)

    def test_exited_wrapper_cannot_leave_a_running_descendant(self):
        for code in (1, 0):
            with self.subTest(exit_code=code), tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                pid_file = root / 'child.pid'
                binary = root / 'authored-probe'
                binary.write_text('#!/usr/bin/env python3\nimport subprocess,sys,pathlib\n'
                                  'child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(30)"],'
                                  'stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)\n'
                                  f'pathlib.Path({str(pid_file)!r}).write_text(str(child.pid))\n'
                                  f'raise SystemExit({code})\n')
                binary.chmod(0o755)
                run = resources.run_probe(binary, 'winit-skia-software', platform.system(), timeout=2)
                child = int(pid_file.read_text())
                stopped = False
                try:
                    deadline = time.monotonic() + 1
                    while time.monotonic() < deadline:
                        status = subprocess.run(['/bin/ps', '-p', str(child), '-o', 'stat='],
                                                capture_output=True, text=True, timeout=1).stdout.strip()
                        if not status or status.startswith('Z'):
                            stopped = True
                            break
                        time.sleep(0.01)
                finally:
                    try:
                        os.kill(child, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                self.assertTrue(stopped, 'owned descendant must stop even after wrapper exits')
                self.assertEqual(run['analysis']['measurement'], 'fail')


if __name__ == '__main__':
    unittest.main()
