"""Nonqualifying CI-only replay of the original retained software cohort."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess

SOURCE = '22a873d47904a88e848fdbae2b4b284c10b19888'
MANIFESTS = {
    'manifest.json': '7f25cab9cab5e2674c348279849653cb4d78b76d19d6112370523fb197fccd7f',
    'desktop/manifest.json': 'e33ae3907e9758dc7b1eb647dffb59660fbb4d882c82c5aeaf36f4e892beb14a',
    'resources/manifest.json': '68054ff401a12b0703c1dda6470371876517f314e52978e0e78fcd99944966eb',
}

def require(value, message):
    if not value:
        raise ValueError(message)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded

def main():
    require(os.environ.get('GITHUB_ACTIONS') == 'true' and
            (platform.system(), platform.machine()) == ('Darwin', 'x86_64'),
            'native execution is restricted to disposable Intel GitHub Actions')
    root = Path.cwd()
    require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == SOURCE,
            'original workload checkout required')
    raw = root/'target/task19-original-scale'
    manifests = {}
    for name, expected in MANIFESTS.items():
        path = raw/name
        require(digest(path) == expected, 'externally pinned original manifest: '+name)
        data = json.loads(path.read_text())
        require(data['source_commit'] == SOURCE and data['target'] == 'x86_64-apple-darwin',
                'original source/target: '+name)
        for relative, checksum in data['files'].items():
            require(digest(path.parent/relative) == checksum, 'original payload seal: '+relative)
        manifests[name] = data
    for name, checksum in manifests['manifest.json']['fixtures'].items():
        require(digest(root/name) == checksum, 'unchanged frozen fixture: '+name)
    require(digest(root/'Cargo.lock') == manifests['manifest.json']['cargo_lock_sha256'],
            'original locked graph')
    binaries = raw/'resources'
    for name, facts in manifests['resources/manifest.json']['executables'].items():
        require(digest(binaries/name) == facts['sha256'] and
                (binaries/name).stat().st_size == facts['bytes'], 'original executable: '+name)
        profile = facts['profile']
        require(profile['opt_level'] == '3' and not profile['debug_assertions'] and
                not profile['overflow_checks'], 'original production profile')
    for name in ['librairii-desktop', 'scale-native', 'input-macos', 'resource-macos']:
        (binaries/name).chmod(0o755)
    native = module('retained_native_input', root/'qualification/section-16/native.py')
    resources = module('retained_native_resources', root/'qualification/section-16/resources.py')
    output = root/'target/task19-intel-replay'
    output.mkdir()
    receipt = {
        'purpose': 'Nonqualifying narrow latency diagnostic; original frozen per-cohort workload and gates',
        'release_qualified': False, 'original_source': SOURCE,
        'driver_source': os.environ['DIAGNOSTIC_DRIVER_SOURCE'],
        'original_artifact_id': 13042959390, 'original_run_id': 41195747425,
        'original_zip_sha256': '0d05da53700bd94ff52eb05c6e956d33c1e8ceddf249989bcca54da010775cd6',
        'original_manifest_sha256': MANIFESTS,
        'frozen_workload_changed': False, 'original_gate_functions_changed': False,
        'cohort_order': 'software-only narrowed diagnostic; full suite qualification remains separate',
        'trials': [], 'diagnostic_complete': False,
    }
    ledger = output/'diagnostic.json'
    def save():
        ledger.write_text(json.dumps(receipt, indent=2)+'\n')
    save()
    failed = False
    for trial in range(4):
        entry = {'trial': trial, 'original_warmup_position': trial == 0, 'passed': False}
        started = []
        try:
            started.append('product')
            quiet = resources.product_run(binaries/'librairii-desktop', binaries/'scale-native',
                    output/f'software-{trial}-product', 'winit-skia-software', binaries/'resource-macos', native)
            require(quiet['complete'], 'original product observation incomplete')
            started.append('loaded')
            loaded = native.inspect(binaries/'scale-native', output/f'software-{trial}-loaded',
                    'winit-skia-software', binaries/'input-macos')
            entry.update(passed=True, keyboard_p95_ms=loaded['input']['input_p95_ms'],
                         pointer_p95_ms=loaded['input']['pointer_p95_ms'],
                         completion_p95_ms=loaded['input']['navigation_p95_ms'],
                         quiet_complete=quiet['complete'])
        except Exception as error:
            failed = True
            entry['error'] = str(error)
        receipt['trials'].append(entry)
        save()
        for suffix in started:
            report = output/f'software-{trial}-{suffix}/report.json'
            require(report.is_file(), 'stop diagnostics if owned-run retirement report is absent')
            result = json.loads(report.read_text())
            require(result['workers_joined'] and result['fixture_retired'],
                    'stop diagnostics unless each owned product/workload retired')
        print(json.dumps(entry), flush=True)
    receipt['diagnostic_complete'] = True
    save()
    if failed:
        raise SystemExit('Original native gate failed in at least one diagnostic trial; every verdict retained')

if __name__ == '__main__':
    main()
