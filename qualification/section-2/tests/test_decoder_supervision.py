"""A failed containment control must prevent every hostile allocation probe."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import decoder_containment as supervisor


class FailClosed(unittest.TestCase):
    def test_failed_control_records_tooling_error_without_running_experiments(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "not-executed"
            binary.write_bytes(b"\x7fELFsynthetic regression placeholder")
            output = root / "observations.json"
            with patch.dict(os.environ, {"AO_SESSION_ID": "librairii-4"}), \
                    patch.object(sys, "argv", ["supervisor", "--binary", str(binary), "--output", str(output)]), \
                    patch.object(supervisor, "docker", side_effect=["{}", "test-kernel"]), \
                    patch.object(supervisor, "controls", side_effect=AssertionError("control refused")), \
                    patch.object(supervisor, "experiments") as experiments:
                with self.assertRaisesRegex(AssertionError, "control refused"):
                    supervisor.main()
                experiments.assert_not_called()
            report = json.loads(output.read_text())
            self.assertEqual(report["adversarialCases"], [])
            self.assertEqual(report["toolingError"], "control refused")
            self.assertIn("not a decoder verdict", report["status"])


if __name__ == "__main__":
    unittest.main()
