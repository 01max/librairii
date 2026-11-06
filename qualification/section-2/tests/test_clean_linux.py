"""A successful app exit alone does not prove a clean/offline package runtime."""
import importlib.util
import pathlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('clean_linux', pathlib.Path(__file__).parents[1] / 'check-clean-linux.py')
clean = importlib.util.module_from_spec(spec)
spec.loader.exec_module(clean)

TRACE = ('[1.0] -> xdg_wm_base@2.get_xdg_surface(new id xdg_surface@23, wl_surface@21)\n'
         '[1.1] -> xdg_surface@23.get_toplevel(new id xdg_toplevel@24)\n'
         '[1.2] xdg_surface@23.configure(10)\n[1.3] -> xdg_surface@23.ack_configure(10)\n'
         '[1.4] -> wl_surface@21.attach(wl_buffer@50, 0, 0)\n[1.5] -> wl_surface@21.commit()\n')


class CleanLinux(unittest.TestCase):
    def fixture(self, root):
        (root / 'runtime-facts.txt').write_text('development_tools=absent\narchitecture=x86_64\nuid=1001\ncwd=/tmp\nclean_runtime=passed\n')
        (root / 'linked-libraries.txt').write_text('libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6\n')
        (root / 'packages.txt').write_text('libc6 2.39\nweston 13.0.0\n')
        for session in ('x11', 'wayland'):
            for renderer in ('normal', 'cpu'):
                name = session + '-' + renderer
                (root / name).mkdir()
                (root / name / 'multilingual.png').write_bytes(b'first')
                (root / name / 'filtered.png').write_bytes(b'changed')
                (root / (name + '.exit')).write_text('0\n')
                (root / (name + '.log')).write_text(TRACE + clean.wayland.CPU + '\nrenderer_smoke=passed;')
            (root / (session + '-relaunch.exit')).write_text('0\n')
            (root / (session + '-relaunch.log')).write_text('quit_smoke=passed;')
        return {'HostConfig': {'NetworkMode': 'none', 'ReadonlyRootfs': True, 'Privileged': False},
                'Mounts': [{'Destination': '/opt/librairii', 'RW': False}, {'Destination': '/evidence', 'RW': True}]}

    def test_runtime_is_offline_nonroot_without_development_tools_or_source_mounts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            info = self.fixture(root)
            self.assertTrue(all(clean.analyze(root, info)[0].values()))
            for key, value in [('NetworkMode', 'bridge'), ('ReadonlyRootfs', False), ('Privileged', True)]:
                changed = {**info, 'HostConfig': {**info['HostConfig'], key: value}}
                self.assertFalse(all(clean.analyze(root, changed)[0].values()))
            changed = {**info, 'Mounts': info['Mounts'] + [{'Destination': '/source', 'RW': False}]}
            self.assertFalse(all(clean.analyze(root, changed)[0].values()))
            facts = root / 'runtime-facts.txt'
            facts.write_text(facts.read_text().replace('uid=1001', 'uid=0'))
            self.assertFalse(all(clean.analyze(root, info)[0].values()))

    def test_development_packages_and_unresolved_libraries_fail_even_after_successful_render(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            info = self.fixture(root)
            for package in ('python3', 'libfontconfig1-dev', 'nodejs', 'rustc', 'libwebkit2gtk-4.1-0'):
                (root / 'packages.txt').write_text('libc6 2.39\n' + package + ' 1\n')
                self.assertFalse(clean.analyze(root, info)[0]['no_development_or_browser_packages'])
            (root / 'linked-libraries.txt').write_text('libmissing.so => not found\n')
            self.assertFalse(clean.analyze(root, info)[0]['all_linked_libraries_resolve'])

    def test_all_sessions_frames_cpu_surface_protocol_and_relaunch_are_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            info = self.fixture(root)
            log = root / 'wayland-cpu.log'
            original = log.read_text()
            for text in (original.replace('ack_configure', 'unrelated'), original.replace('renderer_smoke=passed;', ''),
                         original.replace(clean.wayland.CPU, 'Skia renderer (skia backend wgpu;')):
                log.write_text(text)
                self.assertFalse(clean.analyze(root, info)[0]['wayland-cpu'])
            log.write_text(original)
            (root / 'wayland-relaunch.log').write_text('exited without owned cleanup')
            self.assertFalse(clean.analyze(root, info)[0]['wayland_relaunch_owned_shutdown'])
            (root / 'x11-normal.exit').write_text('1\n')
            self.assertFalse(clean.analyze(root, info)[0]['x11-normal'])

    def test_oversized_runtime_log_is_refused_before_reading_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            info = self.fixture(root)
            (root / 'weston.log').write_bytes(b'x' * (clean.wayland.LIMIT + 1))
            with self.assertRaisesRegex(ValueError, 'evidence size/count limit'):
                clean.analyze(root, info)
