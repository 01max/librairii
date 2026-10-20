"""Exercise the admission boundary only on fresh synthetic roots."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('preservation_gate', Path(__file__).resolve().parents[1] / 'check_preservation.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


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
