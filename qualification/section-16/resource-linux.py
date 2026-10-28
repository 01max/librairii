"""Public AT-SPI readiness of the actual production executable on a private bus."""
import collections
import json
import sys
import time
import dbus
import pyatspi as a
pid = int(sys.argv[1])
bus = dbus.SessionBus()
status = dbus.Interface(bus.get_object('org.a11y.Bus', '/org/a11y/bus'), 'org.freedesktop.DBus.Properties')
status.Set('org.a11y.Status', 'IsEnabled', dbus.Boolean(True))
start = time.monotonic()
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
                    bounds = node.queryComponent().getExtents(0)
                    print(json.dumps({'pid': pid, 'x': bounds.x + bounds.width // 2, 'y': bounds.y + bounds.height // 2, 'name': node.name}))
                    sys.exit(0)
                print(json.dumps({'pid': pid, 'actual_story': {'role': node.getRoleName(), 'name': node.name}, 'method': 'actual product first loaded story in public AT-SPI tree'}))
                sys.exit(0)
            queue.extend(node.getChildAtIndex(i) for i in range(min(node.childCount, 200)))
        except (dbus.DBusException, RuntimeError):
            pass
    time.sleep(.001)
raise TimeoutError('actual product first story accessibility readiness deadline')
