"""SPDX 2.3 for actual Cargo build inputs plus the pinned native notice union."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import tomllib
import uuid

from .files import copy_file, copy_tree, digest, inventory, write_json


def identifier(name, version, origin):
    key = json.dumps([name, version, origin], separators=(',', ':')).encode()
    return 'SPDXRef-Package-' + hashlib.sha256(key).hexdigest()[:32]


def notice_files(package, destination):
    source = Path(package['manifest_path']).parent
    found = 0
    for entry in sorted(source.iterdir()):
        if not entry.name.upper().startswith(('LICENSE', 'NOTICE', 'COPYING')):
            continue
        if entry.is_dir() and not entry.is_symlink():
            copy_tree(entry, destination / entry.name)
            found += len(inventory(entry))
        else:
            copy_file(entry, destination / entry.name)
            found += 1
    return found


def write(root, directory, release, metadata, events):
    packages = {p['id']: p for p in metadata['packages']}
    compiled = {}
    for event in events:
        compiled.setdefault(event['package_id'], set()).update(event['features'])
    if not compiled or not set(compiled).issubset(packages):
        raise ValueError('actual compiled graph is absent from Cargo metadata')
    locked = tomllib.loads((root / 'Cargo.lock').read_text())['package']
    notices = directory / 'notices'
    notices.mkdir()
    records, references = [], {}
    feature_rows = []
    for package_id in sorted(compiled):
        p = packages[package_id]
        local = p['source'] is None
        relative = Path(p['manifest_path']).relative_to(root).as_posix() if local else None
        origin = relative if local else p['source']
        ref = identifier(p['name'], p['version'], origin)
        references[package_id] = ref
        row = {'SPDXID': ref, 'name': p['name'], 'versionInfo': p['version'],
               'downloadLocation': 'NOASSERTION', 'filesAnalyzed': False,
               'licenseConcluded': 'NOASSERTION',
               'licenseDeclared': p['license'] or 'NOASSERTION',
               'copyrightText': 'NOASSERTION'}
        if 'LicenseRef-Slint-' in row['licenseDeclared']:
            row['licenseDeclared'] = 'LicenseRef-Slint-Royalty-free-2.0'
        if not local:
            matches = [r for r in locked if r['name'] == p['name'] and r['version'] == p['version'] and r.get('source') == p['source']]
            if len(matches) != 1 or not matches[0].get('checksum'):
                raise ValueError('compiled registry package lacks an exact locked checksum')
            row['checksums'] = [{'algorithm': 'SHA256', 'checksumValue': matches[0]['checksum']}]
            row['downloadLocation'] = 'https://crates.io/api/v1/crates/' + p['name'] + '/' + p['version'] + '/download'
        else:
            row['sourceInfo'] = 'Repository ' + release['source_commit'] + '; ' + relative
        if not local or relative.startswith('third_party/'):
            folder = notices / (p['name'] + '-' + p['version'])
            folder.mkdir()
            count = notice_files(p, folder)
            supplemental = root / '.config/upstream-notices' / folder.name
            if supplemental.is_dir():
                copy_tree(supplemental, folder / 'supplemental')
                count += sum(Path(path).name.upper().startswith(('LICENSE', 'NOTICE', 'COPYING'))
                             for path in inventory(supplemental))
            if count == 0:
                raise ValueError('no distribution notices found for ' + p['name'])
        feature_rows.append({'name': p['name'], 'version': p['version'],
                             'source': origin, 'features': sorted(compiled[package_id])})
        records.append(row)
    # These checked-in derivative notices are independent of registry metadata.
    copy_tree(root / '.config/upstream-notices', notices / 'source-and-supplemental')
    native = root / 'qualification/section-2/notices/native-skia'
    copy_tree(native, notices / 'native-skia')
    native_data = json.loads((native / 'inventory.json').read_text())
    for path, expected in native_data['files'].items():
        if digest(notices / 'native-skia' / path) != expected:
            raise ValueError('pinned native notice differs from packaged text')
    copy_file(root / 'qualification/section-2/notices/Slint-Royalty-free-2.0.md', notices / 'Slint-Royalty-free-2.0.md')
    relationships = [{'spdxElementId': 'SPDXRef-DOCUMENT', 'relationshipType': 'DESCRIBES', 'relatedSpdxElement': 'SPDXRef-Product'}]
    for ref in references.values():
        relationships.append({'spdxElementId': 'SPDXRef-Product', 'relationshipType': 'DEPENDS_ON', 'relatedSpdxElement': ref,
                              'comment': 'Actual compiler-artifact graph, including build-time inputs; not a dynamic-linkage assertion.'})
    for component in native_data['components']:
        ref = identifier(component['name'], component['revision'], component['source'])
        records.append({'SPDXID': ref, 'name': component['name'], 'versionInfo': component['revision'],
                        'downloadLocation': component['source'], 'filesAnalyzed': False,
                        'licenseConcluded': 'NOASSERTION', 'licenseDeclared': 'NOASSERTION',
                        'copyrightText': 'NOASSERTION',
                        'sourceInfo': 'Pinned conservative native source/notice union; final linkage is qualified separately.'})
        relationships.append({'spdxElementId': 'SPDXRef-Product', 'relationshipType': 'OTHER', 'relatedSpdxElement': ref,
                              'comment': 'Pinned native build-source notice union, not an assertion that every component is linked on this target.'})
    records.insert(0, {'SPDXID': 'SPDXRef-Product', 'name': 'Librairii', 'versionInfo': release['version'],
                       'downloadLocation': 'NOASSERTION', 'filesAnalyzed': False,
                       'licenseConcluded': 'NOASSERTION', 'licenseDeclared': 'NOASSERTION',
                       'copyrightText': 'Copyright 2026 Librairii contributors',
                       'sourceInfo': 'Git ' + release['source_commit'] + '; target ' + release['target']})
    key = release['source_commit'] + '/' + release['target'] + '/' + release['cargo_lock_sha256']
    spdx = {'spdxVersion': 'SPDX-2.3', 'dataLicense': 'CC0-1.0', 'SPDXID': 'SPDXRef-DOCUMENT',
            'name': 'Librairii-' + release['version'] + '-' + release['target'],
            'documentNamespace': 'https://github.com/01max/librairii/spdx/' + str(uuid.uuid5(uuid.NAMESPACE_URL, key)),
            'creationInfo': {'creators': ['Tool: Librairii native release xtask'],
                             'created': datetime.fromtimestamp(release['source_date_epoch'], timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')},
            'packages': records, 'relationships': relationships,
            'hasExtractedLicensingInfos': [{'licenseId': 'LicenseRef-Slint-Royalty-free-2.0',
                                           'name': 'Slint Royalty-free License 2.0',
                                           'extractedText': (notices / 'Slint-Royalty-free-2.0.md').read_text()}]}
    write_json(directory / 'sbom.spdx.json', spdx)
    write_json(directory / 'compiled-dependencies.json', {'source_commit': release['source_commit'],
               'target': release['target'], 'scope': 'actual production Cargo compiler graph, including build-time inputs',
               'dependencies': sorted(feature_rows, key=lambda p: (p['name'], p['version'], p['source']))})
