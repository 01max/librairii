"""Keep VM transport, lifecycle and native-evidence failures from becoming passes."""
import copy
import importlib.util
import io
import json
import pathlib
import socket
import tarfile
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('vm', pathlib.Path(__file__).parents[1] / 'check-linux-vm.py')
vm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vm)


class LinuxVM(unittest.TestCase):
    def test_partial_monitor_line_has_a_whole_reply_deadline_and_preserves_following_lines(self):
        reader, writer = socket.socketpair()
        with reader, writer:
            writer.sendall(b'{')
            lines = vm.SocketLines(reader, time.monotonic() + 0.02)
            with self.assertRaises(TimeoutError):
                lines.readline(100)
        reader, writer = socket.socketpair()
        with reader, writer:
            writer.sendall(b'first\nsecond\n')
            lines = vm.SocketLines(reader, time.monotonic() + 1)
            self.assertEqual(lines.readline(100), b'first\n')
            self.assertEqual(lines.readline(100), b'second\n')

    def test_reboot_has_no_success_reply_and_other_commands_cannot_skip_replies(self):
        class AgentSocket:
            def __init__(self):
                self.requests = []
            def __enter__(self):
                return self
            def __exit__(self, *args):
                pass
            def connect(self, path):
                pass
            def settimeout(self, timeout):
                pass
            def sendall(self, data):
                self.requests.append(json.loads(data.lstrip(b'\xff')))
            def recv(self, size):
                request = self.requests[-1]
                if request['execute'] != 'guest-sync-delimited':
                    raise AssertionError('accepted guest-shutdown must not wait for a reply')
                return (json.dumps({'id': 'sync', 'return': request['arguments']['id']}) + '\n').encode()
        client = AgentSocket()
        with mock.patch.object(vm.socket, 'socket', return_value=client), \
                mock.patch.object(vm.select, 'select', side_effect=lambda readers, *_: (readers, [], [])):
            self.assertIsNone(vm.Monitor('/owned-agent', guest=True).call(
                'guest-shutdown', {'mode': 'reboot'}, expect_reply=False))
        self.assertEqual(client.requests[-1]['arguments'], {'mode': 'reboot'})
        for monitor, command in ((vm.Monitor('/agent', guest=True), 'guest-ping'),
                                 (vm.Monitor('/qmp'), 'quit')):
            with self.assertRaises(ValueError):
                monitor.call(command, expect_reply=False)

    def test_rpc_requires_the_matching_id_and_successful_result(self):
        stream = io.BytesIO(b'{"event":"RESET"}\n{"id":"other","return":true}\n'
                            b'\xff{"id":"owned","return":{"enabled":true}}\n')
        self.assertEqual(vm.receive(stream, 'owned', time.monotonic() + 1), {'enabled': True})
        for value in (b'', b'{"id":"owned","error":{"class":"failed"}}\n',
                      b'{"id":"owned"}\n', b'x' * (vm.LIMIT + 1)):
            with self.subTest(value=value[:60]), self.assertRaises(ValueError):
                vm.receive(io.BytesIO(value), 'owned', time.monotonic() + 1)

    def test_guest_failure_signal_truncation_and_invalid_payload_are_rejected(self):
        self.assertEqual(vm.guest_result({'exitcode': 0, 'out-data': 'b2s='}), 'ok')
        for change in ({'exitcode': 1}, {'signal': 9}, {'out-truncated': True},
                       {'err-truncated': True}, {'out-data': 'not base64'}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                vm.guest_result({'exitcode': 0, **change})

    def test_guest_archive_cannot_escape_alias_duplicate_or_exceed_file_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            for variant in ('complete', 'traversal', 'symlink', 'duplicate', 'oversized'):
                with self.subTest(variant=variant):
                    path = root / (variant + '.tar.gz')
                    with tarfile.open(path, 'w:gz') as stream:
                        names = ['evidence/report.json'] * (2 if variant == 'duplicate' else 1)
                        for name in names:
                            member = tarfile.TarInfo('../escaped' if variant == 'traversal' else name)
                            data = b'{}' if variant != 'oversized' else b'x' * (vm.LIMIT + 1)
                            if variant == 'symlink':
                                member.type, member.linkname = tarfile.SYMTYPE, '/tmp/outside'
                                stream.addfile(member)
                            else:
                                member.size = len(data)
                                stream.addfile(member, io.BytesIO(data))
                    if variant == 'complete':
                        vm.unpack_evidence(path, root / variant)
                        self.assertEqual((root / variant / 'evidence/report.json').read_bytes(), b'{}')
                    else:
                        with self.assertRaises(ValueError):
                            vm.unpack_evidence(path, root / variant)
            self.assertFalse((root / 'escaped').exists())

    def fixture(self, root):
        retained = dict.fromkeys(('archive', 'database', 'profile', 'journal'), 'retained-hash')
        facts = {'boot_id': 'boot-one', 'architecture': 'x86_64', 'interfaces': ['lo'],
                 'tools': dict.fromkeys(vm.TOOLS), 'prohibited_packages': [], 'cwd': '/tmp',
                 'os_release': 'VERSION_ID="24.04"\n', 'application_uid': 1000,
                 'binary_sha256': 'expected', 'retained_files': retained}
        before = {'phase': 'before-reboot', 'errors': [], 'facts': facts}
        after = {'phase': 'after-reboot', 'errors': [], 'facts': {**facts, 'boot_id': 'boot-two'},
                 'removed': True, 'reinstalled_sha256': 'expected', 'before_removal': retained,
                 'after_removal': retained, 'after_reinstall': retained}
        cases = {}
        for name in vm.native.CASES:
            required = {'owned_shutdown', 'fixture_removed', 'native_focus_restored_and_search_input'}
            required |= ({'early_settings_delivered'} if name == 'early-settings' else
                {'actual_file_manager_window_path', 'os_settings_accelerator'} if name == 'reveal' else
                {'real_modal_dialog', 'one_real_portal_call', 'parent_identified', 'parent_associated',
                 'no_parent_rejection', 'actual_picker_result', 'os_settings_accelerator'})
            cases[name] = {'returncode': 0, 'checks': dict.fromkeys(required, True)}
        for session in ('x11', 'wayland'):
            (root / session).mkdir()
            (root / session / 'native-session.json').write_text(json.dumps({'native_integration': 'pass',
                'binary_sha256': 'expected', 'errors': [], 'cases': cases}))
        (root / 'wayland-rendering').mkdir()
        (root / 'wayland-rendering/wayland.json').write_text(json.dumps({'automated_wayland': 'pass',
            'binary_sha256': 'expected', 'errors': [], 'checks': dict.fromkeys(vm.WAYLAND_CHECKS, True)}))
        (root / 'reinstalled.log').write_text('quit_smoke=passed;')
        return before, after

    def test_lifecycle_needs_reboot_offline_identity_no_tools_and_preserved_canaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            before, after = self.fixture(root)
            def verdict(first=before, last=after, kvm=None, offline=True):
                (root / 'before-reboot.json').write_text(json.dumps(first))
                (root / 'after-reboot.json').write_text(json.dumps(last))
                return vm.analyze(root, {'binary_sha256': 'expected'},
                    kvm if kvm is not None else {'enabled': True, 'present': True}, offline)
            self.assertTrue(all(verdict().values()))
            for field, value in [('boot_id', 'boot-one'), ('binary_sha256', 'changed'),
                    ('interfaces', ['lo', 'ens3']), ('application_uid', 0), ('cwd', '/source'),
                    ('tools', {}), ('tools', {**after['facts']['tools'], 'cargo': '/usr/bin/cargo'}),
                    ('retained_files', {}), ('prohibited_packages', ['rustc 1.99.1'])]:
                changed = copy.deepcopy(after)
                changed['facts'][field] = value
                with self.subTest(field=field, value=value):
                    self.assertFalse(all(verdict(last=changed).values()))
            for field, value in [('removed', False), ('after_removal', {}), ('errors', ['failed']),
                                 ('reinstalled_sha256', 'changed')]:
                self.assertFalse(all(verdict(last={**after, field: value}).values()))
            self.assertFalse(all(verdict(kvm={'enabled': False, 'present': True}).values()))
            self.assertFalse(all(verdict(offline=False).values()))
            path = root / 'wayland-rendering/wayland.json'
            report = json.loads(path.read_text())
            for check in vm.WAYLAND_CHECKS:
                path.write_text(json.dumps({**report, 'checks': {k: v for k, v in report['checks'].items() if k != check}}))
                self.assertFalse(all(verdict().values()))


if __name__ == '__main__':
    unittest.main()
