"""Exercise the observer against a sparse native accessibility tree."""
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import runpy
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch


class Node:
    def __init__(self, name='', role=0, children=()):
        self.name, self.role, self.children = name, role, children
        self.childCount = len(children)

    def clear_cache(self): pass
    def get_process_id(self): return 123 if self.name else 0
    def getState(self): return SimpleNamespace(contains=lambda state: True)
    def getRole(self): return self.role
    def getRoleName(self): return 'list item' if self.role == 1 else 'label'
    def queryComponent(self): return SimpleNamespace(getExtents=lambda origin: SimpleNamespace(x=10, y=20, width=200, height=80))
    def getChildAtIndex(self, index): return self.children[index]


class SparseAccessibility(unittest.TestCase):
    def observe(self, point):
        label = Node('Histoire 0001', role=0)
        row = Node('Histoire 0001', role=1)
        desktop = Node(children=(None, label, None, row))
        atspi = SimpleNamespace(Registry=SimpleNamespace(getDesktop=lambda index: desktop), STATE_SHOWING=1, ROLE_LIST_ITEM=1)
        dbus = SimpleNamespace(SessionBus=lambda: SimpleNamespace(get_object=lambda *args: None),
                               Interface=lambda *args: SimpleNamespace(Set=lambda *args: None), Boolean=bool,
                               DBusException=RuntimeError)
        script = Path(__file__).with_name('resource-linux.py')
        output = io.StringIO()
        argv = [str(script), '123'] + (['--point'] if point else [])
        with patch.dict(sys.modules, {'dbus': dbus, 'pyatspi': atspi}), patch.object(sys, 'argv', argv), redirect_stdout(output):
            with self.assertRaises(SystemExit) as stopped:
                runpy.run_path(str(script), run_name='__main__')
        self.assertEqual(stopped.exception.code, 0)
        return json.loads(output.getvalue())

    def test_missing_child_does_not_replace_actual_loaded_story_observation(self):
        receipt = self.observe(False)
        self.assertEqual(receipt['pid'], 123)
        self.assertEqual(receipt['actual_story'], {'role': 'label', 'name': 'Histoire 0001'})

    def test_pointer_still_requires_the_actual_browser_row_after_missing_children(self):
        receipt = self.observe(True)
        self.assertEqual((receipt['pid'], receipt['role'], receipt['first_title_role']), (123, 'list item', 'label'))
        self.assertEqual((receipt['x'], receipt['y']), (110, 60))


if __name__ == '__main__':
    unittest.main()
