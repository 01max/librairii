"""Current desktop native scale observer. No normal profile or public network."""
import json
import math
import re
import signal
import os
from pathlib import Path
import platform
import subprocess
import time


def publish(path, text):
    temporary = path.with_suffix('.command-tmp')
    temporary.write_text(text)
    temporary.replace(path)


def state(output):
    try:
        return json.loads((output / 'state.json').read_text())
    except FileNotFoundError:
        return None


def wait(predicate, seconds=15):
    start = time.monotonic()
    while time.monotonic() - start < seconds:
        value = predicate()
        if value:
            return value
        time.sleep(.001)
    raise TimeoutError('current native observer state deadline')


def linux_input(pid, output):
    xid = subprocess.check_output(['xdotool', 'search', '--sync', '--onlyvisible', '--pid', str(pid)], text=True, timeout=15).splitlines()[-1]
    subprocess.run(['xdotool', 'windowfocus', '--sync', xid], check=True, timeout=10)
    def key(value):
        subprocess.run(['xdotool', 'key', '--clearmodifiers', value], check=True, timeout=10)
    def search():
        for _ in range(64):
            before = state(output)
            if before['query_focused']:
                return
            key('shift+Tab')
            wait(lambda: (s := state(output)) and (s['query_focused'] or s['focus_key'] != before['focus_key']))
        raise ValueError('native reverse Tab did not reach Search')
    search()
    samples, input_states = [], []
    for n in range(64):
        wait(lambda: state(output)['running_import'])
        wanted = 'x' if n % 2 == 0 else ''
        start = time.monotonic()
        key('x' if n % 2 == 0 else 'BackSpace')
        end = wait(lambda: (s := state(output)) and s['query'] == wanted and s, 1)
        samples.append((time.monotonic() - start) * 1000)
        input_states.append(end)
        if not end['running_import']:
            raise ValueError('real import ended during input sample')
    point = json.loads(subprocess.check_output(['/usr/bin/python3', str(Path(__file__).with_name('resource-linux.py')), str(pid), '--point'], text=True, timeout=15))
    pointers, pointer_states = [], []
    for _ in range(64):
        search()
        wait(lambda: state(output)['running_import'])
        start = time.monotonic()
        subprocess.run(['xdotool', 'mousemove', str(point['x']), str(point['y']), 'click', '--delay', '0', '1'], check=True, timeout=10)
        end = wait(lambda: (s := state(output)) and s['focus_key'] == 31 and s, 1)
        pointers.append((time.monotonic() - start) * 1000)
        pointer_states.append(end)
        if not end['running_import']:
            raise ValueError('real import ended during pointer sample')
    navigation, endpoints = [], []
    for n in range(21):
        # Evict the first page using144 other real covers before each trial.
        for page in range(2, 8):
            publish(output / 'page', str(page))
            wait(lambda: (s := state(output)) and s['page'] == page and s['settled'])
        cold_generation = state(output)['first_cover_reloads']
        search()
        wait(lambda: state(output)['running_import'])
        key('x')
        wait(lambda: (s := state(output)) and s['query'] == 'x' and s['total'] == 0 and s['settled'])
        start = time.monotonic()
        key('BackSpace')
        wait(lambda: (s := state(output)) and s['total'] == 1000 and s['rows'] == 24 and s['first_cover_reloads'] > cold_generation and s['settled'])
        for _ in range(32):
            previous = state(output)['focus_key']
            if previous == 31:
                break
            key('Tab')
            wait(lambda: state(output)['focus_key'] != previous)
        if state(output)['focus_key'] != 31:
            raise ValueError('native Tab did not focus actual browser')
        key('End')
        end = wait(lambda: (s := state(output)) and s['selected_index'] == 23 and s['settled'] and s)
        duration = (time.monotonic() - start) * 1000
        if not end['running_import']:
            raise ValueError('real import ended during navigation sample')
        if n:
            navigation.append(duration)
            endpoints.append(end)
        key('Home')
        wait(lambda: (s := state(output)) and s['selected_index'] == 0 and s['settled'])
    return {'input_ms': samples, 'input_states': input_states, 'input_p95_ms': sorted(samples)[60],
            'pointer_ms': pointers, 'pointer_states': pointer_states, 'pointer_p95_ms': sorted(pointers)[60], 'pointer_target': point,
            'navigation_ms': navigation, 'navigation_p95_ms': sorted(navigation)[18], 'endpoints': endpoints,
            'method': 'pre-xdotool native post through actual production UI/controller acknowledgement; bounded24-row page of1000, native Tab/End'}


def timed_arguments(binary, configuration, destination):
    metrics = destination / 'native-time.log'
    clock = ['/usr/bin/time', '-o', str(metrics)]
    clock += ['-l', '-p'] if platform.system() == 'Darwin' else ['-f', 'qualification_time; wall=%e; user=%U; system=%S; maxrss_kib=%M']
    return clock + [str(binary), '--run', str(configuration), str(destination)]


def inspect(binary, destination, backend, helper=None):
    destination.mkdir()
    (destination / '.native-scale').write_text('native-scale-16\n')
    configuration = destination / 'fixture.json'
    owner = subprocess.Popen([str(binary), '--prepare', str(configuration)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=(destination / 'prepare.log').open('w'), text=True)
    prepared = json.loads(owner.stdout.readline())
    if not prepared['prepared']:
        raise ValueError('owned fixture preparation failed')
    process = None
    joined = False
    report = {'backend_request': backend, 'prepared': prepared, 'complete': False}
    try:
        log = (destination / 'execution.log').open('w')
        environment = dict(os.environ, SLINT_BACKEND=backend, SLINT_SCALE_FACTOR='1', SLINT_DEBUG_PERFORMANCE='refresh_lazy,console')
        start = time.monotonic()
        process = subprocess.Popen(timed_arguments(binary, configuration, destination), env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        wait(lambda: (destination / 'ready').exists() and state(destination))
        report['development_launch_ready_ms'] = (time.monotonic() - start) * 1000
        report['initial'] = state(destination)
        actual_pid = report['initial']['pid']
        if os.getpgid(actual_pid) != process.pid:
            raise ValueError('observed native PID is outside the owned process group')
        (destination / 'start-load').touch()
        wait(lambda: (s := state(destination)) and s['running_import'])
        pages = []
        for page in range(1, 43):
            publish(destination / 'page', str(page))
            observed = wait(lambda: (s := state(destination)) and s['page'] == page and s['settled'] and s)
            if observed['rows'] != (16 if page == 42 else 24) or observed['total'] != 1000:
                raise ValueError('bounded actual page/outcome differs')
            pages.append(observed)
        publish(destination / 'page', '1')
        evicted = wait(lambda: (s := state(destination)) and s['page'] == 1 and s['first_cover_evicted'] and s['settled'] and s)
        if evicted['loaded_covers'] != 1000 or evicted['max_rows'] != 24:
            raise ValueError('all1000 real covers, eviction and bounded model observations required')
        report.update(pages=pages, eviction=evicted)
        if platform.system() == 'Darwin':
            if helper is None:
                raise ValueError('native macOS helper required')
            raw = subprocess.run([str(helper), str(actual_pid), str(destination)], capture_output=True, text=True, timeout=240)
            (destination / 'input.log').write_text(raw.stdout + raw.stderr)
            raw.check_returncode()
            measured = json.loads(raw.stdout)
        else:
            measured = linux_input(actual_pid, destination)
        report['input'] = measured
        for key, count in [('input_ms', 64), ('pointer_ms', 64), ('navigation_ms', 20)]:
            if len(measured[key]) != count or any(type(x) not in (int, float) or not math.isfinite(x) or x < 0 for x in measured[key]):
                raise ValueError('complete finite native wall samples required')
        if (len(measured['input_states']) != 64 or len(measured['pointer_states']) != 64 or len(measured['endpoints']) != 20
                or any(not s['running_import'] or s['max_rows'] > 24 for s in measured['input_states'] + measured['pointer_states'] + measured['endpoints'])
                or any(s['total'] != 1000 or s['rows'] != 24 or s['selected_index'] != 23 or not s['settled'] for s in measured['endpoints'])):
            raise ValueError('actual current controller/model endpoints under import required')
        generations = [s['first_cover_reloads'] for s in measured['endpoints']]
        if generations[0] <= evicted['first_cover_reloads'] + 1 or any(b <= a for a, b in zip(generations, generations[1:])):
            raise ValueError('each retained completion must observe a new cold first-cover generation')
        if (len(measured['input_ms']) != 64 or len(measured['navigation_ms']) != 20
                or measured['input_p95_ms'] != sorted(measured['input_ms'])[60]
                or measured['navigation_p95_ms'] != sorted(measured['navigation_ms'])[18]
                or measured['pointer_p95_ms'] != sorted(measured['pointer_ms'])[60] or measured['pointer_p95_ms'] > 100
                or measured['input_p95_ms'] > 100 or measured['navigation_p95_ms'] > 3000):
            raise ValueError('retained native input/completion wall gate failed')
        (destination / 'finish').touch()
        code = process.wait(timeout=20)
        log.close()
        joined = code == 0 and 'scale_complete; workers_joined=true' in (destination / 'execution.log').read_text()
        if not joined:
            raise ValueError('positive current-controller worker shutdown required')
        owner.stdin.write('workers_joined\n')
        owner.stdin.close()
        if owner.wait(timeout=15) != 0 or Path(prepared['owner']).exists():
            raise ValueError('positive fixture retirement required')
        surfaces = set(re.findall(r'Skia renderer \(skia backend ([^;]+);', (destination / 'execution.log').read_text()))
        if len(surfaces) != 1 or ('software' in surfaces) != (backend == 'winit-skia-software'):
            raise ValueError('requested native rendering path was not actually selected')
        report['active_surface'] = sorted(surfaces)
        report['complete'] = True
    finally:
        if process is not None and process.poll() is None:
            (destination / 'finish').touch()
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=10)
        if not joined and process is not None and process.returncode == 0:
            joined = 'scale_complete; workers_joined=true' in (destination / 'execution.log').read_text()
        if owner.poll() is None:
            owner.stdin.write('workers_joined\n' if joined else '')
            owner.stdin.close()
            owner.wait(timeout=15)
        report['workers_joined'] = joined
        report['fixture_retired'] = not Path(prepared['owner']).exists()
        (destination / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return report
