#!/usr/bin/env python3
"""Inspect actual Linux AT-SPI state on an owned private-bus X11 fixture."""
import collections
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import dbus
import pyatspi as a
BINARY = Path(sys.argv[1])
OUTPUT = Path(sys.argv[2])
OUTPUT.mkdir(parents=True, exist_ok=True)
def wait(call, message):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = call()
        if result:
            return result
        time.sleep(0.05)
    raise RuntimeError(message)
def nodes(root):
    queue = collections.deque([(root, 0)])
    count = 0
    while queue and count < 2000:
        node, depth = queue.popleft()
        count += 1
        try:
            if hasattr(node, 'clear_cache'):
                node.clear_cache()
            yield node
            if depth < 30:
                queue.extend((node.getChildAtIndex(i), depth + 1) for i in range(min(node.childCount, 200)))
        except (dbus.DBusException, RuntimeError):
            continue
    if queue:
        raise RuntimeError('unexpectedly unbounded native accessibility tree')
def find(root, predicate):
    for node in nodes(root):
        try:
            if predicate(node):
                return node
        except (dbus.DBusException, RuntimeError):
            continue
    return None
def visible(node):
    return node.getState().contains(a.STATE_SHOWING)
def named(root, label):
    return wait(lambda: find(root, lambda n: n.name == label and visible(n)), 'native accessible name missing: ' + label)
def action(node):
    interface = node.queryAction()
    if interface.nActions < 1 or not interface.doAction(0):
        raise RuntimeError('native accessible action refused: ' + node.name)
def key(value):
    subprocess.run(['xdotool', 'key', '--clearmodifiers', value], check=True, timeout=10)
    time.sleep(0.12)
def tree(root):
    return [{'role': n.getRoleName(), 'name': n.name[:512], 'enabled': n.getState().contains(a.STATE_ENABLED), 'sensitive': n.getState().contains(a.STATE_SENSITIVE), 'selected': n.getState().contains(a.STATE_SELECTED), 'focused': n.getState().contains(a.STATE_FOCUSED)} for n in nodes(root)]
def main():
    bus = dbus.SessionBus()
    status = dbus.Interface(bus.get_object('org.a11y.Bus', '/org/a11y/bus'), 'org.freedesktop.DBus.Properties')
    # GNOME's status contract enables semantic inspection without asserting or
    # starting a screen reader. This bus belongs only to the CI desktop session.
    status.Set('org.a11y.Status', 'IsEnabled', dbus.Boolean(True))
    desktop = a.Registry.getDesktop(0)
    reports = []
    for concept in ['stories', 'workspace']:
        output = OUTPUT / concept
        output.mkdir(exist_ok=True)
        (output / '.librairii-native-fixture').write_text('native-foundations-14\n')
        environment = dict(os.environ, SLINT_BACKEND='winit-skia-software', SLINT_SCALE_FACTOR='1', WINIT_UNIX_BACKEND='x11')
        with (output / 'execution.log').open('w') as log:
            process = subprocess.Popen([str(BINARY), str(output), '--ax-server'] + (['--workspace'] if concept == 'workspace' else []), env=environment, stdout=log, stderr=subprocess.STDOUT)
            try:
                root = wait(lambda: find(desktop, lambda n: n.getRole() == a.ROLE_APPLICATION and find(n, lambda child: child.name == 'Librairii') is not None), 'native fixture did not register with AT-SPI')
                xid = subprocess.check_output(['xdotool', 'search', '--sync', '--onlyvisible', '--pid', str(process.pid)], text=True, timeout=15).splitlines()[-1]
                subprocess.run(['xdotool', 'windowfocus', '--sync', xid], check=True, timeout=10)
                if concept == 'stories':
                    listing = named(root, 'Stories')
                    if not listing.queryComponent().grabFocus():
                        raise RuntimeError('native list refused accessible keyboard focus')
                    key('End')
                    wait(lambda: find(root, lambda n: n.getRole() == a.ROLE_LIST_ITEM and n.getState().contains(a.STATE_SELECTED) and '24' in n.name), 'offscreen accessible selection did not follow keyboard focus')
                    key('Home')
                    wait(lambda: find(root, lambda n: n.getRole() == a.ROLE_LIST_ITEM and n.getState().contains(a.STATE_SELECTED) and n.name.startswith('The Clockwork Forest')), 'native selection did not return home')
                    names = [n.name for n in nodes(root)]
                    if not any('Broken' in name for name in names) or not any('Unavailable' in name for name in names):
                        raise RuntimeError('warnings depend on color or disappeared from accessibility')
                else:
                    named(root, 'Pending review')
                    apply = named(root, 'Apply decisions')
                    if apply.getState().contains(a.STATE_ENABLED) or apply.getState().contains(a.STATE_SENSITIVE):
                        raise RuntimeError('zero-decision Apply is enabled or sensitive')
                    named(root, 'Selection tag')
                    subprocess.run(['xdotool', 'windowsize', '--sync', xid, '560', '400'], check=True, timeout=10)
                    choice = wait(lambda: find(root, lambda n: n.name == 'The next adventure' and n.getRole() == a.ROLE_COMBO_BOX), 'compact review choice missing')
                    if not choice.queryComponent().grabFocus():
                        raise RuntimeError('compact review choice refused keyboard focus')
                    def choice_fits():
                        pane = named(root, 'Pending review').queryComponent().getExtents(0)
                        bounds = choice.queryComponent().getExtents(0)
                        return pane.width < 600 and 80 <= pane.height <= 180 and bounds.height >= 28 and bounds.y >= pane.y and bounds.y + bounds.height <= pane.y + pane.height
                    wait(choice_fits, 'compact review choice remains clipped after focus')
                    reports.append({'concept': 'workspace-compact', 'state': tree(root), 'focused_choice_bounds': {field: getattr(choice.queryComponent().getExtents(0), field) for field in ['x', 'y', 'width', 'height']}})
                    subprocess.run(['xdotool', 'windowsize', '--sync', xid, '1180', '760'], check=True, timeout=10)
                    time.sleep(0.2)
                before = tree(root)
                action(named(root, 'Settings…'))
                named(root, 'Settings')
                search = named(root, 'Search stories')
                if search.getState().contains(a.STATE_ENABLED) or search.getState().contains(a.STATE_SENSITIVE):
                    raise RuntimeError('modal background input remains enabled or sensitive')
                english = named(root, 'English')
                if not english.getState().contains(a.STATE_FOCUSED):
                    raise RuntimeError('settings initial accessible focus is incorrect')
                key('shift+Tab')
                if not named(root, 'Close').getState().contains(a.STATE_FOCUSED):
                    raise RuntimeError('reverse Tab escaped the modal focus cycle')
                key('Tab')
                if not named(root, 'English').getState().contains(a.STATE_FOCUSED):
                    raise RuntimeError('forward Tab did not wrap to initial modal focus')
                key('Tab')
                action(named(root, 'Français'))
                named(root, 'Réglages')
                during = tree(root)
                key('Escape')
                named(root, 'Rechercher des histoires')
                if find(root, lambda n: n.name == 'Réglages' and visible(n)):
                    raise RuntimeError('Escape did not dismiss modal accessible subtree')
                after = tree(root)
                reports.append({'concept': concept, 'before': before, 'modal_french': during, 'after': after})
                key('ctrl+q')
                process.wait(timeout=10)
                if process.returncode != 0:
                    raise RuntimeError('native fixture did not close cleanly')
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
    (OUTPUT / 'report.json').write_text(json.dumps({'schema': 1, 'source': 'actual Linux AT-SPI state/actions and X11 keyboard events on a private CI session', 'checks': 'passed', 'cases': reports}, indent=2) + '\n')
    print('Native AT-SPI roles, warnings, disabled/sensitive states, focus, French labels and keyboard selection passed.')
if __name__ == '__main__':
    main()
