"""Provenance failure regressions use only owned synthetic roots."""
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('source_gate', Path(__file__).resolve().parents[1] / 'check_sources.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


def snapshot(root):
    return {p.relative_to(root).as_posix(): None if p.is_dir() else p.read_bytes()
            for p in root.rglob('*')}


class SourcePolicy(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='librairii-source-policy-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / 'owned Été # %2F'
        source = Path(__file__).resolve().parents[2]
        self.source = source
        (self.root / '.config').mkdir(parents=True)
        shutil.copyfile(source / '.config/upstream-sources.json', self.root / '.config/upstream-sources.json')
        self.query_map_path = self.root / gate.QUERY_MAP
        shutil.copyfile(source / gate.QUERY_MAP, self.query_map_path)
        self.inline_path = self.root / 'crates/librairii-storage/src/migrations.rs'
        self.inline_path.parent.mkdir(parents=True)
        shutil.copyfile(source / 'crates/librairii-storage/src/migrations.rs', self.inline_path)
        for path in ['.config/upstream-sources', '.config/upstream-notices/' + gate.BUNDLE]:
            shutil.copytree(source / path, self.root / path)
        self.registry_path = self.root / '.config/upstream-sources.json'
        self.registry = json.loads(self.registry_path.read_text(encoding='utf-8'))
        self.bundle = self.registry['bundles'][0]
        self.index = json.loads((self.root / self.bundle['source_index']).read_bytes())
        self.private = self.root / gate.DERIVATIVES
        shutil.copytree(source / gate.DERIVATIVES, self.private)
        self.initial_manifest = json.loads((self.private / 'source-manifest.json').read_bytes())
        self.query_map = json.loads(self.query_map_path.read_bytes())

    def save_registry(self, registry):
        self.registry_path.write_text(json.dumps(registry), encoding='utf-8')

    def derivative(self, source_path='golang.org/x/text@v0.43.0/cases/fold.go'):
        original = next(f for p in self.index['packages'] for f in p['files'] if f['path'] == source_path)
        header = bytes.fromhex(original['headerFirst12LinesHex'])
        content = b'\n'.join(line for line in header.splitlines() if line.startswith(b'// Copyright'))
        content += b'\n// Synthetic translated representation; never compiled.\n'
        self.private.mkdir(parents=True, exist_ok=True)
        (self.private / 'synthetic.rs').write_bytes(content)
        selected = gate.selection(source_path)[0]
        record = {'path': 'synthetic.rs', 'bytes': len(content), 'sha256': gate.digest(content),
            'kind': 'translated', 'recipe': 'Synthetic boundary test, not a production translation',
            'sources': [{'path': source_path, 'sha256': original['sha256'],
                'header_sha256': original['first12LinesSha256'], 'selection': selected}],
            'notices': list(gate.NOTICE_REQUIREMENTS[selected])}
        manifest = copy.deepcopy(self.initial_manifest)
        manifest['files'].append(record)
        self.save_manifest(manifest)
        return manifest

    def save_manifest(self, value):
        (self.private / 'source-manifest.json').write_text(json.dumps(value), encoding='utf-8')

    def assert_query_map_rejected(self, changed):
        original = self.query_map_path.read_bytes()
        try:
            data = json.dumps(changed).encode('utf-8')
            self.query_map_path.write_bytes(data)
            registry = copy.deepcopy(self.registry)
            registry['bundles'][0]['query_codec']['map_sha256'] = gate.digest(data)
            self.save_registry(registry)
            # Reach the semantic guard after making this OWNED negative map
            # self-consistent with its registry digest. The production pin is
            # never changed; ordinary raw tampering is covered separately.
            with mock.patch.object(gate, 'QUERY_MAP_SHA', gate.digest(data)):
                with self.assertRaises(ValueError):
                    gate.check(self.root)
        finally:
            self.query_map_path.write_bytes(original)
            self.save_registry(self.registry)

    def test_inline_query_map_and_current_source_are_mandatory(self):
        for path in [self.query_map_path, self.inline_path]:
            original = path.read_bytes()
            try:
                path.unlink()
                with self.subTest(missing=path.relative_to(self.root).as_posix()):
                    with self.assertRaises((ValueError, OSError)):
                        gate.check(self.root)
            finally:
                path.write_bytes(original)
        original = self.inline_path.read_bytes()
        marker = b'mod collection_codec {'
        self.assertEqual(original.count(marker), 1)
        try:
            self.inline_path.write_bytes(original.replace(
                marker, marker + b'\n// Owned negative fixture changes the mapped fragment.'))
            with self.assertRaises(ValueError):
                gate.check(self.root)
        finally:
            self.inline_path.write_bytes(original)

    def test_duplicate_query_map_fields_are_rejected(self):
        original = self.query_map_path.read_bytes()
        value = json.loads(original)
        key = next(iter(value))
        duplicate = json.dumps(key) + ':' + json.dumps(value[key])
        try:
            data = ('{' + duplicate + ',' + original.decode('utf-8').lstrip()[1:]).encode('utf-8')
            self.query_map_path.write_bytes(data)
            registry = copy.deepcopy(self.registry)
            registry['bundles'][0]['query_codec']['map_sha256'] = gate.digest(data)
            self.save_registry(registry)
            with mock.patch.object(gate, 'QUERY_MAP_SHA', gate.digest(data)):
                with self.assertRaisesRegex(ValueError, 'duplicate'):
                    gate.check(self.root)
        finally:
            self.query_map_path.write_bytes(original)
            self.save_registry(self.registry)

    def test_query_lookup_preserves_all_original_49_source_identities_and_historical_index_bytes(self):
        historical_path = (Path('.config/upstream-sources') / gate.BUNDLE / 'source-index.json')
        lookup_bytes = (self.source / historical_path).read_bytes()
        self.assertEqual((self.root / historical_path).read_bytes(), lookup_bytes)
        historical = json.loads(lookup_bytes)
        query_sources = historical.pop('querySourceFiles')
        historical.pop('querySourceProvenance')
        self.assertEqual(len(query_sources), gate.SOURCE_COUNT - 49)
        original_bytes = (json.dumps(historical, indent=2) + '\n').encode('utf-8')
        self.assertEqual(gate.digest(original_bytes),
            '10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99')
        original_sources = [record for package in historical['packages'] for record in package['files']]
        original_sources += historical['supplementalSourceFiles']
        self.assertEqual(len(original_sources), 49)
        observed = gate.check(self.root)['source_identities']
        by_path = {record['path']: record for record in observed}
        self.assertEqual(len(by_path), gate.SOURCE_COUNT)
        for original in original_sources:
            with self.subTest(source=original['path']):
                self.assertEqual(by_path[original['path']], original)

    def test_query_origins_require_exact_set_and_bounded_selection(self):
        self.assertEqual(len(self.query_map['origins']), gate.SOURCE_COUNT - 49)
        for mutation in ['missing', 'extra', 'duplicate', 'selection']:
            changed = copy.deepcopy(self.query_map)
            if mutation == 'missing':
                changed['origins'].pop()
            elif mutation == 'extra':
                extra = copy.deepcopy(changed['origins'][0])
                extra['path'] = 'Go1.26.5/src/encoding/json/unselected.go'
                changed['origins'].append(extra)
            elif mutation == 'duplicate':
                changed['origins'].append(copy.deepcopy(changed['origins'][0]))
            else:
                changed['origins'][0]['selection'] = 'normalization'
            with self.subTest(mutation=mutation):
                self.assert_query_map_rejected(changed)

    def test_query_origin_source_and_header_hashes_cannot_be_repinned(self):
        for field in ['sha256', 'header_sha256']:
            changed = copy.deepcopy(self.query_map)
            changed['origins'][0][field] = '0' * 64
            with self.subTest(field=field):
                self.assert_query_map_rejected(changed)

    def test_inline_query_fragment_hash_and_materialization_cannot_be_fabricated(self):
        for field, replacement in [
                ('sha256', '0' * 64), ('fragment_sha256', '0' * 64),
                ('bytes', 0), ('fragment_bytes', 0),
                ('path', 'crates/librairii-core/src/query/unreleased.rs')]:
            changed = copy.deepcopy(self.query_map)
            changed['current_inline'][field] = replacement
            with self.subTest(field=field):
                self.assert_query_map_rejected(changed)
        for field in ['begin', 'end', 'materialization']:
            changed = copy.deepcopy(self.query_map)
            del changed['current_inline'][field]
            with self.subTest(missing=field):
                self.assert_query_map_rejected(changed)

    def test_future_core_leaf_remains_unmaterialized_and_unapproved(self):
        self.assertIsNone(self.query_map['future_core_leaf'])
        self.assertIsNone(self.bundle['query_codec']['future_core_leaf'])
        fabricated = {'path': 'crates/librairii-core/src/query/unreleased.rs',
            'bytes': 0, 'sha256': gate.digest(b''), 'header_sha256': gate.digest(b'')}
        self.assertFalse((self.root / fabricated['path']).exists())
        changed = copy.deepcopy(self.query_map)
        changed['future_core_leaf'] = fabricated
        self.assert_query_map_rejected(changed)
        registry = copy.deepcopy(self.registry)
        registry['bundles'][0]['query_codec']['future_core_leaf'] = fabricated
        self.save_registry(registry)
        try:
            with self.assertRaises(ValueError):
                gate.check(self.root)
        finally:
            self.save_registry(self.registry)
        self.assertFalse((self.root / fabricated['path']).exists())

    def test_query_map_binds_lookup_and_historical_index_separately(self):
        self.assertNotEqual(self.query_map['lookup_index_sha256'],
            self.query_map['historical_index_sha256'])
        self.assertEqual(self.query_map['lookup_index_sha256'], self.bundle['source_index_sha256'])
        self.assertEqual(self.query_map['historical_index_sha256'],
            '10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99')
        for field in ['lookup_index_sha256', 'historical_index_sha256']:
            changed = copy.deepcopy(self.query_map)
            other = ('historical_index_sha256' if field == 'lookup_index_sha256'
                else 'lookup_index_sha256')
            changed[field] = changed[other]
            with self.subTest(field=field):
                self.assert_query_map_rejected(changed)

    def test_lookup_helper_bindings_cannot_restore_historical_or_fabricated_constants(self):
        self.assertEqual(len(self.query_map['lookup_bindings']), 4)
        for binding in self.query_map['lookup_bindings']:
            path = self.root / binding['path']
            original = path.read_bytes()
            expected = binding['lookup_index_sha256'].encode('ascii')
            self.assertEqual(original.count(expected), 1)
            for replacement in [binding['historical_index_sha256'], '0' * 64]:
                try:
                    content = original.replace(expected, replacement.encode('ascii'))
                    path.write_bytes(content)
                    manifest = copy.deepcopy(self.initial_manifest)
                    record = next(item for item in manifest['files']
                        if self.private / item['path'] == path)
                    record.update(bytes=len(content), sha256=gate.digest(content))
                    self.save_manifest(manifest)
                    with self.subTest(helper=binding['path'], replacement=replacement):
                        with self.assertRaises(ValueError):
                            gate.check(self.root)
                finally:
                    path.write_bytes(original)
                    self.save_manifest(self.initial_manifest)
        changed = copy.deepcopy(self.query_map)
        changed['lookup_bindings'][0]['constant'] = 'INVENTED_LOOKUP_ALIAS'
        self.assert_query_map_rejected(changed)

    def test_historical_reference_bytes_and_original_manifest_records_remain_fixed(self):
        self.assertEqual(len(self.query_map['historical_derivatives']['files']), 19)
        self.assertEqual(len(self.query_map['fixed_references']), 3)
        historical = {record['path']: record
            for record in self.query_map['historical_derivatives']['files']}
        for record in self.query_map['fixed_references']:
            self.assertEqual(record, historical[record['path']])
            path = self.private / record['path']
            original = path.read_bytes()
            self.assertEqual(gate.digest(original), record['sha256'])
            self.assertEqual(len(original), record['bytes'])
            try:
                changed_bytes = original + b'\n'
                path.write_bytes(changed_bytes)
                manifest = copy.deepcopy(self.initial_manifest)
                current = next(item for item in manifest['files'] if item['path'] == record['path'])
                current.update(bytes=len(changed_bytes), sha256=gate.digest(changed_bytes))
                self.save_manifest(manifest)
                with self.subTest(reference=record['path']):
                    with self.assertRaises(ValueError):
                        gate.check(self.root)
            finally:
                path.write_bytes(original)
                self.save_manifest(self.initial_manifest)

    def test_registered_sources_and_notices_pass_without_writes(self):
        before = snapshot(self.root)
        report = gate.check(self.root)
        self.assertEqual(len(report['source_identities']), gate.SOURCE_COUNT)
        self.assertEqual(report['state'], 'private-provenance-recorded')
        self.assertIsNone(report['product_license'])
        self.assertEqual(report['private_derivatives'], self.initial_manifest['files'])
        self.assertEqual(len(report['private_derivatives']), 19)
        self.assertIn('regionTypes.ccTLD-bit1-value2', report['excluded'])
        self.assertEqual(snapshot(self.root), before)

    def test_missing_changed_or_extra_source_metadata_and_notices_fail(self):
        for relative in [self.bundle['source_index'],
                         self.bundle['notices_root'] + '/LICENSE-CLDR32-UNICODE.txt']:
            path = self.root / relative
            original = path.read_bytes()
            for change in [None, original + b'\n']:
                with self.subTest(path=relative, change=change is None):
                    if change is None:
                        path.unlink()
                    else:
                        path.write_bytes(change)
                    with self.assertRaises((ValueError, OSError)):
                        gate.check(self.root)
                    path.write_bytes(original)
        for parent in [self.bundle['notices_root'], str(Path(self.bundle['source_index']).parent)]:
            path = self.root / parent / 'unrecorded.txt'
            path.write_bytes(b'unrecorded synthetic input')
            with self.assertRaises(ValueError):
                gate.check(self.root)
            path.unlink()

        notice = self.bundle['notices'][0]
        path = self.root / self.bundle['notices_root'] / notice['path']
        original = path.read_bytes()
        replacement = b'Forged upstream terms in an owned negative fixture.\n'
        path.write_bytes(replacement)
        changed = copy.deepcopy(self.registry)
        changed_notice = changed['bundles'][0]['notices'][0]
        changed_notice.update(bytes=len(replacement), sha256=gate.digest(replacement), upstream_path=None)
        self.save_registry(changed)
        with self.assertRaisesRegex(ValueError, 'notice origin'):
            gate.check(self.root)
        # A self-consistent re-pin cannot turn an upstream notice into authored policy.
        path.write_bytes(original)
        self.save_registry(self.registry)

    def test_alias_or_traversal_paths_are_rejected_before_reading(self):
        for relative in ['../outside', '/absolute', 'C:/absolute', 'a\\b', 'a//b', 'a/./b']:
            with self.subTest(path=relative), self.assertRaises(ValueError):
                gate.safe(self.root, relative)
        # No native link privilege is required to exercise the fail-closed branch.
        with mock.patch.object(Path, 'is_symlink', lambda path: path.name == 'source-index.json'):
            with self.assertRaisesRegex(ValueError, 'aliased'):
                gate.check(self.root)
        self.assertFalse((self.root.parent / 'outside').exists())

    def test_private_derivatives_require_complete_source_and_content_records(self):
        manifest = self.derivative()
        self.assertEqual(len(gate.check(self.root)['private_derivatives']), 20)
        original = (self.private / 'synthetic.rs').read_bytes()
        (self.private / 'synthetic.rs').write_bytes(original + b'changed')
        with self.assertRaisesRegex(ValueError, 'changed'):
            gate.check(self.root)
        (self.private / 'synthetic.rs').write_bytes(original)
        (self.private / 'unrecorded.rs').write_bytes(b'foreign synthetic file')
        with self.assertRaisesRegex(ValueError, 'unrecorded'):
            gate.check(self.root)
        (self.private / 'unrecorded.rs').unlink()
        for field, replacement in [('recipe', ''), ('sources', []), ('notices', [])]:
            changed = copy.deepcopy(manifest)
            changed['files'][-1][field] = replacement
            self.save_manifest(changed)
            with self.subTest(field=field), self.assertRaises(ValueError):
                gate.check(self.root)
        self.save_manifest(manifest)
        (self.private / 'source-manifest.json').unlink()
        with self.assertRaises(OSError):
            gate.check(self.root)

    def test_dropped_header_and_unapproved_dns_selection_fail(self):
        manifest = self.derivative()
        content = b'// dropped upstream copyright\n'
        (self.private / 'synthetic.rs').write_bytes(content)
        manifest['files'][-1].update(bytes=len(content), sha256=gate.digest(content))
        self.save_manifest(manifest)
        with self.assertRaisesRegex(ValueError, 'copyright'):
            gate.check(self.root)
        manifest = self.derivative('golang.org/x/text@v0.43.0/internal/language/tables.go')
        self.assertEqual(len(gate.check(self.root)['private_derivatives']), 20)
        for field, value in [('selection', 'language-default-whole-table'),
                             ('sha256', '0' * 64), ('header_sha256', '0' * 64)]:
            changed = copy.deepcopy(manifest)
            changed['files'][-1]['sources'][0][field] = value
            self.save_manifest(changed)
            with self.subTest(field=field), self.assertRaises(ValueError):
                gate.check(self.root)

    def test_inventory_contains_selected_sources_and_copies_exact_notices(self):
        before = snapshot(self.root / '.config')
        report = gate.check(self.root)
        gate.inventory(self.root, report)
        destination = self.root / 'target/quality/notices' / gate.BUNDLE
        for record in report['notices']:
            source = self.root / self.bundle['notices_root'] / record['path']
            self.assertEqual((destination / record['path']).read_bytes(), source.read_bytes())
        summary = (destination.parent / 'NOTICES-frozen-sources.txt').read_text(encoding='utf-8')
        self.assertIn('Product license remains undecided', summary)
        self.assertIn('private-provenance-recorded', summary)
        self.assertEqual(snapshot(self.root / '.config'), before)

    def test_unlicensed_repository_and_scope_cannot_be_reassigned(self):
        changed = copy.deepcopy(self.registry)
        changed['product_license'] = 'BSD-3-Clause'
        self.save_registry(changed)
        with self.assertRaises(ValueError):
            gate.check(self.root)

        changed = copy.deepcopy(self.registry)
        changed['bundles'][0]['scope'] = 'public-core-query'
        self.save_registry(changed)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        changed = copy.deepcopy(self.registry)
        path = 'golang.org/x/text@v0.43.0/internal/language/tables.go'
        changed['bundles'][0]['source_selections'][path] = ['normalization']
        self.save_registry(changed)
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_fresh_autocrlf_checkout_runs_actual_checker_and_rejects_unprotected_bytes(self):
        source = Path(__file__).resolve().parents[2]
        self.derivative()
        shutil.copyfile(source / '.gitattributes', self.root / '.gitattributes')
        (self.root / 'xtask').mkdir()
        shutil.copyfile(source / 'xtask/check_sources.py', self.root / 'xtask/check_sources.py')
        hooks = self.root.parent / 'empty-hooks'
        hooks.mkdir()

        def git(*args):
            return subprocess.run(['git', '-c', 'core.hooksPath=' + str(hooks),
                '-c', 'commit.gpgsign=false', '-c', 'user.name=Synthetic fixture',
                '-c', 'user.email=fixture@example.invalid', *args],
                cwd=self.root.parent, capture_output=True, check=True)

        git('init', str(self.root))
        git('-C', str(self.root), 'add', '.')
        git('-C', str(self.root), 'commit', '-m', 'synthetic source-policy fixture')
        for mode in ['protected', 'negative-index', 'negative-derivative']:
            fresh = self.root.parent / ('fresh ' + mode + ' Été #%2F')
            git('clone', '--no-local', '--no-checkout', '--config', 'core.autocrlf=true', str(self.root), str(fresh))
            if mode != 'protected':
                # Owned negative controls override upstream or private byte protection.
                attributes = fresh / '.git/info/attributes'
                attributes.parent.mkdir(exist_ok=True)
                rule = ('.config/upstream-sources/** text\n' if mode == 'negative-index'
                    else gate.DERIVATIVES + '/** text\n')
                attributes.write_text(rule, encoding='utf-8')
            git('-C', str(fresh), 'checkout', '--detach', 'HEAD')
            before = snapshot(self.root.parent)
            result = subprocess.run([sys.executable, '-I', '-B', '-X', 'utf8',
                str(fresh / 'xtask/check_sources.py')], cwd=self.root.parent,
                capture_output=True, encoding='utf-8')
            self.assertEqual(snapshot(self.root.parent), before)
            if mode == 'protected':
                self.assertEqual(result.returncode, 0, result.stderr)
                report = json.loads(result.stdout)
                self.assertEqual(len(report['source_identities']), gate.SOURCE_COUNT)
                self.assertEqual(len(report['private_derivatives']), 20)
            else:
                self.assertNotEqual(result.returncode, 0)
                if mode == 'negative-index':
                    self.assertIn('changed authenticated source index', result.stderr)
                    converted = fresh / self.bundle['source_index']
                else:
                    self.assertIn('changed source-policy file', result.stderr)
                    converted = fresh / gate.DERIVATIVES / 'synthetic.rs'
                self.assertIn(b'\r\n', converted.read_bytes())
