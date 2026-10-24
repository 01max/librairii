"""An Orca transcript alone is insufficient evidence of speech or full coverage."""
import array
import copy
import importlib.util
import pathlib
import sys
import tempfile
import unittest


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).parents[1] / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


observer = load('orca_observer', 'check-accessibility-linux.py')
driver = load('orca_driver', 'accessibility-linux/drive.py')


class AccessibilityLinux(unittest.TestCase):
    def test_modal_state_rejects_enabled_or_missing_background_and_disabled_dialog_controls(self):
        def button(name, enabled, focus=False):
            return {'name': name, 'role': 'push button', 'states': {
                'enabled': enabled, 'sensitive': enabled, 'showing': True, 'focused': focus}}
        nodes = [button('Settings', False), button('List', False), button('English', True),
            button('Français', True), button('Close', True, True)]
        self.assertTrue(driver.modal_controls_confined(nodes))
        for index in range(len(nodes)):
            changed = copy.deepcopy(nodes)
            changed[index]['states']['enabled'] = not changed[index]['states']['enabled']
            self.assertFalse(driver.modal_controls_confined(changed))
            self.assertFalse(driver.modal_controls_confined(nodes[:index] + nodes[index + 1:]))
        changed = copy.deepcopy(nodes)
        changed[0]['states']['sensitive'] = True
        self.assertFalse(driver.modal_controls_confined(changed))

    def test_key_echo_and_other_labels_cannot_replace_a_focus_announcement(self):
        text = "12:00:00 - SPEECH OUTPUT: 'Settings push button.' {'established': False}\n"
        self.assertTrue(driver.utterance_observed(text, 'Settings push button.'))
        for log in ("12:00 SPEECH OUTPUT: 'tab' {'established':False}",
                'Settings push button.', text.replace('push button', 'panel')):
            self.assertFalse(driver.utterance_observed(log, 'Settings push button.'))

    def test_audio_diagnostics_are_separate_and_cannot_turn_silence_into_speech(self):
        with tempfile.TemporaryDirectory() as temporary:
            desktop = driver.native.Desktop.__new__(driver.native.Desktop)
            desktop.root = pathlib.Path(temporary)
            desktop.children, desktop.logs, desktop.overflow = [], [], []
            child = driver.start_audio(desktop, [sys.executable, '-c',
                'import os; os.write(2,b"warning!"*1000); os.write(1,bytes(32000))'])
            self.assertEqual(child.wait(timeout=5), 0)
            desktop.close()
            self.assertEqual(desktop.overflow, [])
            self.assertEqual((desktop.root / 'speech-audio-stderr.txt').stat().st_size, 8000)
            pcm = (desktop.root / 'speech.pcm').read_bytes()
            self.assertEqual(len(pcm), 32000)
            self.assertEqual(driver.audio_facts(pcm)['nonzero_samples'], 0)

    def test_silent_or_truncated_audio_cannot_be_speech_evidence(self):
        self.assertEqual(driver.audio_facts(bytes(32000))['nonzero_samples'], 0)
        data = array.array('h', [0, -500, 1000, -32768]).tobytes()
        if sys.byteorder != 'little':
            values = array.array('h', [0, -500, 1000, -32768])
            values.byteswap()
            data = values.tobytes()
        facts = driver.audio_facts(data)
        self.assertEqual((facts['samples'], facts['nonzero_samples'], facts['peak']), (4, 3, 32768))
        with self.assertRaises(ValueError):
            driver.audio_facts(b'x')

    def test_speech_and_every_baseline_witness_are_required_without_full_qualification(self):
        report = {'baseline': 'pass', 'errors': [], 'binary_sha256': 'expected',
            'observation_method': 'per-focus-utterance-audio-and-modal-state-v4',
            'session': 'x11', 'reader_preferences': {'enable_action_keys': False},
            'application_returncode': 0, 'baseline_checks': dict.fromkeys(observer.REQUIRED, True),
            'speech_steps': {name: {'audio': {'nonzero_samples': 1601}} for name in observer.SPEECH_STEPS},
            'audio': {'nonzero_samples': 1601}, 'full_accessibility_qualified': False}
        self.assertTrue(observer.baseline_passed(report, 'expected', 'x11'))
        wayland = {**report, 'session': 'wayland', 'reader_preferences': {'enable_action_keys': True}}
        self.assertTrue(observer.baseline_passed(wayland, 'expected', 'wayland'))
        self.assertFalse(observer.baseline_passed(wayland, 'expected', 'x11'))
        self.assertFalse(observer.baseline_passed(report, 'expected', 'wayland'))
        self.assertFalse(observer.baseline_passed({**wayland, 'reader_preferences': {'enable_action_keys': False}}, 'expected', 'wayland'))
        for name in observer.SPEECH_STEPS:
            changed = copy.deepcopy(report)
            del changed['speech_steps'][name]
            self.assertFalse(observer.baseline_passed(changed, 'expected', 'x11'))
        for key in observer.REQUIRED:
            changed = copy.deepcopy(report)
            del changed['baseline_checks'][key]
            self.assertFalse(observer.baseline_passed(changed, 'expected', 'x11'))
        for change in ({'errors': ['failure']}, {'application_returncode': 1},
                {'audio': {'nonzero_samples': 0}}, {'full_accessibility_qualified': True},
                {'reader_preferences': {}}, {'reader_preferences': {'enable_action_keys': True}},
                {'session': 'unknown'}, {'observation_method': 'per-focus-utterance-and-audio-completion-v2'},
                {'binary_sha256': 'different'}):
            self.assertFalse(observer.baseline_passed({**report, **change}, 'expected', 'x11'))
