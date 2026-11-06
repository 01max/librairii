#!/usr/bin/env python3
"""Fixed-order startup diagnostics with explicit binary identities; never qualification."""
import argparse
from contextlib import contextmanager, nullcontext
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SPECS = {
    'baseline': {
        'artifact_id': 13510371418,
        'source': '799a518d9452ea351140f2f3d3674e67a8a46dea',
        'manifest': 'c04428e117a8d3d4b305601188a8d861243dadd3a8da0284cf16c8fc1d173f81',
        'lock': 'c4a2ffd16f8f929b8aa57b01cd0c256ecd187b8f4bd790fb45299ad75eb68225',
        'librairii-desktop': 'e93ce437374a53c76bd197eddec9a4ba5a58cc3c00eb22eee56adc096f74c614',
        'scale-native': 'c0b283bdaf721edefdad16604241e8e5105c8b58ee426399f822490dfc774998',
    },
    'candidate': {
        'artifact_id': 13516343300,
        'source': '1711a8b3774a19bc9de6c5cbd5b67eb935d6416c',
        'manifest': '788f30f827b56f0c093c99e8bf80faba480101fb03722eb51092fe8e7aea8e36',
        'lock': '2a7fbcd1421472927e0a734a77529b24f95fd2086f1ce6c232e8385861e12da8',
        'librairii-desktop': 'c1475e7a7040bc9472369641b0405825f589789b8413399aa867ce925bc23034',
        'scale-native': '68b98f05651c441f13e8ac1a2cb3792c29a4bc31b2eb73277d0840e32cd21f7a',
    },
}
HELPER_SHA = 'b3b43bbdd784c7b61ebeeb7f600326f2011954b763f0bd8a1d4714e8eda38fc9'
ORDER = ('candidate', 'baseline', 'baseline', 'candidate',
         'baseline', 'candidate', 'candidate', 'baseline')
BACKENDS = ('winit-skia', 'winit-skia-software')


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
        for name, sha in [('librairii-desktop', pins['librairii-desktop']),
                          ('scale-native', pins['scale-native']), ('resource-macos', HELPER_SHA)]:
            checked(root / name, sha)
            require(manifest['files'][name] == sha, 'manifest executable checksum differs')
        roots[label] = root
    return roots


def module(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def host_snapshot(path):
    result = subprocess.run(['/bin/ps', '-axo', 'pid,ppid,%cpu,%mem,comm'],
                            capture_output=True, check=True, timeout=5)
    require(len(result.stdout) <= 1048576, 'host process snapshot exceeds bound')
    path.write_bytes(result.stdout)


def build_instrumented(output, source):
    require(not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT),
            'instrumented build requires clean committed source')
    lock = hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest()
    require(lock == SPECS['candidate']['lock'], 'instrumented build must retain the candidate lockfile')
    folder = output / 'instrumented'
    folder.mkdir()
    command = ['cargo', 'build', '--locked', '--release', '--target', 'x86_64-apple-darwin',
               '-p', 'librairii-desktop', '--bin', 'librairii-desktop', '--message-format=json']
    with (folder / 'compiler-artifacts.jsonl').open('w') as out, (folder / 'build.log').open('w') as err:
        subprocess.run(command, cwd=ROOT, stdout=out, stderr=err, check=True, timeout=5400)
    events = [json.loads(line) for line in (folder / 'compiler-artifacts.jsonl').read_text().splitlines()]
    matches = [e for e in events if e.get('reason') == 'compiler-artifact'
               and e.get('target', {}).get('name') == 'librairii-desktop'
               and e['target']['kind'] == ['bin'] and e.get('executable')]
    require(len(matches) == 1 and matches[0]['profile']['opt_level'] == '3'
            and not matches[0]['profile']['debug_assertions']
            and not matches[0]['profile']['overflow_checks'], 'exact release binary required')
    built = Path(matches[0]['executable'])
    expected = ROOT / 'target/x86_64-apple-darwin/release/librairii-desktop'
    require(built == expected and built.is_file() and not built.is_symlink(), 'unexpected build output')
    binary = folder / 'librairii-desktop'
    shutil.copy2(built, binary)
    require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == source
            and not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT)
            and hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest() == lock,
            'instrumented source changed during build')
    return binary, {'kind': 'instrumented diagnostic build', 'source': source,
                    'cargo_lock_sha256': lock, 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                    'bytes': binary.stat().st_size, 'profile': matches[0]['profile'],
                    'compiler': subprocess.check_output(['rustc', '-vV'], cwd=ROOT, text=True),
                    'command': command, 'archived_observer_source': SPECS['candidate']['source']}


@contextmanager
def startup_sample(resources, output):
    """Sample only the discovered owned product PID; these timings are perturbed."""
    original = resources.child_pid
    sampler = None
    with (output / 'startup-profile-command.log').open('wb') as log:
        def observed_child(parent):
            nonlocal sampler
            pid = original(parent)
            if pid is not None and sampler is None:
                require(os.getpgid(pid) == parent, 'profile target escaped owned product group')
                sampler = subprocess.Popen(
                    ['/usr/bin/sample', str(pid), '3', '1', '-file', str(output / 'startup-profile.txt')],
                    stdout=log, stderr=subprocess.STDOUT)
            return pid
        resources.child_pid = observed_child
        try:
            yield
        finally:
            resources.child_pid = original
            if sampler is not None:
                try:
                    sampler.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    sampler.terminate()
                    try:
                        sampler.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        sampler.kill()
                        sampler.wait(timeout=5)
                    raise ValueError('owned startup profiler exceeded its deadline')
            require(sampler is not None and sampler.returncode == 0, 'startup profiler did not complete')
            profile = output / 'startup-profile.txt'
            require(profile.is_file() and 0 < profile.stat().st_size <= 16777216,
                    'bounded startup profile required')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-root', type=Path, required=True)
    parser.add_argument('--verify-only', action='store_true')
    parser.add_argument('--sample-first', action='store_true',
                        help='Profile the first owned app launch; perturbs timing and never qualifies it')
    parser.add_argument('--build-instrumented', action='store_true',
                        help='Build and identify this committed diagnostic source instead of executing the archived candidate app')
    args = parser.parse_args()
    require(not (args.sample_first and args.build_instrumented), 'use one instrumentation method at a time')
    roots = verify(args.input_root.absolute())
    if args.verify_only:
        print(json.dumps({'verified': list(roots), 'executed': False}))
        return
    require(platform.system() == 'Darwin' and platform.machine() == 'x86_64'
            and os.environ.get('GITHUB_ACTIONS') == 'true'
            and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii',
            'execution is restricted to the disposable native Intel CI host')
    output = ROOT / 'target/startup-comparison/observations'
    output.mkdir(parents=True, exist_ok=False)
    (output / 'host').mkdir()
    resources = module('diagnostic_resources', 'qualification/section-16/resources.py')
    native = module('diagnostic_native', 'qualification/section-16/native.py')
    for root in roots.values():
        for name in ('librairii-desktop', 'scale-native', 'resource-macos'):
            (root / name).chmod(0o700)
    helper = roots['candidate'] / 'resource-macos'
    report = {
        'format': 'librairii.dependency-startup-diagnostic', 'schema': 1,
        'acceptance_evidence': False, 'causal_fix_proven': False, 'complete': False,
        'original_failed_sample_replaced': False,
        'first_launch_profiled': args.sample_first,
        'profiling_perturbs_first_launch_timing': args.sample_first,
        'candidate_override': None,
        'scope': 'Pinned archived baseline and explicitly identified candidate binary; fresh synthetic profile per launch; no qualification prelude or loaded-workload RSS claim.',
        'diagnostic_source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'inputs': SPECS, 'shared_helper_sha256': HELPER_SHA,
        'order_per_backend': ORDER, 'backends': BACKENDS, 'samples': [],
    }
    destination = output / 'report.json'
    try:
        candidate_binary = roots['candidate'] / 'librairii-desktop'
        if args.build_instrumented:
            candidate_binary, report['candidate_override'] = build_instrumented(output, report['diagnostic_source'])
            destination.write_text(json.dumps(report, indent=2) + '\n')
        for backend in BACKENDS:
            for label in ORDER:
                number = len(report['samples'])
                name = f'{number:02d}-{label}-{backend}'
                host_snapshot(output / 'host' / (name + '-before.txt'))
                root = roots[label]
                with startup_sample(resources, output) if args.sample_first and number == 0 else nullcontext():
                    quiet = resources.product_run(candidate_binary if label == 'candidate' else root / 'librairii-desktop', root / 'scale-native',
                                                  output / name, backend, helper, native)
                require(quiet['complete'] and quiet['workers_joined'] and quiet['fixture_retired'],
                        'positive shutdown and fixture retirement required before next trial')
                host_snapshot(output / 'host' / (name + '-after.txt'))
                log = (output / name / 'execution.log').read_text()
                row = {
                    'number': number, 'label': label, 'backend': backend,
                    'profiled_timing': args.sample_first and number == 0,
                    'instrumented_binary': args.build_instrumented and label == 'candidate',
                    'startup_ms': quiet['launch_to_first_ui_timer_ms'],
                    'within_original_startup_limit': quiet['launch_to_first_ui_timer_ms'] <= 1000,
                    'idle_cpu_percent': quiet['idle_cpu_percent_one_core'],
                    'stages': re.findall(r'native_startup; stage=([^;]+);(?: phase=[^;]+;)? elapsed_ms=([\d.]+)', log),
                    'window_stages': re.findall(r'native_startup_detail; stage=([^;]+); elapsed_ms=([\d.]+)', log),
                    'shader_compilation_errors': log.count('Shader compilation error'),
                    'raw_report': name + '/report.json',
                }
                report['samples'].append(row)
                destination.write_text(json.dumps(report, indent=2) + '\n')
                print(json.dumps(row), flush=True)
        require(len(report['samples']) == 16, 'all sixteen ordered samples required')
        verify(args.input_root.absolute())
        if report['candidate_override'] is not None:
            require(hashlib.sha256(candidate_binary.read_bytes()).hexdigest() == report['candidate_override']['sha256']
                    and subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == report['diagnostic_source']
                    and not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT),
                    'instrumented binary/source changed during observation')
        report['complete'] = True
    finally:
        report['files'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                           for p in output.rglob('*') if p.is_file() and p != destination}
        destination.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
