"""Actual production release startup/idle and loaded current-view peak RSS."""
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('retained_resource_clocks', HERE.parent / 'section-2/measure-resources.py')
clocks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(clocks)


def child_pid(parent):
    if platform.system() == 'Linux':
        values = Path(f'/proc/{parent}/task/{parent}/children').read_text().split()
    else:
        values = [line.split()[0] for line in subprocess.check_output(['/bin/ps', '-ax', '-o', 'pid=,ppid='], text=True).splitlines()
                  if line.split()[1] == str(parent)]
    return int(values[0]) if len(values) == 1 else None


def product_run(binary, observer, destination, backend, helper, native):
    destination.mkdir()
    configuration = destination / 'fixture.json'
    owner = subprocess.Popen([str(observer), '--prepare', str(configuration)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=(destination / 'prepare.log').open('w'), text=True)
    prepared = json.loads(owner.stdout.readline())
    config = json.loads(configuration.read_text())
    environment = dict(os.environ, HOME=config['home'], XDG_CONFIG_HOME=config['config'], XDG_DATA_HOME=config['data'],
                       SLINT_BACKEND=backend, SLINT_DEBUG_PERFORMANCE='refresh_lazy,console')
    environment.pop('LIBRAIRII_DATA_ROOT', None)
    environment.pop('SLINT_SCALE_FACTOR', None)
    log = (destination / 'execution.log').open('w')
    metrics = destination / 'native-time.log'
    clock = ['/usr/bin/time', '-o', str(metrics)]
    clock += ['-l', '-p'] if platform.system() == 'Darwin' else ['-f', 'qualification_time; wall=%e; user=%U; system=%S; maxrss_kib=%M']
    process = None
    joined = False
    report = {'backend_request': backend, 'complete': False, 'samples': [], 'fixture_owner': prepared['owner']}
    try:
        started = time.monotonic()
        process = subprocess.Popen(clock + [str(binary)], env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        pid = native.wait(lambda: child_pid(process.pid), 5)
        if os.getpgid(pid) != process.pid:
            raise ValueError('actual production PID escaped the owned group')
        command = [str(helper), 'ready', str(pid)] if platform.system() == 'Darwin' else ['/usr/bin/python3', str(HERE / 'resource-linux.py'), str(pid)]
        ready = subprocess.run(command, capture_output=True, text=True, timeout=12)
        ready.check_returncode()
        report['launch_to_ready_ms'] = (time.monotonic() - started) * 1000
        report['ready'] = json.loads(ready.stdout)
        if report['ready']['pid'] != pid:
            raise ValueError('actual product readiness PID differs')
        idle_start = time.monotonic()
        # Fixed15-second quiet phase, excluding two settling seconds. Every raw
        # sample stays retained; no slow run is retried or substituted.
        while time.monotonic() - idle_start < 15:
            collection = time.monotonic()
            sample = clocks.sample_process(pid, platform.system())
            finished = time.monotonic()
            sample.update(monotonic_seconds=finished, since_ready_seconds=finished-idle_start,
                          collection_seconds=finished-collection)
            report['samples'].append(sample)
            time.sleep(.25)
        samples = [s for s in report['samples'] if 2 <= s['since_ready_seconds'] <= 14.5 and s['collection_seconds'] <= .1]
        if len(samples) < 20 or samples[-1]['monotonic_seconds'] - samples[0]['monotonic_seconds'] < 6:
            raise ValueError('sustained actual-product idle sample window missing')
        if any(b['cpu_seconds'] < a['cpu_seconds'] for a, b in zip(samples, samples[1:])):
            raise ValueError('process CPU clock moved backwards')
        report['idle_cpu_percent_one_core'] = 100 * (samples[-1]['cpu_seconds']-samples[0]['cpu_seconds']) / (samples[-1]['monotonic_seconds']-samples[0]['monotonic_seconds'])
        if platform.system() == 'Darwin':
            quit_result = subprocess.run([str(helper), 'quit', str(pid)], capture_output=True, text=True, timeout=12)
            quit_result.check_returncode()
            report['quit'] = json.loads(quit_result.stdout)
        else:
            xid = subprocess.check_output(['xdotool', 'search', '--sync', '--onlyvisible', '--pid', str(pid)], text=True, timeout=10).splitlines()[-1]
            subprocess.run(['xdotool', 'windowfocus', '--sync', xid, 'key', '--clearmodifiers', 'ctrl+q'], check=True, timeout=10)
            report['quit'] = {'native_quit_posted': True}
        joined = process.wait(timeout=20) == 0
        if not joined:
            raise ValueError('actual production main failed its joined shutdown')
        report['native_time'] = clocks.time_metrics(metrics.read_text(), platform.system())
        surface = set(re.findall(r'Skia renderer \(skia backend ([^;]+);', (destination / 'execution.log').read_text()))
        if len(surface) != 1 or ('software' in surface) != (backend == 'winit-skia-software'):
            raise ValueError('actual product renderer differs')
        report['active_surface'] = sorted(surface)
        report['complete'] = True
    finally:
        if process is not None and process.poll() is None:
            # Failure diagnostics survive; this group contains only our product.
            import signal
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=10)
        owner.stdin.write('workers_joined\n' if joined else '')
        owner.stdin.close()
        owner.wait(timeout=15)
        report['workers_joined'] = joined
        report['fixture_retired'] = not Path(prepared['owner']).exists()
        (destination / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    if not report['fixture_retired']:
        raise ValueError('actual product fixture was retained without positive retirement')
    return report


def inspect(binary, observer, destination, helper, native):
    destination.mkdir()
    reports = []
    for name, backend in [('gpu-api', 'winit-skia'), ('software', 'winit-skia-software')]:
        for trial in range(4):
            quiet = product_run(binary, observer, destination / f'{name}-{trial}-product', backend, helper, native)
            loaded = native.inspect(observer, destination / f'{name}-{trial}-loaded', backend,
                                    destination.parent / 'input-macos' if platform.system() == 'Darwin' else None)
            load_time = clocks.time_metrics((destination / f'{name}-{trial}-loaded/native-time.log').read_text(), platform.system())
            peaks = [quiet['native_time']['peak_rss_bytes'], load_time['peak_rss_bytes']]
            if any(type(n) is not int or n <= 0 for n in peaks):
                raise ValueError('native product and workload peak RSS receipts required')
            report = {'warmup': trial == 0, 'backend': backend, 'quiet': quiet, 'loaded': loaded,
                      'loaded_native_time': load_time, 'peak_rss_bytes': max(peaks)}
            if trial and (quiet['launch_to_ready_ms'] > 1000 or quiet['idle_cpu_percent_one_core'] > 5
                          or report['peak_rss_bytes'] > 512 * 1024 * 1024):
                reports.append(report)
                (destination / 'report.json').write_text(json.dumps(reports, indent=2) + '\n')
                raise ValueError('actual release startup/idle/loaded RSS gate failed; failed raw sample retained')
            reports.append(report)
            (destination / 'report.json').write_text(json.dumps(reports, indent=2) + '\n')
    return reports
