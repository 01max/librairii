#!/usr/bin/env python3
"""[DEBUG-task17-replay] Fixed-size intrinsic replay; never qualification."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'target/intel-input-replay'
SOURCE = 'c5a007a780e4fb113773f1283de0435e6655da18'
ARCHIVE_SHA = '7c3cf5cd9921dc9587975833116ea47ce2aeb5c136e0f50c3faadf6af83f923d'
PINS = {
    'control': 'c3b940cafc5dd972d23acdde65bf7f45cb7e4f6b5ca1eb42c932c8f9223beea1',
    'traced': '349063144190200ad8ecc8493e0b3fe5929d87cb0991bf0c22e20fc3e622e6f7',
    'original-input': 'd9bb311530cd69f6db2e721db9ad4b4474e8b7e2ab4fbdc0b2dc12caf3ad2d7e',
    'traced-input': 'db91fdf8d8ba591e5539fc11754206d009789c1304cf7f71e94af25e5aaea518',
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
    assert platform.system() == 'Darwin' and platform.machine() == 'x86_64'
    assert not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT)
    OUT.mkdir(mode=0o700, parents=True)
    archive = OUT / 'source-artifact.zip'
    endpoint = 'repos/01max/librairii/actions/artifacts/12934773026'
    (OUT / 'provider.json').write_bytes(subprocess.check_output(['gh', 'api', endpoint]))
    with archive.open('xb') as stream:
        subprocess.run(['gh', 'api', endpoint + '/zip'], stdout=stream, check=True)
    assert archive.stat().st_size == 41023117 and sha(archive.read_bytes()) == ARCHIVE_SHA
    with zipfile.ZipFile(archive) as z:
        assert len(z.namelist()) == len(set(z.namelist()))
        source = json.loads(z.read('diagnostic.json'))
        assert source['source'] == SOURCE and source['complete'] and source['target'] == 'x86_64-apple-darwin'
        for name, expected in PINS.items():
            data = z.read(name)
            assert sha(data) == expected
            if name in ['control', 'traced']:
                p = source[name]['profile']
                assert p['opt_level'] == '3' and not p['debug_assertions'] and not p['overflow_checks']
            path = OUT / name
            with path.open('xb') as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            path.chmod(0o700)
            assert path.stat().st_nlink == 1 and 'x86_64' in subprocess.check_output(['file', str(path)], text=True)
    os.environ.pop('GH_TOKEN', None)
    os.environ.pop('GITHUB_TOKEN', None)
    assert sha((ROOT / 'Cargo.lock').read_bytes()) == '7bdb5b1de31a5e58578cf7616ca79a9875b1e7b7454d502c7972dab445d6c0f8'
    receipt = {
        'scope': 'diagnostic only; fixed twelve workloads; every failure retained; no historical rescore',
        'complete': False, 'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'binary_source': SOURCE, 'source_artifact': 12934773026, 'source_artifact_sha256': ARCHIVE_SHA,
        'binary_pins': PINS, 'control': source['control'], 'traced': source['traced'],
        'platform': platform.platform(),
        'hardware': {key: subprocess.check_output(['sysctl', '-n', value], text=True).strip()
                     for key, value in [('cpu', 'machdep.cpu.brand_string'), ('memory', 'hw.memsize'),
                                        ('logical_cpus', 'hw.logicalcpu'), ('model', 'hw.model')]},
        'plan': [[f'{i:02d}-{label}', traced, backend] for i, (label, traced, backend) in enumerate([
            ('control-software', False, 'winit-skia-software'),
            ('traced-software', True, 'winit-skia-software'),
            ('traced-software', True, 'winit-skia-software'),
            ('traced-gpu', True, 'winit-skia'),
            ('traced-software', True, 'winit-skia-software'),
            ('traced-software', True, 'winit-skia-software'),
            ('control-software', False, 'winit-skia-software'),
            ('traced-software', True, 'winit-skia-software'),
            ('traced-software', True, 'winit-skia-software'),
            ('traced-gpu', True, 'winit-skia'),
            ('traced-software', True, 'winit-skia-software'),
            ('traced-software', True, 'winit-skia-software'),
        ])], 'cohorts': [],
    }
    def publish():
        (OUT / 'diagnostic.json').write_text(json.dumps(receipt, indent=2) + '\n')
    publish()
    spec = importlib.util.spec_from_file_location('native', ROOT / 'qualification/section-16/native.py')
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    for name, traced, backend in receipt['plan']:
        destination = OUT / name
        cohort = {'name': name, 'traced': traced, 'error': None}
        try:
            native.inspect(OUT / ('traced' if traced else 'control'), destination, backend,
                           OUT / ('traced-input' if traced else 'original-input'))
        except Exception as error:
            cohort['error'] = f'{type(error).__name__}: {error}'
        try:
            report = json.loads((destination / 'report.json').read_text())
        except (OSError, ValueError) as error:
            report = {}
            cohort['report_error'] = f'{type(error).__name__}: {error}'
        cohort.update(workers_joined=report.get('workers_joined', False), fixture_retired=report.get('fixture_retired', False),
                      gate_complete=report.get('complete', False),
                      workload_complete=cohort['error'] in [None, 'ValueError: retained native input/completion wall gate failed'],
                      input_p95_ms=report.get('input', {}).get('input_p95_ms'))
        if traced:
            cohort['phases_complete'] = False
            try:
                measured = json.loads((destination / 'helper-phases.json').read_text())
                phases = json.loads((destination / 'native-phases.json').read_text())
                cohort['phases_complete'] = (measured['clock_bounds_valid'] and len(measured['calibrations']) == 64
                                             and len(measured['samples']) == 64 and bool(phases))
            except (OSError, ValueError, KeyError, TypeError) as error:
                cohort['phase_error'] = f'{type(error).__name__}: {error}'
        receipt['cohorts'].append(cohort)
        publish()
        print(json.dumps(cohort), flush=True)
        if not cohort['workers_joined'] or not cohort['fixture_retired']:
            raise ValueError('positive owned retirement required before another cohort')
    for name, expected in PINS.items():
        assert sha((OUT / name).read_bytes()) == expected
    receipt['complete'] = all(c['workload_complete'] and c.get('phases_complete', not c['traced']) for c in receipt['cohorts'])
    publish()
    if not receipt['complete']:
        raise ValueError('fixed intrinsic replay incomplete')


if __name__ == '__main__':
    main()
