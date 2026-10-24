#!/usr/bin/env python3
"""Observe localized window sizes and genuine mixed-scale Wayland transitions."""
import argparse
import hashlib
import importlib.util
import json
import math
import os
import pathlib
import platform
import uuid

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('display_linux_native', HERE / 'check-native-linux.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
REQUIRED = {'native_compact_intermediate_wide', 'localized_modal_focus_and_dismiss',
    'actual_text_control_selection_contrast', 'compact_review_keyboard_reachable', 'idle_without_nonessential_motion', 'normal_shutdown', 'fixture_retired'}


def qualified(value, session):
    checks = value.get('checks', {})
    required = REQUIRED | ({'actual_mixed_scale_transitions'} if session == 'wayland' else set())
    sizes = value.get('sizes', [])
    if (value.get('errors') != [] or value.get('returncode') != 0 or not required <= checks.keys()
            or not all(checks.values()) or len(sizes) != 3):
        return False
    for g, (width, height) in zip(sizes, ((560, 400), (900, 760), (1280, 800))):
        if (abs(g.get('logical_width', 0) - width) > 1 or abs(g.get('logical_height', 0) - height) > 1
                or g.get('compact') is not (width < 850) or g.get('wide') is not (width >= 1180)
                or len(g.get('contrast_ratios', [])) != 3
                or not all(type(v) in (int, float) and math.isfinite(v) and v >= 4.5 for v in g['contrast_ratios'])):
            return False
    if session == 'wayland':
        transitions = value.get('scale_transitions', [])
        if len(transitions) != 4:
            return False
        for g, scale in zip(transitions, (2, 1, 2, 1)):
            if (g.get('scale') != scale or g.get('rows') != 1000
                    or abs(g.get('logical_width', 0) - 1280) > 1 or abs(g.get('logical_height', 0) - 800) > 1
                    or g.get('compact') is not False or g.get('wide') is not True
                    or abs(g.get('width', 0) - g.get('logical_width', -100) * scale) > 1
                    or abs(g.get('height', 0) - g.get('logical_height', -100) * scale) > 1):
                return False
    return True


def inspect(package, archive, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine(), manifest['target']) != ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'):
        raise ValueError('matching native Linux package required')
    native.wayland.runtime.verified_binary(package, manifest)
    native.clean.resources.package_bytes(package, manifest)
    archive_id = native.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    image, containers = 'librairii-display-' + uuid.uuid4().hex, []
    report = {'schema': 1, 'source': manifest['source'], 'target': manifest['target'],
        'binary_sha256': manifest['binary_sha256'], 'archive': archive_id, 'errors': [],
        'checks': {}, 'commands': {}, 'sessions': {}, 'toolkit_selected': False}
    def run(name, argv, timeout=30):
        value = native.wayland.command(argv, os.environ.copy(), output / (name + '.log'), timeout=timeout)
        report['commands'][name] = value
        if not native.wayland.passed(value):
            raise RuntimeError(name + ' failed; inspect retained display evidence')
        return value
    try:
        run('build', ['docker', 'build', '--platform=linux/amd64', '--tag', image,
            '--build-arg', f'QUALIFICATION_UID={os.getuid()}', '--build-arg', f'QUALIFICATION_GID={os.getgid()}',
            str(HERE / 'native-linux')], 900)
        for session in ('x11', 'wayland'):
            evidence = output / session
            evidence.mkdir()
            name = image + '-' + session
            containers.append(name)
            try:
                run(session, ['docker', 'run', '--name', name, '--network=none', '--read-only', '--init',
                    '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', f'{os.getuid()}:{os.getgid()}',
                    '--tmpfs', '/tmp:rw,exec,nosuid,mode=1777,size=512m',
                    '--mount', f'type=bind,src={package},dst=/opt/librairii,readonly',
                    '--mount', f'type=bind,src={evidence.resolve()},dst=/evidence',
                    '--env', 'EXPECTED_BINARY_SHA256=' + manifest['binary_sha256'], image,
                    '--session', session, '--display-only'], 180)
            except RuntimeError as error:
                report['errors'].append(str(error))
            files = [p for p in evidence.rglob('*') if p.is_file()]
            if len(files) > 256 or any(p.stat().st_size > native.wayland.LIMIT for p in files):
                raise ValueError('bounded display evidence limit')
            value = json.loads((evidence / 'display/display-session.json').read_text())
            report['sessions'][session] = value
            report['checks'][session] = qualified(value, session)
            value['file_hashes'] = {str(p.relative_to(evidence)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    except (OSError, ValueError, RuntimeError) as error:
        report['errors'].append(str(error))
    finally:
        for i, name in enumerate(containers):
            value = native.wayland.command(['docker', 'rm', '--force', name], os.environ.copy(), output / f'remove-{i}.log')
            report['checks']['retired-' + name] = native.wayland.passed(value)
        value = native.wayland.command(['docker', 'image', 'rm', image], os.environ.copy(), output / 'remove-image.log')
        report['checks']['image_retired'] = native.wayland.passed(value)
    report['result'] = 'pass' if {'x11', 'wayland'} <= report['checks'].keys() and all(report['checks'].values()) and not report['errors'] else 'fail'
    (output / 'display-linux.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('package', 'archive', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    args = parser.parse_args()
    value = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.output.resolve())
    print(json.dumps({'result': value['result'], 'errors': value['errors']}))
    raise SystemExit(0 if value['result'] == 'pass' else 1)
