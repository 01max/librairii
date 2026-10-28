"""Current production view geometry and actual virtual-monitor transitions."""
import json
import os
from pathlib import Path
import platform
import re
import struct
import shutil
import subprocess
import tempfile


def inspect(binary, destination, backend, native, scale=None, environment=None, mixed=None):
    destination.mkdir()
    (destination / '.native-scale').write_text('native-scale-16\n')
    config = destination / 'fixture.json'
    owner = subprocess.Popen([str(binary), '--prepare', str(config)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=(destination / 'prepare.log').open('w'), text=True)
    prepared = json.loads(owner.stdout.readline())
    env = dict(environment or os.environ, SLINT_BACKEND=backend, SLINT_DEBUG_PERFORMANCE='refresh_lazy,console')
    env.pop('SLINT_SCALE_FACTOR', None)
    if scale is not None:
        env['SLINT_SCALE_FACTOR'] = str(scale)
    log = (destination / 'execution.log').open('w')
    app = subprocess.Popen([str(binary), '--run', str(config), str(destination)], env=env, stdout=log, stderr=subprocess.STDOUT)
    joined = False
    report = {'backend_request': backend, 'requested_scale': scale, 'frames': [], 'scale_transitions': [], 'complete': False}
    try:
        native.wait(lambda: (destination / 'ready').exists() and native.state(destination))
        def capture(label, sequence):
            native.publish(destination / 'capture', str(sequence))
            state = native.wait(lambda: (s := native.state(destination)) and s['capture'] == sequence and s)
            path = destination / f'capture-{sequence}.png'
            data = path.read_bytes()
            if data[:8] != b'\x89PNG\r\n\x1a\n':
                raise ValueError('current rendered PNG required')
            width, height = struct.unpack('>II', data[16:24])
            if (scale is not None and state['slint_scale'] != scale
                    or abs(width-state['logical_width']*state['slint_scale']) > 1
                    or abs(height-state['logical_height']*state['slint_scale']) > 1):
                raise ValueError('actual render density or physical/logical frame dimensions differ')
            if width != state['native']['width'] or height != state['native']['height']:
                raise ValueError('capture does not match observed native surface')
            if (state['rows'] != 24 or state['total'] != 1000 or not state['french']
                    or state['browser_height'] < state['row_height'] or state['row_height'] < 76):
                raise ValueError('short/French viewport cannot hold its real story row')
            report['frames'].append({'label': label, 'file': path.name, 'width': width, 'height': height, 'state': state})
        cases = [('wide-long-french', 1180, 760), ('short-long-french', 1180, 400), ('narrow-short-long-french', 560, 400), ('intermediate', 980, 600), ('restore-wide', 1180, 760)]
        for sequence, (label, width, height) in enumerate(cases, 1):
            native.publish(destination / 'geometry', json.dumps({'sequence': sequence, 'width': width, 'height': height}))
            native.wait(lambda: (s := native.state(destination)) and s['geometry_sequence'] == sequence
                        and abs(s['logical_width']-width) <= 1 and abs(s['logical_height']-height) <= 1)
            capture(label, sequence)
        if mixed is not None:
            for sequence, (output, expected) in enumerate([('HEADLESS-2', 2), ('HEADLESS-1', 1), ('HEADLESS-2', 2), ('HEADLESS-1', 1)], 6):
                result = mixed(f'[pid={app.pid}] move container to output {output}')
                if not all(v.get('success') for v in result):
                    raise ValueError('owned compositor refused actual monitor move')
                state = native.wait(lambda: (s := native.state(destination)) and s['native']['scale'] == expected
                                    and s['slint_scale'] == expected and s['native']['monitor']['scale'] == expected
                                    and abs(s['logical_width']-1180) <= 1 and abs(s['logical_height']-760) <= 1 and s)
                report['scale_transitions'].append(state)
                capture('actual-virtual-monitor-' + str(sequence), sequence)
        (destination / 'finish').touch()
        joined = app.wait(timeout=20) == 0 and 'scale_complete; workers_joined=true' in (destination / 'execution.log').read_text()
        if not joined:
            raise ValueError('display worker join required')
        surfaces = set(re.findall(r'Skia renderer \(skia backend ([^;]+);', (destination / 'execution.log').read_text()))
        if len(surfaces) != 1 or ('software' in surfaces) != (backend == 'winit-skia-software'):
            raise ValueError('display renderer surface differs from its requested path')
        report['active_surface'] = sorted(surfaces)
        report['complete'] = True
    finally:
        if app.poll() is None:
            (destination / 'finish').touch()
            try:
                app.wait(timeout=20)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait()
        owner.stdin.write('workers_joined\n' if joined else '')
        owner.stdin.close()
        owner.wait(timeout=15)
        report['workers_joined'] = joined
        report['fixture_retired'] = not Path(prepared['owner']).exists()
        log.close()
        (destination / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    if not report['fixture_retired']:
        raise ValueError('display fixture retirement required')
    return report


def mixed_monitors(binary, destination, native):
    destination.mkdir()
    runtime = Path(tempfile.mkdtemp(prefix='librairii-scale-display-'))
    runtime.chmod(0o700)
    config = runtime / 'sway.conf'
    config.write_text('output * mode 1920x1080\nxwayland disable\nseat seat0 fallback true\ndefault_border pixel 1\nfor_window [app_id=".*"] floating enable\n')
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS='headless', WLR_RENDERER='pixman',
               WLR_LIBINPUT_NO_DEVICES='1', GDK_BACKEND='wayland', WINIT_UNIX_BACKEND='wayland', XDG_SESSION_TYPE='wayland')
    env.pop('DISPLAY', None)
    env.pop('WAYLAND_SOCKET', None)
    log = (destination / 'compositor.log').open('w')
    compositor = subprocess.Popen(['sway', '--config', str(config)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        socket = native.wait(lambda: next((p for p in runtime.glob('wayland-*') if p.is_socket()), None))
        ipc = native.wait(lambda: next(runtime.glob('sway-ipc.*.sock'), None))
        env['WAYLAND_DISPLAY'] = socket.name
        def command(value):
            return json.loads(subprocess.check_output(['swaymsg', '-s', str(ipc), value], env=env, text=True, timeout=10))
        command('output HEADLESS-1 mode 1920x1080 scale 1 position 0 0')
        command('create_output')
        command('output HEADLESS-2 mode 3840x2160 scale 2 position 1920 0')
        outputs = json.loads(subprocess.check_output(['swaymsg', '-s', str(ipc), '-t', 'get_outputs'], env=env, text=True, timeout=10))
        if {o['name']: o['scale'] for o in outputs} != {'HEADLESS-1': 1.0, 'HEADLESS-2': 2.0}:
            raise ValueError('actual two-scale compositor output inventory differs')
        (destination / 'outputs.json').write_text(json.dumps(outputs, indent=2) + '\n')
        return inspect(binary, destination / 'application', 'winit-skia-software', native, environment=env, mixed=command)
    finally:
        compositor.terminate()
        try:
            compositor.wait(timeout=10)
        except subprocess.TimeoutExpired:
            compositor.kill()
            compositor.wait(timeout=10)
        log.close()
        # This path has no automatic deleting destructor: an uncertain process
        # lifetime retains its runtime instead of removing live socket files.
        shutil.rmtree(runtime)
        if runtime.exists():
            raise ValueError('owned compositor runtime was not retired')
