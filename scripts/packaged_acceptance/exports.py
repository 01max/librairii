"""Independent, frozen scope and exact-byte witnesses for GUI exports."""
import json
from pathlib import Path

from native_release.files import digest, regular


def freeze(rows, root, uuids, source_type, groups, destination):
    selected = [row for row in rows if row['uuid'] in uuids]
    if not uuids or len(selected) != len(uuids) or len({row['uuid'] for row in selected}) != len(selected):
        raise ValueError('export expectation requires every independently selected UUID exactly once')
    items, files = {}, {}
    for row in selected:
        path = root/row['managed_path']
        if not path.resolve().is_relative_to(root) or row['validation_state'] != 'valid':
            raise ValueError('synthetic export source is not a valid owned archive')
        regular(path)
        if path.stat().st_size != row['byte_size'] or digest(path) != row['sha256']:
            raise ValueError('synthetic managed archive differs from its stored bytes')
        name = row['original_filename']
        if Path(name).name != name or name in files:
            raise ValueError('synthetic export requires distinct original filenames')
        skipped = row['is_broken'] == 1
        items[row['uuid']] = {'story_id': row['id'], 'resolved_story_id': row['id'],
            'output_name': name, 'archive_relative_path': row['managed_path'], 'archive_sha256': row['sha256'],
            'status': 'skipped' if skipped else 'succeeded',
            'outcome_code': 'story_marked_broken' if skipped else 'exported',
            'export_skip_code': 'story_marked_broken' if skipped else None,
            'total_bytes': 0 if skipped else row['byte_size'],
            'completed_bytes': 0 if skipped else row['byte_size']}
        if not skipped:
            files[name] = {'sha256': row['sha256'], 'bytes': row['byte_size']}
    if not files:
        raise ValueError('export acceptance must copy actual eligible archives')
    partial = len(files) != len(items)
    return {'source_type': source_type, 'groups': groups, 'destination': str(destination),
            'status': 'partially_succeeded' if partial else 'succeeded',
            'error_code': 'partial_export' if partial else None, 'items': items, 'files': files,
            'total_bytes': sum(value['bytes'] for value in files.values())}


def verify(expected, operation, items, files):
    header = {'kind': 'export', 'status': expected['status'], 'error_code': expected['error_code'],
              'export_source_type': expected['source_type'], 'export_destination': expected['destination'],
              'completed_items': len(expected['items']), 'total_items': len(expected['items']),
              'total_bytes': expected['total_bytes'], 'cancel_requested': 0}
    if not operation.get('finished_at') or any(operation.get(key) != value for key,value in header.items()):
        raise ValueError('export operation differs from its exact frozen scope/outcome')
    if (json.loads(operation['export_source_shelf_ids']) != [row['id'] for row in expected['groups']]
            or json.loads(operation['export_source_shelf_names']) != [row['name'] for row in expected['groups']]):
        raise ValueError('export group provenance differs from the selected scope')
    observed = {}
    for item in items:
        uuid = item['story_uuid']
        if item['operation_id'] != operation['id'] or uuid in observed or uuid not in expected['items']:
            raise ValueError('export contains a duplicate, foreign or unexpected item')
        observed[uuid] = {key: item.get(key) for key in expected['items'][uuid]}
    if observed != expected['items']:
        raise ValueError('export item identities, eligibility or terminal outcomes differ')
    actual = {name: {'sha256': facts['sha256'], 'bytes': facts['bytes']} for name,facts in files.items()}
    if actual != expected['files']:
        raise ValueError('export filenames, counts, sizes or hashes differ from frozen eligible archives')
    return {'expected': expected, 'operation': operation, 'items': items, 'files': files}
