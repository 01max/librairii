#!/usr/bin/env python3
"""Observe dead-key and Japanese IME input on the disposable native CI account."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import subprocess
import traceback

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('input_native_macos', HERE / 'check-native-macos.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
REQUIRED = {'french_dead_key_and_azerty', 'japanese_preedit', 'japanese_candidate_commit',
    'japanese_cancel_without_commit', 'input_sources_restored', 'normal_shutdown', 'fixture_removed'}


def passed(report, binary_sha):
    return (report.get('binary_sha256') == binary_sha and report.get('errors') == []
        and report.get('returncode') == 0 and REQUIRED <= report.get('checks', {}).keys()
        and all(report['checks'].values()) and report.get('full_input_display_qualified') is False)


def sources_by_id(inventory):
    sources = inventory['sources']
    result = {source['id']: source for source in sources}
    if len(result) != len(sources) or inventory['current'] not in result:
        raise ValueError('input-source inventory identities are incomplete or duplicated')
    return result


def restore_sources(observer, before):
    # Re-read effective states after every mutation: enabling a parent can also
    # enable its modes. Restore modes before their parent, then verify all bits.
    observer.call('input-source', source=before['current'], operation='select')
    original = sources_by_id(before)
    current = sources_by_id(observer.call('input-sources'))
    extra = [value for key, value in current.items() if value['enabled']
        and key in original and not original[key]['enabled'] and value['enable_capable']]
    extra.sort(key=lambda value: 'InputMode' not in value['type'])
    for value in extra:
        current = sources_by_id(observer.call('input-sources'))
        if current[value['id']]['enabled']:
            observer.call('input-source', source=value['id'], operation='disable')
    after = observer.call('input-sources')
    effective = sources_by_id(after)
    if (after['current'] != before['current'] or set(effective) != set(original)
            or any(effective[key]['enabled'] != value['enabled'] for key, value in original.items())):
        raise ValueError('input sources were not fully restored')
    return after


def inspect(package, archive, helper, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system() != 'Darwin' or os.environ.get('GITHUB_ACTIONS') != 'true'
            or os.environ.get('TASK2_INPUT_CI') != '1'
            or (platform.system(), platform.machine()) != native.common.wayland.runtime.TARGETS[manifest['target']]):
        raise ValueError('matching disposable hosted macOS CI account required; local settings are never changed')
    binary = native.common.wayland.runtime.verified_binary(package, manifest)
    native.common.clean.resources.package_bytes(package, manifest)
    archive_id = native.common.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    rendezvous = output / 'app'
    report = {'schema': 1, 'source': manifest['source'], 'target': manifest['target'],
        'binary_sha256': manifest['binary_sha256'], 'archive': archive_id,
        'checks': {}, 'errors': [], 'returncode': None, 'full_input_display_qualified': False,
        'toolkit_selected': False, 'method': 'native-CGEvent-TIS-and-propagated-Winit-IME-v1'}
    app = subprocess.Popen([str(binary), '--native-probe=' + str(rendezvous), '--native-case=input'],
        env={**os.environ, 'SLINT_BACKEND': 'winit-skia-software'}, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, start_new_session=True)
    capture = native.capture_output(app, output / 'app.log')
    observer = native.Observer(helper, output, app.pid)
    before, fixture = None, None
    try:
        observer.wait(lambda: (rendezvous / 'ready.txt').exists())
        fixture = pathlib.Path((rendezvous / 'fixture.txt').read_text())
        report['fixture'] = str(fixture)
        observer.call('activate')
        observer.wait(lambda: observer.status()['frontmost_pid'] == app.pid)
        before = observer.call('input-sources')
        report['input_sources_before'] = before
        sources = sources_by_id(before)
        japanese = [s for s in sources.values() if s['id'].startswith('com.apple.inputmethod.Kotoeri.')
            and s['id'].endswith('.Japanese') and s['select_capable']]
        if len(japanese) != 1:
            raise ValueError('exact installed Japanese Roman-input mode required')
        japanese = japanese[0]
        parents = [s for s in sources.values() if s.get('bundle') == japanese.get('bundle')
            and 'InputMethodModeEnabled' in s['type']]
        if len(parents) != 1:
            raise ValueError('Japanese input method parent is not unique')

        def select(source):
            current = sources_by_id(observer.call('input-sources'))[source['id']]
            if not current['enabled']:
                observer.call('input-source', source=source['id'], operation='enable')
            observer.call('input-source', source=source['id'], operation='select')
            observer.wait(lambda: observer.status()['keyboard_input_source'] == source['id'])

        def query(value):
            return observer.wait(lambda: (s := native.state(rendezvous)) and s[0] == value and s)

        def trace():
            path = rendezvous / 'ime-events.txt'
            return path.read_text() if path.exists() else ''

        french = sources['com.apple.keylayout.French']
        select(french)
        # Physical ANSI positions: ^ then e, é, space, a/z on French AZERTY.
        # Never attach a Unicode string to a key event or set the field value.
        observer.call('keycodes', codes=[33, 14, 19, 49, 12, 13])
        report['french_state'] = query('êé az')
        observer.capture('french-dead-key')
        report['checks']['french_dead_key_and_azerty'] = True
        observer.call('key', key='cmd+a')
        observer.call('key', key='delete')
        query('')
        parent = parents[0]
        if not sources_by_id(observer.call('input-sources'))[parent['id']]['enabled']:
            observer.call('input-source', source=parent['id'], operation='enable')
        select(japanese)
        offset = len(trace())
        # n i h o n through the actual OS input method. The public Winit trace
        # must show preedit before commit; final Unicode text alone cannot pass.
        observer.call('keycodes', codes=[45, 34, 4, 31, 45])
        preedit = observer.wait(lambda: (t := trace()[offset:]) and 'Preedit("にほん"' in t and t)
        report['japanese_preedit_trace'] = preedit
        report['japanese_preedit_state'] = native.state(rendezvous)
        if report['japanese_preedit_state'][0] != '' or 'Commit(' in preedit:
            raise ValueError('Japanese preedit prematurely changed committed Search text')
        observer.capture('japanese-preedit')
        report['checks']['japanese_preedit'] = True
        observer.call('key', key='space')
        observer.capture('japanese-candidate')
        observer.call('key', key='return')
        report['japanese_committed_state'] = query('日本')
        report['japanese_commit_trace'] = observer.wait(lambda:
            (t := trace()[offset:]) and 'Commit("日本")' in t and t)
        report['checks']['japanese_candidate_commit'] = True
        observer.capture('japanese-committed')
        observer.call('key', key='cmd+a')
        observer.call('key', key='delete')
        query('')
        offset = len(trace())
        observer.call('keycodes', codes=[45, 34, 4, 31, 45])
        observer.wait(lambda: 'Preedit("にほん"' in trace()[offset:])
        observer.call('key', key='escape')
        observer.call('key', key='escape')
        cancelled = observer.wait(lambda: (t := trace()[offset:]) and 'Preedit("", None)' in t and t)
        if 'Commit(' in cancelled:
            raise ValueError('cancelled Japanese composition committed text')
        report['japanese_cancelled_state'] = query('')
        report['japanese_cancel_trace'] = cancelled
        report['checks']['japanese_cancel_without_commit'] = True
    except Exception:
        report['errors'].append(traceback.format_exc())
        report['last_app_state'] = native.state(rendezvous)
        try:
            observer.capture('failure')
        except Exception as error:
            report['errors'].append('failure capture: ' + str(error))
    finally:
        # Always attempt settings restoration before retiring the target app,
        # including any failed composition/selection stage. Restoration failure
        # is fatal and cannot be hidden by otherwise passing input assertions.
        if before is not None:
            try:
                report['input_sources_after'] = restore_sources(observer, before)
                report['checks']['input_sources_restored'] = True
            except Exception:
                report['errors'].append('input-source restoration: ' + traceback.format_exc())
        try:
            (rendezvous / 'finish').touch()
            report['returncode'] = app.wait(timeout=10)
            report['checks']['normal_shutdown'] = (app.returncode == 0
                and 'fixture_retired=true' in (rendezvous / 'retired.txt').read_text())
            report['checks']['fixture_removed'] = fixture is not None and not fixture.exists()
        except Exception:
            report['errors'].append('normal shutdown: ' + traceback.format_exc())
        report['errors'] += native.retire(app, capture)
        report['result'] = 'pass' if passed(report, manifest['binary_sha256']) else 'fail'
        files = [p for p in output.rglob('*') if p.is_file()]
        if len(files) > 512 or any(p.stat().st_size > native.LIMIT for p in files):
            report['errors'].append('input evidence size/count limit')
            report['result'] = 'fail'
        report['file_hashes'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
        (output / 'input-macos.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('package', 'archive', 'helper', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    args = parser.parse_args()
    result = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True),
        args.helper.resolve(strict=True), args.output.resolve())
    print(json.dumps({'result': result['result'], 'errors': result['errors']}))
    raise SystemExit(0 if result['result'] == 'pass' else 1)
