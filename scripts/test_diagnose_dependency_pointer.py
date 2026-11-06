"""Headless checks of diagnostic refusal and cleanup boundaries."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location('pointer_diagnostic', Path(__file__).with_name('diagnose-dependency-pointer.py'))
diag = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(diag)


class DiagnosticBoundaries(unittest.TestCase):
    def report(self, passed=True):
        return {'complete': passed, 'workers_joined': True, 'fixture_retired': True,
                'input': {'pointer_p95_ms': 90 if passed else 100.913134,
                          'input_p95_ms': 64.636408, 'navigation_p95_ms': 1686.810252,
                          'pointer_dispatch_bounds': {'p95_lower_ms': 89.044763, 'p95_upper_ms': 100.913134}}}

    def observed(self, report, error=None, verify_error=None, surfaces=('software',)):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / 'trial'
            def inspect(*args):
                destination.mkdir()
                (destination / 'report.json').write_text(json.dumps(report))
                (destination / 'dispatch-events.json').write_text('[]')
                (destination / 'execution.log').write_text('\n'.join('Skia renderer (skia backend ' + value + '; diagnostic)' for value in surfaces))
                if error:
                    raise error
                return report
            native = Mock()
            native.inspect.side_effect = inspect
            native.dispatch_protocol.return_value.verify_events.side_effect = verify_error
            result = diag.observe(native, Path('/unused/binary'), destination, Path('/unused/helper'))
            native.dispatch_protocol.return_value.verify_events.assert_called_once_with(report['input'], [])
            if error:
                self.assertEqual(json.loads((destination / 'diagnostic-error.json').read_text())['error'], str(error))
            return result

    def test_passing_trial_requires_witness_verification(self):
        self.assertTrue(self.observed(self.report())['original_gate_passed'])

    def test_numerical_failure_is_retained_after_positive_cleanup(self):
        result = self.observed(self.report(False), ValueError(diag.NUMERICAL_FAILURE))
        self.assertFalse(result['original_gate_passed'])
        self.assertEqual(result['pointer_p95_ms'], 100.913134)
        self.assertTrue(result['all_dispatch_witnesses_verified'])

    def test_unconfirmed_cleanup_blocks_next_trial(self):
        for key in ['workers_joined', 'fixture_retired']:
            with self.subTest(key=key):
                report = self.report(False)
                report[key] = False
                with self.assertRaisesRegex(ValueError, 'no next trial'):
                    self.observed(report, ValueError(diag.NUMERICAL_FAILURE))

    def test_unrelated_error_is_not_treated_as_a_budget_failure(self):
        with self.assertRaisesRegex(ValueError, 'non-numerical'):
            self.observed(self.report(False), TimeoutError('owned window deadline'))

    def test_missing_joined_callback_witness_blocks_next_trial(self):
        with self.assertRaisesRegex(ValueError, 'missing witness'):
            self.observed(self.report(False), ValueError(diag.NUMERICAL_FAILURE), ValueError('missing witness'))

    def test_false_complete_cannot_be_accepted_without_the_recorded_error(self):
        with self.assertRaisesRegex(ValueError, 'completion flag'):
            self.observed(self.report(False))

    def test_numerical_failure_still_requires_exact_software_surface(self):
        for surfaces in [(), ('wgpu',), ('software', 'wgpu')]:
            with self.subTest(surfaces=surfaces), self.assertRaisesRegex(ValueError, 'software rendering path'):
                self.observed(self.report(False), ValueError(diag.NUMERICAL_FAILURE), surfaces=surfaces)

    def test_host_guard_refuses_local_and_self_hosted_execution(self):
        env = {'GITHUB_ACTIONS': 'true', 'GITHUB_REPOSITORY': '01max/librairii', 'RUNNER_ENVIRONMENT': 'github-hosted'}
        with patch.dict(os.environ, env, clear=True), patch.object(diag.platform, 'system', return_value='Darwin'), patch.object(diag.platform, 'machine', return_value='x86_64'):
            diag.require_host()
            for key, value in [('GITHUB_ACTIONS', 'false'), ('GITHUB_REPOSITORY', 'another/repo'), ('RUNNER_ENVIRONMENT', 'self-hosted')]:
                with self.subTest(key=key), patch.dict(os.environ, {key: value}):
                    with self.assertRaisesRegex(ValueError, 'disposable GitHub-hosted'):
                        diag.require_host()
        with patch.dict(os.environ, env, clear=True), patch.object(diag.platform, 'system', return_value='Linux'):
            with self.assertRaises(ValueError):
                diag.require_host()


if __name__ == '__main__':
    unittest.main()
