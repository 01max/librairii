"""Exercise renderer orchestration without starting any native application."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('interaction', Path(__file__).resolve().parents[2] / 'scripts/verify-native-interaction.py')
interaction = importlib.util.module_from_spec(spec)
spec.loader.exec_module(interaction)


class RendererOutcomes(unittest.TestCase):
    def test_uncertain_cleanup_stops_before_other_native_process(self):
        for report in [None, [], {'workers_joined': False, 'fixture_retired': True},
                       {'workers_joined': True, 'fixture_retired': False},
                       {'workers_joined': 1, 'fixture_retired': True}]:
            with self.subTest(report=report), tempfile.TemporaryDirectory() as directory:
                seen = []
                def inspect(binary, destination, backend, helper):
                    seen.append(backend)
                    destination.mkdir()
                    if report is not None:
                        (destination / 'report.json').write_text(json.dumps(report))
                    raise ValueError('failed and custody uncertain')
                with patch.object(interaction.native, 'inspect', side_effect=inspect):
                    cases, failures = interaction.inspect_renderers(Path('unused'), Path(directory), None)
                self.assertEqual(seen, ['winit-skia'])
                self.assertEqual(len(failures), 1)
                self.assertFalse(cases[0]['complete'])

    def test_successful_renderers_return_both_results(self):
        with patch.object(interaction.native, 'inspect', return_value={'complete': True}) as inspect:
            cases, failures = interaction.inspect_renderers(Path('unused'), Path('unused'), None)
        self.assertEqual(inspect.call_count, 2)
        self.assertEqual(len(cases), 2)
        self.assertEqual(failures, [])

    def test_failed_gpu_gate_does_not_hide_software_after_positive_cleanup(self):
        seen = []
        with tempfile.TemporaryDirectory() as directory:
            def inspect(binary, destination, backend, helper):
                seen.append(backend)
                destination.mkdir()
                report = {'complete': backend == 'winit-skia-software', 'workers_joined': True, 'fixture_retired': True}
                (destination / 'report.json').write_text(json.dumps(report))
                if not report['complete']:
                    raise ValueError('retained native dispatch/completion gate failed')
                return report
            with patch.object(interaction.native, 'inspect', side_effect=inspect):
                try:
                    cases, failures = interaction.inspect_renderers(Path('unused'), Path(directory), None)
                except ValueError:
                    cases, failures = [], []
            self.assertEqual(seen, ['winit-skia', 'winit-skia-software'])
            self.assertEqual(len(failures), 1)
            self.assertFalse(all(case['complete'] for case in cases))


if __name__ == '__main__':
    unittest.main()
