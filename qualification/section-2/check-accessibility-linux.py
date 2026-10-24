#!/usr/bin/env python3
"""Run and retain the packaged app's private Orca baseline on native Linux."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import shutil
import tempfile
import uuid

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('native_common', HERE / 'check-native-linux.py')
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
REQUIRED = {'app_accessibility_tree', 'keyboard_search', 'keyboard_settings_and_focus_return',
    'orca_spoke_search_or_settings', 'speech_services_alive', 'normal_application_quit',
    'actual_synthesized_audio', 'owned_fixture_removed', 'orca_search_label_completed',
    'orca_settings_labels_completed'}
SPEECH_STEPS = {'frame', 'search', 'settings', 'close', 'english', 'restored'}


def baseline_passed(report, binary_sha):
    return (report.get('baseline') == 'pass' and report.get('errors') == []
        and report.get('observation_method') == 'per-focus-utterance-and-audio-completion-v3'
        and report.get('session') in ('x11', 'wayland')
        and report.get('reader_preferences') == {'enable_action_keys': report['session'] != 'x11'}
        and report.get('binary_sha256') == binary_sha and report.get('application_returncode') == 0
        and REQUIRED <= report.get('baseline_checks', {}).keys()
        and set(report.get('speech_steps', {})) == SPEECH_STEPS
        and all(step.get('audio', {}).get('nonzero_samples', 0) > 1600
            for step in report['speech_steps'].values())
        and all(report['baseline_checks'].values())
        and report.get('audio', {}).get('nonzero_samples', 0) > 1600
        and report.get('full_accessibility_qualified') is False)


def inspect(package, archive, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine(), manifest['target']) != ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'):
        raise ValueError('Orca baseline requires the matching native Linux host/package')
    common.wayland.runtime.verified_binary(package, manifest)
    common.clean.resources.package_bytes(package, manifest)
    archive_id = common.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    suffix = uuid.uuid4().hex
    image, containers = 'librairii-orca-' + suffix, []
    report = {'schema': 1, 'source': manifest['source'], 'target': manifest['target'],
        'binary_sha256': manifest['binary_sha256'], 'archive': archive_id, 'commands': {},
        'sessions': {}, 'checks': {}, 'errors': [], 'orca_baseline': 'fail',
        'full_accessibility_qualified': False, 'toolkit_selected': False}
    env = os.environ.copy()

    def run(name, argv, timeout=30):
        result = common.wayland.command(argv, env, output / (name + '.log'), timeout=timeout)
        report['commands'][name] = result
        if not common.wayland.passed(result):
            raise RuntimeError(name + ' failed; inspect retained Orca evidence')
        return result

    try:
        # Copy only these three observer sources; never send build outputs,
        # repository files or host credentials to the Docker build context.
        with tempfile.TemporaryDirectory(prefix='librairii-orca-context-') as temporary:
            context = pathlib.Path(temporary)
            for name in ('Dockerfile', 'drive.py'):
                shutil.copyfile(HERE / 'accessibility-linux' / name, context / name)
            shutil.copyfile(HERE / 'native-linux/drive.py', context / 'native-drive.py')
            run('build', ['docker', 'build', '--platform=linux/amd64', '--tag', image,
                '--build-arg', f'QUALIFICATION_UID={os.getuid()}',
                '--build-arg', f'QUALIFICATION_GID={os.getgid()}', str(context)], 900)
        report['image'] = json.loads(run('image', ['docker', 'image', 'inspect', image])['output'])[0]['Id']
        for session in ('x11', 'wayland'):
            evidence = output / session
            evidence.mkdir()
            name = 'librairii-orca-' + session + '-' + suffix
            containers.append(name)
            try:
                run(session, ['docker', 'run', '--name', name, '--network=none', '--read-only', '--init',
                    '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', f'{os.getuid()}:{os.getgid()}',
                    '--tmpfs', '/tmp:rw,exec,nosuid,mode=1777,size=512m',
                    '--mount', f'type=bind,src={package},dst=/opt/librairii,readonly',
                    '--mount', f'type=bind,src={evidence.resolve()},dst=/evidence',
                    '--env', 'EXPECTED_BINARY_SHA256=' + manifest['binary_sha256'], image,
                    '--session', session], 180)
            except RuntimeError as error:
                report['errors'].append(str(error))
            files = [p for p in evidence.rglob('*') if p.is_file()]
            if len(files) > 512 or any(p.stat().st_size > common.wayland.LIMIT for p in files):
                raise ValueError('accessibility evidence size/count limit')
            try:
                value = json.loads((evidence / 'accessibility-session.json').read_text())
                report['sessions'][session] = value
                report['checks'][session] = baseline_passed(value, manifest['binary_sha256'])
                if session == 'wayland':
                    value['application_protocol_checks'] = common.wayland.protocol_checks(
                        (evidence / 'application.log').read_text())
                    application_env = value.get('application_environment', {})
                    report['checks']['native_wayland_application'] = (
                        application_env.get('DISPLAY') is None and bool(application_env.get('WAYLAND_DISPLAY'))
                        and application_env.get('GDK_BACKEND') == 'wayland'
                        and all(value['application_protocol_checks'].values()))
                value['file_hashes'] = {str(p.relative_to(evidence)): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in files}
            except (OSError, ValueError) as error:
                report['checks'][session] = False
                report['errors'].append(session + ': ' + str(error))
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        report['errors'].append(str(error))
    finally:
        for index, name in enumerate(containers):
            result = common.wayland.command(['docker', 'rm', '--force', name], env,
                output / f'remove-{index}.log', timeout=30)
            report['commands']['remove-' + name] = result
            report['checks']['retired-' + name] = common.wayland.passed(result)
        result = common.wayland.command(['docker', 'image', 'rm', image], env, output / 'remove-image.log')
        report['commands']['remove_image'] = result
        report['checks']['image_retired'] = common.wayland.passed(result)
        if all(report['checks'].get(s, False) for s in ('x11', 'wayland')) and all(report['checks'].values()) and not report['errors']:
            report['orca_baseline'] = 'pass'
        (output / 'accessibility-linux.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('package', 'archive', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    args = parser.parse_args()
    result = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.output.resolve())
    print(json.dumps({'result': result['orca_baseline'], 'errors': result['errors']}))
    raise SystemExit(0 if result['orca_baseline'] == 'pass' else 1)
