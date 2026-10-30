"""Exact source, host, compiler and actual Cargo executable identities."""
import json
import os
import platform
import re
import struct
import subprocess
import tomllib

TARGETS = {
    'aarch64-apple-darwin': ('Darwin', 'arm64', '15.0'),
    'x86_64-apple-darwin': ('Darwin', 'x86_64', '15.0'),
    'x86_64-unknown-linux-gnu': ('Linux', 'x86_64', 'Ubuntu 24.04 / glibc 2.39'),
}


def output(root, arguments, **kwargs):
    return subprocess.check_output(arguments, cwd=root, text=True, **kwargs).strip()


def clean_source(root):
    if output(root, ['git', 'status', '--porcelain', '--untracked-files=normal']):
        raise ValueError('release requires a clean committed source tree')
    return output(root, ['git', 'rev-parse', 'HEAD'])


def plan(root, target, require_clean=True):
    if target not in TARGETS:
        raise ValueError('unsupported release target')
    version = (root / 'VERSION').read_text().strip()
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('VERSION must contain an exact three-component product version')
    source = clean_source(root) if require_clean else output(root, ['git', 'rev-parse', 'HEAD'])
    toolchain = tomllib.loads((root / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    features = tomllib.loads((root / '.config/rust-checks.toml').read_text())['targets'][target]
    if features:
        raise ValueError('new shipping features need explicit packaging qualification')
    from .files import digest
    return {
        'format': 'librairii.native-release', 'schema': 1,
        'version': version, 'source_commit': source,
        'source_tree': output(root, ['git', 'rev-parse', 'HEAD^{tree}']),
        'source_date_epoch': int(output(root, ['git', 'show', '-s', '--format=%ct', 'HEAD'])),
        'cargo_lock_sha256': digest(root / 'Cargo.lock'),
        'native_toolkit_pin_sha256': digest(root / '.config/native-toolkit.json'),
        'target': target, 'features': [], 'rust_version': toolchain,
        'minimum_os': TARGETS[target][2],
        'profile': {'opt_level': '3', 'debug_assertions': False, 'overflow_checks': False},
        'public_release_qualified': False,
        'working_tree_dirty': bool(output(root, ['git', 'status', '--porcelain', '--untracked-files=normal'])),
    }


def native_host(root, release):
    system, arch, _ = TARGETS[release['target']]
    if platform.system() != system or platform.machine() != arch:
        raise ValueError('packaging requires the matching native host')
    compiler = output(root, ['rustc', '-vV'])
    if ('host: ' + release['target']) not in compiler.splitlines() or ('release: ' + release['rust_version']) not in compiler.splitlines():
        raise ValueError('release requires the pinned compiler on the native target')
    evidence = {'rustc': compiler, 'system': system, 'arch': arch,
                'os_release': platform.release(), 'os_version': platform.version()}
    cargo = output(root, [os.environ.get('CARGO', 'cargo'), '--version'])
    if not cargo.startswith('cargo ' + release['rust_version'] + ' '):
        raise ValueError('release requires the pinned Cargo version')
    evidence['cargo'] = cargo
    if release['target'] == 'x86_64-apple-darwin':
        brand = output(root, ['sysctl', '-n', 'machdep.cpu.brand_string'])
        translated = subprocess.run(['sysctl', '-in', 'sysctl.proc_translated'], capture_output=True, text=True, check=False)
        if 'Intel' not in brand or translated.stdout.strip() == '1':
            raise ValueError('Intel release requires actual Intel hardware')
        evidence['cpu'] = brand
    return evidence


def cargo_executable(events_path, release):
    events = []
    with events_path.open() as stream:
        for line in stream:
            event = json.loads(line)
            if event.get('reason') == 'compiler-artifact':
                events.append(event)
    selected = [e for e in events if e['target']['name'] == 'librairii-desktop'
                and e['target']['kind'] == ['bin'] and e.get('executable')]
    if len(selected) != 1:
        raise ValueError('Cargo must report exactly one production executable')
    actual = selected[0]
    if any(actual['profile'].get(key) != value for key, value in release['profile'].items()):
        raise ValueError('Cargo did not compile the exact production release profile')
    return actual, events


def architecture(path, target):
    with path.open('rb') as stream:
        head = stream.read(32)
    if len(head) < 32:
        raise ValueError('release binary has an incomplete native header')
    if target.endswith('linux-gnu'):
        if head[:6] != b'\x7fELF\x02\x01' or struct.unpack_from('<H', head, 18)[0] != 62:
            raise ValueError('release binary is not native Linux x86-64 ELF')
    else:
        expected = 0x100000C if target.startswith('aarch64') else 0x1000007
        if head[:4] != b'\xcf\xfa\xed\xfe' or struct.unpack_from('<I', head, 4)[0] != expected:
            raise ValueError('release binary is not the exact thin native Mach-O architecture')


def verify_embedded(root, binary, release):
    observed = json.loads(output(root, [str(binary), '--build-info']))
    expected = {key: release[key] for key in ['version', 'source_commit', 'source_tree', 'cargo_lock_sha256', 'target']}
    if any(observed.get(key) != value for key, value in expected.items()) or observed.get('debug_assertions') is not False or observed.get('runtime') != 'Rust/Slint':
        raise ValueError('compiled product identity does not match the release source/target/graph')
    return observed
