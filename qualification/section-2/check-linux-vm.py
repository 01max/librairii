#!/usr/bin/env python3
"""Exercise an authenticated package in an owned, offline KVM guest; no shared folders."""
import argparse
import base64
import contextlib
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import select
import shutil
import signal
import socket
import subprocess
import tarfile
import tempfile
import time
import uuid

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('vm_native', HERE / 'check-native-linux.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
LIMIT = native.wayland.LIMIT
IMAGE_URL = 'https://cloud-images.ubuntu.com/releases/noble/release-20261017/ubuntu-24.04-server-cloudimg-amd64.img'
IMAGE_SHA = '04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94'
PACKAGES = ('qemu-guest-agent libfontconfig1 libfreetype6 libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2 '
            'libegl1 libgl1 libvulkan1 mesa-vulkan-drivers fonts-noto-cjk python3-pyatspi python3-dbus '
            'python3-gi dbus-x11 at-spi2-core xdg-desktop-portal xdg-desktop-portal-gtk xdg-utils file '
            'util-linux xfconf thunar xvfb xauth openbox xdotool x11-utils scrot sway wtype grim procps weston wayland-utils').split()


TOOLS = {'rustc', 'cargo', 'go', 'node', 'npm', 'gcc', 'g++', 'clang'}
WAYLAND_CHECKS = {'versions_recorded', 'compositor_protocols', 'normal_renderer', 'cpu_renderer',
    'edit', 'focus', 'quit', 'thumbnail', 'no_x11_fallback', 'compositor_survived_cases',
    'compositor_closed', 'runtime_socket_removed_by_compositor', 'temporary_runtime_retired'}


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def receive(stream, expected, deadline):
    """Ignore asynchronous events, never accept a different RPC's result."""
    while time.monotonic() < deadline:
        line = stream.readline(LIMIT + 1)
        if not line or len(line) > LIMIT:
            raise ValueError('missing or oversized monitor reply')
        reply = json.loads(line.lstrip(b'\xff'))
        if reply.get('id') != expected:
            continue
        if 'error' in reply or 'return' not in reply:
            raise ValueError('monitor command failed: ' + repr(reply))
        return reply['return']
    raise TimeoutError('monitor reply deadline')


class SocketLines:
    """A whole-line deadline also bounds a peer that keeps trickling bytes."""
    def __init__(self, client, deadline):
        self.client, self.deadline, self.buffer = client, deadline, bytearray()

    def readline(self, limit):
        while b'\n' not in self.buffer:
            if len(self.buffer) >= limit:
                raise ValueError('oversized monitor reply')
            remaining = self.deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.client], [], [], remaining)[0]:
                raise TimeoutError('whole monitor reply deadline')
            chunk = self.client.recv(min(65536, limit - len(self.buffer)))
            if not chunk:
                raise ValueError('monitor disconnected before complete reply')
            self.buffer.extend(chunk)
        line, following = self.buffer.split(b'\n', 1)
        self.buffer = bytearray(following)
        if len(line) + 1 > limit:
            raise ValueError('oversized monitor reply')
        return bytes(line) + b'\n'


class Monitor:
    def __init__(self, path, guest=False):
        self.path, self.guest = str(path), guest

    def call(self, command, arguments=None, timeout=10, expect_reply=True):
        if not expect_reply and not (self.guest and command == 'guest-shutdown'):
            raise ValueError('only guest shutdown has a no-success-reply protocol')
        deadline = time.monotonic() + timeout
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(timeout)
            client.connect(self.path)
            stream = SocketLines(client, deadline)
            if self.guest:
                token = uuid.uuid4().int & ((1 << 53) - 1)
                client.sendall(b'\xff' + json.dumps({'execute': 'guest-sync-delimited',
                    'arguments': {'id': token}, 'id': 'sync'}).encode() + b'\n')
                if receive(stream, 'sync', deadline) != token:
                    raise ValueError('guest-agent synchronization mismatch')
            else:
                greeting = stream.readline(LIMIT + 1)
                if len(greeting) > LIMIT or 'QMP' not in json.loads(greeting):
                    raise ValueError('invalid QMP greeting')
                client.sendall(b'{"execute":"qmp_capabilities","id":"caps"}\n')
                receive(stream, 'caps', deadline)
            request = {'execute': command, 'id': 'request', 'arguments': arguments or {}}
            client.sendall(json.dumps(request).encode() + b'\n')
            if not expect_reply:
                # Success intentionally has no reply. A changed boot ID and
                # successful reconnection below are the required reboot proof.
                return None
            return receive(stream, 'request', deadline)


def guest_result(result):
    if (result.get('exitcode') != 0 or result.get('signal') is not None
            or result.get('out-truncated') or result.get('err-truncated')):
        detail = base64.b64decode(result.get('out-data', ''), validate=True).decode('utf-8', errors='replace')
        raise ValueError('guest command failed: exit=' + str(result.get('exitcode')) + '; ' + detail[:4000])
    return base64.b64decode(result.get('out-data', ''), validate=True).decode('utf-8')


@contextlib.contextmanager
def interruption_cleanup():
    def interrupted(number, frame):
        raise RuntimeError('VM qualification interrupted by signal ' + str(number))
    previous = {number: signal.signal(number, interrupted) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        yield
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)


def unpack_evidence(archive, destination):
    with tarfile.open(archive, 'r:gz') as stream:
        members = stream.getmembers()
        if len(members) > 1024 or sum(m.size for m in members) > 128 * 1024 * 1024:
            raise ValueError('guest evidence inventory limit')
        seen = set()
        for member in members:
            path = pathlib.PurePosixPath(member.name)
            if (path.is_absolute() or '..' in path.parts or not path.parts or path.parts[0] != 'evidence'
                    or str(path) != member.name.rstrip('/') or member.name in seen
                    or not (member.isdir() or member.isfile()) or member.size > LIMIT):
                raise ValueError('unsafe or oversized guest evidence')
            seen.add(member.name)
        stream.extractall(destination, members=members, filter='data')


def analyze(root, identity, kvm, network_removed):
    before = json.loads((root / 'before-reboot.json').read_text())
    after = json.loads((root / 'after-reboot.json').read_text())
    a, b = before.get('facts', {}), after.get('facts', {})
    checks = {'kvm_hardware_acceleration': kvm == {'enabled': True, 'present': True},
              'network_device_removed_before_app': network_removed,
              'both_lifecycle_phases_complete': before.get('errors') == [] and after.get('errors') == []
                  and before.get('phase') == 'before-reboot' and after.get('phase') == 'after-reboot',
              'new_boot_same_installed_binary': bool(a.get('boot_id')) and bool(b.get('boot_id'))
                  and a['boot_id'] != b['boot_id']
                  and a.get('binary_sha256') == b.get('binary_sha256') == identity['binary_sha256'],
              'native_offline_fresh_user': all(f.get('architecture') == 'x86_64' and f.get('interfaces') == ['lo']
                  and f.get('application_uid', 0) > 0 and f.get('cwd') == '/tmp'
                  and 'VERSION_ID="24.04"' in f.get('os_release', '') for f in (a, b)),
              'no_development_or_browser_packages': all(set(f.get('tools', {})) == TOOLS and not any(f['tools'].values())
                  and f.get('prohibited_packages') == [] for f in (a, b)),
              'remove_reinstall_same_verified_package': after.get('removed') is True
                  and after.get('reinstalled_sha256') == identity['binary_sha256'],
              'separate_canaries_preserved': len(a.get('retained_files', {})) == 4
                  and all(value == a['retained_files'] for value in (b.get('retained_files'),
                      after.get('before_removal'), after.get('after_removal'), after.get('after_reinstall'))),
              'reinstalled_owned_shutdown': 'quit_smoke=passed;' in (root / 'reinstalled.log').read_text()}
    expected = {'before-reboot': {'x11'}, 'after-reboot': {'wayland', 'wayland-runtime', 'reinstalled'}}
    checks['all_owned_desktop_service_groups_retired'] = all(
        {u.get('unit') for u in phase.get('desktop_units', [])} ==
        {'librairii-qualification-' + name + '.service' for name in expected[phase['phase']]}
        and len(phase['desktop_units']) == len(expected[phase['phase']])
        and all(u.get('command_returncode') == 0 and u.get('command_errors') == []
            and u.get('cgroup_removed') is True and u.get('stop_returncode') in (None, 0)
            and u.get('cgroup') == '/sys/fs/cgroup/system.slice/' + u['unit']
            for u in phase['desktop_units']) for phase in (before, after))
    for session in ('x11', 'wayland'):
        report = json.loads((root / session / 'native-session.json').read_text())
        checks[session + '_all_native_cases'] = native.session_passed(report, identity['binary_sha256'])
    wayland = json.loads((root / 'wayland-rendering/wayland.json').read_text())
    checks['installed_wayland_normal_cpu_gui'] = (wayland.get('automated_wayland') == 'pass'
        and not wayland.get('errors') and wayland.get('binary_sha256') == identity['binary_sha256']
        and WAYLAND_CHECKS <= wayland.get('checks', {}).keys() and all(wayland['checks'].values()))
    return checks


def inspect(package, archive, output):
    if (platform.system(), platform.machine()) != ('Linux', 'x86_64'):
        raise ValueError('VM qualification needs a native Linux x86-64 host')
    manifest = json.loads((package / 'manifest.json').read_text())
    if manifest['target'] != 'x86_64-unknown-linux-gnu':
        raise ValueError('wrong VM package target')
    native.wayland.runtime.verified_binary(package, manifest)
    native.clean.resources.package_bytes(package, manifest)
    archive_id = native.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    report = {'schema': 1, 'source': manifest['source'], 'binary_sha256': manifest['binary_sha256'],
        'archive': archive_id, 'base_image': {'url': IMAGE_URL, 'sha256': IMAGE_SHA},
        'checks': {}, 'commands': {}, 'errors': [], 'isolated_linux_vm': 'fail',
        'full_product_upgrade_qualified': False, 'physical_input_ime_qualified': False,
        'macos_clean_host_qualified': False, 'toolkit_selected': False,
        'scope': 'Fresh Ubuntu24.04 KVM guest, declared runtime/observer packages, no app source/development toolchains/shared folders; offline installed GUI, reboot, removal/reinstallation and separate canary preservation. Not product-data upgrade or physical desktop qualification.'}
    process = None
    overall_deadline = time.monotonic() + 1800
    with interruption_cleanup(), tempfile.TemporaryDirectory(prefix='librairii-vm-', dir='/tmp') as temporary:
        work = pathlib.Path(temporary)
        qmp, agent = Monitor(work / 'qmp'), Monitor(work / 'agent', guest=True)

        def run(name, argv, timeout=30):
            result = native.wayland.command(argv, os.environ.copy(), output / (name + '.log'),
                timeout=min(timeout, max(0, overall_deadline - time.monotonic())))
            report['commands'][name] = result
            if not native.wayland.passed(result):
                raise RuntimeError(name + ' failed')
            return result

        def alive():
            if time.monotonic() >= overall_deadline:
                raise TimeoutError('overall VM qualification deadline')
            if process is not None and process.poll() is not None:
                raise RuntimeError('owned VM exited prematurely')
            for name in ('qemu.log', 'serial.log'):
                path = output / name
                if path.exists() and path.stat().st_size > LIMIT:
                    raise ValueError('VM console output limit')

        def guest_exec(path, args, timeout=30):
            pid = agent.call('guest-exec', {'path': path, 'arg': args, 'capture-output': True})['pid']
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                alive()
                result = agent.call('guest-exec-status', {'pid': pid})
                if result.get('exited'):
                    return guest_result(result)
                time.sleep(0.2)
            raise TimeoutError('guest command deadline')

        def wait_guest(predicate, timeout):
            deadline = time.monotonic() + timeout
            last = ''
            while time.monotonic() < deadline:
                alive()
                try:
                    result = predicate()
                    if result:
                        return result
                except (OSError, ValueError, TimeoutError) as error:
                    last = str(error)
                time.sleep(1)
            raise TimeoutError('VM readiness deadline: ' + last)

        def collect(name):
            handle = agent.call('guest-file-open', {'path': '/var/lib/qualification/results.tar.gz', 'mode': 'rb'})
            path = output / (name + '.tar.gz')
            size = 0
            try:
                with path.open('xb') as stream:
                    while True:
                        reply = agent.call('guest-file-read', {'handle': handle, 'count': 262144})
                        data = base64.b64decode(reply.get('buf-b64', ''), validate=True)
                        size += len(data)
                        if len(data) != reply.get('count') or size > 64 * 1024 * 1024:
                            raise ValueError('guest evidence transfer limit/mismatch')
                        stream.write(data)
                        if reply.get('eof'):
                            break
                        if not data:
                            raise ValueError('guest evidence transfer made no progress')
            finally:
                agent.call('guest-file-close', {'handle': handle})
            destination = output / name
            unpack_evidence(path, destination)
            return destination / 'evidence'

        try:
            if not os.access('/dev/kvm', os.R_OK | os.W_OK):
                raise ValueError('KVM unavailable; emulation is not accepted as native qualification')
            base = work / 'base.img'
            run('download', ['curl', '--fail', '--location', '--max-time', '600', '--output', str(base), IMAGE_URL], 610)
            if digest(base) != IMAGE_SHA:
                raise ValueError('Ubuntu cloud-image digest mismatch')
            run('overlay', ['qemu-img', 'create', '-f', 'qcow2', '-F', 'qcow2', '-b', str(base), str(work / 'guest.qcow2'), '12G'])
            payload = work / 'payload'
            payload.mkdir()
            for source, name in [(HERE / 'linux-vm/guest.py', 'guest.py'), (HERE / 'linux-vm/provision.sh', 'provision.sh'),
                    (HERE / 'native-linux/drive.py', 'drive.py'), (HERE / 'check-wayland.py', 'check-wayland.py'),
                    (HERE / 'inspect-runtime.py', 'inspect-runtime.py'), (archive, 'package.tar.gz')]:
                shutil.copyfile(source, payload / name)
            identity = {'package_name': package.name, 'binary_sha256': manifest['binary_sha256'],
                        'archive_sha256': archive_id['sha256']}
            (payload / 'identity.json').write_text(json.dumps(identity))
            run('payload', ['genisoimage', '-quiet', '-output', str(work / 'payload.iso'), '-volid', 'LIBRAIRII_DATA', '-joliet', '-rock', str(payload)])
            (work / 'meta-data').write_text('instance-id: librairii-' + uuid.uuid4().hex + '\nlocal-hostname: qualification\n')
            config = {'users': [{'name': 'qualification', 'lock_passwd': True, 'shell': '/bin/bash'}],
                'ssh_pwauth': False, 'disable_root': True, 'package_update': True, 'packages': PACKAGES,
                'runcmd': [['/bin/bash', '-c', 'mkdir -p /mnt/qualification; mount -o ro /dev/disk/by-label/LIBRAIRII_DATA /mnt/qualification; /bin/bash /mnt/qualification/provision.sh']]}
            # JSON is valid YAML; all seed content is owned, without host credentials.
            (work / 'user-data').write_text('#cloud-config\n' + json.dumps(config))
            run('seed', ['cloud-localds', str(work / 'seed.iso'), str(work / 'user-data'), str(work / 'meta-data')])
            argv = ['qemu-system-x86_64', '-accel', 'kvm', '-cpu', 'host', '-smp', '2', '-m', '3072',
                '-display', 'none', '-serial', 'file:' + str(output / 'serial.log'), '-monitor', 'none',
                '-qmp', 'unix:' + str(work / 'qmp') + ',server=on,wait=off',
                '-chardev', 'socket,path=' + str(work / 'agent') + ',server=on,wait=off,id=agent',
                '-device', 'virtio-serial-pci', '-device', 'virtserialport,chardev=agent,name=org.qemu.guest_agent.0',
                '-drive', 'file=' + str(work / 'guest.qcow2') + ',format=qcow2,if=virtio',
                '-drive', 'file=' + str(work / 'seed.iso') + ',format=raw,media=cdrom,readonly=on,index=0',
                '-drive', 'file=' + str(work / 'payload.iso') + ',format=raw,media=cdrom,readonly=on,index=1',
                '-netdev', 'user,id=net0', '-device', 'virtio-net-pci,netdev=net0,id=nic0']
            report['vm_argv'] = argv
            with (output / 'qemu.log').open('xb') as log:
                process = subprocess.Popen(argv, stdout=log, stderr=log, start_new_session=True)
            report['kvm'] = wait_guest(lambda: qmp.call('query-kvm'), 20)
            if report['kvm'] != {'enabled': True, 'present': True}:
                raise ValueError('VM is not hardware accelerated')

            def provisioned():
                status = guest_exec('/bin/sh', ['-c', 'if test -f /var/lib/qualification/provision-status; then cat /var/lib/qualification/provision-status; elif test -f /var/lib/cloud/instance/boot-finished; then printf failed; else printf pending; fi'])
                if status == 'failed':
                    raise RuntimeError('guest provisioning failed; inspect serial log')
                return status == 'ready'

            wait_guest(provisioned, 900)
            # Remove the entire NIC and backend before the first app starts.
            qmp.call('device_del', {'id': 'nic0'})
            wait_guest(lambda: guest_exec('/bin/sh', ['-c', 'ls /sys/class/net']).strip() == 'lo', 30)
            qmp.call('netdev_del', {'id': 'net0'})
            report['offline_pci'] = qmp.call('query-pci')
            report['network_removed'] = 'nic0' not in json.dumps(report['offline_pci'])
            if not report['network_removed']:
                raise ValueError('NIC still present before app launch')
            try:
                report['before_reboot'] = json.loads(guest_exec('/usr/bin/python3', ['-I', '-B', '/opt/qualification/guest.py', 'before-reboot'], 600))
            finally:
                collect('before')
            old_boot = report['before_reboot']['facts']['boot_id']
            agent.call('guest-shutdown', {'mode': 'reboot'}, expect_reply=False)
            wait_guest(lambda: (value := guest_exec('/bin/cat', ['/proc/sys/kernel/random/boot_id']).strip()) != old_boot and value, 180)
            try:
                report['after_reboot'] = json.loads(guest_exec('/usr/bin/python3', ['-I', '-B', '/opt/qualification/guest.py', 'after-reboot'], 900))
            finally:
                evidence = collect('after')
            report['checks'] = analyze(evidence, identity, report['kvm'], report['network_removed'])
            report['runtime_files'] = {str(p.relative_to(evidence)): digest(p) for p in evidence.rglob('*') if p.is_file()}
        except (OSError, ValueError, KeyError, RuntimeError, TimeoutError, tarfile.TarError) as error:
            report['errors'].append(type(error).__name__ + ': ' + str(error))
        finally:
            if process is not None:
                try:
                    qmp.call('quit', timeout=2)
                except (OSError, ValueError, TimeoutError):
                    native.wayland.kill_group(process.pid)
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    native.wayland.kill_group(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
                report['checks']['owned_vm_reaped'] = process.poll() is not None
            for name in ('serial.log', 'qemu.log'):
                path = output / name
                if path.exists() and path.stat().st_size > LIMIT:
                    report['errors'].append(name + ' output exceeded limit')
                    with path.open('r+b') as stream:
                        stream.truncate(LIMIT)
    report['checks']['owned_disks_removed'] = not work.exists()
    if report['checks'].get('owned_vm_reaped') and all(report['checks'].values()) and not report['errors']:
        report['isolated_linux_vm'] = 'pass'
    (output / 'linux-vm.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True, type=pathlib.Path)
    parser.add_argument('--archive', required=True, type=pathlib.Path)
    parser.add_argument('--output', required=True, type=pathlib.Path)
    args = parser.parse_args()
    result = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.output.resolve())
    print(json.dumps({'checks': result['checks'], 'errors': result['errors']}, indent=2))
    raise SystemExit(0 if result['isolated_linux_vm'] == 'pass' else 1)
