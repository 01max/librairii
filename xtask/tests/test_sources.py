"""Provenance failure regressions use only owned synthetic roots."""
import copy
import importlib.util
import json
import os
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


def bundle(registry):
    return next(entry for entry in registry['bundles'] if entry['id'] == gate.BUNDLE)


class SourcePolicy(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='librairii-source-policy-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / 'owned Été # %2F'
        source = Path(__file__).resolve().parents[2]
        self.source = source
        (self.root / '.config').mkdir(parents=True)
        shutil.copyfile(source / '.config/upstream-sources.json', self.root / '.config/upstream-sources.json')
        for name in ['rust-test-baseline.json', 'rust-python-test-baseline.json']:
            shutil.copyfile(source / '.config' / name, self.root / '.config' / name)
        self.query_map_path = self.root / gate.QUERY_MAP
        shutil.copyfile(source / gate.QUERY_MAP, self.query_map_path)
        self.storage_path = self.root / gate.STORAGE_CALLER
        self.storage_path.parent.mkdir(parents=True)
        # Policy fixture only: the released storage child will perform this
        # delegation. Never copy the retired inline implementation as authority.
        self.storage_path.write_text('use librairii_core::collection_codec;\n', encoding='utf-8')
        self.core_path = self.root / gate.CORE_PATH
        self.core_path.parent.mkdir(parents=True)
        shutil.copyfile(source / gate.CORE_PATH, self.core_path)
        for name in (*gate.RUNTIME_LEAVES, 'crates/librairii-core/src/collection_codec/url.rs', 'crates/librairii-core/src/tagging/unicode15.rs', 'xtask/extract_tag_categories.py'):
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / name, destination)
        for path in ['.config/upstream-sources', '.config/upstream-notices/' + gate.BUNDLE]:
            shutil.copytree(source / path, self.root / path)
        self.registry_path = self.root / '.config/upstream-sources.json'
        self.registry = json.loads(self.registry_path.read_text(encoding='utf-8'))
        self.bundle = bundle(self.registry)
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
            bundle(registry)['query_codec']['map_sha256'] = gate.digest(data)
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

    def selected_derivatives(self):
        # Reset an OWNED fixture to the actual relocated nineteen-file closure.
        if self.private.exists():
            shutil.rmtree(self.private)
        shutil.copytree(self.source / gate.DERIVATIVES, self.private)

    def assert_selected_rejected_without_writes(self):
        before = snapshot(self.root.parent)
        with self.assertRaises(ValueError):
            gate.check_selected(self.root)
        self.assertEqual(snapshot(self.root.parent), before)


    def test_materialized_core_query_map_and_current_source_are_mandatory(self):
        for path in [self.query_map_path, self.core_path]:
            original = path.read_bytes()
            try:
                path.unlink()
                with self.subTest(missing=path.relative_to(self.root).as_posix()):
                    with self.assertRaises((ValueError, OSError)):
                        gate.check(self.root)
            finally:
                path.write_bytes(original)
        original = self.core_path.read_bytes()
        try:
            self.core_path.write_bytes(original + b'\n// Owned negative fixture changes the materialized core leaf.\n')
            with self.assertRaises(ValueError):
                gate.check(self.root)
        finally:
            self.core_path.write_bytes(original)

    def test_duplicate_query_map_fields_are_rejected(self):
        original = self.query_map_path.read_bytes()
        value = json.loads(original)
        key = next(iter(value))
        duplicate = json.dumps(key) + ':' + json.dumps(value[key])
        try:
            data = ('{' + duplicate + ',' + original.decode('utf-8').lstrip()[1:]).encode('utf-8')
            self.query_map_path.write_bytes(data)
            registry = copy.deepcopy(self.registry)
            bundle(registry)['query_codec']['map_sha256'] = gate.digest(data)
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
        historical.pop('runtimeSourceFiles')
        historical.pop('runtimeSourceProvenance')
        query_sources = historical.pop('querySourceFiles')
        historical.pop('querySourceProvenance')
        self.assertEqual(len(query_sources), len(gate.QUERY_SELECTIONS))
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
        self.assertEqual(len(self.query_map['origins']), len(gate.QUERY_SELECTIONS))
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

    def test_historical_inline_identity_and_materialized_core_leaf_cannot_be_fabricated(self):
        for field, replacement in [
                ('sha256', '0' * 64), ('fragment_sha256', '0' * 64),
                ('bytes', 0), ('fragment_bytes', 0),
                ('path', 'crates/librairii-core/src/query/unreleased.rs')]:
            changed = copy.deepcopy(self.query_map)
            changed['historical_inline'][field] = replacement
            with self.subTest(historical_field=field):
                self.assert_query_map_rejected(changed)
        for field in ['begin', 'end', 'materialization']:
            changed = copy.deepcopy(self.query_map)
            del changed['historical_inline'][field]
            with self.subTest(missing_historical_field=field):
                self.assert_query_map_rejected(changed)
        for field, replacement in [('path', 'crates/librairii-storage/src/migrations.rs'),
                                   ('bytes', 0), ('sha256', '0' * 64), ('kind', 'authored')]:
            changed = copy.deepcopy(self.query_map)
            changed['core_leaf'][field] = replacement
            with self.subTest(core_field=field):
                self.assert_query_map_rejected(changed)

    def test_materialized_core_headers_and_removed_storage_duplicates_are_enforced(self):
        self.assertEqual(self.query_map['core_leaf']['path'], gate.CORE_PATH)
        self.assertNotIn('future_core_leaf', self.query_map)
        self.assertNotIn('future_core_leaf', self.bundle['query_codec'])
        original = self.core_path.read_bytes()
        originals = self.index['querySourceFiles']
        expected_headers = {line for record in originals
            for line in bytes.fromhex(record['headerFirst12LinesHex']).splitlines()
            if line.startswith(b'// Copyright')}
        self.assertTrue(expected_headers)
        for line in expected_headers:
            with self.subTest(header=line):
                self.assertIn(line, original)
        stripped = b'\n'.join(line for line in original.split(b'\n')
            if not line.startswith(b'// Copyright'))
        try:
            self.core_path.write_bytes(stripped)
            changed = copy.deepcopy(self.query_map)
            changed['core_leaf'].update(bytes=len(stripped), sha256=gate.digest(stripped))
            self.assert_query_map_rejected(changed)
        finally:
            self.core_path.write_bytes(original)
        changed = copy.deepcopy(self.query_map)
        changed['future_core_leaf'] = None
        self.assert_query_map_rejected(changed)
        old_private = self.root / 'crates/librairii-storage/src/migrations/compatibility'
        self.assertFalse(old_private.exists())
        shutil.copytree(self.private, old_private)
        try:
            before = snapshot(self.root)
            with self.assertRaises(ValueError):
                gate.check(self.root)
            self.assertEqual(snapshot(self.root), before)
        finally:
            shutil.rmtree(old_private)
        storage = self.storage_path.read_bytes()
        for duplicate in [b'mod compatibility;\n', b'mod collection_codec {\n}\n']:
            try:
                self.storage_path.write_bytes(storage + duplicate)
                before = snapshot(self.root)
                with self.subTest(duplicate=duplicate), self.assertRaises(ValueError):
                    gate.check(self.root)
                self.assertEqual(snapshot(self.root), before)
            finally:
                self.storage_path.write_bytes(storage)

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

    def test_runtime_query_index_retains_all_prior_62_identities_and_prior_lookup_bytes(self):
        previous = copy.deepcopy(self.index)
        added = previous.pop('runtimeSourceFiles')
        provenance = previous.pop('runtimeSourceProvenance')
        self.assertEqual(gate.digest((json.dumps(previous, indent=2) + '\n').encode()),
            '13dfd314abfda9a9553d152a1d8a5586f8bd1205a08c177af9bbec57a51ae822')
        self.assertEqual(provenance['previousQuerySourceIndexSha256'], gate.PREVIOUS_QUERY_INDEX_SHA)
        self.assertEqual([item['path'] for item in added],
            ['oracle73a93303/internal/library/query.go'])
        prior = [item for package in previous['packages'] for item in package['files']]
        prior += previous['supplementalSourceFiles'] + previous['querySourceFiles']
        self.assertEqual(len(prior), 62)
        observed = {item['path']: item for item in gate.check(self.root)['source_identities']}
        self.assertEqual(len(observed), 63)
        for item in prior:
            with self.subTest(origin=item['path']):
                self.assertEqual(observed[item['path']], item)
        changed = copy.deepcopy(self.query_map)
        changed['previous_query_index_sha256'] = changed['historical_index_sha256']
        self.assert_query_map_rejected(changed)

    def test_runtime_query_leaves_require_materialized_exact_bytes_and_headers(self):
        self.assertEqual([item['path'] for item in self.query_map['runtime']['leaves']],
            list(gate.RUNTIME_LEAVES))
        for record in self.query_map['runtime']['leaves']:
            path = self.root / record['path']
            original = path.read_bytes()
            for changed in [None, original + b'\n// Owned runtime leaf mutation.\n']:
                try:
                    if changed is None:
                        path.unlink()
                    else:
                        path.write_bytes(changed)
                    before = snapshot(self.root)
                    with self.subTest(leaf=record['path'], missing=changed is None):
                        with self.assertRaises((ValueError, OSError)):
                            gate.check(self.root)
                    self.assertEqual(snapshot(self.root), before)
                finally:
                    path.write_bytes(original)
        for mutation in ['missing', 'extra', 'duplicate', 'header', 'kind']:
            changed = copy.deepcopy(self.query_map)
            leaves = changed['runtime']['leaves']
            if mutation == 'missing':
                leaves.pop()
            elif mutation in ('extra', 'duplicate'):
                leaves.append(copy.deepcopy(leaves[0]))
                if mutation == 'extra':
                    leaves[-1]['path'] = 'crates/librairii-core/src/query/unselected.rs'
            elif mutation == 'header':
                leaves[0]['header_sha256'] = '0' * 64
            else:
                leaves[0]['kind'] = 'authored'
            with self.subTest(mutation=mutation):
                self.assert_query_map_rejected(changed)
        extra = self.root / 'crates/librairii-core/src/query/unselected.rs'
        extra.write_bytes(b'// No selected runtime origin.\n')
        before = snapshot(self.root)
        with self.assertRaisesRegex(ValueError, 'extra materialized runtime'):
            gate.check(self.root)
        self.assertEqual(snapshot(self.root), before)

    def test_runtime_query_origins_and_scope_cannot_expand_to_sql_or_saved_conversion(self):
        self.assertEqual(self.query_map['runtime']['origins'], [
            {'path': path, 'sha256': next(item['sha256'] for item in
                self.index['supplementalSourceFiles'] + self.index['runtimeSourceFiles']
                if item['path'] == path),
             'header_sha256': next(item['first12LinesSha256'] for item in
                self.index['supplementalSourceFiles'] + self.index['runtimeSourceFiles']
                if item['path'] == path), 'selection': role}
            for path, role in gate.RUNTIME_SELECTIONS.items()])
        for mutation in ['missing', 'duplicate', 'foreign', 'selection', 'source_hash',
                         'source_header', 'scope', 'page_scope', 'excluded']:
            changed = copy.deepcopy(self.query_map)
            runtime = changed['runtime']
            if mutation == 'missing':
                runtime['origins'].pop()
            elif mutation == 'duplicate':
                runtime['origins'].append(copy.deepcopy(runtime['origins'][0]))
            elif mutation == 'foreign':
                runtime['origins'][0]['path'] = 'oracle73a93303/internal/library/unselected.go'
            elif mutation == 'selection':
                runtime['origins'][0]['selection'] = 'query-product'
            elif mutation == 'source_hash':
                runtime['origins'][0]['sha256'] = '0' * 64
            elif mutation == 'source_header':
                runtime['origins'][0]['header_sha256'] = '0' * 64
            elif mutation == 'scope':
                runtime['scope'] = 'all-library-query-and-saved-canonicalization'
            elif mutation == 'page_scope':
                runtime['selection_scopes']['runtime-query-page'] += ' Also execute SQL.'
            else:
                runtime['excluded'] = []
            with self.subTest(mutation=mutation):
                self.assert_query_map_rejected(changed)
        registry = copy.deepcopy(self.registry)
        registry['bundles'][0]['source_selections'][
            'oracle73a93303/internal/library/query.go'].append('query-product')
        self.save_registry(registry)
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_runtime_query_binding_does_not_relax_saved_derivatives_or_notice_guards(self):
        changed = copy.deepcopy(self.query_map)
        changed['runtime']['notices'] = ['LICENSE-GO-BSD.txt']
        self.assert_query_map_rejected(changed)
        path = self.private / 'normalization_data.rs'
        original = path.read_bytes()
        changed_bytes = original + b'\n// A runtime source does not authorize new frozen data.\n'
        try:
            path.write_bytes(changed_bytes)
            manifest = copy.deepcopy(self.initial_manifest)
            record = next(item for item in manifest['files'] if item['path'] == path.name)
            record.update(bytes=len(changed_bytes), sha256=gate.digest(changed_bytes))
            self.save_manifest(manifest)
            with self.assertRaisesRegex(ValueError, 'historical derivative'):
                gate.check(self.root)
        finally:
            path.write_bytes(original)
            self.save_manifest(self.initial_manifest)
        notice = self.root / self.bundle['notices_root'] / 'LICENSE-UNICODE-V3.txt'
        original = notice.read_bytes()
        try:
            notice.write_bytes(original + b'\n')
            before = snapshot(self.root)
            with self.assertRaises(ValueError):
                gate.check(self.root)
            self.assertEqual(snapshot(self.root), before)
        finally:
            notice.write_bytes(original)

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

        retained = self.root.parent / 'owned-retained-selected-closure'
        self.private.rename(retained)
        self.assert_selected_rejected_without_writes()
        retained.rename(self.private)
        for name in ['mod.rs', 'source-manifest.json']:
            with self.subTest(missing=name):
                path = self.private / name
                original = path.read_bytes()
                path.unlink()
                self.assert_selected_rejected_without_writes()
                path.write_bytes(original)

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
        changed_notice = bundle(changed)['notices'][0]
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

        self.selected_derivatives()
        manifest_path = self.private / 'source-manifest.json'
        original_manifest = manifest_path.read_bytes()
        path = self.private / 'normalization.rs'
        original = path.read_bytes()
        path.write_bytes(original + b'\n// owned changed-byte control\n')
        self.assert_selected_rejected_without_writes()
        manifest = json.loads(original_manifest)
        record = next(row for row in manifest['files'] if row['path'] == path.name)
        record.update(bytes=path.stat().st_size, sha256=gate.digest(path.read_bytes()))
        self.save_manifest(manifest)
        # The materialized core map and CLI selected-byte gate both reject re-pins.
        with self.assertRaises(ValueError):
            gate.check(self.root)
        self.assert_selected_rejected_without_writes()
        path.write_bytes(original)
        manifest_path.write_bytes(original_manifest)
        manifest = json.loads(original_manifest)
        record = next(row for row in manifest['files'] if row['path'] == 'normalization_data.rs')
        record['kind'] = 'translated'
        self.save_manifest(manifest)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        self.assert_selected_rejected_without_writes()
        manifest_path.write_bytes(original_manifest)
        path = self.private / 'verify_codec.py'
        renamed = self.private / 'unselected_verify_codec.py'
        path.rename(renamed)
        manifest = json.loads(original_manifest)
        record = next(row for row in manifest['files'] if row['path'] == path.name)
        record['path'] = renamed.name
        self.save_manifest(manifest)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        self.assert_selected_rejected_without_writes()

        self.selected_derivatives()
        for directory in [False, True]:
            extra = self.private / 'unselected'
            if directory:
                extra.mkdir()
            else:
                extra.write_bytes(b'owned unselected control')
            self.assert_selected_rejected_without_writes()
            if directory:
                extra.rmdir()
            else:
                extra.unlink()
        path = self.private / 'mod.rs'
        peer = self.root.parent / 'owned-hard-link-peer'
        path.rename(peer)
        os.link(peer, path)
        try:
            self.assert_selected_rejected_without_writes()
        finally:
            path.unlink()
            peer.rename(path)

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
        reordered = copy.deepcopy(self.registry)
        reordered['bundles'].reverse()
        self.save_registry(reordered)
        before = snapshot(self.root.parent)
        self.assertEqual(gate.check(self.root)['bundle'], gate.BUNDLE)
        self.assertEqual(snapshot(self.root.parent), before)
        own = bundle(self.registry)
        other = next(entry for entry in self.registry['bundles'] if entry['id'] != gate.BUNDLE)
        for entries in [[], [own], [other], [own, own], [other, other],
                        [own, other, {'id': 'unapproved-source-bundle'}],
                        [own, {'id': 'unapproved-source-bundle'}]]:
            with self.subTest(ids=[entry['id'] for entry in entries]):
                changed = copy.deepcopy(self.registry)
                changed['bundles'] = entries
                self.save_registry(changed)
                before = snapshot(self.root.parent)
                with self.assertRaises(ValueError):
                    gate.check(self.root)
                self.assertEqual(snapshot(self.root.parent), before)
        self.save_registry(self.registry)

        changed = copy.deepcopy(self.registry)
        changed['product_license'] = 'BSD-3-Clause'
        self.save_registry(changed)
        with self.assertRaises(ValueError):
            gate.check(self.root)

        changed = copy.deepcopy(self.registry)
        bundle(changed)['scope'] = 'public-core-query'
        self.save_registry(changed)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        changed = copy.deepcopy(self.registry)
        path = 'golang.org/x/text@v0.43.0/internal/language/tables.go'
        bundle(changed)['source_selections'][path] = ['normalization']
        self.save_registry(changed)
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_fresh_autocrlf_checkout_runs_actual_checker_and_rejects_unprotected_bytes(self):
        source = Path(__file__).resolve().parents[2]
        self.selected_derivatives()
        shutil.copyfile(source / '.gitattributes', self.root / '.gitattributes')
        (self.root / 'xtask').mkdir(exist_ok=True)
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
                self.assertEqual(len(report['private_derivatives']), 19)
            else:
                self.assertNotEqual(result.returncode, 0)
                if mode == 'negative-index':
                    self.assertIn('changed authenticated source index', result.stderr)
                    converted = fresh / self.bundle['source_index']
                else:
                    self.assertIn('changed source-policy file', result.stderr)
                    converted = fresh / gate.DERIVATIVES / 'normalization.rs'
                self.assertIn(b'\r\n', converted.read_bytes())

    def test_url_handover_requires_exact_origin_scope_and_materialized_leaf(self):
        for field in ('origin', 'oracle', 'scope'):
            changed = copy.deepcopy(self.query_map)
            changed['url_codec'][field] = 'unapproved'
            self.assert_query_map_rejected(changed)
        leaf = self.root / self.query_map['url_codec']['leaf']['path']
        original = leaf.read_bytes()
        try:
            leaf.write_bytes(original + b'// changed URL decoder\n')
            with self.assertRaises(ValueError):
                gate.check(self.root)
        finally:
            leaf.write_bytes(original)
        gate.check(self.root)

    def test_tag_categories_are_exact_scoped_and_read_only(self):
        before = snapshot(self.root)
        gate.check(self.root)
        self.assertEqual(before, snapshot(self.root))
        for change in ['source', 'source_sha256', 'selection', 'notices', 'recipe']:
            changed = copy.deepcopy(self.query_map)
            if change == 'notices':
                changed['tagging'][change] = []
            elif change == 'recipe':
                changed['tagging'][change]['path'] = 'xtask/check_sources.py'
            else:
                changed['tagging'][change] = 'unapproved'
            self.assert_query_map_rejected(changed)
        leaf = self.root / self.query_map['tagging']['path']
        original = leaf.read_bytes()
        try:
            leaf.write_bytes(original + b'// changed category\n')
            with self.assertRaises(ValueError):
                gate.check(self.root)
        finally:
            leaf.write_bytes(original)
        gate.check(self.root)
