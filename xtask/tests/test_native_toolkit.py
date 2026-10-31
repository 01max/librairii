"""Run the production reveal-owner helper without opening any native window."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class NativeToolkitTests(unittest.TestCase):
    def test_repeated_native_show_preserves_first_reveal_owner(self):
        source = ROOT/'third_party/slint-backend-winit/first_frame_reveal.rs'
        with tempfile.TemporaryDirectory(prefix='librairii-first-frame-') as temporary:
            executable = Path(temporary)/'first-frame-tests'
            subprocess.run(['rustc', '--edition=2024', '--test', str(source), '-o', str(executable)],
                           check=True, capture_output=True, text=True, timeout=30)
            result = subprocess.run([str(executable)], check=True, capture_output=True, text=True, timeout=5)
            self.assertIn('tests::nested_show_preserves_first_opacity_until_submission ... ok', result.stdout)
            self.assertIn('tests::unavailable_window_does_not_prevent_later_reveal ... ok', result.stdout)
            self.assertIn('2 passed; 0 failed; 0 ignored', result.stdout)
