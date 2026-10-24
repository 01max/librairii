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
            if kind in ('files', 'folder') else {'actual_finder_path', 'verified_finder_window_closed', 'explicit_return_from_finder'}
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


def canonical_paths(paths):
    # AppKit may return /var while the fixture was created under /private/var.
    # Resolve existing objects before fixture retirement; never compare suffixes
    # or rewrite a textual prefix, which could accept another directory.
    result = []
    for raw in paths:
        path = pathlib.Path(raw)
        if not path.is_absolute():
            raise ValueError('native callback path is not absolute')
        result.append(str(path.resolve(strict=True)))
    return result


def picker_root(snapshot):
    # Public AX window->child-sheet relationship is the parenting witness.
    return next((node for node in snapshot['nodes'] if node['role'] == 'AXSheet'
        and node['selector'].get('sheet') is not None and not node['selector']['path']
        and node.get('modal') is not False), None)


def finder_fixture_window(snapshot, fixture):
    for node in snapshot['nodes']:
        if node['role'] != 'AXWindow':
            continue
        for value in (node.get('document'), node.get('url')):
            if not value:
                continue
            try:
                url = urllib.parse.urlparse(value)
                if url.scheme != 'file' or url.netloc not in ('', 'localhost'):
                    continue
                candidate = pathlib.Path(urllib.parse.unquote(url.path)).resolve(strict=True)
            except (OSError, ValueError, RuntimeError):
                continue
            if candidate == fixture:
                return node
    return None


def verified_finder_snapshot(snapshot, fixture):
    window = finder_fixture_window(snapshot, fixture)
    if window is not None:
        return snapshot, window, {'method': 'root-file-url', 'directory': str(fixture)}
    # Finder15 exposes file URLs on its actual content images but no URL on the
    # window. A title identifies only a candidate; both exact existing fixture
    # children in one native IconView are the independent location witness.
    nodes = snapshot.get('owned_candidate_diagnostic_tree', [])
    windows = [n for n in nodes if n['role'] == 'AXWindow' and not n['selector']['path']
        and n.get('identifier') == 'FinderWindow' and n.get('title') == fixture.name]
    if len(windows) != 1:
        return None
    window = windows[0]
    views = [n for n in nodes if same_root(n, window) and n['role'] == 'AXList'
        and n.get('identifier') == 'IconView' and n.get('subrole') == 'AXCollectionList']
    if len(views) != 1:
        return None
    view_path = views[0]['selector']['path']
    expected = {fixture / name for name in ('Été 日本語 故事.zip', 'Deuxième archive 02.pk')}
    images = [n for n in nodes if same_root(n, window) and n['role'] == 'AXImage'
        and n['selector']['path'][:len(view_path)] == view_path
        and n.get('title') in {p.name for p in expected}]
    if len(images) != 2:
        return None
    observed = []
    for node in images:
        try:
            url = urllib.parse.urlparse(node.get('url', ''))
            if url.scheme != 'file' or url.netloc not in ('', 'localhost'):
                return None
            path = pathlib.Path(urllib.parse.unquote(url.path)).resolve(strict=True)
            if not path.is_file() or path.parent != fixture or path.name != node['title']:
                return None
            observed.append(path)
        except (OSError, ValueError, RuntimeError):
            return None
    if set(observed) != expected:
        return None
    verified = {**snapshot, 'nodes': nodes, 'windows': snapshot.get('owned_candidate_cg_windows', [])}
    return verified, window, {'method': 'exact-native-icon-view-file-urls',
        'directory': str(fixture), 'content_view': views[0], 'observed_files': images}


def finder_window_absent(snapshot):
    return (snapshot.get('document_windows_observed') is True
        and snapshot.get('owned_document_window_present') is False
        and not snapshot.get('nodes') and not snapshot.get('owned_title_candidates'))


def go_folder_fields(snapshot, root):
    sheets = [n for n in snapshot['nodes'] if same_root(n, root) and n['role'] == 'AXSheet'
              and n.get('identifier') == 'GoToWindow']
    return [n for n in snapshot['nodes'] if same_root(n, root) and n['role'] == 'AXTextField'
        and n.get('identifier') == 'PathTextField' and n.get('value_settable')
        and n.get('enabled') is not False and any(
            n['selector']['path'][:len(s['selector']['path'])] == s['selector']['path'] for s in sheets)]


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
            raise RuntimeError(action + ' failed: ' + json.dumps({key: result[key]
                for key in ('returncode', 'errors')}) + '; ' + result['output'][:2000])
        # The helper retains stage diagnostics before its final JSON reply, so
        # a killed AX command still identifies where observation stopped.
        return json.loads(result['output'].splitlines()[-1])

    def snapshot(self, name=None, document=None, summary=False):
        value = self.call('snapshot', summary=summary, **({'document': str(document)} if document else {}))
        if name:
            (self.output / (name + '.json')).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
        return value

    def status(self):
        return self.call('status')

    def wait(self, predicate, timeout=12):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = predicate()
            if value:
                return value
            time.sleep(0.05)
        raise TimeoutError('native macOS observation deadline')

    def button(self, snapshot, root, titles):
        def ready(value):
            matches = [n for n in value['nodes'] if same_root(n, root) and n['role'] == 'AXButton'
                and n.get('title') in titles]
            if len(matches) > 1:
                raise ValueError('native button not unique: ' + repr(titles))
            return matches[0] if matches and matches[0].get('enabled') is True else None
        button = ready(snapshot) or self.wait(lambda: ready(self.snapshot()))
        self.call('press', selector=selector(button))

    def go_folder(self, directory, root):
        self.call('key', key='cmd+shift+g')
        def location():
            snapshot = self.snapshot()
            fields = go_folder_fields(snapshot, root)
            return (snapshot, fields[0]) if len(fields) == 1 else None
        snapshot, field = self.wait(location)
        (self.output / 'go-folder.json').write_text(json.dumps(snapshot, ensure_ascii=False, indent=2) + '\n')
        reply = self.call('set', selector=selector(field), value=str(directory))
        if reply.get('value') != str(directory):
            raise ValueError('native Go to Folder value readback differs')
        self.call('key', key='return')
        self.wait(lambda: not any(same_root(n, root) and n.get('identifier') == 'GoToWindow'
            for n in self.snapshot()['nodes']))

    def rows(self, fixture, root, multiple):
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
        snapshot = snapshot or self.status()
        windows = [w for w in snapshot['windows'] if w.get('kCGWindowAlpha', 0) > 0]
        if not windows:
            raise ValueError('no actual target window for screenshot')
        window = windows[0]  # CoreGraphics supplies front-to-back order for the owned PID.
        result = common.wayland.command(['/usr/sbin/screencapture', '-x', '-l', str(window['kCGWindowNumber']),
            str(self.output / (name + '.png'))], os.environ.copy(), self.output / (name + '-capture.log'), timeout=10)
        if not common.wayland.passed(result) or not (self.output / (name + '.png')).is_file():
            raise RuntimeError('actual native window capture failed')


def close_finder(finder, fixture):
    observed = finder.snapshot(document=fixture)
    result = verified_finder_snapshot(observed, fixture)
    if result is None:
        if finder_window_absent(observed):
            return  # The verified document window has already closed.
        raise ValueError('owned Finder path no longer verified during cleanup')
    snapshot, window, _ = result
    controls = [n for n in snapshot['nodes'] if same_root(n, window) and
        n.get('subrole') == 'AXCloseButton']
    if len(controls) != 1:
        raise ValueError('owned Finder close control not unique')
    finder.call('press', selector=selector(controls[0]))
    def closed():
        value = finder.snapshot(document=fixture)
        return finder_window_absent(value)
    finder.wait(closed)


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
        observer.wait(lambda: observer.status()['frontmost_pid'] == app.pid)
        if kind in ('files', 'folder'):
            snapshot = observer.wait(lambda: (s := observer.snapshot(summary=True)) and picker_root(s) and s)
            root_node = picker_root(snapshot)
            report['parent_sheet'] = root_node
            report['checks']['real_parented_modal_sheet'] = True
            if expected != 'cancelled':
                observer.call('key', key='cmd+2')
                snapshot = observer.wait(lambda: (s := observer.snapshot()) and any(
                    same_root(n, root_node) and n.get('identifier') == 'View Options'
                    and n.get('description') == 'List' for n in s['nodes']) and s)
            else:
                snapshot = observer.snapshot()
            (output / 'picker.json').write_text(json.dumps(snapshot, ensure_ascii=False, indent=2) + '\n')
            observer.capture('picker', snapshot)
            if expected == 'cancelled':
                observer.button(snapshot, root_node, ('Cancel',))
            else:
                destination = fixture
                if expected == 'rejected':
                    # The system temporary parent can contain thousands of
                    # unrelated rows. Use a fresh owned directory outside the
                    # fixture, retaining the same actual rejection contract.
                    destination = output / 'outside-folder'
                    destination.mkdir()
                    if destination.resolve(strict=True).is_relative_to(fixture):
                        raise ValueError('rejection fixture is inside the accepted root')
                    report['outside_folder'] = str(destination)
                observer.go_folder(destination, root_node)
                if kind == 'files':
                    observer.rows(fixture, root_node, name == 'files-multiple')
                snapshot = observer.snapshot('selection')
                observer.capture('selection', snapshot)
                observer.button(snapshot, root_node, ('Open', 'Choose', 'Select'))
            observer.wait(lambda: (rendezvous / 'callback-complete.txt').exists())
            paths = (rendezvous / 'selected-paths.txt').read_text().splitlines()
            wanted = [] if expected == 'cancelled' else [str(destination)] if expected == 'rejected' else (
                [str(fixture / 'Été 日本語 故事.zip')] + ([str(fixture / 'Deuxième archive 02.pk')] if name == 'files-multiple' else [])
                if kind == 'files' else [str(fixture)])
            report['selected_paths'] = paths
            report['callback_outcome'] = (rendezvous / 'outcome.txt').read_text()
            report['canonical_selected_paths'] = canonical_paths(paths)
            report['canonical_expected_paths'] = canonical_paths(wanted)
            report['checks']['exact_native_callback'] = (report['callback_outcome'] == expected
                and len(paths) == len(wanted)
                and set(report['canonical_selected_paths']) == set(report['canonical_expected_paths']))
            report['checks']['unforced_parent_focus_restored'] = observer.status()['frontmost_pid'] == app.pid
        elif kind == 'reveal':
            observer.wait(lambda: (rendezvous / 'callback-complete.txt').exists())
            def finder_window():
                for pid in observer.call('finder')['pids']:
                    finder = Observer(helper, output, pid)
                    snapshot = finder.snapshot(document=fixture)
                    verified = verified_finder_snapshot(snapshot, fixture)
                    if verified is not None:
                        return finder, *verified
                return None
            finder, snapshot, window, location = observer.wait(finder_window)
            owned_finder = finder
            report['observed_finder_window'] = window
            report['observed_finder_location'] = location
            report['checks']['actual_finder_path'] = ((rendezvous / 'outcome.txt').read_text() == 'requested'
                and location['directory'] == str(fixture))
            finder.capture('revealed', snapshot)
            # Finder has OS lifetime. Only the window opened at our fixture is closed.
            close_finder(finder, fixture)
            report['checks']['verified_finder_window_closed'] = True
            owned_finder = None
            observer.call('activate')  # Explicit user return from the external file manager.
            observer.wait(lambda: observer.status()['frontmost_pid'] == app.pid)
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
        report['keyboard_input_source'] = observer.status()['keyboard_input_source']
        observer.call('digits', text='0042')
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
