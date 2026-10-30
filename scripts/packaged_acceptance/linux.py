"""Actual AT-SPI/keyboard operations on the private installed-product desktop."""
import json
from pathlib import Path
import time


class Driver:
    def __init__(self, source, output, pid, desktop):
        self.desktop, self.output, self.pid = desktop, output, pid
        self.portal_pid = None
        self.picker_index = 0
        self.application = self.wait(lambda: desktop.find(
            lambda node: node.getRole() == desktop.a.ROLE_APPLICATION and node.get_process_id() == pid))
        self.wait(lambda: desktop.active_pid() == pid)

    def wait(self, predicate, timeout=15):
        deadline = time.monotonic()+timeout
        while time.monotonic() < deadline:
            try:
                value = predicate()
                if value:
                    return value
            except (FileNotFoundError, KeyError):
                pass
            time.sleep(.05)
        raise TimeoutError('installed GUI observation deadline')

    def nodes(self, root=None):
        return self.desktop.nodes(self.application if root is None else root)

    def snapshot(self, name=None):
        nodes = []
        a = self.desktop.a
        for node in self.nodes():
            try:
                state = node.getState()
                value = {'name': node.name, 'role': node.getRoleName(),
                         'enabled': state.contains(a.STATE_ENABLED), 'showing': state.contains(a.STATE_SHOWING),
                         'focused': state.contains(a.STATE_FOCUSED)}
                try:
                    text = node.queryText()
                    value['value'] = text.getText(0, min(text.characterCount, 4096))
                except NotImplementedError:
                    pass
                nodes.append(value)
            except (RuntimeError, self.desktop.dbus.DBusException):
                continue
        report = {'pid': self.pid, 'frontmost_pid': self.desktop.active_pid(), 'nodes': nodes}
        if name:
            (self.output/(name+'.json')).write_text(json.dumps(report, indent=2, ensure_ascii=False)+'\n')
            self.desktop.screenshot(self.output/(name+'.png'))
        return report

    def find(self, name, *, prefix=False, enabled=True, root=None, roles=None, actionable=False):
        a = self.desktop.a
        for node in self.nodes(root):
            try:
                if ((node.name.startswith(name) if prefix else node.name == name)
                        and (not enabled or node.getState().contains(a.STATE_ENABLED))
                        and (roles is None or node.getRoleName() in roles)):
                    if actionable:
                        try:
                            actions = node.queryAction()
                            if not any(actions.getName(index).lower() in ('click','press','activate','toggle')
                                       for index in range(actions.nActions)):
                                continue
                        except NotImplementedError:
                            continue
                    return node
            except (RuntimeError, self.desktop.dbus.DBusException):
                continue
        return None

    def action(self, node):
        actions = node.queryAction()
        for index in range(actions.nActions):
            if actions.getName(index).lower() in ('click', 'press', 'activate', 'toggle'):
                if not actions.doAction(index):
                    raise ValueError('native accessibility action refused')
                time.sleep(.08)
                return
        raise ValueError('native control has no activation action: '+node.name)

    def press(self, name, *, prefix=False):
        node = self.wait(lambda: self.find(name, prefix=prefix, actionable=True))
        self.action(node)
        return {'name': node.name, 'role': node.getRoleName()}

    def focus(self, name, *, prefix=False, roles=None):
        node = self.wait(lambda: self.find(name, prefix=prefix, roles=roles))
        if not node.queryComponent().grabFocus():
            raise ValueError('actual native focus refused: '+name)
        self.wait(lambda: node.getState().contains(self.desktop.a.STATE_FOCUSED))

    def key(self, value):
        if self.desktop.active_pid() not in (self.pid, self.portal_pid):
            raise ValueError('keyboard target escaped product/owned portal')
        self.desktop.key(value)
        time.sleep(.08)

    def text(self, name, value):
        self.focus(name, roles=('text', 'entry'))
        self.key('ctrl+a')
        self.desktop.text(value)
        self.wait(lambda: any(node.get('name') == name and node.get('value') == value
                             for node in self.snapshot()['nodes']))

    def choose(self, name, index, *, prefix=False):
        if not 0 <= index < 64:
            raise ValueError('synthetic combo index exceeds the fixture model bound')
        self.focus(name, prefix=prefix, roles=('combo box',))
        self.key('Return')
        for _ in range(64):
            self.key('Up')
        for _ in range(index):
            self.key('Down')
        self.key('Return')

    def picker(self, directory, *, files=False):
        self.picker_index += 1
        prefix = 'picker-'+str(self.picker_index).zfill(2)
        bus = self.desktop.bus
        service = 'org.freedesktop.impl.portal.desktop.gtk'
        owner = self.wait(lambda: bus.get_name_owner(service) if bus.name_has_owner(service) else None)
        self.portal_pid = int(bus.get_object('org.freedesktop.DBus', '/org/freedesktop/DBus')
                              .GetConnectionUnixProcessID(owner, dbus_interface='org.freedesktop.DBus'))
        application = self.wait(lambda: self.desktop.find(
            lambda node: node.getRole() == self.desktop.a.ROLE_APPLICATION
                         and node.get_process_id() == self.portal_pid))
        self.wait(lambda: self.desktop.active_pid() == self.portal_pid)
        before = [{'name': node.name, 'role': node.getRoleName()} for node in self.nodes(application)]
        (self.output/(prefix+'-opened.json')).write_text(json.dumps(
            {'owner': owner, 'portal_pid': self.portal_pid, 'nodes': before}, ensure_ascii=False, indent=2)+'\n')
        self.key('ctrl+l')
        self.desktop.text(str(directory if files else directory.parent)+'/')
        self.key('Return')
        # Location-entry focus survives navigation. Select actual native table
        # rows rather than sending Select All to that entry. For a folder,
        # navigate its parent and explicitly select the named child.
        wanted = {path.name for path in directory.iterdir()} if files else {directory.name}
        selected = select_picker_rows(self, application, wanted)
        (self.output/(prefix+'-selected.json')).write_text(json.dumps(
            {'directory': str(directory), 'files': files, **selected}, ensure_ascii=False, indent=2)+'\n')
        self.desktop.screenshot(self.output/(prefix+'-selected.png'))
        candidates = ('Open', 'Select', 'Choose', 'Ouvrir', 'Sélectionner', 'Choisir')
        button = self.wait(lambda: next((node for name in candidates
                                        if (node := self.find(name, root=application, roles=('push button',),
                                                              actionable=True)) is not None), None))
        self.action(button)
        self.wait(lambda: self.desktop.active_pid() == self.pid)

    def settings(self, french=False):
        self.key('ctrl+comma')

    def reveal(self, destination, files):
        desktop = self.desktop
        application = self.wait(lambda: desktop.find(
            lambda node: node.getRole() == desktop.a.ROLE_APPLICATION
                         and node.name.lower() == 'thunar'))
        pid = application.get_process_id()
        if (Path('/proc')/str(pid)/'exe').resolve() != Path('/usr/bin/thunar').resolve():
            raise ValueError('observed file manager executable is not the qualified Thunar')
        self.wait(lambda: desktop.active_pid() == pid)
        desktop.key('ctrl+l')
        def location():
            for node in desktop.nodes(application):
                try:
                    value = node.queryText().getText(0, -1)
                    if value == str(destination) or value == destination.as_uri():
                        return {'role': node.getRoleName(), 'name': node.name, 'value': value}
                except (NotImplementedError, RuntimeError, desktop.dbus.DBusException):
                    continue
            return None
        observed = self.wait(location)
        desktop.screenshot(self.output/'actual-reveal.png')
        if desktop.active_pid() != pid:
            raise ValueError('owned file manager lost focus before close')
        desktop.key('ctrl+w')
        self.wait(lambda: not desktop.find(
            lambda node: node.getRole() in (desktop.a.ROLE_FRAME, desktop.a.ROLE_DIALOG)
                         and node.get_process_id() == pid and desktop.visible(node)))
        if desktop.session == 'x11':
            windows = desktop.run(['xdotool', 'search', '--onlyvisible', '--pid', str(self.pid)]).split()
            if len(windows) != 1:
                raise ValueError('owned product window not unique after reveal')
            desktop.run(['xdotool', 'windowactivate', '--sync', windows[0]])
        else:
            desktop.run(['swaymsg', '-s', desktop.ipc, '[pid='+str(self.pid)+']', 'focus'])
        self.wait(lambda: desktop.active_pid() == self.pid)
        return {'pid': pid, 'executable': '/usr/bin/thunar', 'observed_location': observed,
                'visible_window_closed': True, 'explicit_return_to_product': True}

    def quit(self, french=False):
        self.key('ctrl+q')


def select_picker_rows(driver, application, wanted):
    """Identify original filenames, then observe exact native row selection."""
    if not wanted:
        raise ValueError('native picker selection requires explicit fixture names')
    observed = {}
    def matches(node):
        if (node.getRole() not in (driver.desktop.a.ROLE_TABLE, driver.desktop.a.ROLE_TREE_TABLE)
                or not driver.desktop.visible(node)):
            return False
        table = node.queryTable()
        if not 0 < table.nRows <= 128 or not 0 < table.nColumns <= 16:
            return False
        names = [{str(child.name) for column in range(table.nColumns)
                  for child in driver.desktop.nodes(table.getAccessibleAt(row, column))}
                 for row in range(table.nRows)]
        found = [values & wanted for values in names]
        if (set().union(*found) != wanted or sum(len(values) for values in found) != len(wanted)
                or any(len(values) > 1 for values in found)):
            return False
        observed.update(names=[sorted(values) for values in names],
                        chosen={index for index, values in enumerate(found) if values})
        return True
    node = driver.wait(lambda: driver.desktop.find(matches, application))
    table = node.queryTable()
    for row in range(table.nRows):
        if row in observed['chosen'] and not table.isRowSelected(row):
            if not table.addRowSelection(row):
                raise ValueError('native picker row selection refused')
        elif row not in observed['chosen'] and table.isRowSelected(row):
            if not table.removeRowSelection(row):
                raise ValueError('native picker row deselection refused')
    selected = driver.wait(lambda: set(table.getSelectedRows()) == observed['chosen'] and list(table.getSelectedRows()))
    return {'wanted_names': sorted(wanted), 'observed_names': observed['names'], 'selected_rows': selected}
