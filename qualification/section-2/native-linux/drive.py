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
import traceback

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
        from gi.repository import Atspi, GLib
        Atspi.set_timeout(1000, 1000)
        self.context = GLib.MainContext.default()
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
            # A published X socket can precede successful client handshakes.
            # Do not lose the one WM process by launching into that interval.
            connection = wait_for(self.x11_server_ready)
            manager = self.start('openbox', ['openbox'])
            registered = wait_for(lambda: self.x11_manager_ready(manager))
            (self.root / 'x11-readiness.json').write_text(json.dumps({
                'server_root_read': connection, 'manager_pid': manager.pid, **registered}, indent=2) + '\n')
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
            # Keep a keyboard device on the headless seat between injected events.
            self.start('keyboard-seat', ['wtype', '-s', '600000'])
            wait_for(lambda: 'keyboard' in self.run(['swaymsg', '-s', self.ipc, '-t', 'get_inputs']))
        config = self.runtime / 'config/xdg-desktop-portal'
        config.mkdir()
        (config / 'portals.conf').write_text('[preferred]\ndefault=gtk\norg.freedesktop.impl.portal.FileChooser=gtk\n')
        self.bus = dbus.SessionBus()
        # Only the private bus receives this owned desktop's activation context.
        activation = {key: os.environ[key] for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'GDK_BACKEND',
            'XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME',
            'XDG_CURRENT_DESKTOP', 'XDG_SESSION_TYPE', 'GTK_MODULES', 'LC_ALL') if key in os.environ}
        self.bus.get_object('org.freedesktop.DBus', '/org/freedesktop/DBus').UpdateActivationEnvironment(
            activation, dbus_interface='org.freedesktop.DBus')
        self.a.Registry.getDesktop(0)  # Activate the private accessibility bus/registry.
        self.start('gtk', ['/usr/libexec/xdg-desktop-portal-gtk'], debug=True)
        wait_for(lambda: self.bus.name_has_owner('org.freedesktop.impl.portal.desktop.gtk'))
        self.start('portal', ['/usr/libexec/xdg-desktop-portal'])
        wait_for(lambda: self.bus.name_has_owner('org.freedesktop.portal.Desktop'))
        wait_for(lambda: self.bus.name_has_owner('org.freedesktop.impl.portal.desktop.gtk'))
        self.start('monitor', ['stdbuf', '-oL', 'dbus-monitor', '--session',
            "interface='org.freedesktop.portal.FileChooser'", "interface='org.freedesktop.portal.Request'"])
        self.run(['xdg-mime', 'default', 'thunar.desktop', 'inode/directory'])
        self.run(['xfconf-query', '-c', 'thunar', '-p', '/last-location-bar', '--create',
                  '-t', 'string', '-s', 'ThunarLocationEntry'])
        preference = self.run(['xfconf-query', '-c', 'thunar', '-p', '/last-location-bar'])
        assert preference.strip() == 'ThunarLocationEntry'
        (self.root / 'file-manager-preference.txt').write_text(preference)

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

    def x11_server_ready(self):
        try:
            return self.run(['xprop', '-root', '_NET_SUPPORTING_WM_CHECK'])
        except subprocess.CalledProcessError as error:
            if 'unable to open display' in str(error.output).lower():
                return None
            raise

    def x11_manager_ready(self, manager):
        if manager.poll() is not None:
            raise RuntimeError('owned X11 window manager exited before registration')
        root = self.run(['xprop', '-root', '_NET_SUPPORTING_WM_CHECK'])
        match = re.search(r'window id # (0x[0-9a-fA-F]+)', root)
        if match is None or int(match[1], 16) == 0:
            return None
        window = self.run(['xprop', '-id', match[1], '_NET_SUPPORTING_WM_CHECK', '_NET_WM_NAME'])
        child = re.search(r'window id # (0x[0-9a-fA-F]+)', window)
        if child and int(child[1], 16) == int(match[1], 16) and '= "Openbox"' in window:
            return {'manager_root_check': root, 'manager_window_check': window}
        return None

    def key(self, key):
        if self.session == 'x11':
            self.run(['xdotool', 'key', '--clearmodifiers', key])
        elif key.startswith('ctrl+'):
            self.run(['wtype', '-s', '200', '-M', 'ctrl', '-P', 'Control_L', '-s', '50',
                      '-k', key[5:], '-p', 'Control_L', '-m', 'ctrl', '-s', '100'])
        elif key.startswith('shift+'):
            self.run(['wtype', '-s', '200', '-M', 'shift', '-P', 'Shift_L', '-s', '50',
                      '-k', key[6:], '-p', 'Shift_L', '-m', 'shift', '-s', '100'])
        else:
            self.run(['wtype', '-s', '200', '-k', key, '-s', '100'])

    def text(self, text):
        if self.session == 'x11':
            self.run(['xdotool', 'type', '--clearmodifiers', '--delay', '10', text])
        else:
            self.run(['wtype', '-s', '200', '-d', '30', text, '-s', '100'])

    def french_dead_keys(self):
        # Send one native command sequence across ^ then e; XTest may temporarily
        # map missing keysyms in this private X11 server, then restore the map.
        # No composed Unicode string or field-value setter supplies the result.
        keys = ['dead_circumflex', 'e', 'eacute', 'space', 'a', 'z']
        if self.session == 'x11':
            self.run(['xdotool', 'key', '--clearmodifiers', '--delay', '30', *keys])
        else:
            self.run(['wtype', '-s', '200', '-d', '30',
                *[argument for key in keys for argument in ('-k', key)], '-s', '100'])

    def nodes(self, root=None):
        context = getattr(self, 'context', None)
        if context is not None:
            for _ in range(64):
                if not context.pending():
                    break
                context.iteration(False)
        queue = collections.deque([(root if root is not None else self.a.Registry.getDesktop(0), 0)])
        count = 0
        while queue and count < 4000:
            node, depth = queue.popleft()
            count += 1
            try:
                if hasattr(node, 'clear_cache'):
                    node.clear_cache()
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
            # Focus handoff can temporarily have no active window. Absence is
            # not a PID, and must not abort the bounded restoration wait.
            prop = self.run(['xprop', '-root', '_NET_ACTIVE_WINDOW'])
            match = re.search(r'window id # (0x[0-9a-fA-F]+)', prop)
            if match is None:
                if 'not found' in prop or 'no such atom' in prop:
                    return None
                raise ValueError('unexpected X11 active-window property: ' + prop)
            if int(match[1], 16) == 0:
                return None
            try:
                prop = self.run(['xprop', '-id', match[1], '_NET_WM_PID'])
            except subprocess.CalledProcessError as error:
                if 'BadWindow' in str(error.output):
                    return None  # The previous focused window was destroyed.
                raise
            match = re.search(r'_NET_WM_PID\(CARDINAL\) = (\d+)', prop)
            if match:
                return int(match[1])
            if 'not found' in prop or 'no such atom' in prop:
                return None
            raise ValueError('unexpected X11 active-window PID property: ' + prop)
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


def observe_utf8_input(desktop, app_dir, output, report):
    def query(expected):
        return wait_for(lambda: (value := state(app_dir)) and value[0] == expected and value)

    desktop.key('ctrl+a')
    desktop.key('BackSpace')
    query('')
    desktop.french_dead_keys()
    report['utf8_key_input_state'] = query('êé az')
    report['checks']['native_french_dead_key_and_utf8_input'] = True
    desktop.screenshot(output / 'french-utf8-input.png')
    desktop.key('ctrl+a')
    desktop.key('ctrl+c')
    # Cut would overwrite the clipboard and could hide a broken Copy command.
    # Prove Copy independently, using a physical deletion before the first Paste.
    desktop.key('BackSpace')
    report['utf8_copy_cleared_state'] = query('')
    desktop.key('ctrl+v')
    report['utf8_copy_pasted_state'] = query('êé az')
    desktop.screenshot(output / 'french-utf8-copy-pasted.png')
    desktop.key('ctrl+a')
    desktop.key('ctrl+x')
    report['utf8_cut_state'] = query('')
    desktop.key('ctrl+v')
    report['utf8_pasted_state'] = query('êé az')
    report['checks']['native_utf8_clipboard_roundtrip'] = True
    desktop.screenshot(output / 'french-utf8-pasted.png')


def observe_editing_and_performance(desktop, app_dir, output, report):
    def query(expected):
        return wait_for(lambda: (value := state(app_dir)) and value[0] == expected and value)
    def geometry():
        return json.loads((app_dir / 'geometry.json').read_text())
    def fast_key(key):
        assert desktop.active_pid() == report['app_pid'], 'native input lost foreground target'
        desktop.run(['xdotool', 'key', '--clearmodifiers', '--delay', '0', key]
            if desktop.session == 'x11' else ['wtype', '-M', 'shift', '-P', 'Shift_L',
                '-k', key[6:], '-p', 'Shift_L', '-m', 'shift'] if key.startswith('shift+')
                else ['wtype', '-k', key])
    def paste(text, name):
        if desktop.session == 'x11':
            path = output / (name + '.txt')
            path.write_text(text)
            desktop.start(name, ['xclip', '-selection', 'clipboard', '-i', str(path), '-loops', '8', '-quiet'])
        else:
            desktop.start(name, ['wl-copy', '--foreground', '--', text])
        # Observe the actual native clipboard owner before issuing one Paste.
        expected = text
        wait_for(lambda: desktop.run(['xclip', '-selection', 'clipboard', '-o']
            if desktop.session == 'x11' else ['wl-paste', '--no-newline']) == expected)
        desktop.key('ctrl+v')
        return query(text)
    desktop.key('ctrl+a')
    desktop.key('BackSpace')
    query('')
    text = 'Été e\u0301 Z'
    report['editing_states'] = [paste(text, 'latin-clipboard')]
    desktop.key('Left')
    desktop.key('BackSpace')
    report['editing_states'].append(query('Été e\u0301Z'))
    desktop.key('ctrl+z')
    report['editing_states'].append(query(text))
    desktop.key('ctrl+a')
    desktop.key('BackSpace')
    query('')
    report['checks']['native_composed_decomposed_editing'] = True
    long_text = 'Été, façade et bibliothèque : l’histoire très longue. ' * 16
    report['long_french_states'] = [paste(long_text, 'long-clipboard')]
    desktop.key('ctrl+z')
    report['long_french_states'].append(query(''))
    # Ctrl+Shift+Z is posted through native modifier/key events on both routes.
    if desktop.session == 'x11':
        desktop.key('ctrl+shift+z')
    else:
        desktop.run(['wtype', '-M', 'ctrl', '-P', 'Control_L', '-M', 'shift', '-P', 'Shift_L',
            '-k', 'z', '-p', 'Shift_L', '-m', 'shift', '-p', 'Control_L', '-m', 'ctrl'])
    report['long_french_states'].append(query(long_text))
    desktop.screenshot(output / 'long-french-input.png')
    desktop.key('ctrl+a')
    desktop.key('BackSpace')
    query('')
    report['checks']['native_long_french_clipboard_undo_redo'] = True
    (app_dir / 'start-load').touch()
    wait_for(lambda: geometry()['working'])
    samples = []
    for index in range(64):
        expected = 'x' if index % 2 == 0 else ''
        start = time.monotonic()
        fast_key('x' if expected else 'BackSpace')
        while True:
            value = state(app_dir)
            elapsed = (time.monotonic() - start) * 1000
            assert elapsed < 1000, 'native input acknowledgment deadline'
            if value and value[0] == expected:
                samples.append(elapsed)
                break
            time.sleep(0.001)
    report['native_input_performance'] = {'samples_ms': samples, 'p95_ms': sorted(samples)[60],
        'method': 'pre-native-post to actual edited-query/model/20ms-rendezvous acknowledgment'}
    assert geometry()['working'], 'background load ended before input measurement'
    assert sorted(samples)[60] <= 100
    report['checks']['native_input_under_load_budget'] = True
    def wait_fast(predicate):
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            if value := predicate():
                return value
            time.sleep(0.001)
        raise TimeoutError('native performance state acknowledgment deadline')
    def focus(target, reverse=False):
        for _ in range(24):
            previous = geometry()['focus_key']
            if previous == target:
                return
            fast_key('shift+Tab' if reverse else 'Tab')
            wait_fast(lambda: geometry()['focus_key'] != previous)
        raise AssertionError('bounded native keyboard focus not reached')
    samples, endpoints = [], []
    for index in range(21):
        assert geometry()['focus_key'] == 30
        fast_key('x')
        wait_fast(lambda: geometry()['rows'] == 0)
        start = time.monotonic()
        fast_key('BackSpace')
        wait_fast(lambda: geometry()['rows'] == 1000)
        focus(31)
        fast_key('End')
        selected = wait_fast(lambda: (g := geometry()) and g['rows'] == 1000
            and g['selected_id'] == 999 and g['selected_row'] == 999 and g)
        elapsed = (time.monotonic() - start) * 1000
        if index > 0:
            samples.append(elapsed)
            endpoints.append(selected)
        fast_key('Home')
        wait_fast(lambda: geometry()['selected_id'] == 0)
        focus(30, reverse=True)
    report['thousand_navigation'] = {'samples_ms': samples, 'p95_ms': sorted(samples)[18],
        'endpoints': endpoints, 'method': 'native Delete expands0→1000, Tab/End selects999; one warmup plus20 samples'}
    assert sorted(samples)[18] <= 3000
    assert all(g['working'] for g in endpoints), 'background work ended before navigation samples'
    report['checks']['native_thousand_navigation_budget'] = True


def one_case(desktop, name, kind, expected):
    output = desktop.root / name
    app_dir = output / 'app'
    output.mkdir()
    before_monitor = (desktop.root / 'monitor.log').stat().st_size
    before_gtk = (desktop.root / 'gtk.log').stat().st_size
    flags = ['--native-probe=' + str(app_dir), '--native-case=' + kind]
    if kind == 'early-settings':
        flags += ['--settings-early', '--native-load', '--slow-synthetic-work']
    app = desktop.start(name, ['/opt/librairii/qualification-desktop', *flags], debug=True)
    report = {'checks': {}, 'expected': expected, 'app_pid': app.pid}
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
                report['x11_parent'] = {'dialog_xid': xid, 'transient_property': props}
                match = re.search(r'window id # (0x[0-9a-fA-F]+)', props)
                if match:
                    transient = int(match[1], 16)
                    props = desktop.run(['xprop', '-id', hex(transient), '_NET_WM_PID'])
                    pid = re.search(r'= (\d+)', props)
                    parent_pid = int(pid[1]) if pid else None
                    report['x11_parent'].update({'transient': transient, 'parent_pid': parent_pid,
                        'pid_property': props})
            desktop.screenshot(output / 'dialog.png')
            if expected == 'cancelled':
                desktop.button(dialog, ('Cancel',))
            elif kind == 'files':
                wanted = {'Été 日本語 故事.zip', 'Deuxième archive 02.pk'}
                def archive_table(node):
                    if node.getRole() not in (desktop.a.ROLE_TABLE, desktop.a.ROLE_TREE_TABLE):
                        return False
                    table = node.queryTable()
                    if table.nRows != 2 or not 0 < table.nColumns <= 16:
                        return False
                    # GTK can put an unnamed icon/container before the text cell.
                    rows = [{str(child.name) for column in range(table.nColumns)
                        for child in desktop.nodes(table.getAccessibleAt(row, column))}
                        for row in range(2)]
                    names = [sorted(values & wanted) for values in rows]
                    report['selection_table'] = {'name': str(node.name), 'rows': table.nRows,
                        'columns': table.nColumns, 'row_archive_names': names,
                        'observed_names': [sorted(values)[:32] for values in rows]}
                    return all(len(values) == 1 for values in names) and set(sum(names, [])) == wanted
                node = wait_for(lambda: desktop.find(archive_table, dialog))
                table = node.queryTable()
                chosen = {0, 1} if name == 'files-multiple' else {
                    row for row, names in enumerate(report['selection_table']['row_archive_names'])
                    if names == ['Été 日本語 故事.zip']}
                assert len(chosen) == (2 if name == 'files-multiple' else 1)
                for row in range(2):
                    if row in chosen and not table.isRowSelected(row):
                        assert table.addRowSelection(row), 'native file row selection failed'
                    elif row not in chosen and table.isRowSelected(row):
                        assert table.removeRowSelection(row), 'native file row deselection failed'
                report['selected_rows'] = wait_for(lambda: (rows := table.getSelectedRows()) and
                    set(rows) == chosen and list(rows))
                desktop.screenshot(output / 'selected.png')
                desktop.button(dialog, ('Open', 'Select'))
            else:
                if expected == 'rejected':
                    breadcrumb = wait_for(lambda: desktop.find(lambda n: n.name == 'tmp' and
                        n.getRole() in (desktop.a.ROLE_PUSH_BUTTON, desktop.a.ROLE_TOGGLE_BUTTON) and
                        desktop.visible(n), dialog))
                    desktop.action(breadcrumb)
                    files = wait_for(lambda: desktop.find(lambda n: n.name == 'Files' and
                        n.getRole() in (desktop.a.ROLE_TABLE, desktop.a.ROLE_TREE_TABLE) and
                        desktop.visible(n) and 'native-runtime' in
                        {str(child.name) for child in desktop.nodes(n)}, dialog))
                    assert files.querySelection().clearSelection(), 'native folder selection reset failed'
                    wait_for(lambda: not files.queryTable().getSelectedRows())
                    desktop.screenshot(output / 'outside-folder.png')
                desktop.button(dialog, ('Select', 'Open'))
            wait_for(lambda: (app_dir / 'callback-complete.txt').exists())
            monitor = (desktop.root / 'monitor.log').read_bytes()[before_monitor:].decode('utf-8', errors='replace')
            gtk_log = (desktop.root / 'gtk.log').read_bytes()[before_gtk:].decode('utf-8', errors='replace')
            report['checks'].update(parent_proof(desktop.session, monitor, (desktop.root / (name + '.log')).read_text(),
                gtk_log, app.pid, parent_pid, transient))
            report['portal_monitor'] = monitor
            paths = (app_dir / 'selected-paths.txt').read_text().splitlines()
            wanted_paths = ([str(fixture / f) for f in ('Été 日本語 故事.zip', 'Deuxième archive 02.pk')]
                if name == 'files-multiple' else [str(fixture / 'Été 日本語 故事.zip')] if kind == 'files'
                else ['/tmp'] if expected == 'rejected' else [str(fixture)])
            report['selected_paths'] = paths
            report['checks']['actual_picker_result'] = ((app_dir / 'outcome.txt').read_text() == expected
                and len(paths) == (0 if expected == 'cancelled' else len(wanted_paths))
                and set(paths) == (set() if expected == 'cancelled' else set(wanted_paths)))
        elif kind == 'reveal':
            wait_for(lambda: (app_dir / 'callback-complete.txt').exists())
            window = wait_for(lambda: desktop.find(lambda n: n.getRole() == desktop.a.ROLE_FRAME and desktop.visible(n)
                and str(n.name).startswith(fixture.name)))
            entry = wait_for(lambda: desktop.find(lambda n: desktop.visible(n) and
                n.getState().contains(desktop.a.STATE_EDITABLE) and
                n.queryText().getText(0, -1).rstrip('/') == str(fixture), window))
            report['observed_file_manager_path'] = entry.queryText().getText(0, -1)
            report['checks']['actual_file_manager_window_path'] = bool(entry) and (app_dir / 'outcome.txt').read_text() == 'requested'
            desktop.screenshot(output / 'revealed.png')
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
        if kind == 'early-settings':
            observe_utf8_input(desktop, app_dir, output, report)
            observe_editing_and_performance(desktop, app_dir, output, report)
        (app_dir / 'finish').touch()
        report['returncode'] = app.wait(timeout=10)
        report['checks']['owned_shutdown'] = report['returncode'] == 0 and 'fixture_retired=true' in (app_dir / 'retired.txt').read_text()
        report['checks']['fixture_removed'] = not fixture.exists()
    except Exception:
        report['failure'] = traceback.format_exc()
        report['last_app_state'] = state(app_dir)
        raise
    finally:
        if app.poll() is None:
            app.kill()
            app.wait(timeout=5)
        (output / 'case.json').write_text(json.dumps(report, indent=2) + '\n')
    if not report['checks'] or not all(report['checks'].values()):
        raise RuntimeError(name + ' failed native checks')
    return report


def observe_display(desktop, root):
    evidence, app_dir = root / 'display', root / 'display/app'
    evidence.mkdir()
    report = {'checks': {}, 'sizes': [], 'scale_transitions': [], 'errors': []}
    desktop.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'enable-animations', 'false'])
    desktop.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'gtk-theme', 'HighContrast'])
    report['private_os_preferences'] = {
        'enable_animations': desktop.run(['gsettings', 'get', 'org.gnome.desktop.interface', 'enable-animations']).strip(),
        'gtk_theme': desktop.run(['gsettings', 'get', 'org.gnome.desktop.interface', 'gtk-theme']).strip()}
    assert report['private_os_preferences'] == {'enable_animations': 'false', 'gtk_theme': "'HighContrast'"}
    app = desktop.start('display-app', ['/opt/librairii/qualification-desktop',
        '--native-probe=' + str(app_dir), '--native-case=input', '--french', '--display-long-text'], debug=True)
    fixture = None
    def geometry():
        return json.loads((app_dir / 'geometry.json').read_text())
    def frame(name):
        (app_dir / 'capture-request').write_text(name)
        wait_for(lambda: (app_dir / (name + '.png')).is_file())
        desktop.screenshot(evidence / (name + '-native.png'))
    def sway(command):
        value = json.loads(desktop.run(['swaymsg', '-s', desktop.ipc, command]))
        assert value and all(v.get('success') is True for v in value), value
    try:
        wait_for(lambda: (app_dir / 'ready.txt').is_file())
        fixture = pathlib.Path((app_dir / 'fixture.txt').read_text())
        wait_for(lambda: desktop.active_pid() == app.pid)
        xid = None
        if desktop.session == 'wayland':
            sway(f'[pid={app.pid}] border none')
        if desktop.session == 'x11':
            xid = desktop.run(['xdotool', 'search', '--onlyvisible', '--pid', str(app.pid)]).splitlines()[-1]
        for width, height in ((560, 400), (900, 760), (1280, 800)):
            if xid:
                desktop.run(['xdotool', 'windowsize', '--sync', xid, str(width), str(height)])
            else:
                sway(f'[pid={app.pid}] resize set width {width} px height {height} px')
            value = wait_for(lambda: (g := geometry()) and abs(g['logical_width'] - width) <= 1
                and abs(g['logical_height'] - height) <= 1 and g['compact'] == (width < 850)
                and g['wide'] == (width >= 1180) and g)
            assert len(value['contrast_ratios']) == 3 and min(value['contrast_ratios']) >= 4.5
            report['sizes'].append(value)
            frame('size-' + str(width))
            desktop.key('ctrl+comma')
            wait_for(lambda: (s := state(app_dir)) and s[2] == '1' and s[4] == 'true')
            frame('settings-' + str(width))
            desktop.key('Escape')
            wait_for(lambda: (s := state(app_dir)) and s[2] == '0')
            if width == 560:
                for _ in range(20):
                    if geometry()['focus_key'] == 10:
                        break
                    desktop.key('Tab')
                else:
                    raise AssertionError('compact Review not keyboard reachable')
                desktop.key('space')
                wait_for(lambda: geometry()['review_visible'])
                frame('compact-review')
                desktop.key('space')
                wait_for(lambda: not geometry()['review_visible'])
                report['checks']['compact_review_keyboard_reachable'] = True
        report['checks']['native_compact_intermediate_wide'] = True
        report['checks']['localized_modal_focus_and_dismiss'] = True
        report['checks']['actual_text_control_selection_contrast'] = True
        if desktop.session == 'wayland':
            sway('create_output')
            sway('output HEADLESS-2 mode 2560x1800 scale 2 position 1280 0')
            report['outputs'] = json.loads(desktop.run(['swaymsg', '-s', desktop.ipc, '-t', 'get_outputs']))
            assert {v['name']: v['scale'] for v in report['outputs']} == {'HEADLESS-1': 1, 'HEADLESS-2': 2}
            for output, scale in (('HEADLESS-2', 2), ('HEADLESS-1', 1), ('HEADLESS-2', 2), ('HEADLESS-1', 1)):
                sway(f'[pid={app.pid}] move container to output {output}')
                value = wait_for(lambda: (g := geometry()) and g['scale'] == scale and g['rows'] == 1000
                    and abs(g['logical_width'] - 1280) <= 1 and abs(g['logical_height'] - 800) <= 1 and g)
                report['scale_transitions'].append(value)
                frame('scale-' + str(len(report['scale_transitions'])))
            report['checks']['actual_mixed_scale_transitions'] = len(report['scale_transitions']) == 4
        # Remove the caret; the library has no decorative spatial animation.
        desktop.key('Tab')
        time.sleep(0.3)
        frame('motion-first')
        time.sleep(0.4)
        frame('motion-second')
        report['checks']['idle_without_nonessential_motion'] = (
            (app_dir / 'motion-first.png').read_bytes() == (app_dir / 'motion-second.png').read_bytes())
        (app_dir / 'finish').touch()
        report['returncode'] = app.wait(timeout=10)
        report['checks']['normal_shutdown'] = app.returncode == 0 and (app_dir / 'retired.txt').is_file()
        report['checks']['fixture_retired'] = fixture is not None and not fixture.exists()
        assert all(report['checks'].values())
    except Exception:
        report['errors'].append(traceback.format_exc())
        raise
    finally:
        if app.poll() is None:
            app.kill(); app.wait(timeout=5)
        (evidence / 'display-session.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--session', choices=('x11', 'wayland'), required=True)
    parser.add_argument('--display-only', action='store_true')
    args = parser.parse_args()
    root = pathlib.Path('/evidence')
    assert os.uname().machine == 'x86_64' and os.geteuid() != 0
    binary_sha = hashlib.sha256(pathlib.Path('/opt/librairii/qualification-desktop').read_bytes()).hexdigest()
    assert binary_sha == os.environ['EXPECTED_BINARY_SHA256']
    desktop = None
    report = {'session': args.session, 'binary_sha256': binary_sha, 'cases': {}, 'errors': [], 'native_integration': 'fail',
        'input_method': 'native-keysym-French-Latin-UTF8-v2'}
    try:
        desktop = Desktop(args.session, root)
        (root / 'versions.txt').write_text(desktop.run(['dpkg-query', '-W', '-f=${Package} ${Version}\n']))
        for name, kind, expected in ([] if args.display_only else [('files-multiple', 'files', 'selected'), ('files-single', 'files', 'selected'),
                ('files-cancel', 'files', 'cancelled'), ('folder-accept', 'folder', 'selected'),
                ('folder-cancel', 'folder', 'cancelled'), ('folder-reject', 'folder', 'rejected'),
                ('early-settings', 'early-settings', 'delivered'), ('reveal', 'reveal', 'requested')]):
            report['cases'][name] = one_case(desktop, name, kind, expected)
        if args.display_only:
            report['display'] = observe_display(desktop, root)
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
