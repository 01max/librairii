"""Observe actual GTK portal dialogs and Thunar windows in an owned Linux session."""
import argparse
import collections
import hashlib
import json
import os
import pathlib
import re
import threading
import subprocess
import time

LIMIT = 4 * 1024 * 1024


def parent_proof(session, monitor, app_log, gtk_log, app_pid, parent_pid=None, transient=None):
    calls = re.findall(r'interface=org\.freedesktop\.portal\.FileChooser; member=OpenFile\n\s+string "([^"]*)"', monitor)
    result = {'one_real_portal_call': len(calls) == 1 and monitor.count(
        'interface=org.freedesktop.portal.FileChooser; member=OpenFile') == 1,
        'parent_identified': False, 'parent_associated': False}
    if len(calls) != 1:
        return result
    parent = calls[0]
    if session == 'x11':
        if re.fullmatch(r'x11:[0-9a-fA-F]+', parent):
            xid = int(parent[4:], 16)
            result['parent_identified'] = parent_pid == app_pid
            result['parent_associated'] = transient == xid
    elif parent.startswith('wayland:') and len(parent) > 8:
        token = re.escape(parent[8:])
        result['parent_identified'] = bool(re.search(r'zxdg_exported_v2@\d+\.handle\("' + token + r'"\)', app_log))
        imported = re.search(r'-> zxdg_importer_v([12])@\d+\.(?:import_toplevel|import)\(new id zxdg_imported_v[12]@(\d+), "' + token + r'"\)', gtk_log)
        if imported:
            object_name = 'zxdg_imported_v' + imported[1] + '@' + imported[2]
            following = gtk_log[imported.end():]
            association = re.search(r'-> ' + object_name + r'\.set_parent_of\(wl_surface@(\d+)\)', following)
            invalidated = re.search(object_name + r'\.(?:destroyed|destroy)\(', following)
            result['parent_associated'] = bool(association and
                (not invalidated or invalidated.start() > association.start()))
    result['no_parent_rejection'] = 'Failed to associate portal window with parent' not in gtk_log
    return result


def wait_for(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            result = predicate()
            if result:
                return result
        except (FileNotFoundError, ProcessLookupError):
            pass
        time.sleep(0.05)
    raise TimeoutError('native observation deadline')


class Desktop:
    def __init__(self, session, evidence):
        import dbus
        import pyatspi
        from gi.repository import Atspi
        Atspi.set_timeout(1000, 1000)
        self.a, self.dbus, self.session, self.root = pyatspi, dbus, session, evidence
        self.children, self.logs, self.overflow = [], [], []
        self.ipc = None
        self.runtime = pathlib.Path('/tmp/native-runtime')
        self.runtime.mkdir(mode=0o700)
        for name in ('config', 'cache', 'data', 'outside'):
            (self.runtime / name).mkdir()
        os.environ.update({'LC_ALL': 'C.UTF-8', 'XDG_RUNTIME_DIR': str(self.runtime),
            'XDG_CONFIG_HOME': str(self.runtime / 'config'), 'XDG_CACHE_HOME': str(self.runtime / 'cache'),
            'XDG_DATA_HOME': str(self.runtime / 'data'), 'XDG_CURRENT_DESKTOP': 'qualification',
            'XDG_SESSION_TYPE': session, 'GTK_MODULES': 'atk-bridge', 'SLINT_BACKEND': 'winit-skia-software'})
        os.environ.pop('WAYLAND_SOCKET', None)
        if session == 'x11':
            os.environ.pop('WAYLAND_DISPLAY', None)
            os.environ.update({'DISPLAY': ':91', 'GDK_BACKEND': 'x11'})
            self.start('xvfb', ['Xvfb', ':91', '-screen', '0', '1280x900x24', '-nolisten', 'tcp'])
            wait_for(lambda: pathlib.Path('/tmp/.X11-unix/X91').is_socket())
            self.start('openbox', ['openbox'])
        else:
            os.environ.pop('DISPLAY', None)
            os.environ.update({'GDK_BACKEND': 'wayland', 'WLR_BACKENDS': 'headless', 'WLR_RENDERER': 'pixman',
                               'WLR_LIBINPUT_NO_DEVICES': '1'})
            config = self.runtime / 'sway.conf'
            config.write_text('output * mode 1280x900\nxwayland disable\nseat seat0 fallback true\n'
                              'default_border pixel 1\nfor_window [app_id=".*"] floating enable\n')
            self.start('sway', ['sway', '--config', str(config)])
            socket = wait_for(lambda: next((p for p in self.runtime.glob('wayland-*') if p.is_socket()), None))
            os.environ['WAYLAND_DISPLAY'] = socket.name
            self.ipc = str(wait_for(lambda: next(self.runtime.glob('sway-ipc.*.sock'), None)))
        config = self.runtime / 'config/xdg-desktop-portal'
        config.mkdir()
        (config / 'portals.conf').write_text('[preferred]\ndefault=gtk\norg.freedesktop.impl.portal.FileChooser=gtk\n')
        self.bus = dbus.SessionBus()
        self.a.Registry.getDesktop(0)  # Activate the private accessibility bus/registry.
        self.start('gtk', ['/usr/libexec/xdg-desktop-portal-gtk'], debug=True)
        self.start('portal', ['/usr/libexec/xdg-desktop-portal'])
        wait_for(lambda: self.bus.name_has_owner('org.freedesktop.portal.Desktop'))
        wait_for(lambda: self.bus.name_has_owner('org.freedesktop.impl.portal.desktop.gtk'))
        self.start('monitor', ['stdbuf', '-oL', 'dbus-monitor', '--session',
            "interface='org.freedesktop.portal.FileChooser'", "interface='org.freedesktop.portal.Request'"])
        self.run(['xdg-mime', 'default', 'thunar.desktop', 'inode/directory'])

    def start(self, name, argv, debug=False):
        path = self.root / (name + '.log')
        env = {**os.environ, **({'WAYLAND_DEBUG': 'client'} if debug else {})}
        child = subprocess.Popen(argv, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        def capture():
            size = 0
            try:
                with path.open('xb') as stream, child.stdout:
                    while chunk := child.stdout.read1(65536):
                        remaining = LIMIT - size
                        stream.write(chunk[:remaining])
                        stream.flush()
                        size += len(chunk)
                        if size >= LIMIT:
                            self.overflow.append(name)
                            child.kill()
                            break
            except Exception as error:
                self.overflow.append(name + ': capture ' + type(error).__name__ + ': ' + str(error))
                child.kill()
                child.stdout.close()
        reader = threading.Thread(target=capture, daemon=True)
        reader.start()
        self.logs.append(reader)
        self.children.append(child)
        return child

    def run(self, argv):
        return subprocess.check_output(argv, text=True, timeout=10, stderr=subprocess.STDOUT)

    def key(self, key):
        if self.session == 'x11':
            self.run(['xdotool', 'key', '--clearmodifiers', key])
        elif key.startswith('ctrl+'):
            self.run(['wtype', '-M', 'ctrl', '-k', key[5:], '-m', 'ctrl'])
        else:
            self.run(['wtype', '-k', key])

    def text(self, text):
        if self.session == 'x11':
            self.run(['xdotool', 'type', '--clearmodifiers', '--delay', '10', text])
        else:
            self.run(['wtype', '-d', '10', text])

    def nodes(self, root=None):
        queue = collections.deque([(root or self.a.Registry.getDesktop(0), 0)])
        count = 0
        while queue and count < 4000:
            node, depth = queue.popleft()
            count += 1
            try:
                yield node
                if depth < 30:
                    queue.extend((node.getChildAtIndex(i), depth + 1) for i in range(min(node.childCount, 1000)))
            except Exception:
                # Native windows can disappear between two accessibility calls.
                continue

    def find(self, predicate, root=None):
        for node in self.nodes(root):
            try:
                if predicate(node):
                    return node
            except Exception:
                continue
        return None

    def visible(self, node):
        return node.getState().contains(self.a.STATE_SHOWING)

    def dialog(self, title):
        return self.find(lambda n: n.name == title and self.visible(n))

    def action(self, node):
        action = node.queryAction()
        assert action.nActions > 0 and action.doAction(0), 'native control action failed'

    def button(self, dialog, labels):
        node = wait_for(lambda: self.find(lambda n: n.getRole() == self.a.ROLE_PUSH_BUTTON and
            n.name in labels and self.visible(n) and n.getState().contains(self.a.STATE_ENABLED), dialog))
        self.action(node)

    def screenshot(self, path):
        self.run(['scrot', str(path)] if self.session == 'x11' else ['grim', str(path)])

    def active_pid(self):
        if self.session == 'x11':
            return int(self.run(['xdotool', 'getactivewindow', 'getwindowpid']).strip())
        tree = json.loads(self.run(['swaymsg', '-s', self.ipc, '-t', 'get_tree']))
        def focused(node):
            if node.get('focused') and node.get('pid'):
                return node['pid']
            return next((value for child in node.get('nodes', []) + node.get('floating_nodes', [])
                         if (value := focused(child))), None)
        return focused(tree)

    def close(self):
        for child in reversed(self.children):
            if child.poll() is None:
                child.terminate()
        for child in reversed(self.children):
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=3)
        for reader in self.logs:
            reader.join(timeout=3)
        if any(reader.is_alive() for reader in self.logs):
            self.overflow.append("unretired-log-reader")


def state(path):
    try:
        rows = (path / 'state.txt').read_text().splitlines()
        return rows if len(rows) == 5 else None
    except FileNotFoundError:
        return None


def one_case(desktop, name, kind, expected):
    output = desktop.root / name
    app_dir = output / 'app'
    output.mkdir()
    before_monitor = (desktop.root / 'monitor.log').stat().st_size
    before_gtk = (desktop.root / 'gtk.log').stat().st_size
    flags = ['--native-probe=' + str(app_dir), '--native-case=' + kind]
    if kind == 'early-settings':
        flags += ['--settings-early']
    app = desktop.start(name, ['/opt/librairii/qualification-desktop', *flags], debug=True)
    report = {'checks': {}, 'expected': expected}
    try:
        fixture = pathlib.Path(wait_for(lambda: (app_dir / 'fixture.txt').read_text()))
        report['fixture'] = str(fixture)
        if kind in ('files', 'folder'):
            title = 'Choose synthetic archives' if kind == 'files' else 'Choose the synthetic folder'
            dialog = wait_for(lambda: desktop.dialog(title))
            report['checks']['real_modal_dialog'] = dialog.getState().contains(desktop.a.STATE_MODAL)
            parent_pid, transient = None, None
            if desktop.session == 'x11':
                xid = desktop.run(['xdotool', 'search', '--onlyvisible', '--name', '^' + title + '$']).splitlines()[-1]
                props = desktop.run(['xprop', '-id', xid, 'WM_TRANSIENT_FOR'])
                match = re.search(r'window id # (0x[0-9a-fA-F]+)', props)
                if match:
                    transient = int(match[1], 16)
                    props = desktop.run(['xprop', '-id', hex(transient), '_NET_WM_PID'])
                    pid = re.search(r'= (\d+)', props)
                    parent_pid = int(pid[1]) if pid else None
            desktop.screenshot(output / 'dialog.png')
            if expected == 'cancelled':
                desktop.button(dialog, ('Cancel',))
            elif name == 'files-multiple':
                wanted = {'Été 日本語 故事.zip', 'Deuxième archive 02.pk'}
                table = wait_for(lambda: desktop.find(lambda n: n.getRole() in (desktop.a.ROLE_TABLE, desktop.a.ROLE_TREE_TABLE)
                    and wanted <= {str(x.name) for x in desktop.nodes(n)}, dialog))
                selection = table.querySelection()
                assert selection.selectAll(), 'native multi-selection failed'
                desktop.screenshot(output / 'selected.png')
                desktop.button(dialog, ('Open', 'Select'))
            else:
                target = fixture / 'Été 日本語 故事.zip' if kind == 'files' else (
                    desktop.runtime / 'outside' if expected == 'rejected' else fixture)
                desktop.key('ctrl+l')
                entry = wait_for(lambda: desktop.find(lambda n: desktop.visible(n) and
                    n.getState().contains(desktop.a.STATE_EDITABLE), dialog))
                assert entry.queryEditableText().setTextContents(str(target)), 'native location entry failed'
                desktop.key('Return')
                time.sleep(0.3)
                if not (app_dir / 'callback-complete.txt').exists():
                    desktop.button(dialog, ('Open', 'Select'))
            wait_for(lambda: (app_dir / 'callback-complete.txt').exists())
            monitor = (desktop.root / 'monitor.log').read_bytes()[before_monitor:].decode('utf-8', errors='replace')
            gtk_log = (desktop.root / 'gtk.log').read_bytes()[before_gtk:].decode('utf-8', errors='replace')
            report['checks'].update(parent_proof(desktop.session, monitor, (desktop.root / (name + '.log')).read_text(),
                gtk_log, app.pid, parent_pid, transient))
            report['portal_monitor'] = monitor
            paths = (app_dir / 'selected-paths.txt').read_text().splitlines()
            wanted_paths = ([str(fixture / f) for f in ('Été 日本語 故事.zip', 'Deuxième archive 02.pk')]
                if name == 'files-multiple' else [str(fixture / 'Été 日本語 故事.zip')] if kind == 'files'
                else [str(desktop.runtime / 'outside')] if expected == 'rejected' else [str(fixture)])
            report['selected_paths'] = paths
            report['checks']['actual_picker_result'] = ((app_dir / 'outcome.txt').read_text() == expected
                and len(paths) == (0 if expected == 'cancelled' else len(wanted_paths))
                and set(paths) == (set() if expected == 'cancelled' else set(wanted_paths)))
        elif kind == 'reveal':
            wait_for(lambda: (app_dir / 'callback-complete.txt').exists())
            window = wait_for(lambda: desktop.find(lambda n: n.getRole() == desktop.a.ROLE_FRAME and desktop.visible(n)
                and str(n.name).startswith(fixture.name)))
            desktop.key('ctrl+l')
            entry = wait_for(lambda: desktop.find(lambda n: desktop.visible(n) and
                n.getState().contains(desktop.a.STATE_EDITABLE) and
                n.queryText().getText(0, -1).rstrip('/') == str(fixture), window))
            report['observed_file_manager_path'] = entry.queryText().getText(0, -1)
            report['checks']['actual_file_manager_window_path'] = bool(entry) and (app_dir / 'outcome.txt').read_text() == 'requested'
            desktop.screenshot(output / 'revealed.png')
            desktop.key('alt+F4' if desktop.session == 'x11' else 'Escape')
            if desktop.session == 'wayland':
                desktop.run(['thunar', '--quit'])
        else:
            snapshot = wait_for(lambda: (value := state(app_dir)) and value[2] == '1' and value)
            report['checks']['early_settings_delivered'] = snapshot[4] == 'true'
            desktop.key('Escape')
        wait_for(lambda: desktop.active_pid() == app.pid)
        if kind != 'early-settings':
            desktop.key('ctrl+comma')
            snapshot = wait_for(lambda: (value := state(app_dir)) and value[2] == '1' and value)
            report['checks']['os_settings_accelerator'] = snapshot[4] == 'true'
            desktop.key('Escape')
        wait_for(lambda: (value := state(app_dir)) and value[2:4] == ['0', 'true'])
        desktop.text('0042')
        wait_for(lambda: (value := state(app_dir)) and value[:2] == ['0042', '1'])
        report['checks']['native_focus_restored_and_search_input'] = True
        (app_dir / 'finish').touch()
        report['returncode'] = app.wait(timeout=10)
        report['checks']['owned_shutdown'] = report['returncode'] == 0 and 'fixture_retired=true' in (app_dir / 'retired.txt').read_text()
        report['checks']['fixture_removed'] = not fixture.exists()
    finally:
        if app.poll() is None:
            app.kill()
            app.wait(timeout=5)
        (output / 'case.json').write_text(json.dumps(report, indent=2) + '\n')
    if not report['checks'] or not all(report['checks'].values()):
        raise RuntimeError(name + ' failed native checks')
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--session', choices=('x11', 'wayland'), required=True)
    args = parser.parse_args()
    root = pathlib.Path('/evidence')
    assert os.uname().machine == 'x86_64' and os.geteuid() != 0
    binary_sha = hashlib.sha256(pathlib.Path('/opt/librairii/qualification-desktop').read_bytes()).hexdigest()
    assert binary_sha == os.environ['EXPECTED_BINARY_SHA256']
    desktop = None
    report = {'session': args.session, 'binary_sha256': binary_sha, 'cases': {}, 'errors': [], 'native_integration': 'fail'}
    try:
        desktop = Desktop(args.session, root)
        (root / 'versions.txt').write_text(desktop.run(['dpkg-query', '-W', '-f=${Package} ${Version}\n']))
        for name, kind, expected in [('files-multiple', 'files', 'selected'), ('files-single', 'files', 'selected'),
                ('files-cancel', 'files', 'cancelled'), ('folder-accept', 'folder', 'selected'),
                ('folder-cancel', 'folder', 'cancelled'), ('folder-reject', 'folder', 'rejected'),
                ('early-settings', 'early-settings', 'delivered'), ('reveal', 'reveal', 'requested')]:
            report['cases'][name] = one_case(desktop, name, kind, expected)
        report['native_integration'] = 'pass'
    except Exception as error:
        report['errors'].append(type(error).__name__ + ': ' + str(error))
    finally:
        if desktop:
            desktop.close()
            if desktop.overflow:
                report['errors'].append('log output limit: ' + repr(desktop.overflow))
                report['native_integration'] = 'fail'
        (root / 'native-session.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'session': args.session, 'result': report['native_integration'], 'errors': report['errors']}))
    return 0 if report['native_integration'] == 'pass' else 1


if __name__ == '__main__':
    raise SystemExit(main())
