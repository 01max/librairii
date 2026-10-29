#!/usr/bin/env python3
"""Run Task17 against actual current and manifest-frozen Go compositions."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from frozen_go import frozen_module

ROOT = Path(__file__).resolve().parent.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(work, environment, template, name):
    driver = work / 'driver/main.go'
    driver.parent.mkdir()
    driver.write_bytes(template.read_bytes())
    helper = work / name
    subprocess.run(['go', 'build', '-mod=readonly', '-o', str(helper), './driver'],
                   cwd=work, env=environment, check=True, timeout=240)
    return helper


def run_example(example, helpers, environment):
    command = ['cargo', 'run', '--locked', '-p', 'librairii-storage', '--example', example]
    if os.environ.get('RUST_TARGET'):
        command += ['--target', os.environ['RUST_TARGET']]
    result = subprocess.run(command + ['--'] + list(map(str, helpers)), cwd=ROOT,
                            env=environment, text=True, capture_output=True, timeout=240)
    if result.returncode:
        failure = ROOT / 'target/quality' / f'{example}-failure.log'
        failure.parent.mkdir(parents=True, exist_ok=True)
        failure.write_text(result.stdout + '\n' + result.stderr)
        raise RuntimeError(f'{example} failed; full output: {failure}\n{result.stderr[-4096:]}')
    return json.loads(result.stdout)


def main():
    template = ROOT / 'scripts/handover-app-probe.go.in'
    with frozen_module(ROOT, 'task17-frozen-') as (frozen, environment, manifest):
        original = build(frozen, environment, template, 'original-application')
        with tempfile.TemporaryDirectory(prefix='task17-current-', dir=ROOT / 'tmp') as current:
            current = Path(current)
            paths = subprocess.check_output(['git', 'ls-files', '-z', '--', 'internal', 'go.mod', 'go.sum'], cwd=ROOT).split(b'\0')
            sources = []
            for raw in filter(None, paths):
                path = Path(os.fsdecode(raw))
                source = ROOT / path
                if source.is_symlink() or not source.is_file() or source.stat().st_nlink != 1:
                    raise ValueError(f'nonregular current source: {path}')
                target = current / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
                sources.append({'path': str(path), 'sha256': digest(source)})
            bridge = build(current, environment, template, 'bridge-application')
            report = run_example('upgrade_handover', [bridge], environment)
            application = run_example('application_handover', [original, bridge], environment)
            report['application_handover'] = application
            report['producer'] = {
                'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'go': subprocess.check_output(['go', 'version'], env=environment, text=True).strip(),
                'rust': subprocess.check_output(['rustc', '-vV'], text=True).strip(),
                'application_driver_sha256': digest(template),
                'bridge_helper_sha256': digest(bridge),
                'frozen_helper_sha256': digest(original),
                'frozen_source_manifest_sha256': hashlib.sha256(manifest).hexdigest(),
                'current_go_sources': sources,
                'example_sha256': digest(ROOT / 'crates/librairii-storage/examples/upgrade_handover.rs'),
            }
            destination = ROOT / 'target/quality/upgrade-handover.json'
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(json.dumps(report, indent=2) + '\n')
            print(f'Upgrade handover: {len(report["cases"])} real adoption/restart/rollback cases passed; full manifests and table snapshots retained at {destination}.')


if __name__ == '__main__':
    main()
