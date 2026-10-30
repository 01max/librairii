"""Install exact candidate bytes into a fresh owned qualification directory."""
from contextlib import contextmanager
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile

from .files import digest, fresh_output, inventory, regular, write_json
from .identity import architecture, runtime_host, verify_embedded
from . import linkage


def artifact_record(directory, source, target):
    directory = Path(directory).resolve(strict=True)
    regular(directory / 'artifact.json')
    release = json.loads((directory / 'artifact.json').read_text())
    if (release.get('format') != 'librairii.native-artifact' or release.get('schema') != 1
            or release.get('source_commit') != source or release.get('target') != target
            or not re.fullmatch(r'[0-9a-f]{40}', source)):
        raise ValueError('artifact does not match the externally selected source/target')
    facts = release['artifact']
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]+', facts['name']):
        raise ValueError('invalid artifact name')
    artifact = directory / facts['name']
    regular(artifact)
    if artifact.stat().st_size != facts['bytes'] or digest(artifact) != facts['sha256']:
        raise ValueError('distributed artifact bytes differ from their manifest')
    checksum = artifact.with_name(artifact.name + '.sha256')
    regular(checksum)
    if checksum.read_text() != facts['sha256'] + '  ' + facts['name'] + '\n':
        raise ValueError('artifact checksum file differs from final bytes')
    return artifact, release


def extract(artifact, destination, release):
    expected = release['payload']
    prefix = 'Librairii-' + release['version'] + '-' + release['target']
    if not re.fullmatch(r'Librairii-[0-9]+\.[0-9]+\.[0-9]+-x86_64-unknown-linux-gnu', prefix):
        raise ValueError('invalid portable archive root')
    seen = set()
    with tarfile.open(artifact) as archive:
        members = archive.getmembers()
        if len(members) > 10000 or sum(member.size for member in members) > 256*1024*1024:
            raise ValueError('portable archive exceeds package bounds')
        for member in members:
            path = Path(member.name)
            if path.is_absolute() or '..' in path.parts or not path.parts or path.parts[0] != prefix:
                raise ValueError('archive member escapes its versioned root')
            relative = Path(*path.parts[1:])
            if path.as_posix() != member.name.rstrip('/') or path.as_posix() in seen:
                raise ValueError('archive member aliases or duplicates a path')
            seen.add(path.as_posix())
            target = destination / relative
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile() and relative.as_posix() in expected:
                facts = expected[relative.as_posix()]
                if member.size != facts['bytes'] or member.mode != facts['mode']:
                    raise ValueError('archive member metadata differs from the sealed payload')
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.extractfile(member) as source, target.open('xb') as output:
                    shutil.copyfileobj(source, output)
                target.chmod(facts['mode'])
            else:
                raise ValueError('unlisted, linked or special archive member')
    if inventory(destination) != expected:
        raise ValueError('installed archive bytes differ from the sealed payload')


@contextmanager
def mounted(artifact, destination):
    mount = destination / 'mount'
    mount.mkdir()
    attached = False
    try:
        result = subprocess.run(['/usr/bin/hdiutil', 'attach', str(artifact), '-readonly', '-nobrowse',
                                 '-mountpoint', str(mount), '-plist'], capture_output=True, check=True, timeout=60)
        evidence = plistlib.loads(result.stdout)
        volumes = [entity for entity in evidence.get('system-entities', []) if entity.get('mount-point')]
        if len(volumes) != 1 or Path(volumes[0]['mount-point']).resolve() != mount.resolve():
            raise ValueError('disk image did not mount at the owned mountpoint')
        attached = True
        write_json(destination / 'mount.json', evidence)
        yield mount
    finally:
        if attached:
            subprocess.run(['/usr/bin/hdiutil', 'detach', str(mount)], check=True, timeout=60)


def inspect(root, directory, source, target, requested_output):
    artifact, release = artifact_record(directory, source, target)
    host = runtime_host(root, target)
    destination = fresh_output(root, requested_output)
    installed = destination / 'installed'
    installed.mkdir()
    if target.endswith('apple-darwin'):
        with mounted(artifact, destination) as mount:
            subprocess.run(['/usr/bin/ditto', '--norsrc', '--noextattr', str(mount / 'Librairii.app'),
                            str(installed / 'Librairii.app')], check=True, timeout=60)
            # Also compare the readme. Every application byte must match the
            # sealed post-signature payload copied out of the mounted DMG.
            shutil.copyfile(mount / 'INSTALL.txt', installed / 'INSTALL.txt')
        binary = installed / 'Librairii.app/Contents/MacOS/librairii-desktop'
        subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(installed / 'Librairii.app')], check=True)
    else:
        extract(artifact, installed, release)
        binary = installed / 'bin/librairii-desktop'
    if inventory(installed) != release['payload']:
        raise ValueError('installed package differs from final sealed payload')
    architecture(binary, target)
    info = verify_embedded(installed, binary, release)
    dependencies = linkage.inspect(installed, binary, target)
    report = {'format': 'librairii.native-package-inspection', 'schema': 1,
              'source_commit': source, 'target': target, 'artifact': release['artifact'], 'host': host,
              'installed': str(installed), 'binary': str(binary), 'binary_sha256': digest(binary),
              'build_info': info, 'linkage': dependencies, 'installed_payload_verified': True,
              'signing': release['signing'], 'gui_qualification': {'state': 'not performed'},
              'public_release_qualified': False}
    write_json(destination / 'inspection.json', report)
    return report
