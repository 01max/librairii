"""Owned X11/Wayland desktop; derived from qualified section-2 native driver."""
import collections

import json

import os

import pathlib

import re

import threading

import subprocess

import time

LIMIT = 4 * 1024 * 1024

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
            # Keep the WM's X connection before setting the map: last-client
            # disconnect would otherwise let Xvfb reset it before app launch.
            self.run(['setxkbmap', '-layout', 'fr', '-variant', 'oss'])
            keymap = self.run(['setxkbmap', '-query'])
            assert re.search(r'^layout:\s+fr\s*$', keymap, re.MULTILINE), keymap
            assert re.search(r'^variant:\s+oss\s*$', keymap, re.MULTILINE), keymap
            (self.root / 'x11-keyboard-map.txt').write_text(keymap)
            symbols = self.run(['xkbcomp', '-xkb', os.environ['DISPLAY'], '-'])
            assert 'Eacute' in symbols, 'native uppercase accented key is unmapped'
            (self.root / 'x11-keyboard-symbols.txt').write_text(symbols)
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
            self.start('keyboard-seat', ['wtype', '-s', '1800000'])
            wait_for(lambda: 'keyboard' in self.run(['swaymsg', '-s', self.ipc, '-t', 'get_inputs']))
        self.bus = dbus.SessionBus()
        # Only the private bus receives this owned desktop's activation context.
        activation = {key: os.environ[key] for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'GDK_BACKEND',
            'XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME',
            'XDG_CURRENT_DESKTOP', 'XDG_SESSION_TYPE', 'GTK_MODULES', 'LC_ALL') if key in os.environ}
        self.bus.get_object('org.freedesktop.DBus', '/org/freedesktop/DBus').UpdateActivationEnvironment(
            activation, dbus_interface='org.freedesktop.DBus')
        self.a.Registry.getDesktop(0)  # Activate the private accessibility bus/registry.

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
        # Send one native command sequence across ^ then e on the owned keymap.
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
