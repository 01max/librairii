"""A failed containment control must prevent every hostile allocation probe."""
import json
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import textwrap
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import decoder_containment as supervisor


class FailClosed(unittest.TestCase):
    def test_optimized_host_configuration_failure_prevents_child_start(self):
        script = textwrap.dedent('''
            import json
            from pathlib import Path
            from unittest.mock import patch
            import decoder_containment as supervisor
            settings = {"Memory": 0, "MemorySwap": supervisor.MEMORY,
                "ReadonlyRootfs": True, "NetworkMode": "none", "PidsLimit": 32,
                "NanoCpus": 1000000000, "CapDrop": ["ALL"],
                "SecurityOpt": ["no-new-privileges"],
                "Ulimits": [{"Name": "cpu", "Soft": 2, "Hard": 2}]}
            error = None
            with patch.object(supervisor, "docker", side_effect=["owned-id", json.dumps(settings), ""]) as docker, \\
                    patch.object(supervisor.subprocess, "run", side_effect=RuntimeError("unexpected child start")) as start:
                try:
                    supervisor.contained(Path("unused-binary"), Path("unused-fixtures"), "smoke")
                except RuntimeError as failure:
                    error = str(failure)
            print(json.dumps({"optimized": not __debug__, "error": error, "childStarted": start.called,
                "dockerCommands": [call.args[0] for call in docker.call_args_list]}))
        ''')
        result = subprocess.run([sys.executable, "-O", "-B", "-c", script],
                                cwd=Path(supervisor.__file__).parent,
                                env=dict(os.environ, AO_SESSION_ID="librairii-4"),
                                capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        observed = json.loads(result.stdout)
        self.assertTrue(observed["optimized"])
        self.assertFalse(observed["childStarted"])
        self.assertIn("configured memory/swap boundary mismatch", observed["error"])
        self.assertEqual(observed["dockerCommands"], ["create", "inspect", "rm"])

    def test_actual_control_validation_survives_both_optimization_routes(self):
        # Execute real controls()/main() in optimized interpreters. Only the
        # container observations and hostile experiment function are mocked;
        # no Docker process, allocation control or hostile fixture can execute.
        script = textwrap.dedent('''
            import json, sys
            from pathlib import Path
            from unittest.mock import patch
            import decoder_containment as supervisor
            binary, output = map(Path, sys.argv[1:])
            failed_smoke = {"exitCode": 0, "records": [
                {"memoryMax": str(supervisor.MEMORY), "swapMax": "0", "limits": "not verified"},
                {"outcome": "smoke_failed"}], "stderr": "", "oomKilled": False,
                "memoryEventsAfter": {"oom_kill": 0}, "wallWatchdog": False,
                "hostElapsedSeconds": 0}
            error = None
            with patch.object(sys, "argv", ["supervisor", "--binary", str(binary), "--output", str(output)]), \\
                    patch.object(supervisor, "docker", side_effect=["{}", "test-kernel"]), \\
                    patch.object(supervisor, "contained", return_value=failed_smoke) as contained, \\
                    patch.object(supervisor, "experiments") as experiments:
                try:
                    supervisor.main()
                except RuntimeError as failure:
                    error = str(failure)
            print(json.dumps({"optimized": not __debug__, "error": error,
                "containedCalls": contained.call_count, "experimentsCalled": experiments.called,
                "report": json.loads(output.read_text())}))
        ''')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "not-executed"
            binary.write_bytes(b"\x7fELFsynthetic regression placeholder")
            for flags, value in ((["-O"], "0"), ([], "1")):
                with self.subTest(flags=flags, PYTHONOPTIMIZE=value):
                    output = root / "observations.json"
                    environment = dict(os.environ, AO_SESSION_ID="librairii-4", PYTHONOPTIMIZE=value)
                    result = subprocess.run([sys.executable, *flags, "-B", "-c", script, str(binary), str(output)],
                                            cwd=Path(supervisor.__file__).parent, env=environment,
                                            capture_output=True, text=True, timeout=10, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    observed = json.loads(result.stdout.splitlines()[-1])
                    self.assertTrue(observed["optimized"])
                    self.assertFalse(observed["experimentsCalled"])
                    self.assertEqual(observed["containedCalls"], 1)
                    self.assertIn("tooling/startup failure", observed["error"])
                    self.assertEqual(observed["report"]["adversarialCases"], [])
                    self.assertIn("not a decoder verdict", observed["report"]["status"])

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
