"""Actual private-source admission and checkout regressions on owned copies."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('import_source_gate', SOURCE / 'xtask/check_import_sources.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


def contents(root):
    return {p.relative_to(root).as_posix():
        (stat.S_IMODE(p.stat().st_mode), None if p.is_dir() else p.read_bytes())
        for p in root.rglob('*')}


class StudioSourcePolicy(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='librairii-import-source-policy-')
        self.addCleanup(self.directory.cleanup)
        self.parent = Path(self.directory.name)
        self.root = self.parent / 'source'
        self.root.mkdir()
        for relative in [gate.METADATA, gate.NOTICES, gate.DERIVATIVES]:
            shutil.copytree(SOURCE / relative, self.root / relative)
        for relative in ['.config/upstream-sources.json', '.gitattributes', 'xtask/check_import_sources.py']:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(SOURCE / relative, target)

    def load(self, relative):
        return json.loads((self.root / relative).read_text(encoding='utf-8'))

    def save(self, relative, value):
        (self.root / relative).write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')

    def test_registered_manifest_and_notices_are_read_only(self):
        before = contents(self.parent)
        report = gate.check(self.root)
        self.assertEqual(report['product_license'], None)
        self.assertEqual(len(report['source_identities']), 18)
        self.assertEqual(len(report['private_derivatives']), 4)
        self.assertEqual(contents(self.parent), before)

    def test_index_schema_or_scope_repinning_cannot_widen_authority(self):
        registry_path = '.config/upstream-sources.json'
        original = (self.root / registry_path).read_bytes()
        for field, replacement in [('scope', 'public-json-parser'), ('derivative_root', 'crates'),
                                    ('required_derivatives', {}), ('selection_notices', {})]:
            with self.subTest(field=field):
                registry = json.loads(original)
                registry['bundles'][0][field] = replacement
                self.save(registry_path, registry)
                with self.assertRaises(ValueError):
                    gate.check(self.root)
        (self.root / registry_path).write_bytes(original)
        registry = self.load(registry_path)
        registry['product_license'] = 'MIT'
        self.save(registry_path, registry)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        for filename, key in [('source-index.json', 'source_index_sha256'),
                              ('source-manifest.schema.json', 'source_schema_sha256')]:
            with self.subTest(filename=filename):
                registry = json.loads(original)
                path = self.root / gate.METADATA / filename
                data = path.read_bytes()
                changed = data + b'\n'
                path.write_bytes(changed)
                registry['bundles'][0][key] = hashlib.sha256(changed).hexdigest()
                self.save(registry_path, registry)
                with self.assertRaises(ValueError):
                    gate.check(self.root)
                path.write_bytes(data)
        (self.root / registry_path).write_bytes(original)

    def test_missing_changed_extra_or_self_repinning_notices_fail(self):
        registry_path = '.config/upstream-sources.json'
        registry_bytes = (self.root / registry_path).read_bytes()
        path = self.root / gate.NOTICES / 'LICENSE-GO-BSD.txt'
        original = path.read_bytes()
        for replacement in [None, original + b'changed']:
            with self.subTest(replacement=replacement is None):
                if replacement is None:
                    path.unlink()
                else:
                    path.write_bytes(replacement)
                with self.assertRaises(ValueError):
                    gate.check(self.root)
                path.write_bytes(original)
        path.write_bytes(original + b'changed')
        for origin in [None, {'kind': 'authored'}]:
            registry = json.loads(registry_bytes)
            notice = registry['bundles'][0]['notices'][0]
            notice.update(bytes=path.stat().st_size, sha256=gate.digest(path.read_bytes()), origin=origin)
            self.save(registry_path, registry)
            with self.assertRaises(ValueError):
                gate.check(self.root)
        path.write_bytes(original)
        (self.root / registry_path).write_bytes(registry_bytes)
        extra = self.root / gate.NOTICES / 'extra.txt'
        extra.write_bytes(b'extra')
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_derivative_set_bytes_and_kinds_fail_closed(self):
        relative = gate.DERIVATIVES + '/source-manifest.json'
        manifest_bytes = (self.root / relative).read_bytes()
        path = self.root / gate.DERIVATIVES / 'mod.rs'
        original = path.read_bytes()
        for replacement in [None, original + b'\n']:
            if replacement is None:
                path.unlink()
            else:
                path.write_bytes(replacement)
            with self.assertRaises(ValueError):
                gate.check(self.root)
            path.write_bytes(original)
        manifest = json.loads(manifest_bytes)
        manifest['files'][0].update(kind='authored', sources=[], notices=[])
        self.save(relative, manifest)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        (self.root / relative).write_bytes(manifest_bytes)
        (self.root / gate.DERIVATIVES / 'unrecorded.rs').write_bytes(b'// extra')
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_original_identity_headers_evidence_and_notice_union_fail_closed(self):
        relative = gate.DERIVATIVES + '/source-manifest.json'
        original = (self.root / relative).read_bytes()
        for key, value in [('sha256', '0' * 64), ('header_sha256', '0' * 64),
                           ('selection', 'reference-only'), ('path', 'Go1.26.5/src/reflect/value.go')]:
            manifest = json.loads(original)
            manifest['files'][0]['sources'][0][key] = value
            self.save(relative, manifest)
            with self.assertRaises(ValueError):
                gate.check(self.root)
        manifest = json.loads(original)
        manifest['files'][0]['notices'] = []
        self.save(relative, manifest)
        with self.assertRaises(ValueError):
            gate.check(self.root)
        path = self.root / gate.DERIVATIVES / 'mod.rs'
        changed = b'\n'.join(line for line in path.read_bytes().split(b'\n')
                             if not line.startswith(b'// Copyright'))
        path.write_bytes(changed)
        manifest = json.loads(original)
        manifest['files'][0].update(bytes=len(changed), sha256=gate.digest(changed))
        self.save(relative, manifest)
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_path_alias_and_traversal_are_rejected(self):
        for path in ['../source', '/absolute', 'a/../b', 'a\\b', 'C:/source', 'a/./b', 'a//b']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                gate.safe(self.root, path)
        original = self.root / gate.DERIVATIVES
        target = self.parent / 'owned-real'
        original.rename(target)
        if os.name == 'nt':
            subprocess.run(['cmd.exe', '/d', '/c', 'mklink', '/J', str(original), str(target)],
                           check=True, capture_output=True)
        else:
            original.symlink_to(target, target_is_directory=True)
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_inventory_copies_exact_notices_and_observes_product_license(self):
        report = gate.check(self.root)
        gate.inventory(self.root, report)
        self.assertIsNone(report['product_license'])
        for notice in report['notices']:
            self.assertEqual((self.root / 'target/quality/notices' / gate.BUNDLE / notice['path']).read_bytes(),
                             (self.root / gate.NOTICES / notice['path']).read_bytes())
        self.assertIn('no behavior, resource, effect or platform qualification',
                      (self.root / 'target/quality/notices/NOTICES-import-frozen-sources.txt')
                      .read_text(encoding='utf-8'))

    def test_fresh_autocrlf_checkout_preserves_actual_source_policy_inputs(self):
        hooks = self.parent / 'empty-hooks'
        hooks.mkdir()
        def git(*args):
            return subprocess.run(['git', '-c', 'core.hooksPath=' + str(hooks),
                '-c', 'commit.gpgsign=false', '-c', 'user.name=Source policy test',
                '-c', 'user.email=source-policy@example.invalid', *args],
                check=True, capture_output=True, encoding='utf-8')
        git('init', '--quiet', str(self.root))
        attributes = (self.root / '.gitattributes').read_bytes()
        for label, rule in [('protected', b''),
                ('metadata-crlf', ('\n' + gate.METADATA + '/* text eol=crlf\n').encode()),
                ('source-crlf', ('\n' + gate.DERIVATIVES + '/*.rs text eol=crlf\n').encode())]:
            (self.root / '.gitattributes').write_bytes(attributes + rule)
            git('-C', str(self.root), '-c', 'core.autocrlf=false', 'add', '.')
            git('-C', str(self.root), 'commit', '--quiet', '-m', label)
            checkout = self.parent / ('checkout Été # %2F ' + label)
            git('-c', 'core.autocrlf=true', 'clone', '--no-local', '--no-checkout', str(self.root), str(checkout))
            # This is a new separately-owned clone, never a shared worktree config.
            git('-C', str(checkout), 'config', 'core.autocrlf', 'true')
            git('-C', str(checkout), 'checkout', '--detach', 'HEAD')
            self.assertEqual(git('-C', str(checkout), 'config', '--get', 'core.autocrlf').stdout.strip(), 'true')
            before = contents(self.parent)
            result = subprocess.run([sys.executable, '-I', '-B', '-X', 'utf8',
                str(checkout / 'xtask/check_import_sources.py')], cwd=self.parent,
                capture_output=True, encoding='utf-8')
            self.assertEqual(contents(self.parent), before)
            if label == 'protected':
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(len(json.loads(result.stdout)['private_derivatives']), 4)
            else:
                self.assertNotEqual(result.returncode, 0, label)
                self.assertIn('changed', result.stderr)
            self.assertEqual(git('--no-optional-locks', '-C', str(checkout), 'status', '--porcelain').stdout, '')
