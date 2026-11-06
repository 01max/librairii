#!/usr/bin/env python3
"""Four fixed software-input comparisons of sealed binaries; never qualification."""
import argparse
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
    args = parser.parse_args()
    roots = verify(args.input_root.absolute())
    if args.verify_only:
        print(json.dumps({'verified': list(roots), 'native_executed': False}))
        return
    require_host()
    output = ROOT / 'target/pointer-comparison/observations'
    output.mkdir(parents=True, exist_ok=False)
    (output / 'host').mkdir()
    report = {
        'format': 'librairii.dependency-pointer-diagnostic', 'schema': 1, 'complete': False,
        'acceptance_evidence': False, 'causal_fix_proven': False, 'original_failed_sample_replaced': False,
        'diagnostic_source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'inputs': SPECS, 'shared_helper_sha256': HELPER_SHA, 'order': ORDER, 'samples': [],
        'scope': 'Four fixed full loaded-library input workloads; same host and helper, fresh synthetic profile per trial. Software renderer only. Original GPU/ordinary-startup prelude omitted. Not full qualification or a cause/repair claim.',
        'retained_failures': [42229788963, 42273630563],
    }
    destination = output / 'report.json'
    try:
        native = module()
        for root in roots.values():
            for name in ('scale-native', 'input-macos'):
                (root / name).chmod(0o700)
        for index, label in enumerate(ORDER):
            name = f'{index:02d}-{label}'
            host_snapshot(output / 'host' / (name + '-before.txt'))
            trial = output / name
            row = observe(native, roots[label] / 'scale-native', trial, roots['candidate'] / 'input-macos')
            row.update(index=index, label=label, raw_report=name + '/report.json')
            report['samples'].append(row)
            destination.write_text(json.dumps(report, indent=2) + '\n')
            host_snapshot(output / 'host' / (name + '-after.txt'))
            print(json.dumps(row), flush=True)
        require(len(report['samples']) == 4, 'all four predeclared trials required')
        verify(args.input_root.absolute())
        report['complete'] = True
    finally:
        report['files'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                           for p in output.rglob('*') if p.is_file() and p != destination}
        destination.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
