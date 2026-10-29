#!/usr/bin/env python3
"""[DEBUG-task17-startup] Counterbalanced observer activation experiment only."""
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'target/intel-startup-diagnostic'
PINS = {
    'librairii-desktop': 'bf5e8a4d3d0facdee492666bf035e6b8843f6357fdacab056de4560496232914',
    'scale-native': 'c3b940cafc5dd972d23acdde65bf7f45cb7e4f6b5ca1eb42c932c8f9223beea1',
    'resource-macos': 'b3b43bbdd784c7b61ebeeb7f600326f2011954b763f0bd8a1d4714e8eda38fc9',
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def late_source():
    text = (ROOT / 'qualification/section-16/resource-macos.swift').read_text()
    assert sha(text.encode()) == '76973ff052312fe6169db8822c2ac5d727ba5c448592f23ef9ea06fa44bd7aaf'
    old = '    running.activate(options: [])'
    assert text.count(old) == 1
    text = text.replace(old, '''    // [DEBUG-task17-startup] Only this observer treatment changes.
    var activationReceipt: [String: Any] = [:]
    func activateOwned() {
        let requested = DispatchTime.now().uptimeNanoseconds
        let accepted = running.activate(options: [])
        activationReceipt = ["requested_ns": requested, "returned_ns": DispatchTime.now().uptimeNanoseconds,
                             "accepted": accepted, "treatment": "after-owned-window; original Quit activation retained"]
    }
    if CommandLine.arguments[1] == "quit" { activateOwned() }''')
    old = '        let windowReadyMilliseconds = Double(DispatchTime.now().uptimeNanoseconds-start)/1e6'
    assert text.count(old) == 1
    text = text.replace(old, old + '\n        activateOwned()')
    old = '"observer_traversal": "owned-AXWindows-preorder"'
    assert text.count(old) == 2
    return text.replace(old, '"activation": activationReceipt, "observer_attach_start_ns": start, ' + old)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
    assert platform.system() == 'Darwin' and platform.machine() == 'x86_64'
    assert not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT)
    OUT.mkdir(mode=0o700, parents=True)
    # The unchanged source binary from the failed run is reused in every cohort.
    archive = OUT / 'source-artifact.zip'
    endpoint = 'repos/01max/librairii/actions/artifacts/12933483272'
    (OUT / 'provider.json').write_bytes(subprocess.check_output(['gh', 'api', endpoint]))
    with archive.open('xb') as stream:
        subprocess.run(['gh', 'api', endpoint + '/zip'], stdout=stream, check=True)
    assert archive.stat().st_size == 109564075
    assert sha(archive.read_bytes()) == '5367f231dc7a9fc17c0bf24e80d45f79ed4f203dcb13e529b473c2f350458e79'
    with zipfile.ZipFile(archive) as z:
        assert len(z.namelist()) == len(set(z.namelist()))
        manifest = json.loads(z.read('resources/manifest.json'))
        assert manifest['source_commit'] == 'ece6d04e3b034f467fd12a7c60c64e1df66a14ef'
        assert manifest['target'] == 'x86_64-apple-darwin'
        assert manifest['cargo_lock_sha256'] == sha((ROOT / 'Cargo.lock').read_bytes())
        for name, expected in PINS.items():
            data = z.read('resources/' + name)
            assert sha(data) == expected
            if name != 'resource-macos':
                profile = manifest['executables'][name]['profile']
                assert profile['opt_level'] == '3' and not profile['debug_assertions'] and not profile['overflow_checks']
            path = OUT / name
            with path.open('xb') as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            path.chmod(0o700)
            assert path.stat().st_nlink == 1 and 'x86_64' in subprocess.check_output(['file', str(path)], text=True)
    assert sha((ROOT / 'preservation/corpus/archives/clockwork-forest.plain.pk').read_bytes()) == '0bdfff356adb1d780faa5e5302b051c6921016e9e24c537cef7bbec151700d73'
    source = OUT / 'late-activation.swift'
    source.write_text(late_source())
    helper = OUT / 'late-activation'
    subprocess.run(['xcrun', 'swiftc', str(source), '-o', str(helper)], check=True)
    receipt = {'scope': 'diagnostic only; all original budget failures remain failed', 'complete': False,
               'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
               'platform': platform.platform(),
               'swift_version': subprocess.check_output(['xcrun', 'swiftc', '--version'], text=True),
               'source_artifact': 12933483272, 'source_artifact_sha256': sha(archive.read_bytes()),
               'binary_pins': PINS, 'late_helper_sha256': sha(helper.read_bytes()), 'late_source_sha256': sha(source.read_bytes()),
               'hardware': {key: subprocess.check_output(['sysctl', '-n', value], text=True).strip()
                            for key, value in [('cpu', 'machdep.cpu.brand_string'), ('memory', 'hw.memsize'),
                                               ('logical_cpus', 'hw.logicalcpu'), ('model', 'hw.model')]},
               'plan': [['original', 'gpu'], ['late', 'gpu'], ['late', 'software'], ['original', 'software'],
                        ['late', 'gpu'], ['original', 'gpu'], ['original', 'software'], ['late', 'software']], 'cohorts': []}
    def publish():
        (OUT / 'diagnostic.json').write_text(json.dumps(receipt, indent=2) + '\n')
    publish()
    native = load('native', ROOT / 'qualification/section-16/native.py')
    resources = load('resources', ROOT / 'qualification/section-16/resources.py')
    os.environ.pop('GH_TOKEN', None)
    os.environ.pop('GITHUB_TOKEN', None)
    for index, (treatment, renderer) in enumerate(receipt['plan']):
        name = f'{index}-{treatment}-{renderer}'
        destination = OUT / name
        cohort = {'name': name, 'treatment': treatment, 'renderer': renderer, 'error': None}
        try:
            resources.product_run(OUT / 'librairii-desktop', OUT / 'scale-native', destination,
                                  'winit-skia' if renderer == 'gpu' else 'winit-skia-software',
                                  OUT / 'resource-macos' if treatment == 'original' else helper, native)
        except Exception as error:
            cohort['error'] = f'{type(error).__name__}: {error}'
        try:
            report = json.loads((destination / 'report.json').read_text())
        except (OSError, ValueError) as error:
            report = {}
            cohort['report_error'] = f'{type(error).__name__}: {error}'
        cohort.update(workers_joined=report.get('workers_joined', False), fixture_retired=report.get('fixture_retired', False),
                      complete=report.get('complete', False), startup_ms=report.get('launch_to_first_ui_timer_ms'),
                      idle_cpu_percent=report.get('idle_cpu_percent_one_core'), peak_rss_bytes=report.get('native_time', {}).get('peak_rss_bytes'))
        cohort['metrics_complete'] = all(type(cohort[k]) in (int, float) and math.isfinite(cohort[k]) and cohort[k] >= 0
                                         for k in ['startup_ms', 'idle_cpu_percent', 'peak_rss_bytes'])
        cohort['gate_passed'] = (cohort['error'] is None and cohort['complete'] and cohort['workers_joined']
                                and cohort['fixture_retired'] and cohort['metrics_complete'] and cohort['startup_ms'] <= 1000
                                and cohort['idle_cpu_percent'] <= 5 and cohort['peak_rss_bytes'] <= 512 * 1024 * 1024)
        receipt['cohorts'].append(cohort)
        publish()
        print(json.dumps(cohort), flush=True)
        if not cohort['workers_joined'] or not cohort['fixture_retired']:
            raise ValueError('owned process/fixture retirement required before advancing')
    for name, expected in PINS.items():
        assert sha((OUT / name).read_bytes()) == expected
    assert sha(helper.read_bytes()) == receipt['late_helper_sha256']
    receipt['complete'] = all(c['error'] is None and c['complete'] and c['metrics_complete']
                              and c['workers_joined'] and c['fixture_retired'] for c in receipt['cohorts'])
    publish()
    if not receipt['complete']:
        raise ValueError('startup activation diagnostic incomplete')


if __name__ == '__main__':
    main()
