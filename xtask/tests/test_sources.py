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
        (self.root / '.config').mkdir(parents=True)
        shutil.copyfile(source / '.config/upstream-sources.json', self.root / '.config/upstream-sources.json')
        for path in ['.config/upstream-sources', '.config/upstream-notices/' + gate.BUNDLE]:
            shutil.copytree(source / path, self.root / path)
        self.registry_path = self.root / '.config/upstream-sources.json'
        self.registry = json.loads(self.registry_path.read_text(encoding='utf-8'))
        self.bundle = self.registry['bundles'][0]
        self.index = json.loads((self.root / self.bundle['source_index']).read_bytes())
        self.private = self.root / gate.DERIVATIVES

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
        manifest = {'version': 1, 'bundle': gate.BUNDLE, 'files': [record]}
        self.save_manifest(manifest)
        return manifest

    def save_manifest(self, value):
        (self.private / 'source-manifest.json').write_text(json.dumps(value), encoding='utf-8')

    def test_registered_sources_and_notices_pass_without_writes(self):
        before = snapshot(self.root)
        report = gate.check(self.root)
        self.assertEqual(len(report['source_identities']), 49)
        self.assertEqual(report['state'], 'awaiting-private-implementation')
        self.assertIsNone(report['product_license'])
        self.assertEqual(report['private_derivatives'], [])
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
        self.assertEqual(len(gate.check(self.root)['private_derivatives']), 1)
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
            changed['files'][0][field] = replacement
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
        manifest['files'][0].update(bytes=len(content), sha256=gate.digest(content))
        self.save_manifest(manifest)
        with self.assertRaisesRegex(ValueError, 'copyright'):
            gate.check(self.root)
        manifest = self.derivative('golang.org/x/text@v0.43.0/internal/language/tables.go')
        self.assertEqual(len(gate.check(self.root)['private_derivatives']), 1)
        for field, value in [('selection', 'language-default-whole-table'),
                             ('sha256', '0' * 64), ('header_sha256', '0' * 64)]:
            changed = copy.deepcopy(manifest)
            changed['files'][0]['sources'][0][field] = value
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
        self.assertIn('awaiting-private-implementation', summary)
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
                self.assertEqual(len(report['source_identities']), 49)
                self.assertEqual(len(report['private_derivatives']), 1)
            else:
                self.assertNotEqual(result.returncode, 0)
                if mode == 'negative-index':
                    self.assertIn('changed authenticated source index', result.stderr)
                    converted = fresh / self.bundle['source_index']
                else:
                    self.assertIn('changed source-policy file', result.stderr)
                    converted = fresh / gate.DERIVATIVES / 'synthetic.rs'
                self.assertIn(b'\r\n', converted.read_bytes())
