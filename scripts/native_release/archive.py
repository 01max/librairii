"""Deterministic portable archives and final-byte artifact records."""
import gzip
from pathlib import Path
import re
import tarfile

from .files import digest, inventory, regular, write_json


def tar_gzip(payload, destination, name, epoch):
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', name) or epoch < 0:
        raise ValueError('invalid archive root or source epoch')
    payload, destination = Path(payload), Path(destination)
    before = inventory(payload)
    directories = {name}
    for relative in before:
        directories.update(name + '/' + str(parent) for parent in Path(relative).parents if str(parent) != '.')
    with destination.open('xb') as output:
        with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=0, compresslevel=9) as compressed:
            with tarfile.open(fileobj=compressed, mode='w', format=tarfile.PAX_FORMAT) as archive:
                for directory in sorted(directories, key=lambda p: (p.count('/'), p)):
                    entry = tarfile.TarInfo(directory)
                    entry.type, entry.mode, entry.mtime = tarfile.DIRTYPE, 0o755, epoch
                    archive.addfile(entry)
                for relative, facts in before.items():
                    entry = tarfile.TarInfo(name + '/' + relative)
                    entry.size, entry.mode, entry.mtime = facts['bytes'], facts['mode'], epoch
                    with (payload / relative).open('rb') as source:
                        archive.addfile(entry, source)
    if inventory(payload) != before:
        raise ValueError('payload changed while assembling its archive')
    return digest(destination)


def seal_artifact(destination, artifact, payload, release, package_format, signing):
    destination, artifact = Path(destination), Path(artifact)
    if artifact.parent != destination or artifact.is_symlink():
        raise ValueError('artifact must be an owned regular output')
    regular(artifact)
    facts = {'name': artifact.name, 'bytes': artifact.stat().st_size, 'sha256': digest(artifact)}
    with artifact.with_name(artifact.name + '.sha256').open('x') as checksum:
        checksum.write(f"{facts['sha256']}  {artifact.name}\n")
    result = {key: release[key] for key in ['version', 'source_commit', 'source_tree', 'source_date_epoch',
               'target', 'features', 'rust_version', 'cargo_lock_sha256', 'native_toolkit_pin_sha256', 'minimum_os']}
    result.update({'format': 'librairii.native-artifact', 'schema': 1, 'artifact': facts,
                   'package_format': package_format, 'payload': inventory(payload),
                   'signing': signing, 'qualification': {'state': 'not performed'},
                   'public_release_qualified': False})
    write_json(destination / 'artifact.json', result)
    return result
