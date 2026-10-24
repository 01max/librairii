"""A build or partial macOS summary cannot qualify missing native observations."""
import copy
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('macos', pathlib.Path(__file__).parents[1] / 'check-native-macos.py')
macos = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macos)


class NativeMacOS(unittest.TestCase):
    def test_diagnostics_cannot_turn_a_failed_native_command_into_a_pass(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        result = {'returncode': 0, 'errors': [],
                  'output': 'observer-stage: reading AXWindows\n{"pid":123,"nodes":[]}\n'}
        with mock.patch.object(macos.common.wayland, 'command', return_value=result):
            self.assertEqual(observer.call('snapshot')['pid'], 123)
        for change in ({'returncode': -9}, {'errors': ['process deadline exceeded']}):
            with self.subTest(change=change), mock.patch.object(macos.common.wayland, 'command',
                    return_value={**result, **change}), self.assertRaises(RuntimeError):
                observer.call('snapshot')

    def test_exited_parent_cannot_leave_a_pipe_reader_or_owned_descendant(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = pathlib.Path(temporary) / 'app.log'
            source = ('import subprocess, sys; '
                'subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"]); '
                'print("parent exited", flush=True)')
            app = subprocess.Popen([sys.executable, '-c', source], stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, start_new_session=True)
            capture = macos.capture_output(app, output)
            started = time.monotonic()
            try:
                self.assertEqual(app.wait(timeout=5), 0)
                errors = macos.retire(app, capture)
                self.assertIn('descendant survived exited app', errors)
                self.assertFalse(capture[1].is_alive())
                self.assertIn('parent exited', output.read_text())
                self.assertLess(time.monotonic() - started, 6)
            finally:
                macos.common.wayland.kill_group(app.pid)
                capture[0].set()
                capture[1].join(timeout=2)

    def fixture(self):
        cases = {}
        for name, kind, _ in macos.CASES:
            checks = {'native_settings_accelerator', 'native_settings_menu', 'native_search_input',
                      'owned_shutdown', 'fixture_removed'}
            checks |= ({'real_parented_modal_sheet', 'exact_native_callback', 'unforced_parent_focus_restored'}
                if kind in ('files', 'folder') else {'actual_finder_path', 'explicit_return_from_finder'}
                if kind == 'reveal' else {'early_settings_delivered', 'native_unicode_clipboard_and_edit_menu'})
            cases[name] = {'errors': [], 'returncode': 0, 'checks': dict.fromkeys(checks, True)}
        return {'binary_sha256': 'expected', 'errors': [], 'cases': cases}

    def test_every_case_and_required_native_witness_is_mandatory(self):
        report = self.fixture()
        self.assertTrue(macos.session_passed(report, 'expected'))
        self.assertFalse(macos.session_passed(report, 'changed'))
        for name, case in report['cases'].items():
            changed = copy.deepcopy(report)
            del changed['cases'][name]
            self.assertFalse(macos.session_passed(changed, 'expected'))
            for key in case['checks']:
                changed = copy.deepcopy(report)
                del changed['cases'][name]['checks'][key]
                self.assertFalse(macos.session_passed(changed, 'expected'))
            for field, value in [('returncode', 1), ('errors', ['failed capture'])]:
                changed = copy.deepcopy(report)
                changed['cases'][name][field] = value
                self.assertFalse(macos.session_passed(changed, 'expected'))

    def test_parenting_requires_public_attached_sheet_not_a_standalone_dialog(self):
        sheet = {'role': 'AXSheet', 'selector': {'window': 0, 'sheet': 0, 'path': []}}
        self.assertEqual(macos.picker_root({'nodes': [sheet]}), sheet)
        for changed in ({**sheet, 'modal': False}, {**sheet, 'role': 'AXWindow'},
                        {**sheet, 'selector': {'window': 0, 'path': []}},
                        {**sheet, 'selector': {'window': 0, 'sheet': 0, 'path': [1]}}):
            self.assertIsNone(macos.picker_root({'nodes': [changed]}))

    def test_selector_fingerprints_do_not_depend_on_mutating_text_values(self):
        node = {'role': 'AXTextField', 'title': 'Go to Folder', 'description': 'Path', 'identifier': 'owned',
                'value': '/tmp/before', 'selector': {'window': 0, 'sheet': 0, 'path': [2, 1]}}
        selected = macos.selector(node)
        self.assertNotIn('value', selected['match'])
        self.assertEqual(selected['match']['identifier'], 'owned')
        self.assertTrue(macos.same_root(node, {**node, 'selector': {'window': 0, 'sheet': 0, 'path': []}}))
        self.assertFalse(macos.same_root(node, {**node, 'selector': {'window': 1, 'sheet': 0, 'path': []}}))


if __name__ == '__main__':
    unittest.main()
