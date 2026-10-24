#!/usr/bin/env python3
"""Exercise a verified Linux package against an owned headless Wayland compositor.

This records protocol/component evidence, not physical input, IME, portals,
screen-reader speech, clean-host support or physical display presentation.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import re
import signal
import subprocess
import tempfile
import time

spec = importlib.util.spec_from_file_location('runtime', pathlib.Path(__file__).with_name('inspect-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

LIMIT = 4 * 1024 * 1024
CPU = 'Skia renderer (skia backend software;'
REMOVED = ('DISPLAY', 'XAUTHORITY', 'WAYLAND_SOCKET', 'WAYLAND_DEBUG',
           '__EGL_VENDOR_LIBRARY_FILENAMES', 'VK_DRIVER_FILES', 'VK_ICD_FILENAMES')


def environment(parent, directory):
    env = {key: value for key, value in parent.items() if key not in REMOVED}
    env.update({'LC_ALL': 'C', 'LANG': 'C', 'XDG_RUNTIME_DIR': str(directory),
                'XDG_CONFIG_HOME': str(directory / 'config'), 'XDG_CACHE_HOME': str(directory / 'cache'),
                'WAYLAND_DISPLAY': 'qualification-wayland', 'XDG_SESSION_TYPE': 'wayland',
                'SLINT_DEBUG_PERFORMANCE': 'refresh_lazy,console'})
    return env


def group_alive(pid):
    try:
        os.killpg(pid, 0)
    except ProcessLookupError:
        return False
    return True


def kill_group(pid, sig=signal.SIGKILL):
    try:
        os.killpg(pid, sig)
    except ProcessLookupError:
        pass


def command(argv, env, log, timeout=30, limit=LIMIT):
    """Bound elapsed time/output, reap the child and retire its process group."""
    result = {'argv': argv, 'returncode': None, 'errors': [], 'log': log.name}
    with log.open('xb') as stream:
        try:
            child = subprocess.Popen(argv, env=env, stdout=stream, stderr=stream, start_new_session=True)
        except OSError as error:
            result['errors'].append(str(error))
        else:
            try:
                deadline = time.monotonic() + timeout
                while child.poll() is None:
                    if time.monotonic() >= deadline:
                        result['errors'].append('process deadline exceeded')
                        break
                    if os.fstat(stream.fileno()).st_size > limit:
                        result['errors'].append('output limit exceeded')
                        break
                    time.sleep(0.02)
                if child.poll() is not None and group_alive(child.pid):
                    result['errors'].append('descendant survived exited child')
            finally:
                try:
                    kill_group(child.pid)
                except PermissionError:
                    # Darwin can reject SIGKILL for a group containing only an
                    # unreaped zombie. Reap our own exited child, then still
                    # require group retirement; a live/unsignalable member is
                    # never accepted on the strength of EPERM alone.
                    if child.poll() is None:
                        raise
                    kill_group(child.pid)
                child.wait(timeout=5)
                result['returncode'] = child.returncode
        if os.fstat(stream.fileno()).st_size > limit:
            if 'output limit exceeded' not in result['errors']:
                result['errors'].append('output limit exceeded')
            stream.truncate(limit)
    result['output'] = log.read_text(errors='replace')
    return result


def protocol_checks(text):
    # Winit enables wayland-backend/client_system: libwayland traces received
    # events without an arrow; Rust's backend uses '<-'. Accept both formats,
    # retaining direction, object identities and message order in either case.
    messages = []
    for line in text.splitlines():
        match = re.fullmatch(r'\[\s*[0-9.]+\](?:\[rs\])?\s*(?:(->|<-)\s*)?'
                             r'([a-z0-9_]+)@(\d+)\.([a-z0-9_]+),?\s*\((.*)\)', line.strip())
        if match:
            direction, interface, identity, method, args = match.groups()
            messages.append((direction == '->', interface, identity, method, args))
    configured, submitted = False, False
    for index, (sent, interface, _, method, args) in enumerate(messages):
        if not (sent and interface == 'xdg_wm_base' and method == 'get_xdg_surface'):
            continue
        mapping = re.fullmatch(r'(?:new id )?xdg_surface@(\d+),\s*wl_surface@(\d+)', args)
        if not mapping:
            continue
        xdg, surface = mapping.groups()
        role, serial, acknowledged, attached = False, None, False, False
        for sent, interface, identity, method, args in messages[index + 1:]:
            if interface == 'xdg_surface' and identity == xdg:
                if sent and method == 'get_toplevel':
                    role = True
                elif not sent and method == 'configure' and role:
                    serial, acknowledged, attached = args, False, False
                elif sent and method == 'ack_configure' and serial is not None and args == serial:
                    configured, acknowledged = True, True
            if sent and interface == 'wl_surface' and identity == surface and acknowledged:
                if method == 'attach':
                    attached = bool(re.match(r'wl_buffer@[1-9][0-9]*[,)]', args))
                elif method == 'commit' and attached:
                    submitted = True
    return {'xdg_window_configured': configured, 'window_buffer_submitted': submitted}


def passed(run):
    return run['returncode'] == 0 and not run['errors']


def missing_socket_rejected(run):
    # wayland-client 0.31.15 ConnectError::NoCompositor, wrapped by Winit.
    return (run['returncode'] is not None and run['returncode'] > 0 and not run['errors']
            and 'Could not find wayland compositor' in run['output']
            and 'os error at ' in run['output']
            and 'Skia renderer (skia backend ' not in run['output'])


def renderer_checks(run, explicit_cpu):
    text = run['output']
    return {'process_completed': passed(run), **protocol_checks(text),
            'component_input_and_scene': 'renderer_smoke=passed;' in text,
            'actual_surface_identified': 'Skia renderer (skia backend ' in text,
            'explicit_cpu_surface': not explicit_cpu or CPU in text,
            'fresh_distinct_frames': (set(run.get('snapshots', {})) == {'multilingual.png', 'filtered.png'}
                                      and len(set(run['snapshots'].values())) == 2)}


def inspect(package, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine(), manifest['target']) != ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'):
        raise ValueError('Wayland qualification requires the matching native Linux x86-64 package')
    if os.geteuid() == 0:
        raise ValueError('run the Wayland probe as an ordinary unprivileged user')
    binary = runtime.verified_binary(package, manifest)
    output.mkdir(parents=True)  # Never replace previous evidence.
    report = {'schema': 1, 'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
              'target': manifest['target'], 'binary_sha256': manifest['binary_sha256'],
              'host': platform.platform(), 'session': 'owned Weston headless/Pixman, no X11 display',
              'runs': {}, 'checks': {}, 'errors': [], 'automated_wayland': 'fail',
              'physical_input_ime_qualified': False, 'first_presentation_qualified': False,
              'portal_picker_reveal_qualified': False, 'screen_reader_qualified': False,
              'clean_host_qualified': False, 'toolkit_selected': False,
              'snapshot_scope': 'Public Slint snapshots re-render on CPU; Wayland protocol traces independently establish buffer submission, not physical presentation.'}
    weston = None
    try:
        with tempfile.TemporaryDirectory(prefix='librairii-wayland-') as temporary:
            directory = pathlib.Path(temporary)
            directory.chmod(0o700)
            env = environment(os.environ, directory)
            (directory / 'config').mkdir()
            (directory / 'cache').mkdir()
            report['child_environment'] = {key: env[key] for key in
                ('XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'WAYLAND_DISPLAY', 'XDG_SESSION_TYPE')}
            report['child_variables_removed'] = list(REMOVED)
            report['versions'] = command(['dpkg-query', '-W', '-f=${Package} ${Version}\n',
                'weston', 'wayland-utils', 'libwayland-client0', 'libwayland-server0', 'libpixman-1-0'],
                env, output / 'versions.log')
            report['checks']['versions_recorded'] = passed(report['versions'])
            argv = ['weston', '--backend=headless-backend.so', '--renderer=pixman',
                    '--shell=kiosk-shell.so', '--socket=' + env['WAYLAND_DISPLAY'],
                    '--idle-time=0', '--width=1280', '--height=900', '--no-config']
            report['compositor_argv'] = argv
            with (output / 'weston.log').open('xb') as stream:
                weston = subprocess.Popen(argv, env=env, stdout=stream, stderr=stream, start_new_session=True)
                try:
                    deadline = time.monotonic() + 10
                    socket = directory / env['WAYLAND_DISPLAY']
                    while not socket.is_socket():
                        if weston.poll() is not None or time.monotonic() >= deadline:
                            raise RuntimeError('Weston failed to create its owned socket within ten seconds')
                        time.sleep(0.05)
                    info = command(['wayland-info'], env, output / 'wayland-info.log')
                    report['runs']['wayland_info'] = info
                    report['checks']['compositor_protocols'] = (passed(info) and
                        all(name in info['output'] for name in ('wl_compositor', 'xdg_wm_base', 'wl_output')))
                    for name, backend in [('normal', 'winit-skia'), ('cpu', 'winit-skia-software')]:
                        frames = output / name
                        frames.mkdir()
                        run = command([str(binary), '--french', '--renderer-smoke=' + str(frames.resolve())],
                            {**env, 'WAYLAND_DEBUG': 'client', 'SLINT_BACKEND': backend}, output / (name + '.log'))
                        run['backend_request'] = backend
                        run['snapshots'] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                            for path in frames.glob('*.png')}
                        run['checks'] = renderer_checks(run, name == 'cpu')
                        report['runs'][name] = run
                        report['checks'][name + '_renderer'] = all(run['checks'].values())
                    for name, flag, marker in [('edit', '--edit-smoke', 'edit_smoke=passed;'),
                            ('focus', '--focus-smoke', 'focus_smoke=passed;'),
                            ('quit', '--quit-smoke', 'quit_smoke=passed;'),
                            ('thumbnail', '--thumbnail-smoke', 'thumbnail_smoke=passed;')]:
                        run = command([str(binary), '--french', flag],
                            {**env, 'SLINT_BACKEND': 'winit-skia-software'}, output / (name + '.log'))
                        report['runs'][name] = run
                        report['checks'][name] = passed(run) and marker in run['output']
                    control = command([str(binary), '--smoke'],
                        {**env, 'WAYLAND_DISPLAY': 'intentionally-absent', 'SLINT_BACKEND': 'winit-skia-software'},
                        output / 'missing-socket.log')
                    report['runs']['missing_socket'] = control
                    report['checks']['no_x11_fallback'] = missing_socket_rejected(control)
                    report['checks']['compositor_survived_cases'] = weston.poll() is None
                    if os.fstat(stream.fileno()).st_size > LIMIT:
                        raise RuntimeError('compositor output limit exceeded')
                finally:
                    kill_group(weston.pid, signal.SIGTERM)
                    try:
                        weston.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        kill_group(weston.pid)
                        weston.wait(timeout=5)
                    if group_alive(weston.pid):
                        report['errors'].append('compositor descendants survived shutdown')
                    kill_group(weston.pid)
                    report['checks']['compositor_closed'] = weston.returncode == 0
                    if os.fstat(stream.fileno()).st_size > LIMIT:
                        stream.truncate(LIMIT)
            report['checks']['runtime_socket_removed_by_compositor'] = not socket.exists()
        report['checks']['temporary_runtime_retired'] = not directory.exists()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report['errors'].append(str(error))
    finally:
        required = ('versions_recorded', 'compositor_protocols', 'normal_renderer', 'cpu_renderer',
                    'edit', 'focus', 'quit', 'thumbnail', 'no_x11_fallback', 'compositor_survived_cases',
                    'compositor_closed', 'runtime_socket_removed_by_compositor', 'temporary_runtime_retired')
        report['automated_wayland'] = ('pass' if all(report['checks'].get(name, False) for name in required)
                                       and not report['errors'] else 'fail')
        (output / 'wayland.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = inspect(args.package.resolve(strict=True), args.output.resolve())
    print(json.dumps({'checks': report['checks'], 'errors': report['errors']}, indent=2))
    raise SystemExit(0 if report['automated_wayland'] == 'pass' else 1)
