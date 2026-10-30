"""No-clobber release outputs and deterministic payload inventories."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat


def digest(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def write_json(path, value):
    with Path(path).open('x', encoding='utf-8') as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + '\n')


def regular(path):
    facts = Path(path).lstat()
    if not stat.S_ISREG(facts.st_mode):
        raise ValueError('release input is not a regular file: ' + str(path))
    return facts


def copy_file(source, destination, executable=False):
    regular(source)
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Creating without replacement also rejects dangling output symlinks.
    with Path(source).open('rb') as src, destination.open('xb') as dst:
        shutil.copyfileobj(src, dst, 1024 * 1024)
    destination.chmod(0o755 if executable else 0o644)
    if digest(source) != digest(destination):
        raise ValueError('copied release input changed: ' + str(source))


def copy_tree(source, destination):
    source, destination = Path(source), Path(destination)
    if not stat.S_ISDIR(source.lstat().st_mode):
        raise ValueError('release source directory must be real')
    destination.mkdir()
    for entry in sorted(source.iterdir()):
        if stat.S_ISDIR(entry.lstat().st_mode):
            copy_tree(entry, destination / entry.name)
        else:
            copy_file(entry, destination / entry.name)


def inventory(directory):
    result = {}
    directory = Path(directory)
    if not stat.S_ISDIR(directory.lstat().st_mode):
        raise ValueError('release payload must be a real directory')
    for parent, directories, files in os.walk(directory, followlinks=False):
        for name in directories:
            if not stat.S_ISDIR((Path(parent) / name).lstat().st_mode):
                raise ValueError('links and special directories are forbidden in payloads')
        for name in files:
            path = Path(parent) / name
            facts = regular(path)
            result[path.relative_to(directory).as_posix()] = {
                'bytes': facts.st_size, 'sha256': digest(path),
                'mode': 0o755 if facts.st_mode & 0o111 else 0o644,
            }
    return dict(sorted(result.items()))


def fresh_output(root, requested):
    boundary = (Path(root) / 'target' / 'native-release').resolve()
    # An existing ancestor link must not redirect even the boundary itself.
    for path in [Path(root) / 'target', Path(root) / 'target' / 'native-release']:
        if path.is_symlink():
            raise ValueError('release output ancestor is a link')
    destination = Path(requested)
    if not destination.is_absolute():
        destination = Path(root) / destination
    destination = destination.absolute()
    if '..' in destination.parts or not destination.is_relative_to(boundary) or destination == boundary:
        raise ValueError('release output must be a new directory below target/native-release')
    cursor = boundary
    for part in destination.relative_to(boundary).parts:
        cursor /= part
        if cursor.is_symlink():
            raise ValueError('release output ancestor is a link')
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.mkdir()  # Existing outputs are never removed or reused.
    return destination
