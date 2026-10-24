"""Exercise the CI decision against actual Git histories, including unsafe skips."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('legacy-ci-changes.py').resolve()
SPEC = importlib.util.spec_from_file_location('legacy_changes', SCRIPT)
CHANGES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHANGES)


class LegacyChangesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.previous = Path.cwd()
        os.chdir(self.temp.name)
        self.addCleanup(os.chdir, self.previous)
        self.git('init', '-q')
        # These tiny, disposable histories need no background maintenance. A
        # maintenance process can still write .git after commit returns and
        # race TemporaryDirectory cleanup on the hosted Git version.
        self.git('config', '--local', 'maintenance.auto', 'false')
        self.git('config', '--local', 'gc.auto', '0')
        self.write('crates/core/original.rs')
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', *args], text=True).strip()

    def write(self, name):
        path = Path(name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('synthetic test input\n')

    def commit(self):
        self.git('add', '.')
        self.git('-c', 'user.name=CI test', '-c', 'user.email=ci@example.invalid',
                 'commit', '-qm', 'synthetic change')
        return self.git('rev-parse', 'HEAD')

    def test_rust_only_and_deleted_files_skip_legacy(self):
        Path('crates/core/original.rs').unlink()
        self.write('xtask/src/check.rs')
        self.write('Cargo.lock')
        self.assertFalse(CHANGES.needs_legacy(self.base, self.commit()))

    def test_legacy_and_unknown_paths_run(self):
        for path in ('frontend/src/app.ts', 'internal/app.go', 'new-build-input',
                     '.github/workflows/ci.yml', 'docs/release-platforms.md'):
            with self.subTest(path=path):
                base = self.git('rev-parse', 'HEAD')
                self.write(path)
                self.assertTrue(CHANGES.needs_legacy(base, self.commit()))

    def test_more_than_300_rust_paths_cannot_hide_legacy_path(self):
        for index in range(305):
            self.write(f'crates/core/{index}.rs')
        self.write('zz-legacy.go')
        self.assertTrue(CHANGES.needs_legacy(self.base, self.commit()))

    def test_both_sides_of_rename_are_examined(self):
        Path('crates/core/original.rs').rename('legacy.go')
        legacy_head = self.commit()
        self.assertTrue(CHANGES.needs_legacy(self.base, legacy_head))
        Path('legacy.go').rename('crates/core/restored.rs')
        self.assertTrue(CHANGES.needs_legacy(legacy_head, self.commit()))

    def test_spaces_and_newlines_in_names(self):
        self.write('crates/core/space and\nnewline.rs')
        self.assertFalse(CHANGES.needs_legacy(self.base, self.commit()))
        self.write('a legacy\nfile.go')
        self.assertTrue(CHANGES.needs_legacy(self.base, self.commit()))

    def test_invalid_or_unavailable_history_runs(self):
        for ref in (None, '', '0' * 40, 'f' * 40, 'HEAD', '--help'):
            with self.subTest(ref=ref):
                self.assertTrue(CHANGES.needs_legacy(ref, self.base))
                self.assertTrue(CHANGES.needs_legacy(self.base, ref))

    def test_manual_dispatch_forces_legacy_and_emits_job_output(self):
        output = Path('github-output').resolve()
        subprocess.run(['python3', '-I', '-B', str(SCRIPT)], check=True,
                       env={**os.environ, 'EVENT_NAME': 'workflow_dispatch',
                            'BASE_SHA': self.base, 'HEAD_SHA': self.base,
                            'GITHUB_OUTPUT': str(output)})
        self.assertEqual(output.read_text(), 'legacy=true\n')


if __name__ == '__main__':
    unittest.main()
