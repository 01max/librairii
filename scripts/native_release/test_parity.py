"""Archive mutations keep observed execution separate from parity approval."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest
from unittest import mock
import xml.etree.ElementTree as ET
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release import parity
from native_release import test_evidence as fixtures


class ParityObservations(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.EvidenceManifest()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.plan = self.fixture.plan
        self.input = self.root / 'input.json'
        self.output = self.root / 'parity.json'
        self.contract = json.loads((parity.ROOT / parity.CONTRACT).read_bytes())
        self.sources = {}
        for key in ('planning_register', 'implementation_index'):
            relative = self.contract[key]['path']
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((parity.ROOT / relative).read_bytes())
        index = json.loads((self.root / self.contract['implementation_index']['path']).read_bytes())
        for row in index['rows']:
            self.sources.update({p: (parity.ROOT / p).read_bytes() for p in row['source_sha256']})
        for row in self.contract['rows']:
            for case in row['automated_cases']:
                self.sources[case['source']] = (parity.ROOT / case['source']).read_bytes()
        identities = sorted({(c['suite'], c['name']) for r in self.contract['rows'] for c in r['automated_cases']})
        required = [{'suite': s, 'name': n} for s, n in identities]
        self.sources['.config/rust-test-baseline.json'] = fixtures.encoded({'required': required})
        suites = {}
        for suite, name in identities:
            record = suites.setdefault(suite, {'status': 'listed', 'binary-id': suite,
                                               'build-platform': 'target', 'testcases': {}})
            record['testcases'][name] = {'ignored': False, 'filter-match': {'status': 'matches'}}
        junit = ET.Element('testsuites', tests=str(len(identities)), skipped='0', failures='0', errors='0')
        for suite, name in identities:
            ET.SubElement(junit, 'testcase', classname=suite, name=name)
        callbacks = {c: 'passed' for r in self.contract['rows'] for c in r['native_callbacks']}
        checks = {c: {'synthetic': True} for r in self.contract['rows'] for c in r['installed_checks']}
        for row in self.plan['targets']:
            target = row['target']
            files = {}
            for scale in ('1', '2'):
                member = 'scale-' + scale + '/report.json'
                raw = fixtures.encoded(callbacks)
                files[member] = hashlib.sha256(raw).hexdigest()
                self.mutate(row['reports']['quality'], 'quality/native-ui/' + member, lambda _: raw)
            self.mutate(row['reports']['quality'], 'quality/native-ui/manifest.json',
                                lambda raw: fixtures.encoded(dict(json.loads(raw), files=files)))
            discovery = {'rust-build-meta': {'platforms': {'host': {'platform': {'triple': target}},
                         'targets': [{'platform': {'triple': target}}]}},
                         'rust-suites': suites, 'test-count': len(identities)}
            self.mutate(row['reports']['quality'], 'quality/test-discovery.json', lambda _: fixtures.encoded(discovery))
            self.mutate(row['reports']['quality'], 'nextest/ci/junit.xml', lambda _: ET.tostring(junit))
            package = self.fixture.packages[target]
            names = ['x11-fresh', 'x11-upgraded', 'wayland-fresh', 'wayland-upgraded'] if 'linux' in target else ['fresh', 'upgraded']
            method = {'format': 'librairii.packaged-gui-method', 'schema': 1,
                      'source_commit': fixtures.SOURCE, 'files': {'observer.py': fixtures.facts(b'observer')}}
            processes = [{'name': name, 'retired': True} for name in
                         ('workflow', 'reopen', 'recovery', 'reinstall', 'offline')]
            lanes = []
            for name in names:
                member = 'gui/' + name + ('/case/' if 'linux' in target else '/')
                raw = b'inert raw observation'
                self.mutate(row['reports']['installed'], member + 'observed.txt', lambda _: raw)
                case = 'fresh' if name.endswith('fresh') else 'upgraded'
                report = {'format': 'librairii.packaged-gui', 'schema': 1,
                          'source_commit': fixtures.SOURCE, 'target': target, 'artifact': package['artifact'],
                          'host': 'synthetic', 'case': case,
                          'session': name.split('-', 1)[0] if 'linux' in target else 'macOS',
                          'backend': 'winit-skia' if case == 'fresh' else 'winit-skia-software',
                          'method': method, 'processes': processes, 'signing': package['signing'],
                          'public_release_qualified': False, 'complete': True, 'errors': [],
                          'checks': checks, 'files': {'observed.txt': fixtures.facts(raw)}}
                report_data = fixtures.encoded(report)
                self.mutate(row['reports']['installed'], member + 'report.json', lambda _: report_data)
                lanes.append({'name': name, 'passed': True, 'checks': list(checks),
                              'processes': processes, 'sha256': hashlib.sha256(report_data).hexdigest()})
            self.mutate(row['reports']['installed'], 'gui/qualification.json',
                                lambda raw: fixtures.encoded(dict(json.loads(raw), host='synthetic',
                                                                   method=method, lanes=lanes)))
        self.save_contract()

    def mutate(self, record, name, change):
        with zipfile.ZipFile(self.root / record["path"]) as z:
            members = {n: z.read(n) for n in z.namelist()}
        members[name] = change(members.get(name, b""))
        record.update(self.fixture.archive(record["path"], members))

    def save_contract(self):
        path = self.root / parity.CONTRACT
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(fixtures.encoded(self.contract))

    def run_audit(self):
        self.input.write_bytes(fixtures.encoded(self.plan))
        with mock.patch.object(parity, 'ROOT', self.root), \
                mock.patch.object(parity, 'source_bytes', side_effect=lambda source, p: self.sources[p]), \
                mock.patch.object(parity.subprocess, 'check_output', return_value=fixtures.TREE + '\n'):
            return parity.audit(self.input, self.output)

    def refused(self):
        with self.assertRaises((ValueError, KeyError)):
            self.run_audit()
        self.assertFalse(self.output.exists())

    def test_all_sixty_rows_observe_exact_cases_without_claiming_parity_or_release(self):
        before = {p: p.read_bytes() for p in self.root.rglob('*') if p.is_file()}
        result = self.run_audit()
        self.assertEqual(len(result['rows']), 60)
        self.assertEqual(len(result['targets']), 3)
        self.assertTrue(result['reviewed_source_sha256'])
        for flag in ('provider_authenticated', 'semantic_coverage_verified',
                     'numerical_acceptance_verified', 'public_release_qualified'):
            self.assertIs(result[flag], False)
        self.assertTrue(all(p.read_bytes() == raw for p, raw in before.items()))
        with self.assertRaises(FileExistsError):
            self.run_audit()

    def test_missing_failed_skipped_duplicate_or_foreign_native_execution_is_refused(self):
        record = self.plan['targets'][0]['reports']['quality']
        path = self.root / record['path']
        original = path.read_bytes()
        def failed(raw):
            root = ET.fromstring(raw)
            ET.SubElement(next(root.iter('testcase')), 'failure')
            return ET.tostring(root)
        def duplicate(raw):
            root = ET.fromstring(raw)
            root.append(copy.deepcopy(next(root.iter('testcase'))))
            return ET.tostring(root)
        changes = [('nextest/ci/junit.xml', failed), ('nextest/ci/junit.xml', duplicate),
                   ('nextest/ci/junit.xml', lambda raw: raw.replace(b'skipped="0"', b'skipped="1"')),
                   ('quality/test-discovery.json', lambda raw: fixtures.encoded({**json.loads(raw), "test-count": 0})),
                   ('quality/source.txt', lambda _: b'f' * 40)]
        for member, change in changes:
            with self.subTest(member=member):
                self.mutate(record, member, change)
                self.refused()
                path.write_bytes(original)
                record.update(fixtures.facts(original))

    def test_missing_callback_or_changed_sealed_observation_cannot_pass(self):
        record = self.plan['targets'][0]['reports']['quality']
        self.mutate(record, 'quality/native-ui/scale-1/report.json', lambda _: b'{}')
        self.refused()
        cases = {(c['suite'], c['name']) for c in self.contract['rows'][0]['automated_cases']}
        with self.assertRaises(ValueError):
            parity.row_observations(self.contract['rows'][0], cases, {'report.json': {}}, {})

    def test_installed_lane_stage_payload_and_hash_changes_are_refused(self):
        record = self.plan['targets'][0]['reports']['installed']
        path = self.root / record['path']
        original = path.read_bytes()
        for member, change in [
                ('gui/qualification.json', lambda raw: fixtures.encoded(dict(json.loads(raw), lanes=[]))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), checks={}))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), format='other'))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), case='upgraded'))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), session='wayland'))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), backend='winit-skia-software'))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), method={}))),
                ('gui/fresh/report.json', lambda raw: fixtures.encoded(dict(json.loads(raw), processes=[]))),
                ('gui/fresh/observed.txt', lambda _: b'changed'),
                ('inspection/installed/bin/librairii', lambda _: b'changed installed bytes')]:
            with self.subTest(member=member):
                self.mutate(record, member, change)
                self.refused()
                path.write_bytes(original)
                record.update(fixtures.facts(original))
        with zipfile.ZipFile(path) as z:
            fresh = z.read('gui/fresh/report.json')
        self.mutate(record, 'gui/upgraded/report.json', lambda _: fresh)
        def reuse_fresh(raw):
            qualification = json.loads(raw)
            qualification['lanes'][1]['sha256'] = hashlib.sha256(fresh).hexdigest()
            return fixtures.encoded(qualification)
        self.mutate(record, 'gui/qualification.json', reuse_fresh)
        self.refused()

    def test_contract_omission_owner_source_and_unknown_decision_are_refused(self):
        original = copy.deepcopy(self.contract)
        for mutate in [lambda c: c['rows'].pop(),
                       lambda c: c['rows'][0].update(accountable_owner=''),
                       lambda c: c['rows'][0]['automated_cases'][0].update(source_sha256='0' * 64),
                       lambda c: c['rows'][0]['scope_decisions'].append('unapproved-exclusion')]:
            mutate(self.contract)
            self.save_contract()
            self.refused()
            self.contract = copy.deepcopy(original)

    def test_all_three_targets_and_each_mapped_case_are_required(self):
        removed = self.plan['targets'].pop()
        self.refused()
        self.plan['targets'].append(removed)
        candidate = self.plan['targets'][0]['candidate']
        for member in ('package/artifact.json', 'stage/stage.json'):
            self.mutate(candidate, member, lambda raw: fixtures.encoded(dict(json.loads(raw), version='9.9.9')))
        with self.assertRaisesRegex(ValueError, 'candidate versions, source or graph differ across targets'):
            self.run_audit()
        self.assertFalse(self.output.exists())
        with self.assertRaises(ValueError):
            parity.row_observations(self.contract['rows'][0], set(), {}, {})

    def test_parsed_input_bytes_cannot_be_bound_to_another_plans_hash(self):
        other = copy.deepcopy(self.plan)
        other['targets'][0]['reports']['quality']['artifact_id'] += 1000
        read = Path.read_bytes
        def replaced(path):
            return fixtures.encoded(other) if path == self.input else read(path)
        with mock.patch.object(Path, 'read_bytes', replaced):
            self.refused()

    def test_incomplete_scale_and_unsigned_boundary_remain_explicit(self):
        row = self.plan['targets'][0]
        self.mutate(row['reports']['scale'], 'desktop/manifest.json',
                            lambda raw: fixtures.encoded(dict(json.loads(raw), complete=False)))
        self.refused()
        row['signed_candidate'] = row['candidate']
        self.refused()
