"""Owned VM observer; never installed with the application or run on a user's host."""
import hashlib
import json
import os
import pathlib
import pwd
import re
import shutil
import subprocess
import sys
import tarfile

ROOT = pathlib.Path('/var/lib/qualification')
EVIDENCE = ROOT / 'evidence'
PACKAGE = pathlib.Path('/opt/librairii')
CANARY = pathlib.Path('/home/qualification/retained-data')
LIMIT = 4 * 1024 * 1024
UNITS = []


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def retained():
    return {str(p.relative_to(CANARY)): sha(p) for p in CANARY.rglob('*') if p.is_file()}


def install(identity):
    archive = ROOT / 'package.tar.gz'
    if sha(archive) != identity['archive_sha256'] or PACKAGE.exists():
        raise ValueError('archive mismatch or existing installation')
    stage = ROOT / 'staging'
    stage.mkdir()
    # Host archive_identity has authenticated the exact regular-file inventory.
    with tarfile.open(archive, 'r:gz') as stream:
        stream.extractall(stage, filter='data')
    source = stage / identity['package_name']
    source.rename(PACKAGE)
    stage.rmdir()
    if sha(PACKAGE / 'qualification-desktop') != identity['binary_sha256']:
        raise ValueError('installed executable differs')


def user_run(name, args, identity, timeout=600):
    destination = EVIDENCE / name
    destination.mkdir()
    user = pwd.getpwnam('qualification')
    os.chown(destination, user.pw_uid, user.pw_gid)
    link = pathlib.Path('/evidence')
    if link.is_symlink():
        link.unlink()
    elif link.exists():
        raise ValueError('unexpected evidence path')
    link.symlink_to(destination)
    # The earlier private desktop is fully retired before another starts.
    shutil.rmtree('/tmp/native-runtime', ignore_errors=True)
    unit = 'librairii-qualification-' + name + '.service'
    cgroup = pathlib.Path('/sys/fs/cgroup/system.slice') / unit
    argv = ['/usr/bin/systemd-run', '--quiet', '--wait', '--pipe', '--collect', '--unit=' + unit,
            '--property=Type=exec', '--property=User=qualification', '--property=Group=qualification',
            '--property=Slice=system.slice', '--property=WorkingDirectory=/tmp',
            '--property=KillMode=control-group', '--property=TimeoutStopSec=3s',
            '--property=RuntimeMaxSec=' + str(timeout - 15) + 's', '/usr/bin/env', '-i',
            'PATH=/usr/bin:/bin', 'HOME=/home/qualification', 'TMPDIR=/tmp', 'LC_ALL=C.UTF-8',
            'EXPECTED_BINARY_SHA256=' + identity['binary_sha256'], 'SLINT_BACKEND=winit-skia-software',
            '/usr/bin/dbus-run-session', '--', *args]
    # The observer's bounded command runner handles both output and group cleanup.
    import importlib.util
    spec = importlib.util.spec_from_file_location('wayland', '/opt/qualification/check-wayland.py')
    wayland = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(wayland)
    record = {'unit': unit, 'cgroup': str(cgroup), 'command_returncode': None,
              'command_errors': [], 'cgroup_removed': False, 'stop_returncode': None}
    UNITS.append(record)
    try:
        result = wayland.command(argv, os.environ.copy(), EVIDENCE / (name + '.log'), timeout=timeout)
        record['command_returncode'] = result['returncode']
        record['command_errors'] = result['errors']
        if not wayland.passed(result):
            raise RuntimeError(name + ' failed: ' + repr(result['errors']))
    finally:
        # A service owns even double-forked/private D-Bus desktop helpers. Stop
        # this exact owned unit on failures too; no process-name/global cleanup.
        if cgroup.exists():
            stop = subprocess.run(['/usr/bin/systemctl', 'stop', unit], capture_output=True, timeout=8)
            record['stop_returncode'] = stop.returncode
        record['cgroup_removed'] = not cgroup.exists()
        if not record['cgroup_removed'] or record['stop_returncode'] not in (None, 0):
            raise RuntimeError(name + ' owned desktop service did not retire')


def facts(identity):
    interfaces = [p.name for p in pathlib.Path('/sys/class/net').iterdir()]
    tools = {tool: shutil.which(tool) for tool in ('rustc', 'cargo', 'go', 'node', 'npm', 'gcc', 'g++', 'clang')}
    packages = subprocess.check_output(['dpkg-query', '-W', '-f=${Package} ${Version}\n'], text=True)
    (EVIDENCE / 'packages.txt').write_text(packages)
    prohibited = [line for line in packages.splitlines() if re.match(
        r'^(?:rustc|cargo|golang\S*|nodejs|npm|gcc(?:-[0-9]+)?|g\+\+(?:-[0-9]+)?|clang(?:-[0-9]+)?|chromium\S*|firefox\S*|libwebkit\S*) ', line)]
    if interfaces != ['lo'] or any(tools.values()) or prohibited or os.uname().machine != 'x86_64':
        raise ValueError('guest must be native x86-64, offline and without development/browser packages')
    return {'boot_id': pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
            'architecture': os.uname().machine, 'interfaces': interfaces, 'tools': tools,
            'prohibited_packages': prohibited, 'os_release': pathlib.Path('/etc/os-release').read_text(),
            'binary_sha256': sha(PACKAGE / 'qualification-desktop'), 'retained_files': retained(),
            'application_uid': pwd.getpwnam('qualification').pw_uid, 'cwd': os.getcwd()}


def main(phase):
    os.chdir('/tmp')
    identity = json.loads((ROOT / 'identity.json').read_text())
    result = {'phase': phase, 'errors': []}
    try:
        if phase == 'before-reboot':
            install(identity)
            CANARY.mkdir()
            for name, content in {'archive-Été.zip': b'authored retained archive canary',
                                  'library.sqlite': b'authored unrelated database canary',
                                  'profile.json': b'{"fixture":"preserve"}',
                                  'recovery.json': b'{"fixture":"pending"}'}.items():
                (CANARY / name).write_bytes(content)
            result['facts'] = facts(identity)
            user_run('x11', ['/usr/bin/python3', '-I', '-B', '/opt/qualification/drive.py', '--session', 'x11'], identity)
        elif phase == 'after-reboot':
            result['facts'] = facts(identity)
            user_run('wayland', ['/usr/bin/python3', '-I', '-B', '/opt/qualification/drive.py', '--session', 'wayland'], identity)
            user_run('wayland-runtime', ['/usr/bin/python3', '-I', '-B', '/opt/qualification/check-wayland.py',
                     '--package', str(PACKAGE), '--output', str(EVIDENCE / 'wayland-rendering')], identity)
            result['before_removal'] = retained()
            shutil.rmtree(PACKAGE)
            result['removed'] = not PACKAGE.exists()
            result['after_removal'] = retained()
            install(identity)
            result['reinstalled_sha256'] = sha(PACKAGE / 'qualification-desktop')
            user_run('reinstalled', ['/usr/bin/xvfb-run', '-a', str(PACKAGE / 'qualification-desktop'), '--quit-smoke'], identity)
            result['after_reinstall'] = retained()
        else:
            raise ValueError('unknown lifecycle phase')
    except Exception as error:
        result['errors'].append(type(error).__name__ + ': ' + str(error))
    finally:
        result['desktop_units'] = UNITS
        (EVIDENCE / (phase + '.json')).write_text(json.dumps(result, indent=2) + '\n')
        with tarfile.open(ROOT / 'results.tar.gz', 'w:gz') as stream:
            stream.add(EVIDENCE, arcname='evidence')
    print(json.dumps(result))
    return int(bool(result['errors']))


if __name__ == '__main__':
    raise SystemExit(main(sys.argv[1]))
