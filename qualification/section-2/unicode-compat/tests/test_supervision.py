"""Safe qualification prerequisite/protocol tests; no oracle/toolchain execution."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1] / "probe.py"
SPEC = importlib.util.spec_from_file_location("unicode_qualification_probe", SOURCE)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class QualificationSafety(unittest.TestCase):
    def test_changed_frozen_file_is_rejected_and_restored_bytes_verify(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rows = []
            for i in range(436):
                p = root / str(i)
                p.write_bytes(b"synthetic")
                rows.append({"path": str(i), "bytes": 9, "sha256": probe.digest(b"synthetic")})
            self.assertEqual(probe.identities(root, rows), 436)
            (root / "17").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "identity mismatch"):
                probe.identities(root, rows)
            (root / "17").write_bytes(b"synthetic")
            self.assertEqual(probe.identities(root, rows), 436)

    def test_duplicate_or_oversized_synthetic_cases_fail_closed(self):
        row = {"name": "same", "input": "é"}
        for rows in [[], [row, row], [{"name": "large", "input": "é" * 3000}]]:
            with self.assertRaises(RuntimeError):
                probe.bounded_cases(rows)
        self.assertEqual(probe.bounded_cases([row]), [row])

    def test_source_path_escape_is_refused_before_reading_outside_the_oracle(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "owned"
            root.mkdir()
            outside = Path(temporary) / "outside"
            outside.write_bytes(b"synthetic")
            row = {"path": "../outside", "bytes": 9, "sha256": probe.digest(b"synthetic")}
            with self.assertRaisesRegex(RuntimeError, "unsafe oracle source path"):
                probe.identities(root, [row] * 436)

    def test_tooling_failure_cannot_be_reported_as_a_compatibility_result(self):
        with patch.object(probe.subprocess, "run", return_value=subprocess.CompletedProcess(
                ["not-executed"], 9, stdout=b"PASS", stderr=b"synthetic tooling failure")):
            with self.assertRaisesRegex(RuntimeError, "not compatibility result"):
                probe.command(["not-executed"], Path("."))


if __name__ == "__main__":
    unittest.main()
