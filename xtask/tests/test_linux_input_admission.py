"""Run Linux admission and retirement with inert processes and real protocol data."""
import copy
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch


ROOT = Path(__file__).resolve().parents[2]


def load(name):
    path = ROOT / 'qualification/section-16' / (name + '.py')
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


native = load('native')
fixtures = load('test_dispatch')
protocol = load('dispatch')


class LinuxInputAdmission(unittest.TestCase):
    def test_captured_linux_delay_keeps_failed_wall_and_callback_bound_separate(self):
        path = ROOT / 'qualification/section-16/fixtures/linux-keyboard-publication-delay.json'
        captured = json.loads(path.read_text())
        bounds = protocol.bounds(captured['samples'], captured['calibrations'], 'search')
        self.assertAlmostEqual(sorted(captured['original_wall_ms'])[60], 126.699703)
        self.assertGreater(captured['original_wall_p95_ms'], 100)
        self.assertAlmostEqual(bounds['p95_upper_ms'], 9.953264)
        self.assertEqual([row['wall_ms'] for row in bounds['samples']], captured['original_wall_ms'])

    def helper(self, callback=5, publication=200, posting=1):
        samples, clocks = fixtures.observations(callback, publication, posting)
        measured = fixtures.DispatchTests().helper()
        measured['clock_calibrations'] = clocks
        for prefix, kind, key in [('input', 'search', 'search'),
                                  ('pointer', 'browser-focus', 'browser_focus')]:
            rows = copy.deepcopy(samples)
            if prefix == 'pointer':
                for row in rows:
                    row['ack'].update(kind=kind, query='')
                    row['ack']['native_ns'] += 100 * 10**9
                    for field in ('start_ns', 'posted_ns', 'observed_ns'):
                        row[field] += 100 * 10**9
            measured[prefix + '_dispatch_samples'] = rows
            measured[prefix + '_states'] = [dict(
                dispatch={key: row['ack']}, query=row['ack']['query'], focus_key=31,
                running_import=True, active_imports=1, max_rows=24) for row in rows]
            measured[prefix + '_ms'] = [(r['observed_ns'] - r['start_ns']) / 1e6 for r in rows]
            measured[prefix + '_p95_ms'] = sorted(measured[prefix + '_ms'])[60]
        measured['navigation_ms'] = [500] * 20
        measured['navigation_p95_ms'] = 500
        measured['endpoints'] = [dict(running_import=True, active_imports=1, max_rows=24,
            total=1000, rows=24, selected_index=23, settled=True, first_cover_reloads=n+3)
            for n in range(20)]
        return measured

    def cohort(self, raw):
        def observe(pid, output, measured):
            measured.update(copy.deepcopy(raw))
            return measured
        return observe

    def test_linux_entrypoint_keeps_walls_and_gates_actual_dispatch(self):
        for callback, publication, posting in [(5, 200, 1), (205, 10, 1), (125, 10, 120), (305, 10, 1)]:
            raw = self.helper(callback, publication, posting)
            with self.subTest(callback=callback), tempfile.TemporaryDirectory() as directory:
                with patch.object(native, 'linux_input_cohort', side_effect=self.cohort(raw)):
                    measured = native.linux_input(123, Path(directory))
                self.assertEqual(measured['input_method_version'], 3)
                self.assertEqual(measured['input_observer_wall_ms'], raw['input_ms'])
                self.assertEqual(measured['pointer_observer_wall_ms'], raw['pointer_ms'])
                self.assertEqual(measured['navigation_ms'], raw['navigation_ms'])
                self.assertEqual(measured['input_p95_ms'] <= 300, callback <= 300)
                self.assertEqual(measured['pointer_p95_ms'] <= 300, callback <= 300)
                if callback <= 300:
                    native.verify_input_timings(measured)
                else:
                    with self.assertRaisesRegex(ValueError, 'gate failed'):
                        native.verify_input_timings(measured)
                self.assertEqual(json.loads((Path(directory) / 'linux-dispatch-raw.json').read_text()), raw)
        with patch.object(native.platform, 'system', return_value='Linux'):
            self.assertEqual(native.input_method_version(), 3)

    def test_linux_refuses_incomplete_clocks_and_keeps_original_observations(self):
        raw = self.helper()
        raw['clock_calibrations'].pop()
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(native, 'linux_input_cohort', side_effect=self.cohort(raw)):
                with self.assertRaisesRegex(ValueError, 'complete native dispatch'):
                    native.linux_input(123, Path(directory))
            self.assertEqual(json.loads((Path(directory) / 'linux-dispatch-raw.json').read_text()), raw)

    def test_linux_inspection_requires_matching_events_after_worker_join(self):
        raw = self.helper()
        events = [r['ack'] for key in ('input', 'pointer') for r in raw[key + '_dispatch_samples']]
        for n in range(42):
            events.append(dict(kind='search', sequence=n+65, query='x' if n % 2 == 0 else '',
                               native_ns=events[-1]['native_ns']+10**9))
        for corrupt in (False, True):
            with self.subTest(corrupt=corrupt), tempfile.TemporaryDirectory() as directory:
                root, output = Path(directory), Path(directory) / 'run'
                owner_root = root / 'owner'; owner_root.mkdir()
                owner = Mock(pid=99, returncode=None, stdin=io.StringIO(),
                             stdout=io.StringIO(json.dumps({'prepared': True, 'owner': str(owner_root)})+'\n'))
                process = Mock(pid=101, returncode=None)
                owner.poll.side_effect = lambda: owner.returncode
                process.poll.side_effect = lambda: process.returncode
                def retire(timeout):
                    owner_root.rmdir(); owner.returncode = 0; return 0
                owner.wait.side_effect = retire
                def join(timeout):
                    process.returncode = 0
                    (output / 'execution.log').write_text(
                        'Skia renderer (skia backend wgpu; surface: 32 bpp)\nscale_complete; workers_joined=true\n')
                    joined_events = copy.deepcopy(events)
                    if corrupt:
                        joined_events[2]['native_ns'] += 1
                    (output / 'dispatch-events.json').write_text(json.dumps(joined_events))
                    return 0
                process.wait.side_effect = join
                def state(path):
                    page = int((path / 'page').read_text()) if (path / 'page').exists() else 1
                    return dict(pid=102, running_import=True, page=page, settled=True,
                        rows=16 if page == 42 else 24, total=1000, first_cover_evicted=True,
                        loaded_covers=1000, max_rows=24, first_cover_reloads=1)
                def observe(pid, path, measured):
                    measured.update(copy.deepcopy(raw))
                    return measured
                with patch.object(native.subprocess, 'Popen', side_effect=[owner, process]), \
                     patch.object(native.os, 'getpgid', return_value=101), \
                     patch.object(native.platform, 'system', return_value='Linux'), \
                     patch.object(native, 'state', side_effect=state), \
                     patch.object(native, 'linux_input_cohort', side_effect=observe):
                    # The real inspect loop reads its ready marker and all42 pages.
                    original_mkdir = Path.mkdir
                    def mkdir(path, *args, **kwargs):
                        result = original_mkdir(path, *args, **kwargs)
                        if path == output:
                            (path / 'ready').touch()
                        return result
                    with patch.object(Path, 'mkdir', mkdir):
                        if corrupt:
                            with self.assertRaisesRegex(ValueError, 'absent from joined native log'):
                                native.inspect(Path('inert'), output, 'winit-skia')
                        else:
                            report = native.inspect(Path('inert'), output, 'winit-skia')
                            self.assertTrue(report['complete'])
                            self.assertTrue(report['dispatch_witnesses_verified_after_join'])
                            self.assertEqual(report['input_method_version'], 3)
                self.assertEqual(process.returncode, 0)
                self.assertFalse(owner_root.exists())


if __name__ == '__main__':
    unittest.main()
