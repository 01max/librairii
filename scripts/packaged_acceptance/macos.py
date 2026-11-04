"""Reuse the qualified public AX observer against the installed production PID."""
import importlib.util
import json
import os
import time


class Driver:
    def __init__(self, source, output, pid, helper):
        if os.environ.get('GITHUB_ACTIONS') != 'true':
            raise ValueError('native GUI input requires a disposable CI host')
        spec = importlib.util.spec_from_file_location('packaged_mac_observer', source/'qualification/section-2/check-native-macos.py')
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.observer = self.module.Observer(helper, output, pid)
        self.pid, self.output, self.helper = pid, output, helper
        # A spawned process can precede AppKit registration on a warm reopen.
        # A transparent startup window can require activation before painting.
        def registered():
            status = self.observer.call('launch-status')
            if status['pid'] != pid or status['terminated']:
                raise ValueError('owned AppKit launch identity or lifetime changed')
            return status if (status['registered'] and status['finished_launching']
                              and status['regular_activation']) else None
        initial = self.observer.wait(registered)
        (output/'initial-launch-ready.json').write_text(json.dumps(initial, indent=2)+'\n')
        self.observer.call('activate')
        def ready():
            status = self.observer.status()
            return status if status['pid'] == pid and status['frontmost_pid'] == pid and any(
                window.get('kCGWindowOwnerPID') == pid and window.get('kCGWindowLayer') == 0
                and window.get('kCGWindowIsOnscreen') is True and window.get('kCGWindowAlpha', 0) > 0
                for window in status['windows']) else None
        try:
            initial = self.observer.wait(ready)
        except Exception:
            try:
                self.snapshot('initial-window-failure')
            except Exception as error:
                try:
                    (output/'initial-window-failure.log').write_text(str(error)+'\n')
                except Exception:
                    pass
            raise
        (output/'initial-window-ready.json').write_text(json.dumps(initial, indent=2)+'\n')

    def snapshot(self, name=None, *, deadline=None):
        # Named captures also need the observer's bounded read-only wait when
        # AppKit invalidates a tree during a transition. Nested waits share
        # their original deadline; no input or partial capture is repeated.
        parent = self.module._ACTIVE_WAIT.get()
        timeout = max(0, parent.deadline - time.monotonic()) if parent else 12
        if deadline is not None:
            timeout = min(timeout, max(0, deadline-time.monotonic()))
        snapshot = self.observer.wait(lambda: self.observer.call('snapshot-window'), timeout=timeout)
        if name:
            (self.output/(name+'.json')).write_text(json.dumps(snapshot, indent=2, ensure_ascii=False)+'\n')
            self.observer.capture(name, snapshot)
        return snapshot

    def wait(self, predicate, timeout=15):
        def observed():
            try:
                return predicate()
            except FileNotFoundError:
                # The native worker publishes the owned profile asynchronously.
                # Absence can be polled; invalid data and other errors still fail.
                return None
        return self.observer.wait(observed, timeout)

    def find(self, name, *, prefix=False, enabled=True, roles=None, actionable=False, deadline=None):
        snapshot = self.snapshot() if deadline is None else self.snapshot(deadline=deadline)
        matches = [node for node in snapshot['nodes']
                   if any((value.startswith(name) if prefix else value == name)
                          for value in (node.get('title', ''), node.get('description', '')))
                   and (not enabled or node.get('enabled') is not False)
                   and (not actionable or 'AXPress' in node.get('actions', []))
                   and (roles is None or node.get('role') in roles)]
        return next(iter(matches), None)

    def press(self, name, *, prefix=False, deadline=None):
        for attempt in range(2):
            options = {} if deadline is None else {'deadline': deadline}
            wait_options = {} if deadline is None else {'timeout': max(0, deadline-time.monotonic())}
            node = self.wait(lambda: self.find(name, prefix=prefix, actionable=True, **options), **wait_options)
            try:
                request = {}
                if deadline is not None:
                    native_now = time.clock_gettime(time.CLOCK_MONOTONIC)
                    remaining = deadline-time.monotonic()
                    if remaining <= 0:
                        raise TimeoutError('installed GUI action deadline')
                    # Use the explicit shared POSIX clock in the separate helper;
                    # Python's default monotonic clock need not be that clock.
                    # Sample it first so descheduling cannot extend the cutoff.
                    request = {'command_timeout': remaining,
                               'action_deadline_monotonic': native_now+remaining}
                self.observer.call('press', selector=self.module.selector(node), **request)
            except RuntimeError as error:
                # This exact helper refusal occurs in resolve(), before AXPress.
                # Reobserve once; never retry an action refusal or unknown error.
                stale = 'press failed: {"returncode": 1, "errors": []}; invalid("native control changed")'
                if attempt == 0 and str(error).strip() == stale:
                    continue
                raise
            if deadline is not None and time.monotonic() >= deadline:
                raise TimeoutError('installed GUI action deadline')
            time.sleep(.08)
            return node

    def focus(self, name, *, prefix=False, roles=None):
        node = self.wait(lambda: self.find(name, prefix=prefix, roles=roles))
        self.observer.call('focus', selector=self.module.selector(node))
        self.wait(lambda: any(node.get('focused') and any((value.startswith(name) if prefix else value == name)
                             for value in (node.get('title',''),node.get('description','')))
                             for node in self.snapshot()['nodes']))

    def key(self, value):
        self.observer.call('key', key=value.lower())
        time.sleep(.08)

    def text(self, name, value):
        self.focus(name, roles=('AXTextField', 'AXTextArea', 'AXSearchField'))
        self.key('cmd+a')
        if value:
            self.observer.call('clipboard-set', text=value)
            self.key('cmd+v')
        else:
            self.key('delete')
        self.wait(lambda: any(node.get('value') == value and name in (node.get('title'), node.get('description'))
                             for node in self.snapshot()['nodes']))

    def choose(self, name, index, *, prefix=False):
        if not 0 <= index < 64:
            raise ValueError('synthetic combo index exceeds the fixture model bound')
        self.focus(name, prefix=prefix, roles=('AXComboBox', 'AXPopUpButton'))
        # The pinned Slint ComboBox opens with Return, supports clamped Up/Down,
        # and does not handle Home. All these synthetic models contain <64 rows.
        self.key('return')
        for _ in range(64):
            self.key('up')
        for _ in range(index):
            self.key('down')
        self.key('return')

    def picker(self, directory, *, files=False):
        snapshot = self.wait(lambda: (value := self.observer.snapshot(summary=True)) and self.module.picker_root(value) and value)
        root = self.module.picker_root(snapshot)
        # Column view also exposes ancestor-directory rows. Choose native list
        # view before navigation so observation stays within the selected folder.
        self.observer.call('key', key='cmd+2')
        self.observer.go_folder(directory, root)
        if files:
            self.observer.rows(directory, root, True, tuple(sorted(path.name for path in directory.iterdir())))
        self.observer.button(self.observer.snapshot(), root, ('Open', 'Choose', 'Select', 'Ouvrir', 'Choisir'))
        self.wait(lambda: not self.module.picker_root(self.observer.snapshot(summary=True)))
        self.wait(lambda: self.observer.status()['frontmost_pid'] == self.pid)

    def settings(self, french=False):
        self.observer.call('menu', title='Réglages…' if french else 'Settings…')

    def reveal(self, destination, files):
        def appeared():
            for pid in self.observer.call('finder')['pids']:
                finder = self.module.Observer(self.helper, self.output, pid)
                snapshot = finder.snapshot(document=destination)
                if observed := self.module.ready_finder_snapshot(snapshot, destination, pid, tuple(files)):
                    return finder, observed
            return None
        finder, (snapshot, window, location, window_id) = self.wait(appeared)
        finder.capture('actual-reveal', snapshot)
        retired = self.module.close_finder(finder, destination, window_id, tuple(files))
        self.observer.call('activate')
        self.wait(lambda: self.observer.status()['frontmost_pid'] == self.pid)
        return {'pid': finder.pid, 'window_id': window_id, 'location': location,
                'retired': retired, 'explicit_return_to_product': True}

    def quit(self, french=False):
        self.observer.call('menu', title='Quitter Librairii' if french else 'Quit Librairii')
