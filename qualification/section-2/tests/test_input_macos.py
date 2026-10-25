"""Composition cannot pass without cleanup or by substituting committed text."""
import copy
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('input_macos', pathlib.Path(__file__).parents[1] / 'check-input-macos.py')
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class InputMacOS(unittest.TestCase):
    def test_selected_source_alone_cannot_supply_a_live_application_launch_notice(self):
        source = 'com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese'
        family = 'com.apple.inputmethod.Kotoeri.RomajiTyping'
        process = {'pid': 321, 'bundle': 'com.apple.JapaneseIM.RomajiTyping',
            'finished_launching': True, 'terminated': False}
        observed = {'current': source, 'source': {'id': source, 'selected': True, 'bundle': family},
            'processes': [process]}
        self.assertTrue(observer.japanese_launch_notice(observed, source))
        for change in ({'current': 'other'}, {'source': {**observed['source'], 'selected': False}},
                {'source': {**observed['source'], 'bundle': 'unrelated'}}, {'processes': []}):
            self.assertFalse(observer.japanese_launch_notice({**observed, **change}, source))
        for change in ({'pid': True}, {'pid': -1}, {'bundle': 'unrelated'},
                {'finished_launching': False}, {'terminated': True}):
            self.assertFalse(observer.japanese_launch_notice({**observed, 'processes': [{**process, **change}]}, source))

    def test_full_native_selection_requires_exact_foreground_field_text_and_utf16_range(self):
        text = 'êé 日本語 🦊'
        snapshot = {'pid': 12, 'frontmost_pid': 12, 'focused': {'role': 'AXTextField', 'value': text, 'focused': True},
            'selected_text': text, 'selection_observed': True, 'selection_location': 0,
            'selection_length': len(text.encode('utf-16-le')) // 2}
        self.assertTrue(observer.all_text_selected(snapshot, text, 12))
        for change in ({'pid': 13}, {'frontmost_pid': 13}, {'focused': {}},
                {'focused': {**snapshot['focused'], 'focused': False}},
                {'selected_text': text[:-1]}, {'selection_observed': False},
                {'selection_location': 1}, {'selection_length': len(text)}):
            self.assertFalse(observer.all_text_selected({**snapshot, **change}, text, 12))

    def test_initial_native_focus_absence_is_polled_without_repeating_a_menu_action(self):
        absent = {'pid': -1, 'focus_status': -25212, 'focused': {}, 'selection_observed': False}
        partial = {'pid': 12, 'frontmost_pid': 12, 'focused': {'role': 'AXTextField', 'value': 'êé az', 'focused': True},
            'selected_text': 'z', 'selection_observed': True, 'selection_location': 4, 'selection_length': 1}
        complete = {**partial, 'selected_text': 'êé az', 'selection_location': 0, 'selection_length': 5}

        class Effective:
            def __init__(self):
                self.calls = []
                self.values = iter([absent, partial, complete])

            def call(self, action):
                self.calls.append(action)
                return next(self.values)

            def wait(self, predicate):
                self.result = predicate()
                while not self.result:
                    self.result = predicate()
                return self.result

        effective, observations = Effective(), []
        self.assertEqual(observer.wait_for_selection(effective, observations, 'êé az', 12), complete)
        self.assertEqual(observations, [absent, partial, complete])
        self.assertEqual(effective.calls, ['input-selection'] * 3)

    @unittest.skipUnless(sys.platform == 'darwin', 'Carbon input-source lookup requires native macOS')
    def test_actual_missing_carbon_source_filter_is_empty_without_mutating_selection(self):
        # Compile the real read-only TIS helpers, excluding the later AX/event
        # command dispatcher. No helper settings mutation or UI action can run.
        source = pathlib.Path(__file__).parents[1] / 'native-macos/observer.swift'
        preamble = source.read_text().split('func attribute(', 1)[0]
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            fixture = root / 'main.swift'
            fixture.write_text(preamble + '''
let before = inputSource()
let absent = inputSources([kTISPropertyBundleID as String: "dev.librairii.qualification.missing-source"])
guard absent.isEmpty else { fatalError("missing family returned a source") }
let french = inputSources([kTISPropertyInputSourceID as String: "com.apple.keylayout.French"])
guard french.count == 1, before == inputSource() else { fatalError("source lookup changed selection") }
print("missing family empty; installed French present; selected source unchanged")
''')
            subprocess.run(['xcrun', 'swiftc', '-module-cache-path', str(root / 'modules'),
                str(fixture), '-o', str(root / 'probe')], check=True, capture_output=True, timeout=90)
            result = subprocess.run([str(root / 'probe')], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('selected source unchanged', result.stdout)

    def test_escape_requires_a_new_final_clear_after_the_live_preedit(self):
        live = 'Preedit("", None)\nPreedit("にほん", Some((0, 3)))\n'
        clear = 'Preedit("", None)\n'
        self.assertFalse(observer.composition_cancelled(live, ''))
        self.assertFalse(observer.composition_cancelled(live + clear + live, clear + live))
        self.assertFalse(observer.composition_cancelled(live + 'Commit("日本")\n' + clear, clear))
        self.assertTrue(observer.composition_cancelled(live + clear, clear))

    def test_missing_or_failed_input_or_restoration_witness_refuses_qualification(self):
        report = {'binary_sha256': 'expected', 'errors': [], 'returncode': 0,
            'checks': dict.fromkeys(observer.REQUIRED, True), 'full_input_display_qualified': False,
            'include_japanese': False}
        self.assertTrue(observer.passed(report, 'expected'))
        for key in observer.REQUIRED:
            changed = copy.deepcopy(report)
            del changed['checks'][key]
            self.assertFalse(observer.passed(changed, 'expected'))
            changed['checks'][key] = False
            self.assertFalse(observer.passed(changed, 'expected'))
        for changed in ({'binary_sha256': 'other'}, {'errors': ['restore failure']},
                {'returncode': 1}, {'full_input_display_qualified': True}, {'include_japanese': None}):
            self.assertFalse(observer.passed({**report, **changed}, 'expected'))

    def test_deferred_japanese_does_not_gate_latin_but_an_explicit_diagnostic_cannot_claim_missing_evidence(self):
        report = {'binary_sha256': 'expected', 'errors': [], 'returncode': 0,
            'checks': dict.fromkeys(observer.REQUIRED, True), 'full_input_display_qualified': False,
            'include_japanese': False}
        self.assertTrue(observer.passed(report, 'expected'))
        report['include_japanese'] = True
        self.assertFalse(observer.passed(report, 'expected'))
        report['checks'].update(dict.fromkeys(observer.JAPANESE_REQUIRED, True))
        self.assertTrue(observer.passed(report, 'expected'))
        for name in observer.JAPANESE_REQUIRED:
            changed = copy.deepcopy(report)
            del changed['checks'][name]
            self.assertFalse(observer.passed(changed, 'expected'))

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
