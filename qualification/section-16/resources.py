"""Actual production release startup/idle and loaded current-view peak RSS."""
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import select
import subprocess
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('retained_resource_clocks', HERE.parent / 'section-2/measure-resources.py')
clocks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(clocks)


class StartupCapture:
    """Drain ordinary product diagnostics and timestamp actual parent delivery."""
    def __init__(self, pipe, log, launched):
        self.markers = []
        self.errors = []
        self.lock = threading.Lock()
        self.observed = threading.Event()
        self.launched = launched
        self.thread = threading.Thread(target=self._read, args=(pipe, log), daemon=True)
        self.thread.start()

    def _read(self, pipe, log):
        pattern = rb'native_startup; stage=first_ui_timer; pid=(\d+); phase=(Opening|Setup|Adoption|Recovery|Library); size=(\d+)x(\d+); elapsed_ms=([0-9]+\.[0-9]{3})\n'
        total = 0
        try:
            with pipe, log.open('wb') as output:
                while line := pipe.readline(65_537):
                    delivered = time.monotonic()
                    total += len(line)
                    if total <= 1_048_576:
                        output.write(line)
                        output.flush()
                    with self.lock:
                        if (total > 1_048_576 or len(line) > 65_536) and not self.errors:
                            self.errors.append('ordinary product diagnostic output exceeded bound')
                        if total > 1_048_576 or len(line) > 65_536:
                            continue
                        if line.startswith(b'native_startup; stage=first_ui_timer;'):
                            match = re.fullmatch(pattern, line)
                            if match and len(self.markers) < 2:
                                self.markers.append({'pid': int(match[1]), 'phase': match[2].decode(),
                                    'width': int(match[3]), 'height': int(match[4]),
                                    'child_elapsed_ms': float(match[5]),
                                    'parent_launch_monotonic': self.launched,
                                    'parent_delivery_monotonic': delivered,
                                    'parent_launch_to_delivery_ms': (delivered-self.launched)*1000,
                                    'line': line.decode().rstrip('\n')})
                            elif not match and len(self.errors) < 8:
                                self.errors.append('malformed ordinary first UI timer diagnostic')
                            self.observed.set()
        except (OSError, ValueError) as error:
            with self.lock:
                self.errors.append(str(error))
            self.observed.set()

    def receipt(self, pid):
        self.observed.wait(1)
        with self.lock:
            if self.errors or len(self.markers) != 1:
                raise ValueError('one bounded ordinary first UI timer diagnostic required: ' + str(self.errors))
            marker = dict(self.markers[0])
        if marker['pid'] != pid or marker['width'] <= 0 or marker['height'] <= 0:
            raise ValueError('ordinary first UI timer PID/window differs')
        return marker

    def join(self):
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise ValueError('ordinary product diagnostic reader did not finish')

    def finish(self, pid):
        self.join()
        return self.receipt(pid)


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
    capture = None
    pid = None
    metrics = destination / 'native-time.log'
    clock = ['/usr/bin/time', '-o', str(metrics)]
    clock += ['-l', '-p'] if platform.system() == 'Darwin' else ['-f', 'qualification_time; wall=%e; user=%U; system=%S; maxrss_kib=%M']
    process = None
    readiness = None
    joined = False
    report = {'backend_request': backend, 'complete': False, 'samples': [], 'fixture_owner': prepared['owner'],
              'startup_method_version': 2,
              'startup_method': 'parent launch to first ordinary UI timer delivery after show; Opening is not loaded Library',
              'ax_story_method': 'separate mandatory actual loaded-story observation before idle sampling'}
    try:
        preparing = time.monotonic()
        command = [str(helper), 'watch'] if platform.system() == 'Darwin' else ['/usr/bin/python3', str(HERE / 'resource-linux.py'), '--watch']
        readiness = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=(destination / 'readiness.log').open('w'), text=True)
        if not select.select([readiness.stdout], [], [], 12)[0] or json.loads(readiness.stdout.readline()) != {'observer_ready': True}:
            raise ValueError('external native observer initialization failed')
        report['observer_prepare_ms'] = (time.monotonic()-preparing)*1000
        started = time.monotonic()
        report['parent_launch_monotonic'] = started
        process = subprocess.Popen(clock + [str(binary)], env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        capture = StartupCapture(process.stdout, destination / 'execution.log', started)
        pid = native.wait(lambda: child_pid(process.pid), 5)
        if os.getpgid(pid) != process.pid:
            raise ValueError('actual production PID escaped the owned group')
        report['launch_to_pid_ms'] = (time.monotonic()-started)*1000
        readiness.stdin.write(str(pid)+'\n')
        readiness.stdin.close()
        readiness.stdin = None
        observed, _ = readiness.communicate(timeout=12)
        if readiness.returncode != 0:
            raise ValueError('actual product readiness failed; see retained readiness.log')
        report['ax_story_observed_monotonic'] = time.monotonic()
        report['launch_to_ax_story_ms'] = (report['ax_story_observed_monotonic'] - started) * 1000
        report['ax_story'] = json.loads(observed)
        (destination / 'ax-story.json').write_text(observed)
        if report['ax_story']['pid'] != pid:
            raise ValueError('actual product readiness PID differs')
        report['first_ui_timer'] = capture.receipt(pid)
        report['launch_to_first_ui_timer_ms'] = report['first_ui_timer']['parent_launch_to_delivery_ms']
        idle_start = time.monotonic()
        report['idle_start_monotonic'] = idle_start
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
        if capture.finish(pid) != report['first_ui_timer']:
            raise ValueError('ordinary first UI timer diagnostic changed')
        report['native_time'] = clocks.time_metrics(metrics.read_text(), platform.system())
        surface = set(re.findall(r'Skia renderer \(skia backend ([^;]+);', (destination / 'execution.log').read_text()))
        if len(surface) != 1 or ('software' in surface) != (backend == 'winit-skia-software'):
            raise ValueError('actual product renderer differs')
        report['active_surface'] = sorted(surface)
        report['complete'] = True
    finally:
        if readiness is not None and readiness.poll() is None:
            readiness.terminate()
            try:
                readiness.wait(timeout=5)
            except subprocess.TimeoutExpired:
                readiness.kill()
                readiness.wait(timeout=5)
        if process is not None and process.poll() is None:
            # Failure diagnostics survive; this group contains only our product.
            import signal
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=10)
        if capture is not None:
            try:
                capture.join()
                if pid is not None:
                    report['first_ui_timer'] = capture.receipt(pid)
            except ValueError as error:
                report['startup_capture_error'] = str(error)
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
            if (quiet['launch_to_first_ui_timer_ms'] > 1000 or quiet['idle_cpu_percent_one_core'] > 5
                    or report['peak_rss_bytes'] > 512 * 1024 * 1024):
                reports.append(report)
                (destination / 'report.json').write_text(json.dumps(reports, indent=2) + '\n')
                raise ValueError('actual release startup/idle/loaded RSS gate failed; failed raw sample retained')
            reports.append(report)
            (destination / 'report.json').write_text(json.dumps(reports, indent=2) + '\n')
    return reports
