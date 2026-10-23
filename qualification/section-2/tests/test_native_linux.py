"""Require actual parent association and a complete native desktop matrix."""
import importlib.util
import pathlib
import unittest
import tempfile
import sys


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).parents[1] / path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


native = load('native_linux', 'check-native-linux.py')
driver = load('native_driver', 'native-linux/drive.py')


def monitor(parent):
    return 'method call interface=org.freedesktop.portal.FileChooser; member=OpenFile\n   string "' + parent + '"\n'


class NativeLinux(unittest.TestCase):
    def test_x11_parent_requires_the_real_app_pid_and_actual_dialog_transient(self):
        args = ('x11', monitor('x11:123abc'), '', '', 123)
        self.assertTrue(all(driver.parent_proof(*args, parent_pid=123, transient=0x123abc).values()))
        for pid, xid in ((124, 0x123abc), (123, 0x123abd), (None, None)):
            self.assertFalse(all(driver.parent_proof(*args, parent_pid=pid, transient=xid).values()))
        for parent in ('', 'wayland:handle', 'x11:unknown'):
            self.assertFalse(all(driver.parent_proof('x11', monitor(parent), '', '', 123, 123, 0x123abc).values()))

    def test_wayland_parent_requires_the_exported_handle_and_matching_imported_object(self):
        app = '[1.0] zxdg_exported_v2@31.handle("owned-token")\n'
        gtk = ('[1.1] -> zxdg_importer_v2@24.import_toplevel(new id zxdg_imported_v2@32, "owned-token")\n'
               '[1.2] -> zxdg_imported_v2@32.set_parent_of(wl_surface@25)\n')
        self.assertTrue(all(driver.parent_proof('wayland', monitor('wayland:owned-token'), app, gtk, 123).values()))
        for changed in (gtk.replace('owned-token', 'other-token'),
                        gtk.replace('zxdg_imported_v2@32.set_parent', 'zxdg_imported_v2@33.set_parent'),
                        gtk.replace('set_parent_of', 'unrelated'),
                        gtk.replace('[1.2]', '[1.15] zxdg_imported_v2@32.destroyed()\n[1.2]'),
                        gtk + 'Failed to associate portal window with parent window'):
            self.assertFalse(all(driver.parent_proof('wayland', monitor('wayland:owned-token'), app, changed, 123).values()))
        self.assertFalse(all(driver.parent_proof('wayland', monitor('wayland:owned-token'), '', gtk, 123).values()))
        self.assertFalse(all(driver.parent_proof('wayland', monitor('wayland:owned-token') * 2, app, gtk, 123).values()))

    def test_passing_summary_cannot_replace_missing_cases_or_required_checks(self):
        common = {'owned_shutdown', 'fixture_removed', 'native_focus_restored_and_search_input'}
        cases = {}
        for name in native.CASES:
            required = common | ({'early_settings_delivered'} if name == 'early-settings' else
                {'actual_file_manager_window_path', 'os_settings_accelerator'} if name == 'reveal' else
                {'real_modal_dialog', 'one_real_portal_call', 'parent_identified', 'parent_associated',
                 'no_parent_rejection', 'actual_picker_result', 'os_settings_accelerator'})
            cases[name] = {'returncode': 0, 'checks': dict.fromkeys(required, True)}
        report = {'binary_sha256': 'expected', 'native_integration': 'pass', 'errors': [], 'cases': cases}
        self.assertTrue(native.session_passed(report, 'expected'))
        self.assertFalse(native.session_passed(report, 'other'))
        for name in native.CASES:
            without = {**report, 'cases': {k: v for k, v in cases.items() if k != name}}
            self.assertFalse(native.session_passed(without, 'expected'))
            for key in cases[name]['checks']:
                changed = {**report, 'cases': {**cases, name: {**cases[name],
                    'checks': {k: v for k, v in cases[name]['checks'].items() if k != key}}}}
                self.assertFalse(native.session_passed(changed, 'expected'))

    def test_noisy_child_output_is_bounded_and_reported_without_file_size_rlimit(self):
        with tempfile.TemporaryDirectory() as tmp:
            desktop = driver.Desktop.__new__(driver.Desktop)
            desktop.root = pathlib.Path(tmp)
            desktop.children, desktop.logs, desktop.overflow = [], [], []
            child = desktop.start('noisy', [sys.executable, '-c',
                'import os; chunk=b"x"*65536\nwhile True: os.write(1,chunk)'])
            child.wait(timeout=5)
            desktop.close()
            self.assertEqual((desktop.root / 'noisy.log').stat().st_size, driver.LIMIT)
            self.assertEqual(desktop.overflow, ['noisy'])
            self.assertNotEqual(child.returncode, 0)
