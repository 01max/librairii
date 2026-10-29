#!/usr/bin/env python3
"""Run Task17 against actual current and manifest-frozen Go compositions."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from frozen_go import frozen_module

ROOT = Path(__file__).resolve().parent.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def executable_identity(metadata):
    # Reading/executing may update atime; content/ownership changes must fail.
    return (metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_nlink,
            metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns)


def read_executable(path):
    # Cargo may hard-link the named example to its hashed build output. Never
    # execute that shared inode; read only a regular, non-followed source.
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, 'rb') as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError('nonregular rehearsal executable')
        body = source.read()
        if executable_identity(os.fstat(source.fileno())) != executable_identity(before):
            raise ValueError('rehearsal executable changed while reading')
    return body, before


def retain_executable(executable, retained):
    body, metadata = read_executable(executable)
    retained.parent.mkdir(exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix='.executable-', dir=retained.parent)
    try:
        with os.fdopen(descriptor, 'wb') as destination:
            destination.write(body)
            os.fchmod(destination.fileno(), 0o700)
            destination.flush()
            os.fsync(destination.fileno())
        os.replace(temporary, retained)
    finally:
        Path(temporary).unlink(missing_ok=True)
    copied, copied_metadata = read_executable(retained)
    if copied != body or copied_metadata.st_nlink != 1:
        raise ValueError('retained rehearsal executable is not an exact private copy')
    return hashlib.sha256(body).hexdigest(), metadata, copied_metadata


def build(work, environment, template, name):
    driver = work / 'driver/main.go'
    driver.parent.mkdir()
    driver.write_bytes(template.read_bytes())
    helper = work / name
    command = ['go', 'build', '-mod=readonly']
    # Match the retained Ubuntu Go/Wails package's WebKit ABI. The probe runs
    # application services without a window, but still links the native adapter.
    if sys.platform == 'linux':
        command += ['-tags', 'webkit2_41']
    subprocess.run(command + ['-o', str(helper), './driver'],
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
    retained = destination / 'handover-bin' / example
    before, compiler_metadata, executed_metadata = retain_executable(executable, retained)
    result = subprocess.run([str(retained)] + list(map(str, helpers)), cwd=ROOT,
                            env=environment, text=True, capture_output=True, timeout=240)
    (destination / f'{example}-execution.log').write_text(result.stderr)
    if result.returncode:
        failure = ROOT / 'target/quality' / f'{example}-failure.log'
        failure.parent.mkdir(parents=True, exist_ok=True)
        failure.write_text(result.stdout + '\n' + result.stderr)
        raise RuntimeError(f'{example} failed; full output: {failure}\n{result.stderr[-4096:]}')
    for path, metadata in [(executable, compiler_metadata), (retained, executed_metadata)]:
        body, after = read_executable(path)
        if hashlib.sha256(body).hexdigest() != before or executable_identity(after) != executable_identity(metadata):
            raise ValueError('rehearsal executable changed during execution')
    report = json.loads(result.stdout)
    report['execution'] = {'executable_sha256': before, 'bytes': executed_metadata.st_size,
                           'retained_path': str(retained.relative_to(destination)),
                           'executed_path': str(retained), 'executed_links': executed_metadata.st_nlink,
                           'compiler_executable_sha256': before,
                           'compiler_executable_links': compiler_metadata.st_nlink,
                           'compiler_artifact': executables[0]}
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
                'go_build_tags': ['webkit2_41'] if sys.platform == 'linux' else [],
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
