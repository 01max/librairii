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
    def test_full_native_selection_requires_exact_foreground_field_text_and_utf16_range(self):
        text = 'êé 日本語 🦊'
        snapshot = {'pid': 12, 'frontmost_pid': 12, 'focused': {'role': 'AXTextField', 'value': text},
            'selected_text': text, 'selection_observed': True, 'selection_location': 0,
            'selection_length': len(text.encode('utf-16-le')) // 2}
        self.assertTrue(observer.all_text_selected(snapshot, text, 12))
        for change in ({'pid': 13}, {'frontmost_pid': 13}, {'focused': {}},
                {'selected_text': text[:-1]}, {'selection_observed': False},
                {'selection_location': 1}, {'selection_length': len(text)}):
            self.assertFalse(observer.all_text_selected({**snapshot, **change}, text, 12))

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
