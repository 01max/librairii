#!/usr/bin/env python3
"""Qualify French/Latin UTF-8 editing; Japanese IME is an explicit optional diagnostic."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import subprocess
import time
import traceback

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('input_native_macos', HERE / 'check-native-macos.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
REQUIRED = {'french_dead_key_and_azerty', 'native_select_all_and_clear',
    'input_sources_restored', 'normal_shutdown', 'fixture_removed', 'native_composed_decomposed_editing',
    'native_long_french_clipboard_undo_redo', 'native_input_under_load_budget', 'native_thousand_navigation_budget'}
JAPANESE_REQUIRED = {'japanese_preedit', 'japanese_candidate_commit', 'japanese_cancel_without_commit'}


def passed(report, binary_sha):
    required = REQUIRED | (JAPANESE_REQUIRED if report.get('include_japanese') is True else set())
    return (report.get('binary_sha256') == binary_sha and report.get('errors') == []
        and report.get('returncode') == 0 and type(report.get('include_japanese')) is bool and required <= report.get('checks', {}).keys()
        and all(report['checks'].values()) and report.get('full_input_display_qualified') is False)


def sources_by_id(inventory):
    sources = inventory['sources']
    result = {source['id']: source for source in sources}
    if len(result) != len(sources) or inventory['current'] not in result:
        raise ValueError('input-source inventory identities are incomplete or duplicated')
    return result


def composition_cancelled(phase, after_escape):
    preedits = [line for line in after_escape.splitlines() if line.startswith('Preedit(')]
    return bool(preedits) and preedits[-1] == 'Preedit("", None)' and 'Commit(' not in phase


def japanese_launch_notice(observed, source):
    return (observed.get('current') == source and observed.get('source', {}).get('id') == source
        and observed['source'].get('selected') is True
        and observed['source'].get('bundle') == 'com.apple.inputmethod.Kotoeri.RomajiTyping'
        and any(type(process.get('pid')) is int and process['pid'] > 0
            and process.get('bundle') in ('com.apple.inputmethod.Kotoeri.RomajiTyping', 'com.apple.JapaneseIM.RomajiTyping')
            and process.get('finished_launching') is True and process.get('terminated') is False
            for process in observed.get('processes', [])))


def all_text_selected(observed, text, pid):
    return (bool(text) and observed.get('pid') == pid and observed.get('frontmost_pid') == pid
        and observed.get('focused', {}).get('role') == 'AXTextField'
        and observed['focused'].get('focused') is True
        and observed['focused'].get('value') == text and observed.get('selected_text') == text
        and observed.get('selection_observed') is True and observed.get('selection_location') == 0
        and observed.get('selection_length') == len(text.encode('utf-16-le')) // 2)


def wait_for_selection(observer, observations, value, pid):
    def selected():
        snapshot = observer.call('input-selection')
        if len(observations) < 16:
            observations.append(snapshot)
        return snapshot if all_text_selected(snapshot, value, pid) else None
    return observer.wait(selected)


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


def inspect(package, archive, helper, output, *, include_japanese=False):
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
        'toolkit_selected': False, 'method': 'native-French-Latin-UTF8-and-AX-editing-v4',
        'include_japanese': include_japanese,
        'deferred_qualification': ['Japanese IME', 'Chinese IME', 'Arabic input/shaping']}
    app = subprocess.Popen([str(binary), '--native-probe=' + str(rendezvous), '--native-case=input', '--native-load', '--slow-synthetic-work'],
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
        if include_japanese:
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

        def clear_search(value, stage):
            # Observe native selection after one menu command. Never invoke
            # Select All again to turn missing command delivery into a pass.
            report.setdefault('native_select_all_commands', {})[stage] = observer.call('menu', title='Select All', submenu='Edit')
            observations = report.setdefault('native_selection_observations', {}).setdefault(stage, [])
            report.setdefault('native_full_selections', {})[stage] = wait_for_selection(observer, observations, value, app.pid)
            observer.call('key', key='delete')
            query('')

        french = sources['com.apple.keylayout.French']
        select(french)
        # Physical ANSI positions: ^ then e, é, space, a/z on French AZERTY.
        # Never attach a Unicode string to a key event or set the field value.
        observer.call('keycodes', codes=[33, 14, 19, 49, 12, 13])
        report['french_state'] = query('êé az')
        observer.capture('french-dead-key')
        report['checks']['french_dead_key_and_azerty'] = True
        # The French layout maps physical0 to Q even with Command held. Use
        # the actual native menu so clearing cannot accidentally post Quit.
        clear_search('êé az', 'french')
        report['checks']['native_select_all_and_clear'] = True
        text = 'Été e\u0301 Z'
        observer.call('clipboard-set', text=text)
        observer.call('menu', title='Paste', submenu='Edit')
        report['editing_states'] = [query(text)]
        observer.call('key', key='left')
        observer.call('key', key='delete')
        report['editing_states'].append(query('Été e\u0301Z'))
        observer.call('menu', title='Undo', submenu='Edit')
        report['editing_states'].append(query(text))
        observer.call('menu', title='Redo', submenu='Edit')
        report['editing_states'].append(query('Été e\u0301Z'))
        clear_search('Été e\u0301Z', 'composed-decomposed')
        report['checks']['native_composed_decomposed_editing'] = True
        long_text = 'Été, façade et bibliothèque : l’histoire très longue. ' * 16
        observer.call('clipboard-set', text=long_text)
        observer.call('menu', title='Paste', submenu='Edit')
        report['long_french_states'] = [query(long_text)]
        observer.call('menu', title='Undo', submenu='Edit')
        report['long_french_states'].append(query(''))
        observer.call('menu', title='Redo', submenu='Edit')
        report['long_french_states'].append(query(long_text))
        observer.capture('long-french-input')
        clear_search(long_text, 'long-french')
        report['checks']['native_long_french_clipboard_undo_redo'] = True
        (rendezvous / 'start-load').touch()
        observer.wait(lambda: json.loads((rendezvous / 'geometry.json').read_text())['working'])
        report['native_input_performance'] = observer.call('performance-input', directory=str(rendezvous))
        assert len(report['native_input_performance']['samples_ms']) == 64
        assert json.loads((rendezvous / 'geometry.json').read_text())['working'], 'background load ended before measurement'
        assert report['native_input_performance']['p95_ms'] <= 100
        report['checks']['native_input_under_load_budget'] = True
        # Time native keyboard navigation from the complete1000-row Search state
        # to the last real selected row. No direct select/focus setter is used.
        start = time.monotonic()
        for _ in range(24):
            if json.loads((rendezvous / 'geometry.json').read_text())['focus_key'] == 31:
                break
            observer.call('key', key='tab')
        else:
            raise AssertionError('native keyboard list focus not reached')
        observer.call('key', key='end')
        geometry = observer.wait(lambda: (g := json.loads((rendezvous / 'geometry.json').read_text()))
            and g['rows'] == 1000 and g['selected_id'] == 999 and g['selected_row'] == 999 and g)
        report['thousand_navigation'] = {'elapsed_ms': (time.monotonic() - start) * 1000, 'geometry': geometry,
            'endpoint': 'native Tab/End selects last actual row of1000; includes OS dispatch and observer overhead'}
        assert report['thousand_navigation']['elapsed_ms'] <= 3000
        observer.capture('thousand-last-selected')
        report['checks']['native_thousand_navigation_budget'] = True
        if include_japanese:
            # Restore Search by the native OS accelerator before optional input.
            observer.call('key', key='cmd+f')
            parent = parents[0]
            if not sources_by_id(observer.call('input-sources'))[parent['id']]['enabled']:
                observer.call('input-source', source=parent['id'], operation='enable')
            select(japanese)
            report['japanese_method_observations'] = []
            def method_launched():
                observed = observer.call('input-method-status', source=japanese['id'])
                if len(report['japanese_method_observations']) < 16:
                    report['japanese_method_observations'].append(observed)
                return observed if japanese_launch_notice(observed, japanese['id']) else None
            try:
                report['japanese_launch_notice'] = observer.wait(method_launched, timeout=2)
            except TimeoutError:
                # Some applications never post DidFinishLaunching. This optional
                # diagnostic cannot substitute for, or veto, actual IME behavior.
                report['japanese_launch_notice'] = None
            offset = len(trace())
            # n i h o n n through the actual OS input method. The final doubled n
            # completes ん; one trailing n remains live Roman input (にほn).
            # The public Winit trace
            # must show preedit before commit; final Unicode text alone cannot pass.
            observer.call('keycodes', codes=[45, 34, 4, 31, 45, 45])
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
            clear_search('日本', 'japanese')
            offset = len(trace())
            observer.call('keycodes', codes=[45, 34, 4, 31, 45, 45])
            observer.wait(lambda: 'Preedit("にほん"' in trace()[offset:])
            escape_offset = len(trace())
            observer.call('key', key='escape')
            observer.call('key', key='escape')
            cancelled = observer.wait(lambda: (t := trace())
                and composition_cancelled(t[offset:], t[escape_offset:]) and t[offset:])
            report['japanese_cancelled_state'] = query('')
            report['japanese_cancel_trace'] = cancelled
            report['japanese_post_escape_trace'] = trace()[escape_offset:]
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
    parser.add_argument('--include-japanese', action='store_true', help='Run the deferred Japanese diagnostic explicitly; not part of the default gate')
    args = parser.parse_args()
    result = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True),
        args.helper.resolve(strict=True), args.output.resolve(), include_japanese=args.include_japanese)
    print(json.dumps({'result': result['result'], 'errors': result['errors']}))
    raise SystemExit(0 if result['result'] == 'pass' else 1)
