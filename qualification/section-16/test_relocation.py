"""Exercise the real wrapper's ownership/error paths without mounting a volume."""
import importlib.util
from contextlib import redirect_stdout
import io
import json
import plistlib
from pathlib import Path
from types import SimpleNamespace
import tempfile
import subprocess
import sys
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('relocation', Path(__file__).resolve().parents[2] / 'scripts/verify-native-relocation.py')
relocation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relocation)


class RelocationCustody(unittest.TestCase):
    def exercise(self, *, primary=True, cleanup=False, attach=False, receipt_copy=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            binary = root / 'rehearsal'
            binary.write_bytes(b'owned test executable')
            (root / 'scripts').mkdir()
            (root / 'scripts/verify-native-relocation.py').write_bytes(b'test driver')
            example = root / 'crates/librairii-storage/examples/relocation_handover.rs'
            example.parent.mkdir(parents=True)
            example.write_bytes(b'test example')
            work = root / 'owned-work'
            work.mkdir()
            if receipt_copy:
                (work / 'disposal.json').write_text('[]')
            old_stat = Path.stat

            def stat(path, *args, **kwargs):
                if path == work / 'mounted':
                    return SimpleNamespace(st_dev=old_stat(work).st_dev + 1)
                return old_stat(path, *args, **kwargs)

            def run(argv, **kwargs):
                if argv[0] == 'cargo':
                    return SimpleNamespace(stdout=json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'relocation_handover', 'kind': ['example']}, 'executable': str(binary)}))
                if argv[:2] == ['hdiutil', 'create']:
                    (work / 'owned.sparseimage').write_bytes(b'owned image')
                if argv[:2] == ['hdiutil', 'attach'] and attach:
                    raise RuntimeError('primary partial attach failure')
                if argv[0] == str(binary):
                    if primary:
                        raise RuntimeError('primary SQLite I/O failure')
                    return SimpleNamespace(stdout=json.dumps({'forced_termination': [{}] * 8, 'filesystem_failures': [{}, {}], 'volume_root': {'volume_root': 'exact_root_verified'}}))
                return SimpleNamespace(stdout='false\n' if argv[0] == 'git' and 'status' in argv else '')

            failure = None
            with patch.object(relocation, 'ROOT', root), patch.object(relocation.tempfile, 'mkdtemp', return_value=str(work)), patch.object(relocation.platform, 'system', return_value='Darwin'), patch.object(relocation, 'run', side_effect=run), patch.object(relocation, 'detach_owned_image', side_effect=RuntimeError('cleanup detach failure') if cleanup else None, return_value=False) as detach, patch.object(Path, 'stat', stat), redirect_stdout(io.StringIO()), patch.object(relocation.shutil, 'copy2', side_effect=OSError('receipt unavailable') if receipt_copy else None):
                try:
                    relocation.main()
                except Exception as error:
                    failure = error
            detach.assert_called_once_with(work / 'mounted', work / 'owned.sparseimage')
            if primary or cleanup or attach or receipt_copy:
                self.assertIsNotNone(failure)
                self.assertTrue(work.is_dir(), 'uncertain/failed rehearsal must retain its owned image and fixtures')
                self.assertEqual((work / 'owned.sparseimage').read_bytes(), b'owned image')
                receipt = json.loads((root / 'target/quality/native-relocation-failure.json').read_text())
                self.assertFalse(receipt['complete'])
                self.assertEqual(receipt['work'], str(work))
                self.assertEqual(receipt['owned_volume_detached'], not cleanup)
                if primary or attach:
                    self.assertIn('primary', receipt['primary_error'])
                if cleanup:
                    self.assertIn('cleanup detach failure', receipt['cleanup_error'])
                if receipt_copy:
                    self.assertIn('receipt', receipt['cleanup_error'] + str(receipt['cleanup_notes']))
                if primary:
                    self.assertIn('primary SQLite I/O failure', str(failure.__cause__))
                self.assertFalse((root / 'target/quality/native-relocation.json').exists())
            else:
                self.assertIsNone(failure)
                self.assertFalse(work.exists(), 'only a successful positively detached rehearsal may retire')
                self.assertTrue(json.loads((root / 'target/quality/native-relocation.json').read_text())['owned_volume_detached'])

    def test_primary_and_cleanup_errors_retain_image_and_original_cause(self):
        self.exercise(cleanup=True)

    def test_primary_error_retains_image_after_successful_detach(self):
        self.exercise()

    def test_partial_attach_failure_requires_detach_and_retains_image(self):
        self.exercise(primary=False, attach=True, cleanup=True)

    def test_success_retires_only_after_positive_detach(self):
        self.exercise(primary=False)

    def test_cleanup_failure_after_success_retains_image(self):
        self.exercise(primary=False, cleanup=True)

    def test_receipt_copy_failure_retains_primary_and_cleanup_errors(self):
        self.exercise(primary=True, cleanup=True, receipt_copy=True)
        self.exercise(primary=True, receipt_copy=True)
        self.exercise(primary=False, receipt_copy=True)

    def test_failed_real_child_retains_both_streams_and_original_status(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            child = work / 'failure.py'
            child.write_text('import sys\nprint("partial output", flush=True)\nprint("original disk I/O error", file=sys.stderr, flush=True)\nsys.exit(9)\n')
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                relocation.rehearse(Path(sys.executable), child, work, work)
            self.assertEqual(caught.exception.returncode, 9)
            self.assertEqual((work / 'rehearsal.stdout').read_text(), 'partial output\n')
            self.assertEqual((work / 'rehearsal.stderr').read_text(), 'original disk I/O error\n')

    def test_child_failure_survives_both_stream_retention_errors(self):
        failures = [subprocess.CalledProcessError(9, ['owned-child'], output='partial', stderr='failure'),
                    subprocess.TimeoutExpired(['owned-child'], 600, output=b'partial', stderr=b'failure')]
        for original in failures:
            with self.subTest(error=type(original).__name__), patch.object(relocation, 'run', side_effect=original), patch.object(Path, 'write_bytes', side_effect=OSError('receipt unavailable')) as write:
                with self.assertRaises(type(original)) as caught:
                    relocation.rehearse(Path('unused'), Path('unused'), Path('unused'), Path('unused'))
                self.assertIs(caught.exception, original)
                self.assertEqual(write.call_count, 2)
                self.assertEqual(len(original.__notes__), 2)


class ImageDisposal(unittest.TestCase):
    def test_busy_failure_retains_scoped_open_file_diagnostics_without_acceptance(self):
        self.exercise_busy_failure()

    def test_diagnostic_errors_preserve_original_busy_failure(self):
        for error in [FileNotFoundError('lsof unavailable'),
                      subprocess.TimeoutExpired(['lsof'], 10, output=b'partial', stderr=b'unavailable')]:
            with self.subTest(error=type(error).__name__):
                self.exercise_busy_failure(diagnostic_error=error)

    def test_changed_device_inventory_cannot_mask_original_busy_failure(self):
        self.exercise_busy_failure(malformed_inventory=True)

    def exercise_busy_failure(self, *, diagnostic_error=None, malformed_inventory=False):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            mount, image = work / 'mounted', work / 'owned.sparseimage'
            mount.mkdir()
            image.touch()
            devices = [{'content-hint': 'GUID_partition_scheme', 'dev-entry': '/dev/disk2'},
                       {'dev-entry': '/dev/disk3s1'}]
            calls = []

            def command(argv, **kwargs):
                calls.append(argv)
                if argv == ['hdiutil', 'info', '-plist']:
                    final_inventory = len([call for call in calls if call == argv]) == 8
                    rows = [{'image-path': str(image), 'system-entities': 'invalid' if malformed_inventory and final_inventory else devices},
                            {'image-path': '/foreign/image', 'system-entities': [{'dev-entry': '/dev/disk99'}]}]
                    return subprocess.CompletedProcess(argv, 0, plistlib.dumps({'images': rows}).decode(), '')
                if argv[0] == 'lsof':
                    self.assertEqual(argv, ['lsof', '-nP', '-Fpcfn', '--', str(image), '/dev/disk2', '/dev/disk3s1'])
                    self.assertEqual(kwargs['timeout'], 10)
                    if diagnostic_error:
                        raise diagnostic_error
                    return subprocess.CompletedProcess(argv, 1, 'p123\ncdiskimages-help\nn' + str(image) + '\n', 'partial visibility')
                return subprocess.CompletedProcess(argv, 16, '', 'hdiutil: couldn\'t eject "disk2" - Resource busy\n')

            with patch.object(relocation.subprocess, 'run', side_effect=command), patch.object(relocation.time, 'sleep'):
                with self.assertRaisesRegex(RuntimeError, 'remains attached'):
                    relocation.detach_owned_image(mount, image)
            receipt = json.loads((work / 'disposal.json').read_text())
            diagnostics = [row['open_files_diagnostic'] for row in receipt if 'open_files_diagnostic' in row]
            self.assertEqual(len(diagnostics), 1)
            if diagnostic_error or malformed_inventory:
                self.assertIn('error', diagnostics[0])
                if isinstance(diagnostic_error, subprocess.TimeoutExpired):
                    self.assertEqual(bytes.fromhex(diagnostics[0]['stdout_hex']), b'partial')
                    self.assertEqual(bytes.fromhex(diagnostics[0]['stderr_hex']), b'unavailable')
                if malformed_inventory:
                    self.assertFalse(any(argv[0] == 'lsof' for argv in calls))
            else:
                self.assertEqual(diagnostics[0]['returncode'], 1)
                self.assertIn('p123', diagnostics[0]['stdout'])
                self.assertEqual(diagnostics[0]['stderr'], 'partial visibility')
            self.assertEqual(len([argv for argv in calls if argv[:2] == ['hdiutil', 'detach']]), 4)
            self.assertNotIn('/dev/disk99', str(calls))

    def exercise(self, snapshots, exits, *, mounted=False, error=None):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            mount, image = work / 'mounted', work / 'owned.sparseimage'
            mount.mkdir()
            image.touch()
            calls = []
            snapshots = iter(snapshots)
            exits = iter(exits)

            def command(argv, **kwargs):
                calls.append(argv)
                if argv == ['hdiutil', 'info', '-plist']:
                    devices = next(snapshots)
                    rows = [{'image-path': '/unrelated/image.dmg', 'system-entities': [{'dev-entry': '/dev/disk99'}]}]
                    if devices is not None:
                        rows.append({'image-path': str(image), 'image-alias': b'opaque plist alias',
                                     'system-entities': devices})
                    return subprocess.CompletedProcess(argv, 0, plistlib.dumps({'images': rows}).decode(), '')
                return subprocess.CompletedProcess(argv, next(exits), '', 'simulated detach status')

            with patch.object(relocation.subprocess, 'run', side_effect=command), patch.object(relocation.time, 'sleep'), patch.object(relocation.os.path, 'ismount', return_value=mounted):
                if error:
                    with self.assertRaisesRegex(RuntimeError, error):
                        relocation.detach_owned_image(mount, image)
                    result = None
                else:
                    result = relocation.detach_owned_image(mount, image)
            receipt = json.loads((work / 'disposal.json').read_text())
            self.assertNotIn('/unrelated/', json.dumps(receipt))
            self.assertFalse(any('/dev/disk99' in c for c in calls))
            return result, calls

    def test_unmounted_but_attached_image_uses_its_backing_device(self):
        device = [{'content-hint': 'GUID_partition_scheme', 'dev-entry': '/dev/disk4'}]
        forced, calls = self.exercise([device, None], [0])
        self.assertFalse(forced)
        self.assertIn(['hdiutil', 'detach', '/dev/disk4'], calls)

    def test_failed_command_with_verified_disposal_is_not_a_false_failure(self):
        device = [{'content-hint': 'GUID_partition_scheme', 'dev-entry': '/dev/disk4'}]
        self.assertFalse(self.exercise([device, None], [1])[0])

    def test_success_exit_without_disposal_still_fails(self):
        device = [{'content-hint': 'GUID_partition_scheme', 'dev-entry': '/dev/disk4'}]
        self.exercise([device, device], [0], error='remains attached')

    def test_missing_inventory_match_does_not_override_mounted_path(self):
        self.exercise([None], [], mounted=True, error='mountpoint still uncertain')

    def test_ambiguous_or_missing_backing_device_refuses(self):
        for devices in [[], [{'dev-entry': '/dev/disk4'}],
                        [{'content-hint': 'GUID_partition_scheme', 'dev-entry': '/dev/disk4s1'}],
                        [{'content-hint': 'GUID_partition_scheme', 'dev-entry': '/dev/disk4'}] * 2]:
            with self.subTest(devices=devices):
                self.exercise([devices], [], error='unique owned image backing device')

    def test_busy_retry_refreshes_owned_identity_before_force(self):
        devices = [[{'content-hint': 'GUID_partition_scheme', 'dev-entry': f'/dev/disk{n}'}] for n in range(4, 8)]
        forced, calls = self.exercise([devices[0], devices[0], devices[1], devices[1],
                                      devices[2], devices[2], devices[3], None], [16, 16, 16, 0])
        self.assertTrue(forced)
        self.assertIn(['hdiutil', 'detach', '-force', '/dev/disk7'], calls)

    def test_already_detached_image_requires_positive_inventory_and_mount_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            mount = Path(directory) / 'mounted'
            mount.mkdir()
            image = Path(directory) / 'owned.sparseimage'
            image.touch()

            def command(argv, **kwargs):
                if argv == ['hdiutil', 'info', '-plist']:
                    return subprocess.CompletedProcess(argv, 0, plistlib.dumps({'images': []}).decode(), '')
                return subprocess.CompletedProcess(argv, 1, '', 'hdiutil: detach failed - No such file or directory')

            with patch.object(relocation.subprocess, 'run', side_effect=command):
                # Before the fix the wrapper refuses the stale mountpoint,
                # without consulting the available positive disposal evidence.
                self.assertFalse(relocation.detach_owned_image(mount, image))

    def test_receipt_write_failure_does_not_replace_cleanup_error(self):
        with tempfile.TemporaryDirectory() as directory:
            mount = Path(directory) / 'mounted'
            image = Path(directory) / 'owned.sparseimage'
            with patch.object(relocation, 'run', side_effect=RuntimeError('original inventory failure')), patch.object(Path, 'write_text', side_effect=OSError('receipt unavailable')):
                with self.assertRaisesRegex(RuntimeError, 'original inventory failure') as caught:
                    relocation.detach_owned_image(mount, image)
            self.assertIn('receipt unavailable', str(caught.exception.__notes__))


if __name__ == '__main__':
    unittest.main()
