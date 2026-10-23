#!/usr/bin/env python3
"""Prove real Linux GPU-API initialization failure and Skia CPU recovery.

Runs only trusted, manifest-verified qualification packages; makes no system
driver changes. Xvfb is X11 evidence, not a physical/Wayland/remote desktop test.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import signal
import subprocess

spec = importlib.util.spec_from_file_location('runtime_inspection', pathlib.Path(__file__).with_name('inspect-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

CPU = 'Skia renderer (skia backend software;'
GPU = 'Skia renderer (skia backend opengl;'
DONE = 'renderer_smoke=passed;'
FAILURES = ['Failed to initialize Skia wgpu surface:', 'Failed to initialize Skia opengl surface:']
# Pinned Slint 1.18.1 error boundaries; arbitrary application/setup failures are
# not evidence that a compiled graphics API attempted native initialization.
INIT_ERRORS = {
    'opengl': ('Error creating glutin display for native display',
               'Could not find valid OpenGL display configurations:',
               'Unable to find suitable GL config', 'Error creating OpenGL context:',
               'Error creating OpenGL window surface:', 'Failed to initialize OpenGL driver:'),
    'wgpu': ('Error creating wgpu window surface:',
             'Failed to find an appropriate adapter:', 'Failed to create device:'),
}


def command(argv, env, timeout=30):
    """Bound the wrapper and its descendants, retaining partial failure output."""
    try:
        process = subprocess.Popen(argv, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, start_new_session=True)
    except OSError as error:
        return {'argv': argv, 'returncode': None, 'timed_out': False, 'stdout': '', 'stderr': str(error)}
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
    return {'argv': argv, 'returncode': process.returncode, 'timed_out': timed_out,
            'stdout': stdout, 'stderr': stderr}


def checks_for(runs):
    def text(name):
        return runs[name]['stdout'] + '\n' + runs[name]['stderr']

    def passed(name):
        return runs[name]['returncode'] == 0 and not runs[name]['timed_out']

    def rejected(name):
        # Signals, missing tools and timeout are not GPU initialization evidence.
        return (runs[name]['returncode'] is not None and runs[name]['returncode'] > 0
                and not runs[name]['timed_out'] and 'Error:' in text(name)
                and any(marker in text(name) for marker in INIT_ERRORS[name])
                and not any(marker in text(name).lower() for marker in
                            ('unsupported', 'not compiled', 'not built', 'not enabled'))
                and DONE not in text(name))

    return {
        'working_opengl_control': passed('baseline') and GPU in text('baseline') and DONE in text('baseline'),
        'forced_opengl_rejected': rejected('opengl'),
        'forced_wgpu_rejected': rejected('wgpu'),
        'default_attempted_both_gpu_candidates': all(marker in text('recovery') for marker in FAILURES),
        'default_recovered_to_skia_cpu': passed('recovery') and CPU in text('recovery'),
        'recovered_component_input_and_rendering': passed('recovery') and DONE in text('recovery'),
    }


def inspect(package, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine(), manifest['target']) != ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'):
        raise ValueError('GPU recovery probe requires the native Linux x86-64 package and host')
    if os.geteuid() == 0:
        raise ValueError('driver overrides require an ordinary unprivileged user')
    binary = runtime.verified_binary(package, manifest)
    output.mkdir(parents=True)  # New evidence only; never overwrite an earlier run.
    missing_driver = output.resolve() / 'intentionally-absent-driver.json'
    env = {**os.environ, 'LC_ALL': 'C', 'LANG': 'C',
           'SLINT_DEBUG_PERFORMANCE': 'refresh_lazy,console', 'WGPU_BACKEND': 'vulkan'}
    # Winit 0.30 chooses Wayland before DISPLAY when either variable is present.
    # Remove only from these child environments to actually test the Xvfb server.
    env.pop('WAYLAND_DISPLAY', None)
    env.pop('WAYLAND_SOCKET', None)
    # Override, rather than mutate, the host's driver discovery. GLX is disabled
    # on the injected X server; EGL and Vulkan cannot discover an ICD.
    injection = {'__EGL_VENDOR_LIBRARY_FILENAMES': str(missing_driver),
                 'VK_DRIVER_FILES': str(missing_driver), 'VK_ICD_FILENAMES': str(missing_driver)}
    runs = {}
    for name, backend in [('baseline', 'winit-skia-opengl'), ('opengl', 'winit-skia-opengl'),
                          ('wgpu', 'winit-skia-wgpu'), ('recovery', 'winit-skia')]:
        directory = output.resolve() / name
        directory.mkdir()
        injected = name != 'baseline'
        case_env = {**env, **(injection if injected else {}), 'SLINT_BACKEND': backend}
        server = '-screen 0 1280x900x24 -nolisten tcp' + (' -extension GLX' if injected else '')
        argv = ['xvfb-run', '-a', '--server-args=' + server, str(binary), '--french',
                '--renderer-smoke=' + str(directory)]
        runs[name] = command(argv, case_env)
        runs[name]['backend_request'] = backend
        runs[name]['driver_overrides'] = injection if injected else {}
        runs[name]['snapshots'] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                   for path in directory.glob('*.png')}
        print(f'{name}: exit={runs[name]["returncode"]}; timed_out={runs[name]["timed_out"]}', flush=True)
    checks = checks_for(runs)
    checks['fresh_frames_saved'] = all(set(runs[name]['snapshots']) == {'multilingual.png', 'filtered.png'}
                                      for name in ('baseline', 'recovery'))
    versions = command(['dpkg-query', '-W', '-f=${Package} ${Version}\n', 'xvfb', 'libvulkan1',
                        'mesa-vulkan-drivers', 'libegl1', 'libegl-mesa0', 'libglx0', 'libglx-mesa0',
                        'libgl1-mesa-dri', 'fonts-noto-cjk'], env)
    driver = command(['xvfb-run', '-a', 'glxinfo', '-B'], {**env, 'SLINT_BACKEND': 'winit-skia-opengl'})
    checks['driver_context_recorded'] = versions['returncode'] == 0 and driver['returncode'] == 0
    report = {
        'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
        'target': manifest['target'], 'binary_sha256': manifest['binary_sha256'],
        'host': platform.platform(), 'os_release': pathlib.Path('/etc/os-release').read_text(),
        'session': 'X11/Xvfb; no GLX extension in failure cases',
        'common_overrides': {'LC_ALL': 'C', 'LANG': 'C', 'WGPU_BACKEND': 'vulkan',
                             'SLINT_DEBUG_PERFORMANCE': 'refresh_lazy,console'},
        'child_variables_removed': ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET'],
        'driver_versions': versions, 'baseline_glx_driver': driver,
        'runs': runs, 'checks': checks, 'automated_gpu_recovery': 'pass' if all(checks.values()) else 'fail',
        'glyph_visual_review': 'pending', 'physical_input_ime_qualified': False,
        'wayland_qualified': False, 'remote_desktop_qualified': False, 'toolkit_selected': False,
        'snapshot_scope': 'Public Window.take_snapshot re-renders the scene on CPU; surface identity comes from Slint performance diagnostics.',
    }
    (output / 'gpu-recovery.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True, type=pathlib.Path)
    parser.add_argument('--output', required=True, type=pathlib.Path)
    args = parser.parse_args()
    report = inspect(args.package.resolve(strict=True), args.output)
    print(json.dumps(report['checks'], indent=2))
    raise SystemExit(0 if report['automated_gpu_recovery'] == 'pass' else 1)
