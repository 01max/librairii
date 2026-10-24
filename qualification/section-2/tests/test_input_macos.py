"""Composition cannot pass without cleanup or by substituting committed text."""
import copy
import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('input_macos', pathlib.Path(__file__).parents[1] / 'check-input-macos.py')
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class InputMacOS(unittest.TestCase):
    def test_missing_or_failed_input_or_restoration_witness_refuses_qualification(self):
        report = {'binary_sha256': 'expected', 'errors': [], 'returncode': 0,
            'checks': dict.fromkeys(observer.REQUIRED, True), 'full_input_display_qualified': False}
        self.assertTrue(observer.passed(report, 'expected'))
        for key in observer.REQUIRED:
            changed = copy.deepcopy(report)
            del changed['checks'][key]
            self.assertFalse(observer.passed(changed, 'expected'))
            changed['checks'][key] = False
            self.assertFalse(observer.passed(changed, 'expected'))
        for changed in ({'binary_sha256': 'other'}, {'errors': ['restore failure']},
                {'returncode': 1}, {'full_input_display_qualified': True}):
            self.assertFalse(observer.passed({**report, **changed}, 'expected'))

    def test_source_inventory_requires_unique_identities_and_the_actual_current_source(self):
        source = {'id': 'original'}
        self.assertEqual(observer.sources_by_id({'current': 'original', 'sources': [source]}), {'original': source})
        for sources in ([], [source, source], [{'id': 'other'}]):
            with self.assertRaises(ValueError):
                observer.sources_by_id({'current': 'original', 'sources': sources})

    def test_restoration_covers_automatically_enabled_modes_and_checks_effective_state(self):
        # Enabling one method may also enable modes which the driver never
        # explicitly enabled. The original effective state must still return.
        original = {'id': 'original', 'enabled': True, 'enable_capable': True, 'type': 'Layout'}
        parent = {'id': 'parent', 'enabled': False, 'enable_capable': True, 'type': 'InputMethodModeEnabled'}
        mode = {'id': 'mode', 'enabled': False, 'enable_capable': True, 'type': 'InputMode'}
        before = {'current': 'original', 'sources': [original, parent, mode]}

        class Effective:
            def __init__(self, refuses_disable=False):
                self.value = copy.deepcopy(before)
                self.value['current'] = 'mode'
                for source in self.value['sources']:
                    source['enabled'] = True
                self.calls = []
                self.refuses_disable = refuses_disable

            def call(self, action, **request):
                self.calls.append((action, request))
                if action == 'input-source':
                    if request['operation'] == 'select':
                        self.value['current'] = request['source']
                    elif not self.refuses_disable:
                        next(s for s in self.value['sources'] if s['id'] == request['source'])['enabled'] = False
                return copy.deepcopy(self.value)

        effective = Effective()
        self.assertEqual(observer.restore_sources(effective, before), before)
        mutations = [value for action, value in effective.calls if action == 'input-source']
        self.assertEqual(mutations, [{'source': 'original', 'operation': 'select'},
            {'source': 'mode', 'operation': 'disable'}, {'source': 'parent', 'operation': 'disable'}])
        with self.assertRaisesRegex(ValueError, 'not fully restored'):
            observer.restore_sources(Effective(refuses_disable=True), before)
