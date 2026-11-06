"""Exercise the admission boundary only on fresh synthetic roots."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('preservation_gate', Path(__file__).resolve().parents[1] / 'check_preservation.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
runner_spec = importlib.util.spec_from_file_location('preservation_runner', Path(__file__).resolve().parents[1] / 'run_preservation_tests.py')
runner = importlib.util.module_from_spec(runner_spec)
runner_spec.loader.exec_module(runner)


class AdmissionPins(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='librairii-preservation-gate-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / 'preservation/corpus').mkdir(parents=True)
        self.checker = self.root / 'preservation/check.py'
        self.manifest = self.root / 'preservation/corpus/manifest.json'
        self.checker.write_text('# synthetic checker, never executed\n', encoding='utf-8')
        self.manifest.write_text(json.dumps({'oracle_commit': 'synthetic-oracle', 'files': [{'path': 'synthetic.json'}]}), encoding='utf-8')
        self.baseline = {
            'oracle_commit': 'synthetic-oracle', 'hashed_files': 1,
            'checker_sha256': hashlib.sha256(self.checker.read_bytes()).hexdigest(),
            'manifest_sha256': hashlib.sha256(self.manifest.read_bytes()).hexdigest(),
        }

    def test_exact_inputs_pass_without_writes(self):
        before = {p: p.read_bytes() for p in [self.checker, self.manifest]}
        gate.verify_pins(self.root, self.baseline)
        self.assertEqual(before, {p: p.read_bytes() for p in before})

    def test_metacharacter_checkout_runs_read_only_from_another_directory(self):
        source = Path(__file__).resolve().parents[2]
        checkout = self.root / 'checkout Été # %2F'
        (checkout / 'xtask').mkdir(parents=True)
        (checkout / '.config').mkdir()
        shutil.copytree(source / 'preservation', checkout / 'preservation')
        for relative in ['xtask/check_preservation.py', '.config/rust-preservation-baseline.json']:
            shutil.copyfile(source / relative, checkout / relative)

        def contents(root):
            return {
                path.relative_to(root).as_posix(): None if path.is_dir() else path.read_bytes()
                for path in root.rglob('*')
            }

        source_before = contents(source / 'preservation')
        parent_before = contents(self.root)
        result = subprocess.run(
            [sys.executable, '-I', '-B', '-X', 'utf8', str(checkout / 'xtask/check_preservation.py')],
            cwd=self.root, capture_output=True, encoding='utf-8',
        )
        parent_after = contents(self.root)
        self.assertEqual(result.returncode, 0, json.dumps({
            'stderr': result.stderr,
            'created_paths': sorted(parent_after.keys() - parent_before.keys()),
        }, ensure_ascii=False))
        report = json.loads(result.stdout)
        self.assertEqual(report['oracle_commit'], '73a93303a9a44cf0a92eaf525331561ed8c04460')
        self.assertEqual(report['hashed_files'], 146)
        self.assertEqual(parent_after, parent_before)
        self.assertEqual(contents(source / 'preservation'), source_before)

    def test_missing_or_changed_checker_and_manifest_fail(self):
        for path in [self.checker, self.manifest]:
            original = path.read_bytes()
            for replacement in [None, original + b'\n']:
                with self.subTest(path=path.name, replacement=replacement):
                    if replacement is None:
                        path.unlink()
                    else:
                        path.write_bytes(replacement)
                    with self.assertRaises(ValueError):
                        gate.verify_pins(self.root, self.baseline)
                    path.write_bytes(original)

    def test_repinning_hash_alone_cannot_drop_or_change_oracle_inventory(self):
        for key, value in [('oracle_commit', 'different-oracle'), ('hashed_files', 0)]:
            with self.subTest(key=key):
                with self.assertRaises(ValueError):
                    gate.verify_pins(self.root, {**self.baseline, key: value})


class RunnerPolicy(unittest.TestCase):
    def test_missing_or_duplicate_identity_cannot_pass_discovery(self):
        executed = []

        class Cases(unittest.TestCase):
            def test_pass(self):
                executed.append(self.id())

        identity = Cases('test_pass').id()
        for required in [[], [identity, identity], [identity, 'missing.required.test']]:
            with self.subTest(required=required):
                with self.assertRaises(ValueError):
                    runner.run_suite(unittest.TestSuite([Cases('test_pass')]), required, io.StringIO())
        with self.assertRaises(ValueError):
            runner.run_suite(unittest.TestSuite([Cases('test_pass'), Cases('test_pass')]), [identity], io.StringIO())
        self.assertEqual(executed, [])
        result = runner.run_suite(unittest.TestSuite([Cases('test_pass')]), [identity], io.StringIO())
        self.assertEqual(result.testsRun, 1)
        self.assertEqual(result.skipped, [])
        self.assertEqual(executed, [identity])

    def test_skip_expected_failure_and_failure_are_rejected(self):
        class Cases(unittest.TestCase):
            @unittest.skip('synthetic skipped assertion must be rejected')
            def test_skip(self):
                self.fail('not executed')

            @unittest.expectedFailure
            def test_xfail(self):
                self.fail('synthetic expected failure must be rejected')

            def test_failure(self):
                self.fail('synthetic failure must be rejected')

        for name in ['test_skip', 'test_xfail', 'test_failure']:
            with self.subTest(name=name):
                case = Cases(name)
                with self.assertRaises(ValueError):
                    runner.run_suite(unittest.TestSuite([case]), [case.id()], io.StringIO())
