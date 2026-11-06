#!/usr/bin/env python3
"""Two fixed instrumented input workloads; never performance qualification."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
BASE = 'bcd00de38cb2f5a2996bc7a4e693b91ac79abb45'
ALLOWED = {
    '.github/workflows/rust-scale.yml',
    'scripts/diagnose-dependency-pointer.py',
    'scripts/test_diagnose_dependency_pointer.py',
    'scripts/diagnose-dependency-input-timing.py',
    'scripts/test_diagnose_dependency_input_timing.py',
    'crates/librairii-desktop/examples/scale-native.rs',
    'crates/librairii-desktop/examples/scale-native/diagnostic_timing.rs',
}


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


diag = module('pointer_diagnostic', ROOT / 'scripts/diagnose-dependency-pointer.py')


def output(*args):
    return subprocess.check_output(list(args), cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source():
    diag.require(not output('git', 'status', '--porcelain', '--untracked-files=normal'), 'clean committed diagnostic source required')
    changed = set(output('git', 'diff', '--name-only', BASE, 'HEAD').splitlines())
    diag.require(changed <= ALLOWED, 'unexpected product changes in diagnostic source')
    diag.require(digest(ROOT / 'Cargo.lock') == diag.SPECS['candidate']['lock'], 'candidate lock differs')
    return output('git', 'rev-parse', 'HEAD')


def read_timing(path, joined_events):
    diag.require(path.is_file() and path.stat().st_size <= 16777216, 'bounded timing file required')
    value = json.loads(path.read_text())
    diag.require(value.get('format') == 'librairii.observer-timing-diagnostic'
                 and value.get('schema') == 1 and value.get('acceptance_evidence') is False
                 and value.get('instrumented') is True and value.get('overflowed') is False,
                 'complete bounded diagnostic trace required')
    columns = ['start', 'clock_files', 'resize_menus', 'projection', 'model_observation',
               'import_page_offers', 'geometry_capture', 'native_window', 'state_publication', 'end']
    diag.require(value.get('columns') == columns, 'timing column identities differ')
    rows = value.get('rows')
    diag.require(isinstance(rows, list) and 0 < len(rows) <= 20000, 'bounded nonempty timing rows required')
    previous_end = 0
    gaps, totals = [], []
    phases = {name: [] for name in columns[1:]}
    for row in rows:
        diag.require(isinstance(row, list) and len(row) == 10
                     and all(type(v) is int and v >= 0 for v in row), 'ten integer timestamps required')
        reached = [v for v in row if v]
        diag.require(row[0] > 0 and row[-1] >= row[0] and reached == sorted(reached)
                     and row[0] >= previous_end, 'ordered single-thread timing rows required')
        if previous_end:
            gaps.append((row[0] - previous_end) / 1e6)
        previous_end = row[-1]
        totals.append((row[-1] - row[0]) / 1e6)
        if all(row):
            for index, name in enumerate(columns[1:], 1):
                phases[name].append((row[index] - row[index - 1]) / 1e6)
    diag.require(value.get('dispatch_events_before_shutdown') == joined_events,
                 'pre-shutdown timing witnesses differ from joined callback log')
    return {'rows': len(rows), 'max_tick_ms': max(totals), 'max_between_ticks_ms': max(gaps, default=0),
            'phase_max_ms': {name: max(samples, default=0) for name, samples in phases.items()},
            'acceptance_evidence': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-root', type=Path, required=True)
    args = parser.parse_args()
    diag.require_host()
    original_source = source()
    roots = diag.verify(args.input_root.absolute())
    destination = ROOT / 'target/input-timing/observations'
    destination.mkdir(parents=True, exist_ok=False)
    report_path = destination / 'report.json'
    report = {'format': 'librairii.dependency-input-timing', 'schema': 1,
              'diagnostic_source': original_source, 'candidate_base': BASE, 'complete': False,
              'acceptance_evidence': False, 'causal_fix_proven': False, 'instrumented': True,
              'order': ['candidate-instrumented', 'candidate-instrumented'], 'samples': [],
              'cargo_lock_sha256': digest(ROOT / 'Cargo.lock'),
              'scope': 'Two fixed software loaded-input workloads with bounded in-memory observer phase timings; unchanged input helper/method3 and synthetic workload. No startup/GPU prelude, profiler or timing acceptance inference. Stop on unconfirmed cleanup.',
              'retained_failures': [42229788963, 42273630563, 42284007023, 42288641613],
              'source_files': {name: digest(ROOT / name) for name in sorted(ALLOWED)}}
    try:
        target = 'x86_64-apple-darwin'
        report['compiler'] = output('rustc', '-vV')
        with (destination / 'compiler-artifacts.jsonl').open('w') as out, (destination / 'build.log').open('w') as err:
            subprocess.run(['cargo', 'build', '--locked', '--release', '--target', target,
                            '-p', 'librairii-desktop', '--example', 'scale-native', '--message-format=json'],
                           cwd=ROOT, stdout=out, stderr=err, check=True, timeout=5400)
        events = [json.loads(line) for line in (destination / 'compiler-artifacts.jsonl').read_text().splitlines()]
        matches = [e for e in events if e.get('reason') == 'compiler-artifact'
                   and e.get('target', {}).get('name') == 'scale-native'
                   and e['target']['kind'] == ['example'] and e.get('executable')]
        diag.require(len(matches) == 1, 'exact Cargo observer executable required')
        built = matches[0]
        diag.require(built['profile']['opt_level'] == '3' and not built['profile']['debug_assertions']
                     and not built['profile']['overflow_checks'], 'original release compiler profile required')
        binary = destination / 'scale-native'
        shutil.copy2(built['executable'], binary)
        report['executable'] = {'sha256': digest(binary), 'bytes': binary.stat().st_size, 'profile': built['profile']}
        helper = roots['candidate'] / 'input-macos'
        helper.chmod(0o700)
        report['helper_sha256'] = digest(helper)
        report_path.write_text(json.dumps(report, indent=2) + '\n')
        native = diag.module()
        for index in range(2):
            diag.require(source() == original_source, 'source changed during diagnostic')
            diag.checked(binary, report['executable']['sha256'])
            diag.checked(helper, diag.HELPER_SHA)
            trial = destination / f'{index:02d}-candidate-instrumented'
            diag.host_snapshot(destination / f'{index:02d}-host-before.txt')
            row = diag.observe(native, binary, trial, helper)
            joined_events = json.loads((trial / 'dispatch-events.json').read_text())
            row.update(index=index, instrumented=True,
                       timing=read_timing(trial / 'observer-timing.json', joined_events))
            report['samples'].append(row)
            report_path.write_text(json.dumps(report, indent=2) + '\n')
            diag.host_snapshot(destination / f'{index:02d}-host-after.txt')
            print(json.dumps(row), flush=True)
        diag.require(source() == original_source, 'source changed during diagnostic')
        diag.checked(binary, report['executable']['sha256'])
        diag.verify(args.input_root.absolute())
        report['complete'] = len(report['samples']) == 2
    except Exception as failure:
        report['failure'] = {'type': type(failure).__name__, 'message': str(failure)}
        raise
    finally:
        report['files'] = {str(p.relative_to(destination)): digest(p) for p in destination.rglob('*')
                           if p.is_file() and p != report_path}
        report_path.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
