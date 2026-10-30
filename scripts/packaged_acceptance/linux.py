"""Actual AT-SPI/keyboard operations on the private installed-product desktop."""
import json
from pathlib import Path
import time


class Driver:
    def __init__(self, source, output, pid, desktop):
        from gi.repository import GLib
        self.desktop, self.output, self.pid = desktop, output, pid
        self.read_errors = (RuntimeError, desktop.dbus.DBusException, GLib.Error)
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
        return (node for node in self.desktop.nodes(self.application if root is None else root) if node is not None)

    def snapshot(self, name=None, *, root=None):
        nodes = []
        a = self.desktop.a
        for node in self.nodes(root):
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
            except self.read_errors:
                continue
        report = {'pid': self.pid if root is None else root.get_process_id(),
                  'frontmost_pid': self.desktop.active_pid(), 'nodes': nodes}
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
            except self.read_errors:
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
        observed = {'name': node.name, 'role': node.getRoleName()}
        self.action(node)
        return observed

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
        if value:
            self.desktop.text(value)
        else:
            self.key('BackSpace')
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
        if self.desktop.session == 'x11':
            # Leave the empty Recent view before entering a location. In that
            # view GTK can keep folder confirmation disabled despite entry text.
            attempts = open_picker_root(self, application)
            (self.output/(prefix+'-root-place.json')).write_text(json.dumps(attempts, indent=2)+'\n')
            self.wait(lambda: {'etc','tmp','usr'} <= {node.name for node in self.nodes(application)})
        self.key('ctrl+l')
        self.desktop.text(str(directory))
        # Keep GTK's selected trailing slash tentative: committing it with
        # Right can start a second completion into this directory's filenames.
        # The native Open action accepts the exact observed directory directly.
        entered = picker_location(self, application, directory)
        (self.output/(prefix+'-location.json')).write_text(json.dumps(entered, ensure_ascii=False, indent=2)+'\n')
        self.desktop.screenshot(self.output/(prefix+'-location.png'))
        candidates = ('Open', 'Select', 'Choose', 'Ouvrir', 'Sélectionner', 'Choisir')
        def confirm():
            button = self.wait(lambda: next((node for name in candidates
                                            if (node := self.find(name, root=application, roles=('push button',),
                                                                  actionable=True)) is not None), None))
            self.action(button)
        # GTK's default response handles the location entry: SELECT_FOLDER
        # accepts that folder, while OPEN navigates into it without responding.
        # A direct native button action also works without a default Return binding.
        confirm()
        selected = {'observed_location_entry': entered}
        if files:
            selected.update(select_picker_rows(self, application, {path.name for path in directory.iterdir()}))
            (self.output/(prefix+'-selected.json')).write_text(json.dumps(
                {'directory': str(directory), 'files': True, **selected}, ensure_ascii=False, indent=2)+'\n')
            self.desktop.screenshot(self.output/(prefix+'-selected.png'))
            confirm()
        else:
            (self.output/(prefix+'-selected.json')).write_text(json.dumps(
                {'directory': str(directory), 'files': False, **selected}, ensure_ascii=False, indent=2)+'\n')
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
                    # Thunar's native location entry includes a directory slash.
                    if value in (str(destination), str(destination)+'/',
                                 destination.as_uri(), destination.as_uri()+'/'):
                        return {'role': node.getRoleName(), 'name': node.name, 'value': value}
                except (NotImplementedError, *self.read_errors):
                    continue
            return None
        observed = self.wait(location)
        self.snapshot('actual-reveal', root=application)
        if desktop.active_pid() != pid:
            raise ValueError('owned file manager lost focus before close')
        # Operate Thunar's observed native menu rather than assuming a keyboard
        # accelerator was delivered while its location entry retained focus.
        menu = self.wait(lambda: self.find('File', root=application,
                                          roles=('menu', 'menu item'), actionable=True))
        self.action(menu)
        close = self.wait(lambda: self.find('Close Window', root=application,
                                           roles=('menu item',), actionable=True))
        self.snapshot('reveal-close-menu', root=application)
        if desktop.active_pid() != pid:
            raise ValueError('owned file manager lost focus before its Close Window action')
        self.action(close)
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
                  for child in driver.desktop.nodes(table.getAccessibleAt(row, column)) if child is not None}
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
    if not node.queryComponent().grabFocus():
        raise ValueError('native picker table focus refused')
    for row in range(table.nRows):
        if row in observed['chosen'] and not table.isRowSelected(row):
            if not table.addRowSelection(row):
                raise ValueError('native picker row selection refused')
        elif row not in observed['chosen'] and table.isRowSelected(row):
            if not table.removeRowSelection(row):
                raise ValueError('native picker row deselection refused')
    selected = driver.wait(lambda: set(table.getSelectedRows()) == observed['chosen'] and list(table.getSelectedRows()))
    return {'wanted_names': sorted(wanted), 'observed_names': observed['names'], 'selected_rows': selected}


def open_picker_root(driver, application):
    place = driver.wait(lambda: driver.find('/', root=application, roles=('label',)))
    attempts = []
    for _ in range(6):
        if place is None:
            break
        observed = {'name': place.name, 'role': place.getRoleName()}
        attempts.append(observed)
        if observed['role'] == 'list item':
            # GtkPlacesSidebar opens the selected row on Return; focus alone
            # only moves its cursor and would reopen the previous Recent row.
            index = place.getIndexInParent()
            box = place.parent
            if box.getRoleName() != 'list box' or index < 0:
                raise ValueError('native root location has no owning list')
            if not box.querySelection().selectChild(index):
                raise ValueError('native root location selection refused')
            driver.wait(lambda: place.getState().contains(driver.desktop.a.STATE_SELECTED))
            if not place.queryComponent().grabFocus():
                raise ValueError('native root location focus refused')
            driver.wait(lambda: place.getState().contains(driver.desktop.a.STATE_FOCUSED))
            driver.key('Return')
            observed['selected_index'] = index
            observed['focused_native_return'] = True
            return attempts
        place = place.parent
    raise ValueError('native root location list row was not found')


def picker_location(driver, application, directory):
    expected = str(directory)
    def location(allow_completion):
        for node in driver.nodes(application):
            try:
                if not node.getState().contains(driver.desktop.a.STATE_FOCUSED):
                    continue
                text = node.queryText()
                value = text.getText(0, -1)
                observed = {'name': node.name, 'role': node.getRoleName(), 'value': value, 'focused': True}
                if value in (expected, expected+'/'):
                    return observed
                if allow_completion and value.startswith(expected+'/') and text.getNSelections() == 1:
                    start,end = text.getSelection(0)
                    if start == len(expected)+1 and end == len(value):
                        observed['selected_completion'] = [start,end]
                        return observed
            except (NotImplementedError, *driver.read_errors):
                continue
        return None
    entered = driver.wait(lambda: location(True))
    if 'selected_completion' in entered:
        # GTK can complete a common filename prefix after the directory slash.
        # Delete only that observed selected suffix, then require the exact path.
        driver.key('BackSpace')
        exact = driver.wait(lambda: location(False))
        exact['removed_native_completion'] = entered
        return exact
    return entered
