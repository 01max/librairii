#!/usr/bin/env python3
"""Fixed software-input diagnostics of sealed binaries; never qualification."""
import argparse
from contextlib import contextmanager, nullcontext
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SPECS = {
    'baseline': {
        'artifact_id': 13510371418,
        'source': '799a518d9452ea351140f2f3d3674e67a8a46dea',
        'manifest': 'c04428e117a8d3d4b305601188a8d861243dadd3a8da0284cf16c8fc1d173f81',
        'lock': 'c4a2ffd16f8f929b8aa57b01cd0c256ecd187b8f4bd790fb45299ad75eb68225',
        'binary': 'c0b283bdaf721edefdad16604241e8e5105c8b58ee426399f822490dfc774998',
    },
    'candidate': {
        'artifact_id': 13536270170,
        'source': 'fbac0356e4e9e8bed9f4a62ef43c216a9fce5620',
        'manifest': 'd0f7c9a01fc4509edb383bde7415cc5b14f4b1ba4bb9a5c5df2484c8e25623d2',
        'lock': '2a7fbcd1421472927e0a734a77529b24f95fd2086f1ce6c232e8385861e12da8',
        'binary': '68b98f05651c441f13e8ac1a2cb3792c29a4bc31b2eb73277d0840e32cd21f7a',
    },
}
HELPER_SHA = 'f7c1ba77bef29b092b4f569dbf08cb73b279fd4dc4bf2a29c5544a5a6041ff66'
ORDER = ('candidate', 'baseline', 'baseline', 'candidate')
NUMERICAL_FAILURE = 'retained native dispatch/completion gate failed'


def require(value, message):
    if not value:
        raise ValueError(message)


def checked(path, expected):
    require(path.is_file() and not path.is_symlink() and path.resolve() == path.absolute(),
            'regular non-symlink input required: ' + str(path))
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == expected, 'input checksum differs: ' + str(path))
    return data


def verify(input_root):
    roots = {}
    for label, pins in SPECS.items():
        root = input_root / label / 'resources'
        manifest = json.loads(checked(root / 'manifest.json', pins['manifest']))
        require(manifest['source_commit'] == pins['source']
                and manifest['target'] == 'x86_64-apple-darwin'
                and manifest['cargo_lock_sha256'] == pins['lock'], 'source/target/lock differs')
        for name, sha in [('scale-native', pins['binary']), ('input-macos', HELPER_SHA)]:
            checked(root / name, sha)
            require(manifest['files'][name] == sha, 'manifest executable checksum differs')
        roots[label] = root
    return roots


def module():
    spec = importlib.util.spec_from_file_location('pointer_diagnostic_native', ROOT / 'qualification/section-16/native.py')
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def require_host():
    require(platform.system() == 'Darwin' and platform.machine() == 'x86_64'
            and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
            and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted',
            'native execution requires this disposable GitHub-hosted Intel runner')


def host_snapshot(path):
    result = subprocess.run(['/bin/ps', '-axo', 'pid,ppid,%cpu,%mem,comm'], capture_output=True, check=True, timeout=5)
    require(len(result.stdout) <= 1048576, 'host process snapshot exceeds bound')
    path.write_bytes(result.stdout)


@contextmanager
def input_profile(native, destination, helper):
    """Sample only the observed owned app while its input helper runs."""
    require_host()
    original = native.subprocess
    sampler = None
    owned_process = None
    log = None
    body_failed = False
    profile = destination / 'input-profile.txt'

    class ProcessCalls:
        def __getattr__(self, name):
            return getattr(original, name)

        def Popen(self, command, *args, **kwargs):
            nonlocal owned_process
            if kwargs.get('start_new_session'):
                require(owned_process is None, 'only one owned app session may be profiled')
            process = original.Popen(command, *args, **kwargs)
            if kwargs.get('start_new_session'):
                owned_process = process
            return process

        def run(self, command, *args, **kwargs):
            nonlocal sampler, log
            require(isinstance(command, list) and len(command) == 3
                    and command[0] == str(helper) and command[2] == str(destination),
                    'profile requires the exact owned input helper invocation')
            pid = int(command[1])
            observed = json.loads((destination / 'state.json').read_text())
            require(sampler is None and pid == observed['pid'] and pid > 1
                    and owned_process is not None and owned_process.poll() is None
                    and os.getpgid(pid) == owned_process.pid
                    and owned_process.pid != os.getpgrp(), 'profile target is not the observed owned app')
            log = (destination / 'input-profile-command.log').open('wb')
            sampler = original.Popen(['/usr/bin/sample', str(pid), '15', '10', '-file', str(profile)],
                                     stdout=log, stderr=original.STDOUT)
            return original.run(command, *args, **kwargs)

    native.subprocess = ProcessCalls()
    try:
        yield
    except BaseException:
        body_failed = True
        raise
    finally:
        native.subprocess = original
        error = None
        try:
            if sampler is not None:
                try:
                    sampler.wait(timeout=25)
                except original.TimeoutExpired:
                    sampler.terminate()
                    try:
                        sampler.wait(timeout=5)
                    except original.TimeoutExpired:
                        sampler.kill()
                        sampler.wait(timeout=5)
                    raise ValueError('owned input profiler exceeded its deadline')
            require(sampler is not None and sampler.returncode == 0, 'owned input profiler did not complete')
            require(profile.is_file() and 0 < profile.stat().st_size <= 16777216,
                    'bounded input profile required')
        except Exception as failure:
            error = str(failure)
            if not body_failed:
                raise
        finally:
            if log is not None:
                log.close()
            if destination.is_dir():
                (destination / 'profile-status.json').write_text(json.dumps({
                    'complete': error is None, 'error': error, 'observation_failed': body_failed,
                    'acceptance_evidence': False, 'profiling_perturbs_timings': True,
                }, indent=2) + '\n')


def observe(native, binary, destination, helper):
    error = None
    try:
        report = native.inspect(binary, destination, 'winit-skia-software', helper)
    except Exception as failure:
        error = str(failure)
        path = destination / 'report.json'
        report = json.loads(path.read_text()) if path.is_file() else {}
        (destination / 'diagnostic-error.json').write_text(json.dumps({'type': type(failure).__name__, 'error': error}) + '\n')
    require(report.get('workers_joined') is True and report.get('fixture_retired') is True,
            'no next trial without positive worker shutdown and fixture retirement')
    require(error is None or error == NUMERICAL_FAILURE, 'non-numerical observation failure: ' + str(error))
    measured = report['input']
    native.dispatch_protocol().verify_events(measured, json.loads((destination / 'dispatch-events.json').read_text()))
    surfaces = set(re.findall(r'Skia renderer \(skia backend ([^;]+);', (destination / 'execution.log').read_text()))
    require(surfaces == {'software'}, 'requested software rendering path was not actually selected')
    require(report.get('complete') is (error is None), 'completion flag differs from retained verdict')
    return {
        'error': error, 'original_gate_passed': error is None,
        'pointer_p95_ms': measured['pointer_p95_ms'], 'search_p95_ms': measured['input_p95_ms'],
        'completion_p95_ms': measured['navigation_p95_ms'],
        'pointer_p95_lower_ms': measured['pointer_dispatch_bounds']['p95_lower_ms'],
        'pointer_p95_upper_ms': measured['pointer_dispatch_bounds']['p95_upper_ms'],
        'workers_joined': True, 'fixture_retired': True, 'all_dispatch_witnesses_verified': True,
        'active_surface': sorted(surfaces),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-root', type=Path, required=True)
    parser.add_argument('--verify-only', action='store_true')
    parser.add_argument('--profile-input', action='store_true',
                        help='Sample one candidate input workload; perturbs timing and never qualifies it')
    args = parser.parse_args()
    roots = verify(args.input_root.absolute())
    if args.verify_only:
        print(json.dumps({'verified': list(roots), 'native_executed': False}))
        return
    require_host()
    output = ROOT / 'target/pointer-comparison/observations'
    output.mkdir(parents=True, exist_ok=False)
    (output / 'host').mkdir()
    order = ('candidate',) if args.profile_input else ORDER
    report = {
        'format': 'librairii.dependency-pointer-diagnostic', 'schema': 1, 'complete': False,
        'acceptance_evidence': False, 'causal_fix_proven': False, 'original_failed_sample_replaced': False,
        'diagnostic_source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'inputs': SPECS, 'shared_helper_sha256': HELPER_SHA, 'order': order, 'samples': [],
        'input_profiled': args.profile_input, 'profiling_perturbs_timings': args.profile_input,
        'scope': 'Four fixed full loaded-library input workloads; same host and helper, fresh synthetic profile per trial. Software renderer only. Original GPU/ordinary-startup prelude omitted. Not full qualification or a cause/repair claim.',
        'retained_failures': [42229788963, 42273630563],
    }
    if args.profile_input:
        report['scope'] = 'One full loaded-library candidate software input workload with a 15-second, 10ms-interval sampling profile starting at input-helper invocation. Timings are perturbed and cannot qualify or compare performance. Original GPU/ordinary-startup prelude omitted.'
    destination = output / 'report.json'
    try:
        native = module()
        for root in roots.values():
            for name in ('scale-native', 'input-macos'):
                (root / name).chmod(0o700)
        for index, label in enumerate(order):
            name = f'{index:02d}-{label}'
            host_snapshot(output / 'host' / (name + '-before.txt'))
            trial = output / name
            helper = roots['candidate'] / 'input-macos'
            with input_profile(native, trial, helper) if args.profile_input else nullcontext():
                row = observe(native, roots[label] / 'scale-native', trial, helper)
            row.update(index=index, label=label, raw_report=name + '/report.json', profiled_timing=args.profile_input)
            report['samples'].append(row)
            destination.write_text(json.dumps(report, indent=2) + '\n')
            host_snapshot(output / 'host' / (name + '-after.txt'))
            print(json.dumps(row), flush=True)
        require(len(report['samples']) == len(order), 'all predeclared trials required')
        verify(args.input_root.absolute())
        report['complete'] = True
    finally:
        report['files'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                           for p in output.rglob('*') if p.is_file() and p != destination}
        destination.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
