"""A crash or timeout cannot establish denied runtime access."""
import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('clean_macos', pathlib.Path(__file__).parents[1] / 'check-clean-macos.py')
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class CleanMacOS(unittest.TestCase):
    def test_denial_requires_completed_actual_policy_error(self):
        # This is the real shared command-runner schema: timeouts are errors,
        # and successful capture does not add a timed_out field.
        run = {'returncode': 1, 'errors': [], 'output': 'cat: Operation not permitted'}
        self.assertTrue(observer.denied(run))
        for change in ({'returncode': 0}, {'returncode': None}, {'returncode': -9},
                {'returncode': True}, {'errors': ['capture error']}, {'timed_out': True},
                {'output': 'No such file or directory'}):
            self.assertFalse(observer.denied({**run, **change}))
