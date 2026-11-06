"""Headless checks of diagnostic refusal and cleanup boundaries."""
import importlib.util
from contextlib import contextmanager
import json
import os
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
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


class ProfileBoundaries(unittest.TestCase):
    @contextmanager
    def fixture(self, profile='sampled stacks\n', waits=None):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            (destination / 'state.json').write_text(json.dumps({'pid': 8001}))
            helper = Path('/unused/input-helper')
            original = Mock()
            original.TimeoutExpired = diag.subprocess.TimeoutExpired
            original.STDOUT = diag.subprocess.STDOUT
            owned = Mock(pid=8000)
            owned.poll.return_value = None
            sampler = Mock(returncode=0)
            sampler.wait.side_effect = waits
            def launch(command, *args, **kwargs):
                if command[0] != '/usr/bin/sample':
                    return owned
                if profile is not None:
                    (destination / 'input-profile.txt').write_text(profile)
                return sampler
            original.Popen.side_effect = launch
            native = SimpleNamespace(subprocess=original)
            with patch.object(diag, 'require_host') as host, patch.object(diag.os, 'getpgid', return_value=8000), patch.object(diag.os, 'getpgrp', return_value=9000):
                yield SimpleNamespace(native=native, destination=destination, helper=helper,
                                      command=[str(helper), '8001', str(destination)], original=original,
                                      owned=owned, sampler=sampler, host=host)

    def launch(self, f):
        f.native.subprocess.Popen(['owned-app'], start_new_session=True)
        return f.native.subprocess.run(f.command, capture_output=True, timeout=240)

    def test_profiles_only_owned_app_and_restores_module(self):
        with self.fixture() as f:
            with diag.input_profile(f.native, f.destination, f.helper):
                result = self.launch(f)
            self.assertIs(result, f.original.run.return_value)
            f.host.assert_called_once_with()
            self.assertIs(f.native.subprocess, f.original)
            self.assertIsNot(diag.subprocess, f.original)
            f.original.run.assert_called_once_with(f.command, capture_output=True, timeout=240)
            command = f.original.Popen.call_args.args[0]
            self.assertEqual(command, ['/usr/bin/sample', '8001', '15', '10', '-file', str(f.destination / 'input-profile.txt')])
            f.sampler.wait.assert_called_once_with(timeout=25)
            f.sampler.terminate.assert_not_called()
            status = json.loads((f.destination / 'profile-status.json').read_text())
            self.assertTrue(status['complete'])
            self.assertFalse(status['acceptance_evidence'])

    def test_refuses_unowned_pid_and_unexpected_helper(self):
        for mode in ('wrong-helper', 'wrong-pid', 'missing-owner', 'retired-owner', 'wrong-group'):
            with self.subTest(mode=mode), self.fixture() as f:
                with self.assertRaisesRegex(ValueError, 'exact owned|observed owned'):
                    with diag.input_profile(f.native, f.destination, f.helper):
                        if mode != 'missing-owner':
                            f.native.subprocess.Popen(['owned-app'], start_new_session=True)
                        if mode == 'wrong-helper':
                            f.command[0] = '/unexpected/helper'
                        if mode == 'wrong-pid':
                            f.command[1] = '8101'
                        if mode == 'retired-owner':
                            f.owned.poll.return_value = 0
                        if mode == 'wrong-group':
                            f.owned.pid = 8100
                        f.native.subprocess.run(f.command)
                self.assertIs(f.native.subprocess, f.original)
                f.original.run.assert_not_called()
                self.assertFalse(any(c.args[0][0] == '/usr/bin/sample' for c in f.original.Popen.call_args_list))

    def test_timeout_retires_only_owned_profiler_and_refuses_result(self):
        timeout = diag.subprocess.TimeoutExpired('sample', 25)
        for waits, killed in (([timeout, 0], False), ([timeout, timeout, 0], True)):
            with self.subTest(killed=killed), self.fixture(waits=waits) as f:
                with self.assertRaisesRegex(ValueError, 'exceeded its deadline'):
                    with diag.input_profile(f.native, f.destination, f.helper):
                        self.launch(f)
                f.sampler.terminate.assert_called_once_with()
                self.assertEqual(f.sampler.kill.called, killed)
                f.owned.terminate.assert_not_called()
                f.owned.kill.assert_not_called()
                self.assertIs(f.native.subprocess, f.original)
                self.assertFalse(json.loads((f.destination / 'profile-status.json').read_text())['complete'])

    def test_requires_complete_bounded_profile(self):
        for profile in (None, '', 'x' * 16777217):
            with self.subTest(size=None if profile is None else len(profile)), self.fixture(profile=profile) as f:
                with self.assertRaisesRegex(ValueError, 'bounded input profile'):
                    with diag.input_profile(f.native, f.destination, f.helper):
                        self.launch(f)
                self.assertIs(f.native.subprocess, f.original)

    def test_no_helper_and_failed_sampler_cannot_complete(self):
        for invoke in (False, True):
            with self.subTest(invoke=invoke), self.fixture() as f:
                f.sampler.returncode = 1
                with self.assertRaisesRegex(ValueError, 'did not complete'):
                    with diag.input_profile(f.native, f.destination, f.helper):
                        if invoke:
                            self.launch(f)
                self.assertIs(f.native.subprocess, f.original)

    def test_preserves_observer_error_when_profile_also_fails(self):
        with self.fixture() as f:
            with self.assertRaisesRegex(RuntimeError, 'original observer failure'):
                with diag.input_profile(f.native, f.destination, f.helper):
                    raise RuntimeError('original observer failure')
            self.assertIs(f.native.subprocess, f.original)
            status = json.loads((f.destination / 'profile-status.json').read_text())
            self.assertTrue(status['observation_failed'])
            self.assertIn('did not complete', status['error'])


if __name__ == '__main__':
    unittest.main()
