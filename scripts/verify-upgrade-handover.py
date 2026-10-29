#!/usr/bin/env python3
"""Run Task17 against actual current and manifest-frozen Go compositions."""
import hashlib
import json
import os
from pathlib import Path
import shutil
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
    command = ['cargo', 'build', '--locked', '-p', 'librairii-storage', '--example', example, '--message-format=json-render-diagnostics']
    if os.environ.get('RUST_TARGET'):
        command += ['--target', os.environ['RUST_TARGET']]
    built = subprocess.run(command, cwd=ROOT, env=environment, text=True, capture_output=True, timeout=240)
    destination = ROOT / 'target/quality'
    destination.mkdir(parents=True, exist_ok=True)
    (destination / f'{example}-compiler.jsonl').write_text(built.stdout)
    (destination / f'{example}-build.log').write_text(built.stderr)
    if built.returncode:
        raise RuntimeError(f'{example} compilation failed: {built.stderr[-4096:]}')
    artifacts = [json.loads(line) for line in built.stdout.splitlines()]
    executables = [item for item in artifacts if item.get('reason') == 'compiler-artifact' and item.get('target', {}).get('name') == example and item.get('executable')]
    if len(executables) != 1:
        raise ValueError(f'expected one compiler-reported {example} executable')
    executable = Path(executables[0]['executable'])
    if executable.is_symlink() or not executable.is_file() or executable.stat().st_nlink != 1:
        raise ValueError('nonregular rehearsal executable')
    before = digest(executable)
    result = subprocess.run([str(executable)] + list(map(str, helpers)), cwd=ROOT,
                            env=environment, text=True, capture_output=True, timeout=240)
    (destination / f'{example}-execution.log').write_text(result.stderr)
    if result.returncode:
        failure = ROOT / 'target/quality' / f'{example}-failure.log'
        failure.parent.mkdir(parents=True, exist_ok=True)
        failure.write_text(result.stdout + '\n' + result.stderr)
        raise RuntimeError(f'{example} failed; full output: {failure}\n{result.stderr[-4096:]}')
    if digest(executable) != before:
        raise ValueError('rehearsal executable changed during execution')
    report = json.loads(result.stdout)
    retained = destination / 'handover-bin' / example
    retained.parent.mkdir(exist_ok=True)
    shutil.copy2(executable, retained)
    if digest(retained) != before:
        raise ValueError('retained rehearsal executable differs from executed bytes')
    report['execution'] = {'executable_sha256': before, 'bytes': executable.stat().st_size, 'retained_path': str(retained.relative_to(destination)), 'compiler_artifact': executables[0]}
    return report


def main():
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    status = ['git', 'status', '--porcelain', '--untracked-files=normal']
    if subprocess.check_output(status, cwd=ROOT, text=True).strip():
        raise ValueError('handover qualification requires a clean committed source tree')
    template = ROOT / 'scripts/handover-app-probe.go.in'
    with frozen_module(ROOT, 'task17-frozen-') as (frozen, environment, manifest):
        original = build(frozen, environment, template, 'original-application')
        with tempfile.TemporaryDirectory(prefix='task17-current-', dir=ROOT / 'tmp') as current:
            current = Path(current)
            paths = subprocess.check_output(['git', 'ls-files', '-z', '--', 'internal', 'go.mod', 'go.sum'], cwd=ROOT).split(b'\0')
            sources = []
            for raw in filter(None, paths):
                path = Path(os.fsdecode(raw))
                source_file = ROOT / path
                if source_file.is_symlink() or not source_file.is_file() or source_file.stat().st_nlink != 1:
                    raise ValueError(f'nonregular current source: {path}')
                target = current / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source_file.read_bytes())
                sources.append({'path': str(path), 'sha256': digest(source_file)})
            bridge = build(current, environment, template, 'bridge-application')
            report = run_example('upgrade_handover', [bridge], environment)
            application = run_example('application_handover', [original, bridge], environment)
            report['application_handover'] = application
            binaries = ROOT / 'target/quality/handover-bin'
            for helper in [original, bridge]:
                retained = binaries / helper.name
                shutil.copy2(helper, retained)
                if digest(retained) != digest(helper):
                    raise ValueError('retained Go helper differs from executed bytes')
            report['producer'] = {
                'source': source,
                'tree': subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], cwd=ROOT, text=True).strip(),
                'go': subprocess.check_output(['go', 'version'], env=environment, text=True).strip(),
                'rust': subprocess.check_output(['rustc', '-vV'], text=True).strip(),
                'application_driver_sha256': digest(template),
                'bridge_helper_sha256': digest(bridge),
                'frozen_helper_sha256': digest(original),
                'frozen_source_manifest_sha256': hashlib.sha256(manifest).hexdigest(),
                'current_go_sources': sources,
                'example_sha256': digest(ROOT / 'crates/librairii-storage/examples/upgrade_handover.rs'),
                'application_example_sha256': digest(ROOT / 'crates/librairii-storage/examples/application_handover.rs'),
                'shared_support_sha256': digest(ROOT / 'crates/librairii-storage/examples/handover_support/mod.rs'),
                'driver_script_sha256': digest(Path(__file__)),
            }
            destination = ROOT / 'target/quality/upgrade-handover.json'
            destination.parent.mkdir(parents=True, exist_ok=True)
            if subprocess.check_output(status, cwd=ROOT, text=True).strip() or subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() != source:
                raise ValueError('handover qualification source changed during execution')
            destination.write_text(json.dumps(report, indent=2) + '\n')
            print(f'Upgrade handover: {len(report["cases"])} real adoption/restart/rollback cases passed; full manifests and table snapshots retained at {destination}.')


if __name__ == '__main__':
    main()
