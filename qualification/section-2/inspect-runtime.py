#!/usr/bin/env python3
"""Inspect a trusted native qualification package's immediate runtime requirements."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import plistlib
import re
import subprocess

TARGETS = {
    'aarch64-apple-darwin': ('Darwin', 'arm64'),
    'x86_64-apple-darwin': ('Darwin', 'x86_64'),
    'x86_64-unknown-linux-gnu': ('Linux', 'x86_64'),
}


def mac_requirements(linked, commands):
    libraries = [line.strip().split(' (compatibility version')[0]
                 for line in linked.splitlines()[1:] if line.strip()]
    minimum_os, rpaths = [], []
    for block in commands.split('Load command'):
        if re.search(r'\bcmd LC_BUILD_VERSION\b', block):
            minimum_os += re.findall(r'\bminos ([0-9.]+)', block)
        if re.search(r'\bcmd LC_VERSION_MIN_MACOSX\b', block):
            minimum_os += re.findall(r'\bversion ([0-9.]+)', block)
        if re.search(r'\bcmd LC_RPATH\b', block):
            rpaths += re.findall(r'\bpath (.+?) \(offset', block)
    return {
        'linked_libraries': libraries,
        'non_system_libraries': [name for name in libraries
                                 if not name.startswith(('/usr/lib/', '/System/Library/'))],
        'minimum_os_load_commands': sorted(set(minimum_os)),
        'rpaths': rpaths,
    }


def linux_requirements(dynamic, versions, resolved):
    names = sorted(set(re.findall(r'\bName: ([A-Za-z0-9_.]+)', versions)))
    maximum = {}
    for family in ('GLIBC', 'GLIBCXX', 'CXXABI'):
        values = [name[len(family) + 1:] for name in names
                  if re.fullmatch(family + r'_[0-9]+(?:\.[0-9]+)+', name)]
        if values:
            maximum[family] = max(values, key=lambda value: tuple(map(int, value.split('.'))))
    return {
        'needed_sonames': re.findall(r'\(NEEDED\).*?\[([^]]+)\]', dynamic),
        'loader_paths': re.findall(r'\((?:RPATH|RUNPATH)\).*?\[([^]]*)\]', dynamic),
        'required_version_names': names,
        'maximum_numeric_symbol_versions': maximum,
        'unresolved_libraries': re.findall(r'^\s*(\S+)\s+=>\s+not found\s*$', resolved, re.M),
    }


def verified_binary(root, manifest):
    root = root.resolve(strict=True)
    relative = pathlib.PurePosixPath(manifest['executable'])
    if relative.is_absolute() or '..' in relative.parts:
        raise ValueError('unsafe executable path')
    binary = root.joinpath(*relative.parts)
    if not binary.resolve(strict=True).is_relative_to(root):
        raise ValueError('executable escapes package')
    if manifest['files'].get(str(relative)) != manifest['binary_sha256']:
        raise ValueError('executable is not a matching manifest member')
    with binary.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    if actual != manifest['binary_sha256']:
        raise ValueError('executable bytes changed')
    return binary


def inspect(root):
    root = root.resolve(strict=True)
    manifest = json.loads((root / 'manifest.json').read_text())
    if (platform.system(), platform.machine()) != TARGETS[manifest['target']]:
        raise ValueError('runtime inspection requires the matching native host')
    binary = verified_binary(root, manifest)
    if platform.system() == 'Darwin':
        commands = {'linked': ['/usr/bin/otool', '-L', str(binary)],
                    'load_commands': ['/usr/bin/otool', '-l', str(binary)]}
    else:
        # Only trusted locally built probes: ldd can invoke the binary's loader.
        commands = {'dynamic': ['readelf', '-d', str(binary)],
                    'versions': ['readelf', '--version-info', str(binary)],
                    'resolved': ['ldd', str(binary)]}
    outputs = {}
    # Native tools use translated labels; parse one language on every desktop.
    tool_env = {**os.environ, 'LC_ALL': 'C', 'LANG': 'C'}
    for name, command in commands.items():
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=30, env=tool_env)
            outputs[name] = {'returncode': result.returncode,
                             'stdout': result.stdout, 'stderr': result.stderr, 'timed_out': False}
        except subprocess.TimeoutExpired as error:
            def decoded(value):
                return value.decode('utf-8', errors='replace') if isinstance(value, bytes) else (value or '')
            outputs[name] = {'returncode': None, 'stdout': decoded(error.stdout),
                             'stderr': decoded(error.stderr), 'timed_out': True}
        except OSError as error:
            outputs[name] = {'returncode': None, 'stdout': '', 'stderr': str(error),
                             'timed_out': False, 'tool_error': True}
    if platform.system() == 'Darwin':
        requirements = mac_requirements(outputs['linked']['stdout'], outputs['load_commands']['stdout'])
        bundle = next((parent for parent in binary.parents
                       if parent.suffix == '.app' and parent.is_relative_to(root)), None)
        requirements['bundle_minimum_os'] = None
        if bundle is not None:
            info = bundle / 'Contents' / 'Info.plist'
            if not info.resolve(strict=True).is_relative_to(root):
                raise ValueError('bundle metadata escapes package')
            contents = info.read_bytes()
            if hashlib.sha256(contents).hexdigest() != manifest['files'].get(str(info.relative_to(root))):
                raise ValueError('bundle metadata bytes changed')
            requirements['bundle_minimum_os'] = plistlib.loads(contents).get('LSMinimumSystemVersion')
        missing = requirements['non_system_libraries']
        parsed = bool(requirements['linked_libraries'] and requirements['minimum_os_load_commands'])
    else:
        requirements = linux_requirements(*(outputs[name]['stdout'] for name in ('dynamic', 'versions', 'resolved')))
        missing = requirements['unresolved_libraries']
        parsed = bool(requirements['needed_sonames'] and requirements['required_version_names'])
    passed = parsed and not missing and all(value['returncode'] == 0 for value in outputs.values())
    return {
        'schema': 1, 'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
        'target': manifest['target'], 'binary_sha256': manifest['binary_sha256'],
        'binary_bytes': binary.stat().st_size,
        'host': {'system': platform.system(), 'release': platform.release(),
                 'machine': platform.machine(), 'macos': platform.mac_ver()[0]},
        'immediate_link_inspection': 'pass' if passed else 'fail',
        'parsed_metadata_complete': parsed,
        'requirements': requirements, 'tool_outputs': outputs,
        'scope': 'Immediate linked libraries and declared binary ABI only; dynamically loaded libraries/plugins and clean-host availability require separate execution.',
        'minimum_supported_os_qualified': False,
        'gpu_failure_qualified': False, 'clean_host_qualified': False, 'toolkit_selected': False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = inspect(args.package)
    with args.output.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    if report['immediate_link_inspection'] != 'pass':
        raise SystemExit('runtime inspection failed; inspect retained tool outputs')


if __name__ == '__main__':
    main()
