"""Installed observer foreground admission, with inert native command replies."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from packaged_acceptance.linux import Driver


class InstalledX11Activation(unittest.TestCase):
    def launch(self, root, *, windows='27\n', owner=123, refused=False, stays_background=False,
               late=False, ownership_changed=False, changed_before_activation=False, timed_out=False):
        now = [0.0]
        actions = []
        active = [False]
        before_read = [False]
        application = object()
        desktop = SimpleNamespace(session='x11', dbus=SimpleNamespace(DBusException=RuntimeError),
                                  find=lambda _: application, active_pid=lambda: 123 if active[0] else 999)

        def command(argv, **options):
            self.assertGreater(options['timeout'], 0)
            self.assertLessEqual(options['timeout'], 15)
            actions.append(argv)
            if argv[:2] == ['xdotool', 'search']:
                return windows
            if argv[:3] == ['xprop', '-id', '27']:
                changed = (active[0] and ownership_changed) or (before_read[0] and changed_before_activation)
                return f'_NET_WM_PID(CARDINAL) = {456 if changed else owner}\n'
            if argv == ['xprop', '-root', '_NET_ACTIVE_WINDOW']:
                before_read[0] = True
                return ' _NET_ACTIVE_WINDOW(WINDOW): window id # '+('0x1b' if active[0] else '0x2a')+'\n'
            if argv == ['xdotool', 'windowactivate', '27']:
                if refused:
                    raise subprocess.CalledProcessError(1, argv, output='activation refused')
                if timed_out:
                    raise subprocess.TimeoutExpired(argv, options['timeout'], output=b'partial activation')
                active[0] = not stays_background
                if late:
                    now[0] += 16
                return ''
            self.fail('unexpected native command: '+str(argv))

        def sleep(seconds):
            now[0] += seconds

        self.actions = actions
        with patch.dict(sys.modules, {'gi.repository': SimpleNamespace(GLib=SimpleNamespace(Error=RuntimeError))}), \
             patch('packaged_acceptance.linux.subprocess.check_output', side_effect=command), \
             patch('packaged_acceptance.linux.time.monotonic', side_effect=lambda: now[0]), \
             patch('packaged_acceptance.linux.time.sleep', side_effect=sleep):
            return Driver(root, root, 123, desktop)

    def test_background_owned_window_receives_one_native_activation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.launch(root)
            self.assertEqual(self.actions.count(['xdotool', 'windowactivate', '27']), 1)
            receipt = json.loads((root/'initial-window-activation.json').read_text())
            self.assertTrue(receipt['complete'])
            self.assertEqual(receipt['window_id'], 27)
            self.assertIn('0x2a', receipt['before'])
            self.assertIn('0x1b', receipt['after'])

    def test_absent_ambiguous_or_foreign_window_never_receives_activation(self):
        for options in ({'windows': ''}, {'windows': '27\n28\n'}, {'owner': 456},
                        {'changed_before_activation': True}):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with self.assertRaises((TimeoutError, ValueError)):
                    self.launch(root, **options)
                self.assertNotIn(['xdotool', 'windowactivate', '27'], self.actions)
                receipt = json.loads((root/'initial-window-activation.json').read_text())
                self.assertFalse(receipt['complete'])
                self.assertIn('error', receipt)

    def test_failed_or_ineffective_activation_is_not_replayed(self):
        for options in ({'refused': True}, {'timed_out': True}, {'stays_background': True}, {'late': True},
                        {'ownership_changed': True}):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with self.assertRaises((subprocess.CalledProcessError, subprocess.TimeoutExpired, TimeoutError, ValueError)):
                    self.launch(root, **options)
                self.assertEqual(self.actions.count(['xdotool', 'windowactivate', '27']), 1)
                receipt = json.loads((root/'initial-window-activation.json').read_text())
                self.assertFalse(receipt['complete'])
                if options.get('refused'):
                    self.assertEqual(receipt['last_command']['output'], 'activation refused')
                    self.assertEqual(receipt['last_command']['returncode'], 1)
                if options.get('timed_out'):
                    self.assertEqual(receipt['last_command']['output'], 'partial activation')
                    self.assertTrue(receipt['last_command']['timed_out'])
                if options.get('late'):
                    self.assertEqual(receipt['last_command']['argv'], ['xdotool', 'windowactivate', '27'])
                    self.assertEqual(receipt['last_command']['returncode'], 0)


if __name__ == '__main__':
    unittest.main()
