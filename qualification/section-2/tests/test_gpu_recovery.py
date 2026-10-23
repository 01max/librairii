"""GPU recovery verdicts must reject explicit CPU selection and unrelated errors."""
import importlib.util
import os
import pathlib
import sys
import unittest

spec = importlib.util.spec_from_file_location('gpu_recovery', pathlib.Path(__file__).parents[1] / 'check-gpu-recovery.py')
gpu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gpu)


def run(code, text):
    return {'returncode': code, 'timed_out': False, 'stdout': '', 'stderr': text}


class GpuRecovery(unittest.TestCase):
    def success(self):
        return {'baseline': run(0, gpu.GPU + gpu.DONE),
                'opengl': run(1, 'Error: Error creating glutin display for native display Xlib: initialization failed'),
                'wgpu': run(1, 'Error: Failed to find an appropriate adapter: no adapters'),
                'recovery': run(0, '\n'.join(gpu.FAILURES) + gpu.CPU + gpu.DONE)}

    def test_real_failure_controls_and_actual_surface_are_required(self):
        self.assertTrue(all(gpu.checks_for(self.success()).values()))
        for name, changed in [
                ('baseline', run(0, gpu.CPU + gpu.DONE)),
                ('opengl', run(0, gpu.GPU + gpu.DONE)),
                ('baseline', run(0, 'unrecognized renderer skia-opengl' + gpu.GPU + gpu.DONE)),
                ('opengl', run(-11, 'Error: signal')),
                ('wgpu', run(1, 'Error: unsupported WGPU renderer')),
                ('wgpu', run(127, 'command missing')),
                ('recovery', run(0, gpu.CPU + gpu.DONE)),
                ('recovery', run(0, '\n'.join(gpu.FAILURES) + gpu.DONE)),
                ('recovery', run(1, '\n'.join(gpu.FAILURES) + gpu.CPU + gpu.DONE))]:
            with self.subTest(name=name, changed=changed):
                runs = self.success()
                runs[name] = changed
                self.assertFalse(all(gpu.checks_for(runs).values()))

    def test_timeout_is_not_driver_failure_evidence(self):
        runs = self.success()
        runs['opengl']['timed_out'] = True
        self.assertFalse(gpu.checks_for(runs)['forced_opengl_rejected'])

    def test_unrelated_errors_and_missing_build_capabilities_are_not_gpu_failures(self):
        for name, text in [
                ('opengl', 'Error: unrelated application setup failure'),
                ('opengl', 'Error: renderer was not compiled in'),
                ('wgpu', 'Error: wgpu was not compiled in'),
                ('wgpu', 'Error: wgpu helper tool is missing'),
                ('opengl', 'unrecognized renderer skia-opengl; Error: Error creating glutin display for native display Xlib')]:
            with self.subTest(name=name, text=text):
                runs = self.success()
                runs[name] = run(1, text)
                self.assertFalse(gpu.checks_for(runs)['forced_' + name + '_rejected'])

    def test_captured_linux_native_errors_are_accepted_without_component_snapshots(self):
        runs = self.success()
        runs['opengl'] = run(1, 'Error: Error creating glutin display for native display Xlib(XlibDisplayHandle): failed to create EGLDisplay without a reason')
        runs['wgpu'] = run(1, 'Error: Error creating wgpu window surface: Failed to create surface for any enabled backend: {}')
        self.assertTrue(all(gpu.checks_for(runs).values()))

    def test_timeout_retains_output_and_reaps_process_group(self):
        result = gpu.command([sys.executable, '-c', 'import time; print("starting", flush=True); time.sleep(30)'],
                             os.environ.copy(), timeout=0.2)
        self.assertTrue(result['timed_out'])
        self.assertEqual(result['returncode'], -9)
        self.assertIn('starting', result['stdout'])

    def test_missing_launcher_is_retained_as_failure(self):
        result = gpu.command(['/intentionally-absent-qualification-tool'], os.environ.copy())
        self.assertIsNone(result['returncode'])
        self.assertFalse(result['timed_out'])
        self.assertTrue(result['stderr'])


if __name__ == '__main__':
    unittest.main()
