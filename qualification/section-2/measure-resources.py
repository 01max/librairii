#!/usr/bin/env python3
"""Measure trusted native probes; readiness is not visible presentation or OS input."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import re
import selectors
import signal
import statistics
import subprocess
import tarfile
import time

spec = importlib.util.spec_from_file_location('runtime_inspection', pathlib.Path(__file__).with_name('inspect-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def cpu_seconds(value):
    days, clock = value.split('-', 1) if '-' in value else ('0', value)
    pieces = clock.split(':')
    if len(pieces) not in (2, 3):
        raise ValueError('invalid process CPU time')
    result = 0.0
    for piece in pieces:
        result = result * 60 + float(piece)
    return int(days) * 86400 + result


def sample_process(pid, system):
    if system == 'Linux':
        fields = pathlib.Path(f'/proc/{pid}/stat').read_text().rpartition(')')[2].split()
        # fields begin at proc stat column 3; utime/stime are columns 14/15.
        cpu = (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')
        rss = int(fields[21]) * os.sysconf('SC_PAGE_SIZE')
    else:
        result = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'pid=,time=,rss='],
                                capture_output=True, text=True, check=True, timeout=1,
                                env={**os.environ, 'LC_ALL': 'C', 'LANG': 'C'})
        observed, clock, rss_kib = result.stdout.split()
        if int(observed) != pid:
            raise ValueError('unexpected sampled PID')
        cpu, rss = cpu_seconds(clock), int(rss_kib) * 1024
    return {'cpu_seconds': cpu, 'rss_bytes': rss}


def time_metrics(text, system):
    if system == 'Darwin':
        def value(pattern):
            found = re.search(pattern, text, re.M)
            return float(found[1]) if found else None
        peak = value(r'^\s*(\d+)\s+maximum resident set size\s*$')
        return {'wall_seconds': value(r'^real\s+([0-9.]+)\s*$'),
                'user_seconds': value(r'^user\s+([0-9.]+)\s*$'),
                'system_seconds': value(r'^sys\s+([0-9.]+)\s*$'),
                'peak_rss_bytes': int(peak) if peak is not None else None}
    found = re.search(r'^qualification_time; wall=([0-9.]+); user=([0-9.]+); system=([0-9.]+); maxrss_kib=(\d+)\s*$', text, re.M)
    if not found:
        return dict.fromkeys(('wall_seconds', 'user_seconds', 'system_seconds', 'peak_rss_bytes'))
    return {'wall_seconds': float(found[1]), 'user_seconds': float(found[2]),
            'system_seconds': float(found[3]), 'peak_rss_bytes': int(found[4]) * 1024}


def analyze(run, system):
    text = run['output']
    native = time_metrics(text, system)
    # Discard settling and the last half-second before workload delivery. The
    # latter avoids mixing the phase transition with observer/ps latency.
    end = run['idle_end_seconds']
    samples = [s for s in run['samples'] if s['since_ready_seconds'] >= 2
               and end is not None and s['since_ready_seconds'] <= end - 0.5
               and s['collection_seconds'] <= 0.1]
    duration = samples[-1]['monotonic_seconds'] - samples[0]['monotonic_seconds'] if len(samples) >= 2 else 0
    monotonic_cpu = all(b['cpu_seconds'] >= a['cpu_seconds'] for a, b in zip(samples, samples[1:]))
    idle = None
    if duration > 0 and monotonic_cpu:
        idle = {'sample_count': len(samples), 'wall_seconds': duration,
                'cpu_seconds': samples[-1]['cpu_seconds'] - samples[0]['cpu_seconds'],
                'cpu_percent_one_core': 100 * (samples[-1]['cpu_seconds'] - samples[0]['cpu_seconds']) / duration,
                'rss_bytes_min': min(s['rss_bytes'] for s in samples),
                'rss_bytes_max': max(s['rss_bytes'] for s in samples)}
    surface = re.findall(r'Skia renderer \(skia backend ([^;]+);', text)
    checks = {
        'normal_exit': run['returncode'] == 0 and not run['timed_out'],
        'one_ready_and_idle_marker': run['ready_markers'] == 1 and run['idle_end_markers'] == 1,
        'owned_process_sampled': not run['errors'],
        'sustained_idle_window': len(samples) >= 20 and duration >= 6 and monotonic_cpu,
        'native_time_complete': all(value is not None and value >= 0 for value in native.values())
                                and (native['peak_rss_bytes'] or 0) > 0,
        'active_surface_recorded': len(surface) == 1,
        'thumbnail_workload_completed': 'thumbnail_smoke=passed;' in text,
        'owned_shutdown_completed': 'resource_probe_complete; workers_joined=true; fixture_retired=true' in text,
    }
    if run['backend_request'] == 'winit-skia-software':
        checks['requested_cpu_surface_selected'] = surface == ['software']
    return {'checks': checks, 'measurement': 'pass' if all(checks.values()) else 'fail',
            'native_time': native, 'idle': idle, 'active_surface': surface,
            'launch_to_ready_marker_ms': run['launch_to_ready_marker_ms']}


def run_probe(binary, backend, system, timeout=40):
    args = ['/usr/bin/time', '-l', '-p'] if system == 'Darwin' else [
        '/usr/bin/time', '-f', 'qualification_time; wall=%e; user=%U; system=%S; maxrss_kib=%M']
    args += [str(binary), '--resource-probe']
    env = {**os.environ, 'LC_ALL': 'C', 'LANG': 'C', 'SLINT_BACKEND': backend,
           'SLINT_DEBUG_PERFORMANCE': 'refresh_lazy,console'}
    started = time.monotonic()
    run = {'argv': args, 'backend_request': backend, 'returncode': None, 'timed_out': False,
           'output': '', 'ready_markers': 0, 'idle_end_markers': 0,
           'idle_end_seconds': None, 'launch_to_ready_marker_ms': None, 'samples': [], 'errors': []}
    try:
        process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   env=env, start_new_session=True)
    except OSError as error:
        run['errors'].append(str(error))
        run['analysis'] = analyze(run, system)
        return run
    ready, pid, last_sample, pending = None, None, 0.0, b''
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    output = bytearray()
    try:
        while selector.get_map() or process.poll() is None:
            now = time.monotonic()
            if now - started > timeout:
                run['timed_out'] = True
                break
            for key, _ in selector.select(timeout=0.05):
                data = os.read(key.fd, 65536)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                output.extend(data)
                if len(output) > 1_048_576:
                    run['errors'].append('probe output exceeded 1 MiB')
                    raise ValueError('probe output limit')
                pending += data
                while b'\n' in pending:
                    line, pending = pending.split(b'\n', 1)
                    line = line.decode('utf-8', errors='replace')
                    if line.startswith('resource_probe_ready;'):
                        run['ready_markers'] += 1
                        match = re.fullmatch(r'resource_probe_ready; pid=(\d+); scale=([0-9.e+-]+); size=(\d+)x(\d+)', line)
                        if not match or run['ready_markers'] != 1:
                            raise ValueError('invalid or repeated readiness marker')
                        pid = int(match[1])
                        if os.getpgid(pid) != process.pid:
                            raise ValueError('readiness PID is outside the owned process group')
                        ready = time.monotonic()
                        run['launch_to_ready_marker_ms'] = (ready - started) * 1000
                        run['ready'] = {'pid': pid, 'scale': float(match[2]),
                                        'width': int(match[3]), 'height': int(match[4])}
                    if line == 'resource_probe_idle_end':
                        run['idle_end_markers'] += 1
                        run['idle_end_seconds'] = time.monotonic() - ready if ready else None
            now = time.monotonic()
            if ready is not None and run['idle_end_seconds'] is None and now - ready >= 2 and now - last_sample >= 0.25:
                sample_started = now
                stats = sample_process(pid, system)
                now = time.monotonic()
                stats.update({'monotonic_seconds': now, 'since_ready_seconds': now - ready,
                              'collection_seconds': now - sample_started})
                run['samples'].append(stats)
                last_sample = now
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        run['errors'].append(str(error))
    finally:
        # An exited time wrapper can leave a descendant with closed output
        # pipes. Its exit code, including zero, does not prove owned teardown.
        if process.poll() is not None:
            try:
                os.killpg(process.pid, 0)
            except ProcessLookupError:
                pass
            else:
                run['errors'].append('owned process group survives exited wrapper')
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)
        selector.close()
        process.stdout.close()
    run['returncode'] = process.returncode
    run['output'] = output[:1_048_576].decode('utf-8', errors='replace')
    run['analysis'] = analyze(run, system)
    return run


def file_identity(path):
    with path.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'file': path.name, 'bytes': path.stat().st_size, 'sha256': digest}


def archive_identity(path, package, manifest):
    package = package.resolve(strict=True)
    manifest_path = package / 'manifest.json'
    if json.loads(manifest_path.read_text()) != manifest:
        raise ValueError('archive manifest does not match measured package')
    expected = {package.name + '/manifest.json': file_identity(manifest_path)}
    for name, digest in manifest['files'].items():
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('unsafe archive payload path')
        local = package.joinpath(*relative.parts).resolve(strict=True)
        if not local.is_relative_to(package):
            raise ValueError('archive payload escapes measured package')
        expected[package.name + '/' + name] = {'bytes': local.stat().st_size, 'sha256': digest}
    directories = {str(parent) for name in expected for parent in pathlib.PurePosixPath(name).parents
                   if str(parent) != '.'}
    seen = set()
    with tarfile.open(path, 'r|gz') as archive:
        for member in archive:
            name = member.name.rstrip('/')
            relative = pathlib.PurePosixPath(name)
            if relative.is_absolute() or '..' in relative.parts or str(relative) != name or name in seen:
                raise ValueError('unsafe or duplicate archive member')
            seen.add(name)
            if member.isdir() and name in directories:
                continue
            if not member.isfile() or name not in expected:
                raise ValueError('unexpected or alias archive payload')
            identity = expected[name]
            if member.size != identity['bytes']:
                raise ValueError('archive manifest does not match measured package' if name.endswith('/manifest.json')
                                 else 'archive payload size changed')
            with archive.extractfile(member) as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            if digest != identity['sha256']:
                raise ValueError('archive manifest does not match measured package' if name.endswith('/manifest.json')
                                 else 'archive payload bytes changed')
    if not expected.keys() <= seen:
        raise ValueError('archive payload members missing')
    return file_identity(path)


def package_bytes(package, manifest):
    package = package.resolve(strict=True)
    total = (package / 'manifest.json').stat().st_size
    for name, digest in manifest['files'].items():
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('unsafe package member')
        path = package.joinpath(*relative.parts)
        if not path.resolve(strict=True).is_relative_to(package):
            raise ValueError('package member escapes measured package')
        identity = file_identity(path)
        if identity['sha256'] != digest:
            raise ValueError('package member bytes changed')
        total += identity['bytes']
    return total


def hardware(system):
    if system == 'Linux':
        cpu = re.search(r'^model name\s*:\s*(.+)$', pathlib.Path('/proc/cpuinfo').read_text(), re.M)
        memory = re.search(r'^MemTotal:\s*(\d+) kB$', pathlib.Path('/proc/meminfo').read_text(), re.M)
        if not cpu or not memory:
            raise ValueError('native CPU/memory identity unavailable')
        return {'cpu': cpu[1], 'memory_bytes': int(memory[1]) * 1024}
    def sysctl(name):
        return subprocess.check_output(['/usr/sbin/sysctl', '-n', name], text=True, timeout=2).strip()
    return {'cpu': sysctl('machdep.cpu.brand_string'), 'memory_bytes': int(sysctl('hw.memsize'))}


def measure(package, output, archive, disk_image, samples, backends):
    manifest = json.loads((package / 'manifest.json').read_text())
    system = platform.system()
    if (system, platform.machine()) != runtime.TARGETS[manifest['target']]:
        raise ValueError('measurement requires the matching native host')
    binary = runtime.verified_binary(package, manifest)
    distribution = {'executable_bytes': binary.stat().st_size, 'manifest_payload_bytes': package_bytes(package, manifest),
                    'archive': archive_identity(archive, package, manifest) if archive else None,
                    'disk_image': file_identity(disk_image) if disk_image else None,
                    'disk_image_scope': 'supplied builder output size/hash; contents not re-mounted here'}
    output.mkdir(parents=True)
    report = {'schema': 1, 'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
              'target': manifest['target'], 'binary_sha256': manifest['binary_sha256'],
              'host': {'platform': platform.platform(), 'machine': platform.machine(),
                       'logical_cpus': os.cpu_count(), **hardware(system)}, 'distribution': distribution,
              'session': {key: os.environ.get(key) for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'XDG_SESSION_TYPE')},
              'method': {'startup': 'parent launch to first owned UI timer after show; includes observer delivery; not first presentation',
                         'idle': 'cumulative process CPU delta; 100% is one core; normal UI poll timer retained',
                         'settling_seconds': 2, 'end_guard_seconds': 0.5, 'minimum_idle_seconds': 6,
                         'sampling': '/proc stat on Linux; ps time/rss on macOS; raw units converted to bytes',
                         'peak': 'native time maximum RSS during quiet phase plus 1000-PNG/component-input/background workload',
                         'repetition': 'one complete warmup per requested backend, then independent fresh processes; no disk-cache flush',
                         'diagnostics': 'SLINT_DEBUG_PERFORMANCE=refresh_lazy,console'},
              'runs': {}, 'summary': {}, 'automated_resource_measurement': 'pending',
              'physical_input_qualified': False, 'first_presentation_qualified': False,
              'retained_performance_budgets_qualified': False, 'additional_budgets_set': False,
              'toolkit_selected': False}
    destination = output / 'resource-measurements.json'
    for backend in backends:
        runs = report['runs'][backend] = []
        for index in range(samples + 1):
            run = run_probe(binary, backend, system)
            run['kind'] = 'warmup' if index == 0 else 'measured'
            runs.append(run)
            destination.write_text(json.dumps(report, indent=2) + '\n')
            print(f'{backend} {run["kind"]} {index}: {run["analysis"]["measurement"]}', flush=True)
            if run['analysis']['measurement'] != 'pass':
                break  # Keep the failure, do not substitute a retry/sample.
        measured = [run for run in runs if run['kind'] == 'measured' and run['analysis']['measurement'] == 'pass']
        if len(measured) == samples:
            summary = report['summary'][backend] = {'samples': samples}
            values = {
                'launch_to_ready_marker_ms': [run['analysis']['launch_to_ready_marker_ms'] for run in measured],
                'idle_cpu_percent_one_core': [run['analysis']['idle']['cpu_percent_one_core'] for run in measured],
                'peak_rss_bytes': [run['analysis']['native_time']['peak_rss_bytes'] for run in measured],
            }
            for name, items in values.items():
                summary[name] = {'min': min(items), 'median': statistics.median(items), 'max': max(items)}
    passed = len(report['summary']) == len(backends)
    report['automated_resource_measurement'] = 'pass' if passed else 'fail'
    destination.write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True, type=pathlib.Path)
    parser.add_argument('--output', required=True, type=pathlib.Path)
    parser.add_argument('--archive', type=pathlib.Path)
    parser.add_argument('--disk-image', type=pathlib.Path)
    parser.add_argument('--samples', type=int, choices=range(3, 11), default=3)
    parser.add_argument('--backend', action='append', choices=['winit-skia', 'winit-skia-software'])
    args = parser.parse_args()
    result = measure(args.package.resolve(strict=True), args.output, args.archive, args.disk_image,
                     args.samples, list(dict.fromkeys(args.backend or ['winit-skia', 'winit-skia-software'])))
    raise SystemExit(0 if result['automated_resource_measurement'] == 'pass' else 1)
