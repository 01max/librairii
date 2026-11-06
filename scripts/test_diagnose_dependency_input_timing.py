"""Read-only trace validation; never launches an app or native helper."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('input_timing', Path(__file__).with_name('diagnose-dependency-input-timing.py'))
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


class TraceChecks(unittest.TestCase):
    def trace(self):
        return {'format': 'librairii.observer-timing-diagnostic', 'schema': 1,
                'acceptance_evidence': False, 'instrumented': True, 'overflowed': False,
                'columns': ['start', 'clock_files', 'resize_menus', 'projection', 'model_observation',
                            'import_page_offers', 'geometry_capture', 'native_window', 'state_publication', 'end'],
                'rows': [[1_000_000 * n for n in range(1, 11)],
                         [1_000_000 * n for n in range(30, 40)]],
                'dispatch_events_before_shutdown': [{'kind': 'search', 'sequence': 1}]}

    def read(self, value, events=None):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'trace.json'
            path.write_text(json.dumps(value))
            return diagnostic.read_timing(path, events if events is not None else value['dispatch_events_before_shutdown'])

    def test_keeps_tick_work_separate_from_between_tick_delay(self):
        result = self.read(self.trace())
        self.assertEqual(result['max_tick_ms'], 9)
        self.assertEqual(result['max_between_ticks_ms'], 20)
        self.assertEqual(result['phase_max_ms']['projection'], 1)
        self.assertFalse(result['acceptance_evidence'])

    def test_retains_early_return_without_fabricating_phase_measurements(self):
        value = self.trace()
        value['rows'][1] = [30_000_000] + [0] * 8 + [31_000_000]
        result = self.read(value)
        self.assertEqual(result['rows'], 2)
        self.assertEqual(result['phase_max_ms']['projection'], 1)

    def test_rejects_missing_samples_clock_reversal_overlap_and_boolean_time(self):
        for case in ('empty', 'overflow', 'backwards', 'overlap', 'boolean'):
            with self.subTest(case=case):
                value = self.trace()
                if case == 'empty': value['rows'] = []
                elif case == 'overflow': value['overflowed'] = True
                elif case == 'backwards': value['rows'][0][4] = 1
                elif case == 'overlap': value['rows'][1] = value['rows'][0]
                elif case == 'boolean': value['rows'][0][0] = True
                with self.assertRaises(ValueError): self.read(value)

    def test_pre_shutdown_witnesses_must_match_joined_actual_callbacks(self):
        with self.assertRaisesRegex(ValueError, 'joined callback'):
            self.read(self.trace(), [])


if __name__ == '__main__':
    unittest.main()
