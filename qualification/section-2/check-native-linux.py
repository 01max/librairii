#!/usr/bin/env python3
"""Bound actual portal and file-manager observations in private X11/Wayland test desktops."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import uuid

spec = importlib.util.spec_from_file_location('native_clean', pathlib.Path(__file__).with_name('check-clean-linux.py'))
clean = importlib.util.module_from_spec(spec)
spec.loader.exec_module(clean)
wayland = clean.wayland
HERE = pathlib.Path(__file__).resolve().parent
CASES = {'files-multiple', 'files-single', 'files-cancel', 'folder-accept', 'folder-cancel',
         'folder-reject', 'early-settings', 'reveal'}


def session_passed(report, binary_sha):
    common = {'owned_shutdown', 'fixture_removed', 'native_focus_restored_and_search_input'}
    for name in CASES:
        required = common | ({'early_settings_delivered', 'native_french_dead_key_and_utf8_input',
            'native_utf8_clipboard_roundtrip'} if name == 'early-settings' else
            {'actual_file_manager_window_path', 'os_settings_accelerator'} if name == 'reveal' else
            {'real_modal_dialog', 'one_real_portal_call', 'parent_identified', 'parent_associated',
             'no_parent_rejection', 'actual_picker_result', 'os_settings_accelerator'})
        checks = report.get('cases', {}).get(name, {}).get('checks', {})
        if not required <= checks.keys() or not all(checks.values()):
            return False
    return (report.get('native_integration') == 'pass' and report.get('binary_sha256') == binary_sha
            and not report.get('errors') and set(report.get('cases', {})) == CASES
            and all(case.get('checks') and all(case['checks'].values()) and case.get('returncode') == 0
                    for case in report['cases'].values()))


def inspect(package, archive, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine(), manifest['target']) != ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'):
        raise ValueError('native Linux desktop checks need the matching native host/package')
    wayland.runtime.verified_binary(package, manifest)
    clean.resources.package_bytes(package, manifest)
    archive_identity = clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    suffix = uuid.uuid4().hex
    image = 'librairii-native-' + suffix
    env = os.environ.copy()
    report = {'schema': 1, 'source': manifest['source'], 'source_dirty': manifest['source_dirty'],
        'target': manifest['target'], 'binary_sha256': manifest['binary_sha256'], 'archive': archive_identity,
        'base': clean.BASE, 'commands': {}, 'checks': {}, 'sessions': {}, 'errors': [],
        'automated_native_integration': 'fail', 'physical_input_ime_qualified': False,
        'screen_reader_speech_qualified': False, 'macos_integration_qualified': False,
        'clean_host_qualified': False, 'toolkit_selected': False,
        'scope': 'Real GTK portal selections/cancel, parent association, native OS accelerator/input and observed Thunar folder UI in isolated X11/Openbox and Wayland/Sway sessions. Synthetic OS events/AT-SPI actions, not physical input or Orca speech.'}

    def run(name, argv, timeout=30):
        result = wayland.command(argv, env, output / (name + '.log'), timeout=timeout)
        report['commands'][name] = result
        if not wayland.passed(result):
            raise RuntimeError(name + ' failed; inspect retained native desktop evidence')
        return result

    containers = []
    try:
        run('build', ['docker', 'build', '--platform=linux/amd64', '--tag', image,
                     '--build-arg', f'QUALIFICATION_UID={os.getuid()}',
                     '--build-arg', f'QUALIFICATION_GID={os.getgid()}', str(HERE / 'native-linux')], 900)
        report['image'] = json.loads(run('image', ['docker', 'image', 'inspect', image])['output'])[0]['Id']
        for session in ('x11', 'wayland'):
            evidence = output / session
            evidence.mkdir()
            name = 'librairii-native-' + session + '-' + suffix
            containers.append(name)
            try:
                run(session, ['docker', 'run', '--name', name, '--network=none', '--read-only', '--init',
                    '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', f'{os.getuid()}:{os.getgid()}',
                    '--tmpfs', '/tmp:rw,exec,nosuid,mode=1777,size=512m',
                    '--mount', f'type=bind,src={package},dst=/opt/librairii,readonly',
                    '--mount', f'type=bind,src={evidence.resolve()},dst=/evidence',
                    '--env', 'EXPECTED_BINARY_SHA256=' + manifest['binary_sha256'], image,
                    '--session', session], 600)
            except RuntimeError as error:
                report['errors'].append(str(error))
            # Retain a failed first session and still run the other native route.
            files = [p for p in evidence.rglob('*') if p.is_file()]
            if len(files) > 512 or any(p.stat().st_size > wayland.LIMIT for p in files):
                raise ValueError('native evidence size/count limit exceeded')
            try:
                session_report = json.loads((evidence / 'native-session.json').read_text())
            except (OSError, ValueError) as error:
                report['errors'].append(session + ': ' + str(error))
                report['checks'][session] = False
                continue
            report['sessions'][session] = session_report
            report['checks'][session] = session_passed(session_report, manifest['binary_sha256'])
            report['sessions'][session]['file_hashes'] = {str(p.relative_to(evidence)):
                hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        report['errors'].append(str(error))
    finally:
        for index, name in enumerate(containers):
            result = wayland.command(['docker', 'rm', '--force', name], env,
                                     output / f'remove-{index}.log', timeout=30)
            report['commands']['remove-' + name] = result
            report['checks']['retired-' + name] = wayland.passed(result)
        removed = wayland.command(['docker', 'image', 'rm', image], env, output / 'remove-image.log')
        report['commands']['remove_image'] = removed
        report['checks']['image_retired'] = wayland.passed(removed)
        if all(report['checks'].get(s, False) for s in ('x11', 'wayland')) and all(report['checks'].values()) and not report['errors']:
            report['automated_native_integration'] = 'pass'
        (output / 'native-linux.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=pathlib.Path, required=True)
    parser.add_argument('--archive', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.output.resolve())
    print(json.dumps({'checks': report['checks'], 'errors': report['errors']}, indent=2))
    raise SystemExit(0 if report['automated_native_integration'] == 'pass' else 1)
