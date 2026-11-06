"""Exercise startup delivery and refusal contracts with real child pipes."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location('resources', Path(__file__).with_name('resources.py'))
resources = importlib.util.module_from_spec(spec)
spec.loader.exec_module(resources)
EMIT = "print(f'native_startup; stage=first_ui_timer; pid={os.getpid()}; phase=Opening; size=560x400; elapsed_ms=0.000', flush=True)"


class StartupCaptureTests(unittest.TestCase):
    def child(self, body):
        directory = tempfile.TemporaryDirectory(prefix='librairii-startup-pipes-')
        self.addCleanup(directory.cleanup)
        log = Path(directory.name) / 'execution.log'
        launched = time.monotonic()
        process = subprocess.Popen([sys.executable, '-I', '-B', '-c', 'import os,sys,time\n'+body],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        capture = resources.StartupCapture(process.stdout, log, launched)
        def close():
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)
            process.stdin.close()
            capture.join()
        self.addCleanup(close)
        return process, capture, log

    def test_parent_delivery_includes_child_wait_and_retains_opening_phase(self):
        process, capture, log = self.child("print('Étape ordinaire',flush=True)\ntime.sleep(.04)\n"+EMIT)
        self.assertEqual(process.wait(timeout=5), 0)
        receipt = capture.finish(process.pid)
        self.assertGreaterEqual(receipt['parent_launch_to_delivery_ms'], 40)
        self.assertEqual(receipt['child_elapsed_ms'], 0)
        self.assertEqual(receipt['parent_launch_to_delivery_ms'],
                         (receipt['parent_delivery_monotonic']-receipt['parent_launch_monotonic'])*1000)
        self.assertEqual(receipt['phase'], 'Opening')
        self.assertIn('Étape ordinaire\n', log.read_text())
        self.assertIn(receipt['line']+'\n', log.read_text())

    def test_wrong_pid_cannot_authorize_startup(self):
        process, capture, _ = self.child(EMIT.replace('os.getpid()', 'os.getpid()+1'))
        process.wait(timeout=5)
        with self.assertRaisesRegex(ValueError, 'PID/window differs'):
            capture.finish(process.pid)

    def test_late_duplicate_is_rejected_after_first_receipt(self):
        process, capture, _ = self.child(EMIT+'\nsys.stdin.readline()\n'+EMIT)
        self.assertTrue(capture.observed.wait(5))
        capture.receipt(process.pid)
        process.stdin.write(b'release\n')
        process.stdin.flush()
        process.wait(timeout=5)
        with self.assertRaisesRegex(ValueError, 'one bounded'):
            capture.finish(process.pid)

    def test_malformed_marker_is_rejected(self):
        process, capture, _ = self.child(EMIT.replace('560x400', '0x400'))
        process.wait(timeout=5)
        with self.assertRaisesRegex(ValueError, 'PID/window differs'):
            capture.finish(process.pid)
        process, capture, _ = self.child(EMIT.replace('0.000', 'NaN'))
        process.wait(timeout=5)
        with self.assertRaisesRegex(ValueError, 'malformed'):
            capture.finish(process.pid)

    def test_oversized_line_fails_even_with_a_later_valid_marker(self):
        process, capture, _ = self.child("print('x'*66000,flush=True)\n"+EMIT)
        process.wait(timeout=5)
        with self.assertRaisesRegex(ValueError, 'exceeded bound'):
            capture.finish(process.pid)

    def test_total_output_is_bounded_and_pipe_still_drains(self):
        process, capture, log = self.child("for _ in range(300): print('x'*4095,flush=True)\n"+EMIT)
        self.assertEqual(process.wait(timeout=5), 0)
        with self.assertRaisesRegex(ValueError, 'exceeded bound'):
            capture.finish(process.pid)
        self.assertLessEqual(log.stat().st_size, 1_048_576)

    def test_quit_failure_keeps_its_original_trace(self):
        with tempfile.TemporaryDirectory(prefix='librairii-quit-helper-') as directory:
            destination = Path(directory)
            command = [sys.executable, '-I', '-B', '-c', "import sys;print('original AX failure trace',file=sys.stderr,flush=True);sys.exit(3)"]
            with self.assertRaises(subprocess.CalledProcessError):
                resources.native_quit(command, destination)
            self.assertEqual((destination / 'quit-helper.stderr').read_text(), 'original AX failure trace\n')
            self.assertIn('"returncode": 3', (destination / 'quit-helper.json').read_text())

    def test_quit_timeout_keeps_partial_output(self):
        with tempfile.TemporaryDirectory(prefix='librairii-quit-helper-') as directory:
            destination = Path(directory)
            command = [sys.executable, '-I', '-B', '-c', "import sys,time;print('AX wait trace',file=sys.stderr,flush=True);time.sleep(10)"]
            with self.assertRaises(subprocess.TimeoutExpired):
                resources.native_quit(command, destination, timeout=2)
            self.assertEqual((destination / 'quit-helper.stderr').read_text(), 'AX wait trace\n')
            self.assertIn('"timed_out": true', (destination / 'quit-helper.json').read_text())


if __name__ == '__main__':
    unittest.main()
