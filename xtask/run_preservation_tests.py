"""Cumulative Python assertions: missing identities, skips and xfails fail closed."""
import json
from pathlib import Path
import unittest


def identities(suite):
    for test in suite:
        if isinstance(test, unittest.TestSuite):
            yield from identities(test)
        else:
            yield test.id()


def run_suite(suite, required, stream=None):
    discovered = list(identities(suite))
    if not required or len(set(required)) != len(required):
        raise ValueError('Python required test identities must be nonempty and unique')
    if len(set(discovered)) != len(discovered):
        raise ValueError('duplicate Python test identity')
    missing = set(required) - set(discovered)
    if missing:
        raise ValueError(f'missing required Python tests: {sorted(missing)}')
    result = unittest.TextTestRunner(stream=stream, verbosity=2).run(suite)
    if (not result.wasSuccessful() or result.skipped or result.expectedFailures
            or result.testsRun != len(discovered)):
        raise ValueError('Python tests must all execute successfully; skips/xfails/filtering are incomplete')
    return result


def main():
    root = Path(__file__).resolve().parent.parent
    baseline = json.loads((root / '.config/rust-python-test-baseline.json').read_text(encoding='utf-8'))
    suite = unittest.defaultTestLoader.discover(str(root / 'xtask/tests'), pattern='test_preservation.py')
    run_suite(suite, baseline['required'])


if __name__ == '__main__':
    main()
