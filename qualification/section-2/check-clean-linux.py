#!/usr/bin/env python3
"""Run a verified package offline in an immutable-base Linux runtime, without development tools."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import re
import uuid


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).with_name(file))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


wayland = module('clean_wayland', 'check-wayland.py')
resources = module('clean_resources', 'measure-resources.py')
HERE = pathlib.Path(__file__).resolve().parent
BASE = 'ubuntu:24.04@sha256:a2d43bffc46670d7108bb3c0468c67c5557c4c92e83e1e34cbff1322ad1b94dc'


def analyze(evidence, container):
    files = [p for p in evidence.rglob('*') if p.is_file()]
    if len(files) > 64 or any(p.stat().st_size > wayland.LIMIT for p in files):
        raise ValueError('runtime evidence size/count limit exceeded')
    checks = {}
    facts = (evidence / 'runtime-facts.txt').read_text()
    checks['native_nonroot_no_tools'] = ('architecture=x86_64\n' in facts and 'uid=0\n' not in facts
        and 'uid=' in facts and 'development_tools=absent\n' in facts and 'cwd=/tmp\n' in facts)
    checks['complete_run'] = 'clean_runtime=passed\n' in facts
    host, mounts = container['HostConfig'], container['Mounts']
    checks['network_disabled'] = host['NetworkMode'] == 'none'
    checks['readonly_unprivileged_root'] = host['ReadonlyRootfs'] and not host['Privileged']
    checks['no_host_source_or_tool_mounts'] = (len(mounts) == 2
        and {(m['Destination'], m['RW']) for m in mounts} == {('/opt/librairii', False), ('/evidence', True)})
    checks['all_linked_libraries_resolve'] = 'not found' not in (evidence / 'linked-libraries.txt').read_text()
    installed = [line.split()[0] for line in (evidence / 'packages.txt').read_text().splitlines()]
    checks['no_development_or_browser_packages'] = bool(installed) and not any(re.match(
        r'^(?:rustc|cargo|golang.*|nodejs|npm|python[0-9.]*|gcc(?:-[0-9]+)?|g\+\+(?:-[0-9]+)?|clang(?:-[0-9]+)?|git|chromium.*|firefox.*|libwebkit.*|libnode[0-9]*)$', name)
        or name.endswith('-dev') for name in installed)
    runs = {}
    for session in ('x11', 'wayland'):
        for renderer in ('normal', 'cpu'):
            name = session + '-' + renderer
            text = (evidence / (name + '.log')).read_text()
            snapshots = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (evidence / name).glob('*.png')}
            run = {'returncode': int((evidence / (name + '.exit')).read_text()),
                   'errors': [], 'output': text, 'snapshots': snapshots}
            verdict = wayland.renderer_checks(run, renderer == 'cpu')
            if session == 'x11':
                del verdict['xdg_window_configured'], verdict['window_buffer_submitted']
            checks[name] = all(verdict.values())
            runs[name] = {'checks': verdict, 'snapshots': snapshots,
                          'actual_surface': [line for line in text.splitlines() if 'Skia renderer (' in line]}
        checks[session + '_relaunch_owned_shutdown'] = (
            (evidence / (session + '-relaunch.exit')).read_text().strip() == '0'
            and 'quit_smoke=passed;' in (evidence / (session + '-relaunch.log')).read_text())
    return checks, runs


def inspect(package, archive, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine(), manifest['target']) != ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'):
        raise ValueError('clean Linux execution requires the matching native Linux x86-64 host/package')
    wayland.runtime.verified_binary(package, manifest)
    resources.package_bytes(package, manifest)
    archive_identity = resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    report = {'schema': 1, 'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
        'target': manifest['target'], 'binary_sha256': manifest['binary_sha256'], 'archive': archive_identity,
        'base': BASE, 'commands': {}, 'checks': {}, 'errors': [], 'isolated_linux_runtime': 'fail',
        'clean_host_qualified': False, 'macos_clean_host_qualified': False, 'portal_integration_qualified': False,
        'physical_input_qualified': False, 'toolkit_selected': False,
        'scope': 'Offline native Linux runtime container with no development tools/source, actual X11/Wayland GUI submissions and fresh-process relaunch. Not a full desktop install or macOS qualification.'}
    suffix = uuid.uuid4().hex
    image, container = 'librairii-qualification-' + suffix, 'librairii-clean-' + suffix
    env = os.environ.copy()

    def run(name, argv, timeout=30):
        result = wayland.command(argv, env, output / (name + '.log'), timeout=timeout)
        report['commands'][name] = result
        if not wayland.passed(result):
            raise RuntimeError(name + ' failed; inspect retained log')
        return result

    try:
        run('build', ['docker', 'build', '--platform=linux/amd64', '--tag', image, str(HERE / 'clean-linux')], 900)
        report['image'] = json.loads(run('image', ['docker', 'image', 'inspect', image])['output'])[0]['Id']
        evidence = output / 'runtime'
        evidence.mkdir()
        run('execute', ['docker', 'run', '--name', container, '--network=none', '--read-only', '--init',
            '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', f'{os.getuid()}:{os.getgid()}',
            '--tmpfs', '/tmp:rw,exec,nosuid,mode=1777,size=512m',
            '--mount', f'type=bind,src={package},dst=/opt/librairii,readonly',
            '--mount', f'type=bind,src={evidence.resolve()},dst=/evidence',
            '--env', 'EXPECTED_BINARY_SHA256=' + manifest['binary_sha256'], image], 300)
        details = json.loads(run('container', ['docker', 'inspect', container])['output'])[0]
        report['checks'], report['runs'] = analyze(evidence, details)
        report['checks']['container_exited_successfully'] = details['State']['ExitCode'] == 0
        report['checks']['binary_matches_package'] = (evidence / 'executable.sha256').read_text().split()[0] == manifest['binary_sha256']
        report['runtime_files'] = {str(p.relative_to(evidence)): hashlib.sha256(p.read_bytes()).hexdigest()
                                    for p in evidence.rglob('*') if p.is_file()}
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        report['errors'].append(str(error))
    finally:
        for name, argv in [('remove_container', ['docker', 'rm', '--force', container]),
                           ('remove_image', ['docker', 'image', 'rm', image])]:
            report['commands'][name] = wayland.command(argv, env, output / (name + '.log'), timeout=30)
        report['checks']['owned_runtime_removed'] = all(wayland.passed(report['commands'][name])
                                                       for name in ('remove_container', 'remove_image'))
        if report['checks'] and all(report['checks'].values()) and not report['errors']:
            report['isolated_linux_runtime'] = 'pass'
        (output / 'clean-linux.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=pathlib.Path, required=True)
    parser.add_argument('--archive', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.output.resolve())
    print(json.dumps({'checks': report['checks'], 'errors': report['errors']}, indent=2))
    raise SystemExit(0 if report['isolated_linux_runtime'] == 'pass' else 1)
