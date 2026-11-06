"""Reject synthetic-only Wayland claims and bound failed native processes."""
import importlib.util
import os
import pathlib
import signal
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('wayland', pathlib.Path(__file__).parents[1] / 'check-wayland.py')
wayland = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wayland)

TRACE = ('[0.1][rs] -> xdg_wm_base@2.get_xdg_surface(xdg_surface@23, wl_surface@21)\n'
         '[0.2][rs] -> xdg_surface@23.get_toplevel(xdg_toplevel@24)\n'
         '[1.0][rs] <- xdg_surface@23.configure, (10)\n'
         '[1.01][rs] -> xdg_surface@23.ack_configure(10)\n'
         '[1.1][rs] -> wl_surface@21.attach(wl_buffer@50, 0, 0)\n'
         '[1.2][rs] -> wl_surface@21.commit()\n')


class Wayland(unittest.TestCase):
    def success(self):
        return {'returncode': 0, 'errors': [], 'output': TRACE + wayland.CPU + '\nrenderer_smoke=passed;',
                'snapshots': {'multilingual.png': 'first', 'filtered.png': 'changed'}}

    def test_protocol_requires_configuration_and_buffer_commit_on_the_same_surface(self):
        self.assertTrue(all(wayland.protocol_checks(TRACE).values()))
        for changed in (TRACE.replace('configure', 'not_configured'),
                        TRACE.replace('wl_buffer@50', 'nil'),
                        TRACE.replace('wl_surface@21.commit', 'wl_surface@22.commit'),
                        TRACE.replace('wl_surface@21.commit()', ''),
                        TRACE.replace('xdg_surface@23.ack_configure(10)', 'xdg_surface@23.ack_configure(11)'),
                        TRACE.replace('get_toplevel', 'not_a_toplevel'),
                        TRACE.replace('get_xdg_surface', 'no_identity_mapping'),
                        TRACE.replace('wl_surface@21.attach', 'wl_surface@99.attach').replace(
                            'wl_surface@21.commit', 'wl_surface@99.commit'),
                        TRACE.splitlines()[-1] + '\n' + '\n'.join(TRACE.splitlines()[:-1]),
                        'renderer_smoke=passed; French snapshots generated on CPU'):
            with self.subTest(changed=changed):
                self.assertFalse(all(wayland.protocol_checks(changed).values()))

    def test_libwayland_client_trace_has_unprefixed_incoming_messages_and_new_id_arguments(self):
        native = TRACE.replace('[rs]', '').replace('<- ', '').replace('configure, (', 'configure(')
        native = native.replace('get_xdg_surface(xdg_surface@', 'get_xdg_surface(new id xdg_surface@')
        native = native.replace('get_toplevel(xdg_toplevel@', 'get_toplevel(new id xdg_toplevel@')
        self.assertTrue(all(wayland.protocol_checks(native).values()))
        self.assertFalse(all(wayland.protocol_checks(native.replace('xdg_surface@23.configure(10)',
                                                                   'xdg_surface@23.configure(11)')).values()))

    def test_component_success_cannot_replace_actual_wayland_or_cpu_surface_evidence(self):
        self.assertTrue(all(wayland.renderer_checks(self.success(), True).values()))
        for field, changed in [('returncode', 1), ('returncode', -9), ('errors', ['timeout']),
                ('output', wayland.CPU + 'renderer_smoke=passed;'),
                ('output', TRACE + wayland.CPU), ('snapshots', {}),
                ('snapshots', {'multilingual.png': 'same', 'filtered.png': 'same'}),
                ('output', TRACE + 'Skia renderer (skia backend wgpu;\nrenderer_smoke=passed;')]:
            with self.subTest(field=field, changed=changed):
                run = self.success()
                run[field] = changed
                self.assertFalse(all(wayland.renderer_checks(run, True).values()))
        run = self.success()
        run['output'] = run['output'].replace(wayland.CPU, 'Skia renderer (skia backend wgpu;')
        self.assertTrue(all(wayland.renderer_checks(run, False).values()))

    def test_inherited_display_socket_and_driver_overrides_cannot_select_another_session(self):
        parent = {name: 'inherited' for name in wayland.REMOVED}
        parent.update({'WAYLAND_DISPLAY': 'another-session', 'XDG_RUNTIME_DIR': '/another-user'})
        env = wayland.environment(parent, pathlib.Path('/private/owned-runtime'))
        self.assertTrue(all(name not in env for name in wayland.REMOVED))
        self.assertEqual(env['WAYLAND_DISPLAY'], 'qualification-wayland')
        self.assertEqual(env['XDG_RUNTIME_DIR'], '/private/owned-runtime')
        self.assertEqual(parent['DISPLAY'], 'inherited')

    def test_missing_socket_control_requires_the_native_wayland_connection_error(self):
        run = {'returncode': 1, 'errors': [],
               'output': 'Error: os error at linux/wayland/event_loop/mod.rs:89: Could not find wayland compositor'}
        self.assertTrue(wayland.missing_socket_rejected(run))
        for output in ('Error: failed to read fixture', 'Error: failed to create GPU device',
                       'Error: could not initialize backend', 'command missing',
                       run['output'] + wayland.CPU):
            self.assertFalse(wayland.missing_socket_rejected({**run, 'output': output}))
        for changed in ({'returncode': 0}, {'returncode': -9}, {'errors': ['timeout']}):
            self.assertFalse(wayland.missing_socket_rejected({**run, **changed}))

    def test_missing_tools_deadlines_and_excess_output_are_retained_as_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            missing = wayland.command(['/intentionally-absent-wayland-tool'], os.environ.copy(), root / 'missing.log')
            self.assertFalse(wayland.passed(missing))
            self.assertIsNone(missing['returncode'])
            popen = wayland.subprocess.Popen
            def ready_child(*args, **kwargs):
                child = popen(*args, **kwargs)
                try:
                    # Establish the partial-output fixture before testing the
                    # unchanged deadline; interpreter startup is not the gate.
                    deadline = time.monotonic() + 5
                    while 'started' not in (root / 'hung.log').read_text():
                        if child.poll() is not None or time.monotonic() >= deadline:
                            self.fail('hanging fixture did not become ready')
                        time.sleep(0.01)
                    return child
                except BaseException:
                    wayland.kill_group(child.pid)
                    child.wait(timeout=5)
                    raise
            with mock.patch.object(wayland.subprocess, 'Popen', ready_child):
                hung = wayland.command([sys.executable, '-c',
                    'import time; time.sleep(0.3); print("started", flush=True); time.sleep(30)'],
                    os.environ.copy(), root / 'hung.log', timeout=0.15)
            self.assertEqual(hung['returncode'], -9)
            self.assertIn('process deadline exceeded', hung['errors'])
            self.assertIn('started', hung['output'])
            large = wayland.command([sys.executable, '-c', 'print("x" * 10000)'],
                                    os.environ.copy(), root / 'large.log', limit=100)
            self.assertIn('output limit exceeded', large['errors'])
            self.assertEqual((root / 'large.log').stat().st_size, 100)

    def test_descendants_cannot_survive_a_successful_wrapper(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            pidfile = root / 'pid'
            source = ('import os, pathlib, subprocess, sys; '
                      'p=subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], '
                      'stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL); '
                      'pathlib.Path(sys.argv[1]).write_text(str(p.pid))')
            child_pid = None
            try:
                result = wayland.command([sys.executable, '-c', source, str(pidfile)],
                                         os.environ.copy(), root / 'descendant.log')
                child_pid = int(pidfile.read_text())
                self.assertEqual(result['returncode'], 0)
                self.assertIn('descendant survived exited child', result['errors'])
                # The OS may briefly retain a killed orphan as a zombie. It
                # must no longer be a running process after group retirement.
                for _ in range(100):
                    proc = pathlib.Path('/proc') / str(child_pid) / 'stat'
                    try:
                        os.kill(child_pid, 0)
                    except ProcessLookupError:
                        break
                    if proc.exists() and proc.read_text().split(') ', 1)[1].startswith('Z '):
                        break
                    time.sleep(0.01)
                else:
                    self.fail('owned descendant remains alive')
            finally:
                if child_pid is not None:
                    try:
                        os.kill(child_pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    @unittest.skipUnless(sys.platform == 'darwin', 'Darwin killpg rejects an unreaped zombie group')
    def test_output_limit_exit_race_reaps_the_owned_zombie_before_retiring_its_group(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            popen = wayland.subprocess.Popen
            children = []
            def exited_child(*args, **kwargs):
                child = popen(*args, **kwargs)
                children.append(child)
                deadline = time.monotonic() + 5
                while True:
                    with popen(['ps', '-p', str(child.pid), '-o', 'stat='],
                               stdout=wayland.subprocess.PIPE, text=True) as probe:
                        status = probe.communicate(timeout=2)[0].strip()
                    if status.startswith('Z'):
                        break
                    if time.monotonic() >= deadline:
                        self.fail('exited fixture did not reach its unreaped state')
                    time.sleep(0.01)
                real_poll, calls = child.poll, iter((None, None))
                # Hold the two observations immediately before cleanup at their
                # pre-exit value, reproducing the real output-limit/exit race.
                def delayed_poll():
                    try:
                        return next(calls)
                    except StopIteration:
                        return real_poll()
                child.poll = delayed_poll
                return child
            try:
                with mock.patch.object(wayland.subprocess, 'Popen', exited_child):
                    result = wayland.command([sys.executable, '-c', 'print("x" * 10000)'],
                        os.environ.copy(), root / 'large.log', limit=100)
                self.assertEqual(result['returncode'], 0)
                self.assertIn('output limit exceeded', result['errors'])
                self.assertFalse(wayland.group_alive(children[0].pid))
                self.assertEqual((root / 'large.log').stat().st_size, 100)
            finally:
                for child in children:
                    child.wait(timeout=5)

    def test_permission_denied_for_a_live_group_is_never_accepted_as_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            popen, killpg = wayland.subprocess.Popen, os.killpg
            children = []
            def owned_child(*args, **kwargs):
                child = popen(*args, **kwargs)
                children.append(child)
                return child
            try:
                with mock.patch.object(wayland.subprocess, 'Popen', owned_child), \
                        mock.patch.object(wayland.os, 'killpg', side_effect=PermissionError(1, 'denied')), \
                        self.assertRaises(PermissionError):
                    wayland.command([sys.executable, '-c', 'import time; time.sleep(30)'],
                        os.environ.copy(), pathlib.Path(directory) / 'live.log', timeout=0.05)
            finally:
                for child in children:
                    try:
                        killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    child.wait(timeout=5)


if __name__ == '__main__':
    unittest.main()
