"""Build a fresh, source-bound production payload before OS packaging."""
import json
import os
from pathlib import Path
import subprocess

from .files import copy_file, digest, fresh_output, inventory, write_json
from .identity import architecture, cargo_executable, clean_source, native_host, output, plan, verify_embedded
from . import sbom


def build(root, target, requested_output):
    release = plan(root, target)
    host = native_host(root, release)
    modifiers = ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTC_WRAPPER',
                 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_BOOTSTRAP', 'CARGO_BUILD_RUSTFLAGS']
    modifiers += [name for name in os.environ if name.startswith('CARGO_PROFILE_RELEASE_')]
    if any(os.environ.get(name) for name in modifiers):
        raise ValueError('release refuses ambient Rust compiler/profile overrides')
    destination = fresh_output(root, requested_output)
    environment = dict(os.environ)
    environment.update({
        'SOURCE_DATE_EPOCH': str(release['source_date_epoch']),
        'MACOSX_DEPLOYMENT_TARGET': '15.0', 'CARGO_INCREMENTAL': '0',
        'LIBRAIRII_RELEASE_COMMIT': release['source_commit'],
        'LIBRAIRII_RELEASE_TREE': release['source_tree'],
        'LIBRAIRII_RELEASE_LOCK_SHA256': release['cargo_lock_sha256'],
        'LIBRAIRII_RELEASE_TARGET': target,
    })
    cargo = os.environ.get('CARGO', 'cargo')
    command = [cargo, 'build', '--locked', '--release', '--target', target,
               '-p', 'librairii-desktop', '--bin', 'librairii-desktop', '--message-format=json']
    event_path = destination / 'compiler-artifacts.jsonl'
    with event_path.open('x') as events, (destination / 'build.log').open('x') as log:
        subprocess.run(command, cwd=root, env=environment, stdout=events, stderr=log, check=True)
    artifact, compiled = cargo_executable(event_path, release)
    source_binary = Path(artifact['executable'])
    if source_binary.is_symlink() or not source_binary.resolve().is_relative_to((root / 'target').resolve()):
        raise ValueError('Cargo executable is not an owned target build output')
    payload = destination / 'payload'
    payload.mkdir()
    binary = payload / 'bin/librairii-desktop'
    copy_file(source_binary, binary, executable=True)
    architecture(binary, target)
    embedded = verify_embedded(root, binary, release)
    share = payload / 'share/librairii'
    share.mkdir(parents=True)
    write_json(share / 'build-info.json', embedded)
    # Re-run exact source/native notice validation; do not copy an old mutable
    # target/quality directory into a release payload.
    with (destination / 'inventory.log').open('x') as log:
        subprocess.run([cargo, 'xtask', 'inventory'], cwd=root, env=environment,
                       stdout=log, stderr=subprocess.STDOUT, check=True)
    metadata = json.loads(output(root, [cargo, 'metadata', '--locked', '--format-version', '1', '--filter-platform', target]))
    sbom.write(root, share, release, metadata, compiled)
    copy_file(root / 'build/appicon.png', share / 'appicon.png')
    copy_file(root / 'build/appicon.svg', share / 'appicon.svg')
    if clean_source(root) != release['source_commit'] or digest(root / 'Cargo.lock') != release['cargo_lock_sha256']:
        raise ValueError('source or locked graph changed during release build')
    release.update({'stage_complete': True, 'host': host, 'build_command': command,
                    'compiler_profile': artifact['profile'], 'build_info': embedded,
                    'payload': inventory(payload), 'signing': {'state': 'not performed'},
                    'qualification': {'state': 'not performed'}})
    write_json(destination / 'stage.json', release)
    return destination


def verify_stage(directory):
    directory = Path(directory)
    release = json.loads((directory / 'stage.json').read_text())
    if release.get('format') != 'librairii.native-release' or release.get('schema') != 1 or release.get('stage_complete') is not True:
        raise ValueError('incomplete or unsupported release stage')
    if inventory(directory / 'payload') != release['payload']:
        raise ValueError('release payload differs from the sealed final-byte inventory')
    architecture(directory / 'payload/bin/librairii-desktop', release['target'])
    return release
