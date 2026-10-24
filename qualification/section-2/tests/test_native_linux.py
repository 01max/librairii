"""Require actual parent association and a complete native desktop matrix."""
import importlib.util
import pathlib
import unittest
import tempfile
import sys
import subprocess
from unittest import mock


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
    def test_utf8_clipboard_observation_rejects_noop_copy_cut_or_paste(self):
        class Editor:
            def __init__(self, broken=None):
                self.value, self.clipboard, self.selected = '0042', 'old clipboard', False
                self.broken, self.captures = broken, []
            def key(self, key):
                if key == self.broken:
                    return
                if key == 'ctrl+a':
                    self.selected = True
                elif key == 'ctrl+c':
                    self.clipboard = self.value if self.selected else self.clipboard
                elif key == 'BackSpace' and self.selected:
                    self.value, self.selected = '', False
                elif key == 'ctrl+x' and self.selected:
                    self.clipboard, self.value, self.selected = self.value, '', False
                elif key == 'ctrl+v':
                    self.value, self.selected = self.clipboard, False
            def french_dead_keys(self):
                self.value += 'êé az'
            def screenshot(self, path):
                self.captures.append(path.name)
        def observe_once(predicate, **kwargs):
            result = predicate()
            if not result:
                raise TimeoutError('missing actual editor state')
            return result
        for broken in (None, 'ctrl+c', 'ctrl+x', 'ctrl+v'):
            with self.subTest(broken=broken):
                editor, report = Editor(broken), {'checks': {}}
                with mock.patch.object(driver, 'state', side_effect=lambda _: [editor.value, '0', '0', 'true', 'true']), \
                        mock.patch.object(driver, 'wait_for', side_effect=observe_once):
                    if broken is not None:
                        with self.assertRaises(TimeoutError):
                            driver.observe_utf8_input(editor, pathlib.Path('/owned/app'), pathlib.Path('/owned'), report)
                        self.assertNotIn('native_utf8_clipboard_roundtrip', report['checks'])
                    else:
                        driver.observe_utf8_input(editor, pathlib.Path('/owned/app'), pathlib.Path('/owned'), report)
                        self.assertTrue(report['checks']['native_utf8_clipboard_roundtrip'])
                        self.assertEqual(report['utf8_copy_cleared_state'][0], '')
                        self.assertEqual(report['utf8_copy_pasted_state'][0], 'êé az')
                        self.assertEqual(report['utf8_cut_state'][0], '')
                        self.assertEqual(report['utf8_pasted_state'][0], 'êé az')

    def test_x11_readiness_requires_a_client_handshake_and_live_registered_manager(self):
        desktop = driver.Desktop.__new__(driver.Desktop)
        unavailable = subprocess.CalledProcessError(1, ['xprop'], output='xprop: unable to open display :91')
        with mock.patch.object(desktop, 'run', side_effect=unavailable):
            self.assertIsNone(desktop.x11_server_ready())
        broken = subprocess.CalledProcessError(1, ['xprop'], output='unexpected X11 protocol error')
        with mock.patch.object(desktop, 'run', side_effect=broken), self.assertRaises(subprocess.CalledProcessError):
            desktop.x11_server_ready()
        root = '_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x123\n'
        with mock.patch.object(desktop, 'run', return_value=root):
            self.assertEqual(desktop.x11_server_ready(), root)
        manager = mock.Mock()
        manager.poll.return_value = None
        for window in ('_NET_SUPPORTING_WM_CHECK: no such atom',
                '_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x456\n_NET_WM_NAME(UTF8_STRING) = "Openbox"',
                root + '_NET_WM_NAME(UTF8_STRING) = "different manager"'):
            with mock.patch.object(desktop, 'run', side_effect=[root, window]):
                self.assertIsNone(desktop.x11_manager_ready(manager))
        window = root + '_NET_WM_NAME(UTF8_STRING) = "Openbox"\n'
        with mock.patch.object(desktop, 'run', side_effect=[root, window]):
            self.assertEqual(desktop.x11_manager_ready(manager)['manager_window_check'], window)
        manager.poll.return_value = 1
        with mock.patch.object(desktop, 'run') as query, self.assertRaises(RuntimeError):
            desktop.x11_manager_ready(manager)
        query.assert_not_called()

    def test_focus_handoff_absence_cannot_be_a_pid_or_hide_other_errors(self):
        desktop = driver.Desktop.__new__(driver.Desktop)
        desktop.session = 'x11'
        for prop in ('_NET_ACTIVE_WINDOW:  not found.', '_NET_ACTIVE_WINDOW:  no such atom on any window.',
                '_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0'):
            with mock.patch.object(desktop, 'run', return_value=prop):
                self.assertIsNone(desktop.active_pid())
        active = '_NET_ACTIVE_WINDOW(WINDOW): window id # 0x123abc'
        with mock.patch.object(desktop, 'run', side_effect=[active, '_NET_WM_PID(CARDINAL) = 123']):
            self.assertEqual(desktop.active_pid(), 123)
        destroyed = subprocess.CalledProcessError(1, ['xprop'], output='BadWindow (invalid Window parameter)')
        with mock.patch.object(desktop, 'run', side_effect=[active, destroyed]):
            self.assertIsNone(desktop.active_pid())
        for value in ('malformed property', '_NET_WM_PID(CARDINAL) = invalid'):
            with mock.patch.object(desktop, 'run', side_effect=[active, value]), self.assertRaises(ValueError):
                desktop.active_pid()
        unavailable = subprocess.CalledProcessError(1, ['xprop'], output='unable to open display')
        with mock.patch.object(desktop, 'run', side_effect=unavailable), self.assertRaises(subprocess.CalledProcessError):
            desktop.active_pid()

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
            required = common | ({'early_settings_delivered', 'native_french_dead_key_and_utf8_input',
                'native_utf8_clipboard_roundtrip'} if name == 'early-settings' else
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

    def test_log_write_failure_is_reported_and_retires_the_child(self):
        with tempfile.TemporaryDirectory() as tmp:
            desktop = driver.Desktop.__new__(driver.Desktop)
            desktop.root = pathlib.Path(tmp)
            desktop.children, desktop.logs, desktop.overflow = [], [], []
            (desktop.root / 'unwritable.log').mkdir()
            child = desktop.start('unwritable', [sys.executable, '-c', 'import time; time.sleep(30)'])
            child.wait(timeout=5)
            desktop.close()
            self.assertEqual(len(desktop.overflow), 1)
            self.assertIn('unwritable: capture ', desktop.overflow[0])
            self.assertNotEqual(child.returncode, 0)

    def test_falsey_native_leaf_is_not_replaced_by_the_entire_desktop(self):
        class Leaf:
            childCount = 0
            def __bool__(self):
                return False
        desktop = driver.Desktop.__new__(driver.Desktop)
        desktop.a = object()  # Falling back to Registry would be an error.
        leaf = Leaf()
        self.assertEqual(list(desktop.nodes(leaf)), [leaf])
