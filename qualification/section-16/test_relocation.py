"""Exercise the real wrapper's ownership/error paths without mounting a volume."""
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import subprocess
import sys
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('relocation', Path(__file__).resolve().parents[2] / 'scripts/verify-native-relocation.py')
relocation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relocation)


class RelocationCustody(unittest.TestCase):
    def exercise(self, *, primary=True, cleanup=False, attach=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            binary = root / 'rehearsal'
            binary.write_bytes(b'owned test executable')
            (root / 'scripts').mkdir()
            (root / 'scripts/verify-native-relocation.py').write_bytes(b'test driver')
            example = root / 'crates/librairii-storage/examples/relocation_handover.rs'
            example.parent.mkdir(parents=True)
            example.write_bytes(b'test example')
            work = root / 'owned-work'
            work.mkdir()
            old_stat = Path.stat

            def stat(path, *args, **kwargs):
                if path == work / 'mounted':
                    return SimpleNamespace(st_dev=old_stat(work).st_dev + 1)
                return old_stat(path, *args, **kwargs)

            def run(argv, **kwargs):
                if argv[0] == 'cargo':
                    return SimpleNamespace(stdout=json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'relocation_handover', 'kind': ['example']}, 'executable': str(binary)}))
                if argv[:2] == ['hdiutil', 'create']:
                    (work / 'owned.sparseimage').write_bytes(b'owned image')
                if argv[:2] == ['hdiutil', 'attach'] and attach:
                    raise RuntimeError('primary partial attach failure')
                if argv[0] == str(binary):
                    if primary:
                        raise RuntimeError('primary SQLite I/O failure')
                    return SimpleNamespace(stdout=json.dumps({'forced_termination': [{}] * 8, 'filesystem_failures': [{}, {}], 'volume_root': {'volume_root': 'exact_root_verified'}}))
                return SimpleNamespace(stdout='false\n' if argv[0] == 'git' and 'status' in argv else '')

            failure = None
            with patch.object(relocation, 'ROOT', root), patch.object(relocation.tempfile, 'mkdtemp', return_value=str(work)), patch.object(relocation.platform, 'system', return_value='Darwin'), patch.object(relocation, 'run', side_effect=run), patch.object(relocation, 'detach_owned_image', side_effect=RuntimeError('cleanup detach failure') if cleanup else None, return_value=False) as detach, patch.object(Path, 'stat', stat):
                try:
                    relocation.main()
                except Exception as error:
                    failure = error
            detach.assert_called_once_with(work / 'mounted')
            if primary or cleanup or attach:
                self.assertIsNotNone(failure)
                self.assertTrue(work.is_dir(), 'uncertain/failed rehearsal must retain its owned image and fixtures')
                self.assertEqual((work / 'owned.sparseimage').read_bytes(), b'owned image')
                receipt = json.loads((root / 'target/quality/native-relocation-failure.json').read_text())
                self.assertFalse(receipt['complete'])
                self.assertEqual(receipt['work'], str(work))
                self.assertEqual(receipt['owned_volume_detached'], not cleanup)
                if primary or attach:
                    self.assertIn('primary', receipt['primary_error'])
                if cleanup:
                    self.assertIn('cleanup detach failure', receipt['cleanup_error'])
                if primary:
                    self.assertIn('primary SQLite I/O failure', str(failure.__cause__))
                self.assertFalse((root / 'target/quality/native-relocation.json').exists())
            else:
                self.assertIsNone(failure)
                self.assertFalse(work.exists(), 'only a successful positively detached rehearsal may retire')
                self.assertTrue(json.loads((root / 'target/quality/native-relocation.json').read_text())['owned_volume_detached'])

    def test_primary_and_cleanup_errors_retain_image_and_original_cause(self):
        self.exercise(cleanup=True)

    def test_primary_error_retains_image_after_successful_detach(self):
        self.exercise()

    def test_partial_attach_failure_requires_detach_and_retains_image(self):
        self.exercise(primary=False, attach=True, cleanup=True)

    def test_success_retires_only_after_positive_detach(self):
        self.exercise(primary=False)

    def test_cleanup_failure_after_success_retains_image(self):
        self.exercise(primary=False, cleanup=True)

    def test_failed_real_child_retains_both_streams_and_original_status(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            child = work / 'failure.py'
            child.write_text('import sys\nprint("partial output", flush=True)\nprint("original disk I/O error", file=sys.stderr, flush=True)\nsys.exit(9)\n')
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                relocation.rehearse(Path(sys.executable), child, work, work)
            self.assertEqual(caught.exception.returncode, 9)
            self.assertEqual((work / 'rehearsal.stdout').read_text(), 'partial output\n')
            self.assertEqual((work / 'rehearsal.stderr').read_text(), 'original disk I/O error\n')


if __name__ == '__main__':
    unittest.main()
