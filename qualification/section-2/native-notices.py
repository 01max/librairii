"""Verify and copy the pinned native notice supplement without network access."""
import hashlib
import json
import pathlib
import shutil

IJG_ACKNOWLEDGMENT = 'This software is based in part on the work of the Independent JPEG Group.'
COMPONENTS = {'Skia', 'skcms', 'libpng', 'libjpeg-turbo', 'zlib', 'Chromium zlib extensions',
    'Expat', 'Wuffs', 'Vulkan Memory Allocator', 'Khronos Vulkan headers'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(source, bindings, native_licenses):
    inventory_path = source / 'inventory.json'
    if inventory_path.is_symlink():
        raise ValueError('native notice inventory must be a regular owned file')
    inventory = json.loads(inventory_path.read_text())
    if (inventory.get('schema') != 1 or inventory.get('toolkit_selected') is not False
            or len(bindings) != 1
            or bindings[0]['version'] != inventory['skia_bindings_version']
            or bindings[0].get('metadata', {}).get('skia') != inventory['skia_source_ref']):
        raise ValueError('native notice supplement does not match the selected Skia graph')
    components, files = inventory['components'], inventory['files']
    if ({item['name'] for item in components} != COMPONENTS or len(components) != len(COMPONENTS)
            or any(not item['notices'] or not item['revision'] or not item['source']
                or any(name not in files for name in item['notices']) for item in components)):
        raise ValueError('native notice component coverage is incomplete')
    if {path.name for path in source.iterdir()} != set(files) | {'inventory.json'}:
        raise ValueError('native notice file inventory is incomplete')
    for name, digest in files.items():
        path = source / name
        if (not name or pathlib.PurePosixPath(name).name != name or '\\' in name
                or path.is_symlink() or not path.is_file() or sha(path) != digest):
            raise ValueError('native notice payload missing, aliased or changed: ' + name)
    if IJG_ACKNOWLEDGMENT not in (source / 'ATTRIBUTIONS.md').read_text():
        raise ValueError('native binary IJG acknowledgment missing')
    if (not native_licenses or any(path.is_symlink() or not path.is_file()
            or sha(path) != files['LICENSE_SKIA'] for path in native_licenses)):
        raise ValueError('native build Skia notice does not match the pinned supplement')
    return inventory


def include(source, notices, bindings, native_licenses):
    inventory = verify(source, bindings, native_licenses)
    # Validate the complete supplement before writing any of it into a package.
    shutil.copytree(source, notices / 'native-skia')
    shutil.copy2(source / 'LICENSE_SKIA', notices / 'LICENSE_SKIA')
    return {'inventory': 'notices/native-skia/inventory.json',
        'inventory_sha256': sha(source / 'inventory.json'),
        'components': [item['name'] for item in inventory['components']],
        'scope': inventory['scope'], 'final_native_distribution_audit_complete': False}
