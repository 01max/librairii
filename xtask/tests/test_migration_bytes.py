"""Real Git checkout regression for both exact migration source namespaces."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class MigrationCheckout(unittest.TestCase):
    def test_fresh_autocrlf_preserves_storage_go_and_corpus_bytes(self):
        source = Path(__file__).resolve().parents[2]
        namespaces = ['crates/librairii-storage/migrations', 'internal/database/migrations',
                      'preservation/corpus/migrations']

        def snapshot(root):
            return {p.relative_to(root).as_posix(): None if p.is_dir() else p.read_bytes()
                    for p in root.rglob('*')}

        source_before = {name: snapshot(source / name) for name in namespaces}
        with tempfile.TemporaryDirectory(prefix='librairii-sql-checkout-') as temp:
            parent = Path(temp)
            seed = parent / 'source Été # %2F'
            seed.mkdir()
            for relative in ['.gitattributes', '.config/rust-preservation-baseline.json',
                             'xtask/check_preservation.py', 'xtask/check_migration_bytes.py',
                             'preservation/check.py', 'preservation/corpus/manifest.json']:
                destination = seed / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source / relative, destination)
            for relative in namespaces:
                shutil.copytree(source / relative, seed / relative)
            hooks = parent / 'empty-hooks'
            hooks.mkdir()

            def git(*args):
                return subprocess.run(['git', '-c', 'core.hooksPath=' + str(hooks),
                    '-c', 'commit.gpgsign=false', '-c', 'user.name=Synthetic migration fixture',
                    '-c', 'user.email=fixture@example.invalid', *args],
                    cwd=parent, capture_output=True, encoding='utf-8', check=True)

            git('init', str(seed))
            git('-C', str(seed), 'add', '.')
            git('-C', str(seed), 'commit', '-m', 'synthetic exact migration fixture')
            for mode in ['protected', 'storage', 'go']:
                with self.subTest(checkout=mode):
                    fresh = parent / ('fresh ' + mode + ' Été # %2F')
                    git('clone', '--no-local', '--no-checkout', '--config', 'core.autocrlf=true', str(seed), str(fresh))
                    self.assertEqual(git('-C', str(fresh), 'config', '--get', 'core.autocrlf').stdout.strip(), 'true')
                    if mode != 'protected':
                        relative = namespaces[0 if mode == 'storage' else 1]
                        (fresh / '.git/info/attributes').write_text(relative + '/*.sql text\n', encoding='utf-8')
                    git('-C', str(fresh), 'checkout', '--detach', 'HEAD')
                    before = snapshot(parent)
                    result = subprocess.run([sys.executable, '-I', '-B', '-X', 'utf8',
                        str(fresh / 'xtask/check_migration_bytes.py')], cwd=parent,
                        capture_output=True, encoding='utf-8')
                    self.assertEqual(snapshot(parent), before)
                    if mode == 'protected':
                        self.assertEqual(result.returncode, 0, result.stderr)
                        report = json.loads(result.stdout)
                        self.assertEqual(report['migration_count'], 18)
                        for row in report['migrations']:
                            expected = row['admitted_sha256']
                            self.assertEqual([row[k + '_sha256'] for k in ['storage', 'go', 'corpus']], [expected] * 3)
                    else:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn('migration bytes changed: ' + relative, result.stderr)
                        for path in (fresh / relative).glob('*.sql'):
                            self.assertIn(b'\r\n', path.read_bytes())
                        for other in namespaces:
                            if other != relative:
                                self.assertEqual(snapshot(fresh / other), source_before[other])
                    self.assertEqual(git('-C', str(fresh), 'status', '--porcelain').stdout, '')
                    self.assertEqual(snapshot(parent), before)
        self.assertEqual({name: snapshot(source / name) for name in namespaces}, source_before)
