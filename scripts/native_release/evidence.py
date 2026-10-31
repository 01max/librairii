"""Offline custody index for retained release archives; never a release approval."""
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import zipfile

from .files import digest, regular, write_json
from .identity import TARGETS


IDENTITY = ('source_commit', 'source_tree', 'version', 'rust_version',
            'cargo_lock_sha256', 'native_toolkit_pin_sha256', 'features')
REPORTS = {'quality', 'scale', 'installed'}
MAX_JSON = 16 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result


def read_json(data):
    require(len(data) <= MAX_JSON, 'evidence JSON exceeds 16 MiB')
    return json.loads(data, object_pairs_hook=unique_object)


def hexadecimal(value, length):
    require(isinstance(value, str) and re.fullmatch('[0-9a-f]{' + str(length) + '}', value),
            'expected lowercase hexadecimal identity')
    return value


def relative_name(value):
    require(isinstance(value, str) and value and '\\' not in value, 'invalid evidence path')
    path = PurePosixPath(value)
    require(not path.is_absolute() and value == path.as_posix()
            and all(part not in ('.', '..') for part in path.parts), 'noncanonical evidence path')
    return value


def file_record(root, record, *, provider=False):
    require(isinstance(record, dict), 'evidence file record must be an object')
    keys = {'path', 'bytes', 'sha256'} | ({'artifact_id', 'run_id'} if provider else set())
    require(set(record) == keys, 'unexpected or missing evidence file fields')
    name = relative_name(record['path'])
    path = root / name
    require(path.resolve().is_relative_to(root), 'evidence path escapes input directory')
    require(type(record['bytes']) is int and record['bytes'] > 0, 'invalid evidence byte count')
    hexadecimal(record['sha256'], 64)
    if provider:
        require(all(type(record[key]) is int and record[key] > 0 for key in ('artifact_id', 'run_id')),
                'positive provider artifact and workflow run IDs required')
    require(regular(path).st_size == record['bytes'] and digest(path) == record['sha256'],
            'evidence file size or SHA-256 mismatch: ' + name)
    return path


@contextmanager
def archive(root, record):
    path = file_record(root, record, provider=True)
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        require(len(names) == len(set(names)), 'duplicate ZIP member')
        for name in names:
            relative_name(name[:-1] if name.endswith('/') else name)
        yield z


def member_json(z, name):
    require(z.getinfo(name).file_size <= MAX_JSON, 'oversized evidence metadata: ' + name)
    return read_json(z.read(name))


def member_digest(z, name):
    value, size = hashlib.sha256(), 0
    with z.open(name) as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
            size += len(block)
    return {'bytes': size, 'sha256': value.hexdigest()}


def bound(record, source, target):
    require(isinstance(record, dict) and record.get('source_commit') == source and record.get('target') == target,
            'evidence source or target mismatch')


def distribution(z, prefix, source, tree, target):
    metadata = prefix + 'artifact.json'
    record = member_json(z, metadata)
    bound(record, source, target)
    require(record.get('format') == 'librairii.native-artifact' and record.get('schema') == 1
            and record.get('source_tree') == tree, 'package source tree or format mismatch')
    for key in ('cargo_lock_sha256', 'native_toolkit_pin_sha256'):
        hexadecimal(record[key], 64)
    require(record['features'] == [], 'unqualified shipping feature selection')
    kind = 'dmg' if target.endswith('apple-darwin') else 'tar.gz'
    require(record.get('package_format') == kind and record.get('minimum_os') == TARGETS[target][2],
            'package format or qualified OS baseline mismatch')
    require(isinstance(record.get('payload'), dict) and record['payload'], 'package payload inventory required')
    package = record['artifact']
    require(set(package) == {'name', 'bytes', 'sha256'}, 'invalid inner distribution record')
    name = relative_name(package['name'])
    require('/' not in name, 'inner distribution must have a basename')
    hexadecimal(package['sha256'], 64)
    require(type(package['bytes']) is int and package['bytes'] > 0, 'invalid distribution size')
    require(member_digest(z, prefix + name) == {key: package[key] for key in ('bytes', 'sha256')},
            'inner distribution size or SHA-256 mismatch')
    return record, {'member': metadata, **member_digest(z, metadata)}


def candidate(root, selected, source, tree, target):
    with archive(root, selected) as z:
        package, metadata = distribution(z, 'package/', source, tree, target)
        stage = member_json(z, 'stage/stage.json')
        bound(stage, source, target)
        require(stage.get('stage_complete') is True and stage.get('working_tree_dirty') is False,
                'candidate stage is incomplete or dirty')
        require(all(stage.get(key) == package[key] for key in IDENTITY), 'stage/package identity mismatch')
        require(z.getinfo('stage/compiler-artifacts.jsonl').file_size <= MAX_JSON, 'oversized compiler records')
        events = [read_json(line) for line in z.read('stage/compiler-artifacts.jsonl').splitlines()]
        require(all(isinstance(event, dict) for event in events), 'compiler records must be JSON objects')
        executable = [e for e in events if e.get('reason') == 'compiler-artifact'
                      and e.get('target', {}).get('name') == 'librairii-desktop'
                      and e['target'].get('kind') == ['bin'] and e.get('executable')]
        require(len(executable) == 1, 'exactly one production Cargo executable required')
        profile = executable[0]['profile']
        require(profile.get('opt_level') == '3' and profile.get('debug_assertions') is False
                and profile.get('overflow_checks') is False and executable[0].get('features') == [],
                'candidate compiler profile or features mismatch')
        require(sum(e.get('reason') == 'build-finished' and e.get('success') is True for e in events) == 1,
                'successful Cargo build termination required')
        metadata['compiler_records'] = member_digest(z, 'stage/compiler-artifacts.jsonl')
        metadata['host'] = stage['host']
        return package, metadata


def installed(z, package, source, target):
    inspection = member_json(z, 'inspection/inspection.json')
    gui = member_json(z, 'gui/qualification.json')
    for record in (inspection, gui):
        bound(record, source, target)
        require(record.get('artifact') == package['artifact'], 'installed distribution differs from selection')
    require(inspection.get('installed_payload_verified') is True
            and inspection.get('installed_payload') == package['payload'], 'installed inventory mismatch')
    require(gui.get('complete') is True and gui.get('errors') == [], 'incomplete installed GUI report')
    prefix = 'inspection/installed/'
    names = {n[len(prefix):] for n in z.namelist() if n.startswith(prefix) and not n.endswith('/')}
    require(names == set(package['payload']), 'missing or extra retained installed payload')
    for name, facts in package['payload'].items():
        relative_name(name)
        require(member_digest(z, prefix + name) == {key: facts[key] for key in ('bytes', 'sha256')},
                'retained installed bytes mismatch: ' + name)
    return {'installed_file_bytes_matched': len(names), 'host': inspection['host']}


def report(root, kind, selected, package, source, target):
    with archive(root, selected) as z:
        if kind == 'installed':
            facts = installed(z, package, source, target)
            members = ['inspection/inspection.json', 'gui/qualification.json']
        elif kind == 'quality':
            require(z.getinfo('quality/source.txt').file_size <= 128, 'invalid quality source record')
            require(z.read('quality/source.txt').decode().strip() == source, 'quality source mismatch')
            ui = member_json(z, 'quality/native-ui/manifest.json')
            bound(ui, source, target)
            facts = {'host': ui['host']}
            members = ['quality/source.txt', 'quality/native-ui/manifest.json']
        else:
            members = ['manifest.json', 'desktop/manifest.json', 'resources/manifest.json']
            for name in members:
                record = member_json(z, name)
                bound(record, source, target)
                require(record.get('complete') is True
                        and record.get('cargo_lock_sha256') == package['cargo_lock_sha256'],
                        'incomplete scale report or graph mismatch')
            facts = {}
        return dict(selected, metadata=[dict(member=n, **member_digest(z, n)) for n in members],
                    binding_checked=True, acceptance_verified=False, **facts)


def assemble(input_path, output_path):
    input_path, output_path = Path(input_path), Path(output_path)
    root = input_path.resolve().parent
    require(output_path.parent.resolve() == root, 'write the manifest beside its input plan')
    if output_path.exists() or output_path.is_symlink():
        raise FileExistsError('evidence output already exists')
    require(regular(input_path).st_size <= MAX_JSON, 'oversized evidence input plan')
    input_bytes = input_path.read_bytes()
    plan_sha256 = hashlib.sha256(input_bytes).hexdigest()
    plan = read_json(input_bytes)
    require(set(plan) == {'format', 'schema', 'source_commit', 'source_tree', 'targets',
                          'limitations', 'retained_records'}, 'unexpected or missing evidence plan fields')
    require(plan['format'] == 'librairii.release-evidence-input' and plan['schema'] == 1,
            'unsupported evidence input format')
    source, tree = hexadecimal(plan['source_commit'], 40), hexadecimal(plan['source_tree'], 40)
    require(isinstance(plan['targets'], list) and len(plan['targets']) == len(TARGETS)
            and {row['target'] for row in plan['targets']} == set(TARGETS), 'all three distinct targets required')
    require(isinstance(plan['limitations'], list) and all(isinstance(s, str) and s.strip()
            for s in plan['limitations']), 'limitations must be nonempty strings')
    targets, identity, selected_files, provider_ids = [], None, [], set()
    for row in sorted(plan['targets'], key=lambda r: r['target']):
        require(set(row) in ({'target', 'candidate', 'reports'}, {'target', 'candidate', 'signed_candidate', 'reports'}),
                'unexpected or missing target fields')
        target = row['target']
        require(isinstance(row['reports'], dict) and set(row['reports']) == REPORTS,
                'quality, scale and installed reports required for each target')
        archives = [row['candidate'], *row['reports'].values()]
        if 'signed_candidate' in row:
            archives.append(row['signed_candidate'])
        for selected in archives:
            require(selected['artifact_id'] not in provider_ids, 'provider artifact ID reused')
            provider_ids.add(selected['artifact_id'])
        selected_files.extend(archives)
        package, metadata = candidate(root, row['candidate'], source, tree, target)
        selected_identity = {key: package[key] for key in IDENTITY}
        if identity is None:
            identity = selected_identity
        require(identity == selected_identity, 'candidate versions, source or graph differ across targets')
        provenance = dict(row['candidate'], metadata=metadata)
        if 'signed_candidate' in row:
            require(target.endswith('apple-darwin'), 'Linux cannot select a signed Mac distribution')
            with archive(root, row['signed_candidate']) as z:
                signed, signed_metadata = distribution(z, '', source, tree, target)
                signing = signed['signing']
                require(all(signed[key] == package[key] for key in IDENTITY)
                        and signing.get('unsigned_sha256') == package['artifact']['sha256']
                        and all(signing.get(key) is True for key in ('developer_id', 'notarized', 'stapled')),
                        'signed distribution lacks the exact unsigned lineage or signing records')
                package = signed
                provenance['signed_candidate'] = dict(row['signed_candidate'], metadata=signed_metadata)
        targets.append({'target': target, 'distribution': package['artifact'],
                        'minimum_os': package['minimum_os'], 'package_format': package['package_format'],
                        'recorded_signing': package['signing'], 'candidate': provenance,
                        'reports': {kind: report(root, kind, row['reports'][kind], package, source, target)
                                    for kind in sorted(REPORTS)}})
    retained = []
    require(isinstance(plan['retained_records'], list), 'retained records must be a list')
    for record in plan['retained_records']:
        require(set(record) == {'kind', 'conclusion', 'file'} and record['kind'] in
                ('failure', 'diagnostic', 'review', 'parity', 'recovery', 'workflow')
                and record['conclusion'] in ('passed', 'failed', 'cancelled', 'incomplete', 'diagnostic'),
                'invalid retained record classification')
        file_record(root, record['file'])
        retained.append(record)
    # Recheck the selected bytes after indexing. Never bind a changed input plan
    # or a replaced earlier archive to the observations made before that change.
    require(digest(input_path) == plan_sha256, 'evidence input plan changed during assembly')
    for selected in selected_files:
        file_record(root, selected, provider=True)
    for record in retained:
        file_record(root, record['file'])
    result = {'format': 'librairii.release-evidence', 'schema': 1, **identity,
              'assembler': {'module': 'scripts/native_release/evidence.py', 'sha256': digest(__file__)},
              'input_plan': {'name': input_path.name, 'sha256': plan_sha256},
              'paths_relative_to': 'input plan directory', 'targets': targets,
              'limitations': plan['limitations'], 'retained_records': retained,
              'provider_authenticated': False, 'acceptance_verified': False,
              'public_release_qualified': False,
              'review_required': ['Authenticate provider IDs and independently selected archive hashes.',
                                  'Review raw assertions, numerical limits, native observations and retirement.',
                                  'Reconcile every mandatory parity row and the full release rehearsal.',
                                  'Review final Mac signatures, notarization, stapling and matching-host final bytes.',
                                  'Authorize release publication and cutover separately.']}
    write_json(output_path, result)
    return result
