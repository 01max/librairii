"""Public AT-SPI readiness of the actual production executable on a private bus."""
import collections
import json
import sys
import time
import dbus
import pyatspi as a
watching = sys.argv[1] == '--watch'
pid = None if watching else int(sys.argv[1])
bus = dbus.SessionBus()
status = dbus.Interface(bus.get_object('org.a11y.Bus', '/org/a11y/bus'), 'org.freedesktop.DBus.Properties')
status.Set('org.a11y.Status', 'IsEnabled', dbus.Boolean(True))
# Initialize the external observer before the product launch clock starts.
a.Registry.getDesktop(0)
if watching:
    print(json.dumps({'observer_ready': True}), flush=True)
    pid = int(input())
start = time.monotonic()
first_title_role = None
while time.monotonic() - start < 10:
    queue = collections.deque([a.Registry.getDesktop(0)])
    count = 0
    while queue and count < 4096:
        node = queue.popleft()
        count += 1
        try:
            node.clear_cache()
            if node.get_process_id() == pid and node.name.startswith('Histoire 0001') and node.getState().contains(a.STATE_SHOWING):
                if '--point' in sys.argv:
                    if first_title_role is None:
                        first_title_role = node.getRoleName()
                    # The inspector repeats this title in a plain label. Only
                    # the actual browser list-item is a pointer focus target.
                    if node.getRole() != a.ROLE_LIST_ITEM:
                        continue
                    bounds = node.queryComponent().getExtents(0)
                    if bounds.width <= 8 or bounds.height <= 8:
                        continue
                    print(json.dumps({'pid': pid, 'x': bounds.x + bounds.width // 2, 'y': bounds.y + bounds.height // 2,
                                      'name': node.name, 'role': node.getRoleName(), 'first_title_role': first_title_role,
                                      'bounds': {'x': bounds.x, 'y': bounds.y, 'width': bounds.width, 'height': bounds.height}}))
                    sys.exit(0)
                print(json.dumps({'pid': pid, 'actual_story': {'role': node.getRoleName(), 'name': node.name},
                                  'observer_attach_to_story_ms': (time.monotonic()-start)*1000,
                                  'method': 'actual product first loaded story in public AT-SPI tree'}))
                sys.exit(0)
            queue.extend(node.getChildAtIndex(i) for i in range(min(node.childCount, 200)))
        except (dbus.DBusException, RuntimeError):
            pass
    time.sleep(.001)
raise TimeoutError('actual product first story accessibility readiness deadline')
