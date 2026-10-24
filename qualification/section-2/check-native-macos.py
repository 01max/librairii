#!/usr/bin/env python3
"""Observe the packaged app's actual macOS sheets, menus, clipboard and Finder path."""
import argparse
import hashlib
import importlib.util
import itertools
import json
import os
import pathlib
import platform
import select
import subprocess
import threading
import time
import traceback
import urllib.parse

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('mac_native_common', HERE / 'check-native-linux.py')
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
LIMIT = common.wayland.LIMIT
CASES = [('files-multiple', 'files', 'selected'), ('files-single', 'files', 'selected'),
    ('files-cancel', 'files', 'cancelled'), ('folder-accept', 'folder', 'selected'),
    ('folder-cancel', 'folder', 'cancelled'), ('folder-reject', 'folder', 'rejected'),
    ('early-settings', 'early-settings', 'delivered'), ('reveal', 'reveal', 'requested')]
COMMAND_IDS = itertools.count(1)


def session_passed(report, binary_sha):
    if (report.get('binary_sha256') != binary_sha or report.get('errors') != []
            or set(report.get('cases', {})) != {name for name, _, _ in CASES}):
        return False
    for name, kind, _ in CASES:
        case = report['cases'][name]
        required = {'native_settings_accelerator', 'native_settings_menu', 'native_search_input',
                    'owned_shutdown', 'fixture_removed'}
        required |= ({'real_parented_modal_sheet', 'exact_native_callback', 'unforced_parent_focus_restored'}
            if kind in ('files', 'folder') else {'actual_finder_path', 'explicit_return_from_finder'}
            if kind == 'reveal' else {'early_settings_delivered', 'native_unicode_clipboard_and_edit_menu'})
        if (case.get('errors') != [] or case.get('returncode') != 0
                or not required <= case.get('checks', {}).keys() or not all(case['checks'].values())):
            return False
    return True


def selector(node):
    return {**node['selector'], 'match': {key: node.get(key, '') for key in ('role', 'title', 'description', 'identifier')}}


def same_root(node, root):
    a, b = node['selector'], root['selector']
    return a['window'] == b['window'] and a.get('sheet') == b.get('sheet')


def state(directory):
    try:
        rows = (directory / 'state.txt').read_text().splitlines()
        return rows if len(rows) == 5 else None
    except FileNotFoundError:
        return None


def picker_root(snapshot):
    # Public AX window->child-sheet relationship is the parenting witness.
    return next((node for node in snapshot['nodes'] if node['role'] == 'AXSheet'
        and node['selector'].get('sheet') is not None and not node['selector']['path']
        and node.get('modal') is not False), None)


def capture_output(app, path):
    stop, errors = threading.Event(), []
    descriptor = app.stdout.fileno()
    os.set_blocking(descriptor, False)
    def capture():
        try:
            with path.open('xb') as stream, app.stdout:
                size = 0
                while not stop.is_set():
                    if not select.select([descriptor], [], [], 0.1)[0]:
                        continue
                    data = os.read(descriptor, 65536)
                    if not data:
                        break
                    stream.write(data[:max(0, LIMIT - size)])
                    size += len(data)
                    if size >= LIMIT:
                        errors.append('app log limit')
                        common.wayland.kill_group(app.pid)
                        break
        except Exception as error:
            errors.append('capture failure: ' + str(error))
            common.wayland.kill_group(app.pid)
    reader = threading.Thread(target=capture)
    reader.start()
    return stop, reader, errors


def retire(app, capture):
    stop, reader, errors = capture
    if app.poll() is not None and common.wayland.group_alive(app.pid):
        errors.append('descendant survived exited app')
    common.wayland.kill_group(app.pid)
    app.wait(timeout=5)
    reader.join(timeout=1)
    if reader.is_alive():
        errors.append('log reader required forced retirement')
        stop.set()
        reader.join(timeout=1)
    if reader.is_alive():
        errors.append('unretired log reader')
    return errors


class Observer:
    def __init__(self, binary, output, pid):
        self.binary, self.output, self.pid = binary, output, pid

    def call(self, action, **request):
        sequence = next(COMMAND_IDS)
        result = common.wayland.command([str(self.binary), action,
            json.dumps({'pid': self.pid, **request}, ensure_ascii=False)], os.environ.copy(),
            self.output / f'command-{sequence:04}-{action}.log', timeout=12)
        if not common.wayland.passed(result):
            raise RuntimeError(action + ' failed: ' + result['output'][:2000])
        return json.loads(result['output'])

    def snapshot(self, name=None, document=None):
        value = self.call('snapshot', **({'document': str(document)} if document else {}))
        if name:
            (self.output / (name + '.json')).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
        return value

    def wait(self, predicate, timeout=12):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = predicate()
            if value:
                return value
            time.sleep(0.05)
        raise TimeoutError('native macOS observation deadline')

    def button(self, snapshot, root, titles):
        matches = [n for n in snapshot['nodes'] if same_root(n, root) and n['role'] == 'AXButton'
            and n.get('enabled') is not False and n.get('title') in titles]
        if len(matches) != 1:
            raise ValueError('native button not unique: ' + repr(titles))
        self.call('press', selector=selector(matches[0]))

    def go_folder(self, directory, root):
        self.call('key', key='cmd+shift+g')
        def location():
            snapshot = self.snapshot()
            fields = [n for n in snapshot['nodes'] if same_root(n, root)
                and n['role'] in ('AXTextField', 'AXComboBox') and n.get('value_settable')
                and n.get('enabled') is not False]
            preferred = [n for n in fields if 'go to' in (n.get('title', '') + n.get('description', '')).lower()]
            return (snapshot, (preferred or fields)[-1]) if fields else None
        snapshot, field = self.wait(location)
        (self.output / 'go-folder.json').write_text(json.dumps(snapshot, ensure_ascii=False, indent=2) + '\n')
        self.call('set', selector=selector(field), value=str(directory))
        snapshot = self.snapshot()
        go = [n for n in snapshot['nodes'] if same_root(n, root) and n['role'] == 'AXButton'
            and n.get('title') == 'Go' and n.get('enabled') is not False]
        if len(go) == 1:
            self.call('press', selector=selector(go[0]))
        else:
            self.call('key', key='return')
        time.sleep(0.2)

    def rows(self, fixture, root, multiple):
        self.call('key', key='cmd+2')
        wanted = ['Été 日本語 故事.zip'] + (['Deuxième archive 02.pk'] if multiple else [])
        def candidates():
            snapshot = self.snapshot()
            nodes = [n for n in snapshot['nodes'] if same_root(n, root)]
            selected = []
            for filename in wanted:
                names = [n for n in nodes if filename in (n.get('title'), n.get('value'), n.get('description'))]
                found = None
                for name in names:
                    path = name['selector']['path']
                    rows = [n for n in nodes if n['role'] == 'AXRow'
                        and path[:len(n['selector']['path'])] == n['selector']['path']]
                    if rows:
                        found = max(rows, key=lambda n: len(n['selector']['path']))
                        break
                if not found:
                    return None
                selected.append(found)
            parent_path = selected[0]['selector']['path'][:-1]
            parent = next((n for n in nodes if n['selector']['path'] == parent_path), None)
            if parent and all(n['selector']['path'][:-1] == parent_path for n in selected):
                return snapshot, parent, selected
            return None
        snapshot, parent, selected = self.wait(candidates)
        (self.output / 'rows.json').write_text(json.dumps(snapshot, ensure_ascii=False, indent=2) + '\n')
        self.call('select', selector=selector(parent), rows=[selector(n) for n in selected])

    def capture(self, name, snapshot=None):
        snapshot = snapshot or self.snapshot()
        windows = [w for w in snapshot['windows'] if w.get('kCGWindowLayer') == 0]
        if not windows:
            raise ValueError('no actual target window for screenshot')
        window = max(windows, key=lambda w: w.get('kCGWindowBounds', {}).get('Width', 0)
                     * w.get('kCGWindowBounds', {}).get('Height', 0))
        result = common.wayland.command(['/usr/sbin/screencapture', '-x', '-l', str(window['kCGWindowNumber']),
            str(self.output / (name + '.png'))], os.environ.copy(), self.output / (name + '-capture.log'), timeout=10)
        if not common.wayland.passed(result) or not (self.output / (name + '.png')).is_file():
            raise RuntimeError('actual native window capture failed')


def close_finder(finder, fixture):
    snapshot = finder.snapshot(document=fixture)
    windows = [n for n in snapshot['nodes'] if n['role'] == 'AXWindow' and not n['selector']['path']]
    if not windows:
        return  # The verified document window has already closed.
    if len(windows) != 1:
        raise ValueError('owned Finder window not unique during cleanup')
    controls = [n for n in snapshot['nodes'] if same_root(n, windows[0]) and
        n.get('subrole') == 'AXCloseButton']
    if len(controls) != 1:
        raise ValueError('owned Finder close control not unique')
    finder.call('press', selector=selector(controls[0]))


def one_case(binary, helper, root, name, kind, expected):
    output = root / name
    output.mkdir()
    rendezvous = output / 'app'
    argv = [str(binary), '--native-probe=' + str(rendezvous), '--native-case=' + kind]
    if kind == 'early-settings':
        argv.append('--settings-early')
    env = {**os.environ, 'SLINT_BACKEND': 'winit-skia-software'}
    app = subprocess.Popen(argv, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
    capture = capture_output(app, output / 'app.log')
    observer = Observer(helper, output, app.pid)
    report = {'kind': kind, 'checks': {}, 'errors': [], 'app_pid': app.pid, 'returncode': None}
    fixture, owned_finder = None, None
    try:
        observer.wait(lambda: (rendezvous / 'ready.txt').exists())
        fixture = pathlib.Path((rendezvous / 'fixture.txt').read_text())
        report['fixture'] = str(fixture)
        observer.call('activate')  # Initial launch only; picker restoration is never forced.
        observer.wait(lambda: observer.snapshot()['frontmost_pid'] == app.pid)
        if kind in ('files', 'folder'):
            snapshot = observer.wait(lambda: (s := observer.snapshot()) and picker_root(s) and s)
            root_node = picker_root(snapshot)
            report['parent_sheet'] = root_node
            report['checks']['real_parented_modal_sheet'] = True
            observer.snapshot('picker')
            observer.capture('picker')
            if expected == 'cancelled':
                observer.button(snapshot, root_node, ('Cancel',))
            else:
                destination = fixture.parent if expected == 'rejected' else fixture
                observer.go_folder(destination, root_node)
                if kind == 'files':
                    observer.rows(fixture, root_node, name == 'files-multiple')
                snapshot = observer.snapshot('selection')
                observer.capture('selection', snapshot)
                observer.button(snapshot, root_node, ('Open', 'Choose', 'Select'))
            observer.wait(lambda: (rendezvous / 'callback-complete.txt').exists())
            paths = (rendezvous / 'selected-paths.txt').read_text().splitlines()
            wanted = [] if expected == 'cancelled' else [str(fixture.parent)] if expected == 'rejected' else (
                [str(fixture / 'Été 日本語 故事.zip')] + ([str(fixture / 'Deuxième archive 02.pk')] if name == 'files-multiple' else [])
                if kind == 'files' else [str(fixture)])
            report['selected_paths'] = paths
            report['checks']['exact_native_callback'] = ((rendezvous / 'outcome.txt').read_text() == expected
                and len(paths) == len(wanted) and set(paths) == set(wanted))
            report['checks']['unforced_parent_focus_restored'] = observer.snapshot()['frontmost_pid'] == app.pid
        elif kind == 'reveal':
            observer.wait(lambda: (rendezvous / 'callback-complete.txt').exists())
            def finder_window():
                for pid in observer.call('finder')['pids']:
                    finder = Observer(helper, output, pid)
                    snapshot = finder.snapshot(document=fixture)
                    for node in snapshot['nodes']:
                        url = node.get('document') or node.get('url')
                        if node['role'] == 'AXWindow' and url and pathlib.Path(urllib.parse.unquote(
                                urllib.parse.urlparse(url).path)).resolve() == fixture:
                            return finder, snapshot, node
                return None
            finder, snapshot, window = observer.wait(finder_window)
            owned_finder = finder
            report['observed_finder_window'] = window
            report['checks']['actual_finder_path'] = (rendezvous / 'outcome.txt').read_text() == 'requested'
            finder.capture('revealed', snapshot)
            # Finder has OS lifetime. Only the window opened at our fixture is closed.
            close_finder(finder, fixture)
            owned_finder = None
            observer.call('activate')  # Explicit user return from the external file manager.
            report['checks']['explicit_return_from_finder'] = True
        else:
            observer.wait(lambda: (s := state(rendezvous)) and s[2] == '1' and s[4] == 'true')
            report['checks']['early_settings_delivered'] = True
            observer.call('key', key='escape')
        observer.wait(lambda: (s := state(rendezvous)) and s[2:4] == ['0', 'true'])
        observer.call('key', key='cmd+comma')
        observer.wait(lambda: (s := state(rendezvous)) and s[2] == '1' and s[4] == 'true')
        observer.call('key', key='escape')
        observer.wait(lambda: (s := state(rendezvous)) and s[2:4] == ['0', 'true'])
        report['checks']['native_settings_accelerator'] = True
        observer.call('menu', title='Settings…')
        observer.wait(lambda: (s := state(rendezvous)) and s[2] == '1')
        observer.call('key', key='escape')
        observer.wait(lambda: (s := state(rendezvous)) and s[2:4] == ['0', 'true'])
        report['checks']['native_settings_menu'] = True
        if kind == 'early-settings':
            text = 'Été 日本語'
            observer.call('clipboard-set', text=text)
            observer.call('key', key='cmd+a')
            observer.call('key', key='cmd+v')
            observer.wait(lambda: (s := state(rendezvous)) and s[0] == text)
            observer.call('key', key='cmd+a')
            observer.call('key', key='cmd+x')
            observer.wait(lambda: (s := state(rendezvous)) and s[0] == '')
            if observer.call('clipboard-get')['text'] != text:
                raise ValueError('native clipboard cut differs')
            observer.call('menu', title='Paste')
            observer.wait(lambda: (s := state(rendezvous)) and s[0] == text)
            report['checks']['native_unicode_clipboard_and_edit_menu'] = True
            observer.call('key', key='cmd+a')
        observer.call('text', text='0042')
        observer.wait(lambda: (s := state(rendezvous)) and s[:2] == ['0042', '1'])
        report['checks']['native_search_input'] = True
        (rendezvous / 'finish').touch()
        report['returncode'] = app.wait(timeout=10)
        report['checks']['owned_shutdown'] = (report['returncode'] == 0 and
            'fixture_retired=true' in (rendezvous / 'retired.txt').read_text())
        report['checks']['fixture_removed'] = not fixture.exists()
    except Exception:
        report['errors'].append(traceback.format_exc())
        report['last_app_state'] = state(rendezvous)
        try:
            observer.snapshot('failure')
            observer.capture('failure')
        except Exception as error:
            report['errors'].append('failure capture: ' + str(error))
    finally:
        report['errors'] += retire(app, capture)
        if owned_finder is not None:
            try:
                close_finder(owned_finder, fixture)
            except Exception as error:
                report['errors'].append('owned Finder cleanup: ' + str(error))
        (output / 'case.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    if report['errors'] or not report['checks'] or not all(report['checks'].values()):
        raise RuntimeError(name + ' failed; inspect retained native macOS evidence')
    return report


def inspect(package, archive, helper, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if platform.system() != 'Darwin' or (platform.system(), platform.machine()) != common.wayland.runtime.TARGETS[manifest['target']]:
        raise ValueError('matching native macOS host required')
    binary = common.wayland.runtime.verified_binary(package, manifest)
    common.clean.resources.package_bytes(package, manifest)
    archive_id = common.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    report = {'schema': 1, 'source': manifest['source'], 'target': manifest['target'],
        'binary_sha256': manifest['binary_sha256'], 'archive': archive_id, 'cases': {}, 'errors': [],
        'automated_native_macos': 'fail', 'physical_input_ime_qualified': False,
        'screen_reader_speech_qualified': False, 'clean_host_qualified': False, 'toolkit_selected': False}
    try:
        for name, kind, expected in CASES:
            report['cases'][name] = one_case(binary, helper, output, name, kind, expected)
        if session_passed(report, manifest['binary_sha256']):
            report['automated_native_macos'] = 'pass'
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report['errors'].append(str(error))
    files = [p for p in output.rglob('*') if p.is_file()]
    if len(files) > 1024 or any(p.stat().st_size > LIMIT for p in files):
        report['errors'].append('native evidence size/count limit')
        report['automated_native_macos'] = 'fail'
    report['file_hashes'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files if p.stat().st_size <= LIMIT}
    (output / 'native-macos.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('package', 'archive', 'helper', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    args = parser.parse_args()
    result = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True),
        args.helper.resolve(strict=True), args.output.resolve())
    print(json.dumps({'result': result['automated_native_macos'], 'errors': result['errors']}))
    raise SystemExit(0 if result['automated_native_macos'] == 'pass' else 1)
