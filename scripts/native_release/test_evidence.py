"""Real archive mutations protect the offline evidence/human-approval boundary."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock
import warnings
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release import evidence
from native_release.evidence import assemble, TARGETS


SOURCE, TREE, LOCK, TOOLKIT = 'a' * 40, 'b' * 40, 'c' * 64, 'd' * 64


def encoded(value):
    return json.dumps(value, sort_keys=True).encode()


def facts(data):
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


class EvidenceManifest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.plan = {'format': 'librairii.release-evidence-input', 'schema': 1,
                     'source_commit': SOURCE, 'source_tree': TREE, 'targets': [],
                     'limitations': ['Synthetic archives are not native execution evidence.'],
                     'retained_records': []}
        self.packages = {}
        self.counter = 0
        for target in TARGETS:
            identity = {'source_commit': SOURCE, 'source_tree': TREE, 'target': target,
                        'version': '0.5.0', 'rust_version': '1.99.1', 'features': [],
                        'cargo_lock_sha256': LOCK, 'native_toolkit_pin_sha256': TOOLKIT}
            binary = ('inert bytes ' + target).encode()
            payload = {'bin/librairii': dict(facts(binary), mode=0o755)}
            distribution = b'inert distribution ' + binary
            package = dict(identity, format='librairii.native-artifact', schema=1,
                           minimum_os=TARGETS[target][2],
                           package_format='dmg' if target.endswith('darwin') else 'tar.gz',
                           payload=payload, artifact=dict(name='fixture.dmg' if target.endswith('darwin')
                                                         else 'fixture.tar.gz', **facts(distribution)),
                           signing=({'state': 'ad-hoc internal candidate', 'developer_id': False,
                                     'notarized': False, 'stapled': False}
                                    if target.endswith('darwin') else
                                    {'state': 'not applicable (Linux portable archive)'}),
                           public_release_qualified=True)
            self.packages[target] = package
            stage = dict(identity, stage_complete=True, working_tree_dirty=False, host={'target': target})
            events = [{'reason': 'compiler-artifact', 'target': {'name': 'librairii-desktop', 'kind': ['bin']},
                       'executable': 'inert', 'features': [],
                       'profile': {'opt_level': '3', 'debug_assertions': False, 'overflow_checks': False}},
                      {'reason': 'build-finished', 'success': True}]
            candidate = self.archive(target + '-candidate.zip', {
                'package/artifact.json': encoded(package), 'package/' + package['artifact']['name']: distribution,
                'stage/stage.json': encoded(stage),
                'stage/compiler-artifacts.jsonl': b'\n'.join(encoded(e) for e in events)})
            quality = self.archive(target + '-quality.zip', {
                'quality/source.txt': SOURCE.encode(),
                'quality/native-ui/manifest.json': encoded(dict(identity, host='synthetic'))})
            scale = self.archive(target + '-scale.zip', {name: encoded(dict(identity, complete=True))
                                 for name in ['manifest.json', 'desktop/manifest.json', 'resources/manifest.json']})
            installed = self.archive(target + '-installed.zip', {
                'inspection/inspection.json': encoded(dict(identity, artifact=package['artifact'],
                    installed_payload=payload, installed_payload_verified=True, host='synthetic')),
                'gui/qualification.json': encoded(dict(identity, artifact=package['artifact'], complete=True, errors=[])),
                'inspection/installed/bin/librairii': binary})
            self.plan['targets'].append({'target': target, 'candidate': candidate,
                                        'reports': {'quality': quality, 'scale': scale, 'installed': installed}})

    def archive(self, name, members):
        path = self.root / name
        with zipfile.ZipFile(path, 'w') as z:
            for member, data in members.items():
                z.writestr(member, data)
        self.counter += 1
        return {'path': name, **facts(path.read_bytes()), 'artifact_id': self.counter, 'run_id': 100}

    def mutate(self, record, member, change):
        path = self.root / record['path']
        with zipfile.ZipFile(path) as z:
            members = {n: z.read(n) for n in z.namelist()}
        members[member] = change(members[member])
        with zipfile.ZipFile(path, 'w') as z:
            for name, data in members.items():
                z.writestr(name, data)
        record.update(facts(path.read_bytes()))

    def run_manifest(self):
        path = self.root / 'input.json'
        path.write_bytes(encoded(self.plan))
        return assemble(path, self.root / 'manifest.json')

    def assert_refused(self):
        with self.assertRaises((ValueError, KeyError)):
            self.run_manifest()
        self.assertFalse((self.root / 'manifest.json').exists())

    def test_all_three_targets_and_failed_history_are_indexed_without_release_approval(self):
        data = b'original failed result'
        (self.root / 'failed.log').write_bytes(data)
        self.plan['retained_records'] = [{'kind': 'failure', 'conclusion': 'failed',
                                        'file': {'path': 'failed.log', **facts(data)}}]
        before = {p.name: p.read_bytes() for p in self.root.iterdir()}
        result = self.run_manifest()
        self.assertEqual(len(result['targets']), 3)
        self.assertEqual(result['retained_records'], self.plan['retained_records'])
        self.assertFalse(result['public_release_qualified'])
        self.assertFalse(result['acceptance_verified'])
        self.assertFalse(result['provider_authenticated'])
        self.assertTrue(result['review_required'])
        for row in result['targets']:
            self.assertEqual(row['reports']['installed']['installed_file_bytes_matched'], 1)
            self.assertFalse(row['reports']['installed']['acceptance_verified'])
        self.assertEqual(before, {name: (self.root / name).read_bytes() for name in before})

    def test_outer_hash_mismatch_refuses_before_output(self):
        record = self.plan['targets'][0]['reports']['quality']
        (self.root / record['path']).write_bytes(b'different provider bytes')
        self.assert_refused()

    def test_inputs_changed_during_assembly_fail_without_output(self):
        original = evidence.report
        changed = False

        def replace_earlier_archive(*args):
            nonlocal changed
            result = original(*args)
            if not changed:
                path = self.root / self.plan['targets'][0]['candidate']['path']
                path.write_bytes(b'replaced after the earlier candidate was indexed')
                changed = True
            return result

        with mock.patch.object(evidence, 'report', side_effect=replace_earlier_archive):
            self.assert_refused()

    def test_inner_distribution_and_actual_installed_byte_tampering_are_rejected(self):
        for role in ('candidate', 'installed'):
            with self.subTest(role=role):
                row = self.plan['targets'][0]
                record = row['candidate'] if role == 'candidate' else row['reports']['installed']
                original = (self.root / record['path']).read_bytes()
                member = 'package/' + self.packages[row['target']]['artifact']['name'] if role == 'candidate' \
                    else 'inspection/installed/bin/librairii'
                self.mutate(record, member, lambda data: data + b' changed')
                self.assert_refused()
                (self.root / record['path']).write_bytes(original)
                record.update(facts(original))

    def test_cross_source_target_and_graph_reports_are_rejected(self):
        row = self.plan['targets'][0]
        for key, value in [('source_commit', 'e' * 40), ('target', 'x86_64-unknown-linux-gnu'),
                           ('cargo_lock_sha256', 'f' * 64), ('complete', False)]:
            with self.subTest(key=key):
                record = row['reports']['scale']
                original = (self.root / record['path']).read_bytes()
                self.mutate(record, 'resources/manifest.json',
                            lambda data: encoded(dict(json.loads(data), **{key: value})))
                self.assert_refused()
                (self.root / record['path']).write_bytes(original)
                record.update(facts(original))

    def test_missing_target_report_or_incomplete_gui_remains_incomplete(self):
        original = copy.deepcopy(self.plan)
        for case in ('target', 'report', 'gui'):
            with self.subTest(case=case):
                self.plan = copy.deepcopy(original)
                if case == 'target':
                    self.plan['targets'].pop()
                elif case == 'report':
                    self.plan['targets'][0]['reports'].pop('quality')
                else:
                    self.mutate(self.plan['targets'][0]['reports']['installed'], 'gui/qualification.json',
                                lambda data: encoded(dict(json.loads(data), complete=False)))
                self.assert_refused()

    def test_wrong_compiler_profile_and_cross_target_package_graph_are_rejected(self):
        record = self.plan['targets'][1]['candidate']
        original = (self.root / record['path']).read_bytes()
        self.mutate(record, 'stage/compiler-artifacts.jsonl', lambda data: data.replace(b'"3"', b'"0"'))
        self.assert_refused()
        (self.root / record['path']).write_bytes(original)
        record.update(facts(original))
        for name in ('stage/stage.json', 'package/artifact.json'):
            self.mutate(record, name, lambda data: encoded(dict(json.loads(data), cargo_lock_sha256='e' * 64)))
        self.assert_refused()

    def test_duplicate_json_or_zip_identity_and_traversal_are_rejected(self):
        path = self.root / 'input.json'
        path.write_bytes(encoded(self.plan).replace(b'"schema": 1', b'"schema": 1, "schema": 1', 1))
        with self.assertRaises(ValueError):
            assemble(path, self.root / 'manifest.json')
        row = self.plan['targets'][0]['reports']['quality']
        with warnings.catch_warnings(), zipfile.ZipFile(self.root / row['path'], 'a') as z:
            warnings.simplefilter('ignore', UserWarning)
            z.writestr('quality/source.txt', SOURCE)
        row.update(facts((self.root / row['path']).read_bytes()))
        self.assert_refused()
        row['path'] = '../outside.zip'
        self.assert_refused()

    def test_existing_output_and_output_symlink_are_never_overwritten(self):
        path = self.root / 'input.json'
        path.write_bytes(encoded(self.plan))
        output = self.root / 'manifest.json'
        output.write_bytes(b'retained previous manifest')
        with self.assertRaises(FileExistsError):
            assemble(path, output)
        self.assertEqual(output.read_bytes(), b'retained previous manifest')
        output.unlink()
        output.symlink_to(path)
        before = path.read_bytes()
        with self.assertRaises(FileExistsError):
            assemble(path, output)
        self.assertEqual(path.read_bytes(), before)

    def test_signed_distribution_requires_unsigned_lineage_and_exact_final_installed_bytes(self):
        row = self.plan['targets'][0]
        candidate = row['candidate']
        original = (self.root / candidate['path']).read_bytes()
        self.mutate(candidate, 'package/artifact.json', lambda data: encoded(dict(
            json.loads(data), signing={'state': 'Developer ID signed', 'developer_id': True,
                                       'notarized': True, 'stapled': True})))
        self.assert_refused()  # Signing claims in the base candidate cannot skip the signed selection.
        (self.root / candidate['path']).write_bytes(original)
        candidate.update(facts(original))
        package = copy.deepcopy(self.packages[row['target']])
        final = b'inert signed distribution'
        package['signing'] = {'developer_id': True, 'notarized': True, 'stapled': True,
                              'unsigned_sha256': package['artifact']['sha256']}
        package['artifact'] = dict(name='fixture-signed.dmg', **facts(final))
        final_binary = b'inert final signed executable bytes'
        package['payload']['bin/librairii'].update(facts(final_binary))
        row['signed_candidate'] = self.archive('signed.zip', {'artifact.json': encoded(package),
                                                             'fixture-signed.dmg': final})
        self.mutate(row['signed_candidate'], 'artifact.json', lambda data: encoded(dict(
            json.loads(data), signing=dict(package['signing'], unsigned_sha256='f' * 64))))
        self.assert_refused()
        self.mutate(row['signed_candidate'], 'artifact.json', lambda data: encoded(package))
        self.assert_refused()  # The earlier unsigned inspection cannot qualify final bytes.
        for member in ('inspection/inspection.json', 'gui/qualification.json'):
            self.mutate(row['reports']['installed'], member,
                        lambda data: encoded(dict(json.loads(data), artifact=package['artifact'])))
        self.mutate(row['reports']['installed'], 'inspection/inspection.json',
                    lambda data: encoded(dict(json.loads(data), installed_payload=package['payload'])))
        self.assert_refused()  # Matching metadata alone cannot replace actual final bytes.
        self.mutate(row['reports']['installed'], 'inspection/installed/bin/librairii', lambda data: final_binary)
        result = self.run_manifest()
        self.assertEqual(result['targets'][0]['distribution'], package['artifact'])
        self.assertFalse(result['public_release_qualified'])
        self.assertFalse(result['acceptance_verified'])
