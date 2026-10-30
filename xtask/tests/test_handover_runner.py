import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[2] / 'scripts/verify-upgrade-handover.py'
SPEC = importlib.util.spec_from_file_location('handover_runner', MODULE)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class HandoverRunnerTests(unittest.TestCase):
    def run_example(self, root, executable):
        event = {'reason': 'compiler-artifact', 'target': {'name': 'probe'},
                 'executable': str(executable)}
        actual_run = subprocess.run

        def build_or_execute(command, **kwargs):
            if command[0] == 'cargo':
                return subprocess.CompletedProcess(command, 0, json.dumps(event), '')
            return actual_run(command, **kwargs)

        with patch.object(RUNNER, 'ROOT', root), patch.object(RUNNER.subprocess, 'run', build_or_execute):
            return RUNNER.run_example('probe', [], os.environ.copy())

    def executable(self, root, extra=''):
        executable = root / 'cargo-output'
        executable.write_text('#!' + sys.executable + '\nimport json, os, sys\n'
                              + extra + '\nprint(json.dumps({"path": sys.argv[0], '
                              '"links": os.stat(sys.argv[0]).st_nlink}))\n')
        executable.chmod(0o700)
        return executable

    def test_cargo_hardlink_executes_exact_owned_single_link_copy(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = self.executable(root)
            os.link(executable, root / 'cargo-hashed-alias')
            original = executable.read_bytes()
            report = self.run_example(root, executable)
            retained = root / 'target/quality/handover-bin/probe'
            self.assertEqual(report['path'], str(retained))
            self.assertEqual(report['links'], 1)
            self.assertEqual(retained.read_bytes(), original)
            self.assertEqual(executable.stat().st_nlink, 2)
            self.assertEqual(report['execution']['compiler_executable_links'], 2)
            self.assertEqual(report['execution']['executable_sha256'], RUNNER.digest(executable))

    def test_compiler_symlink_and_fifo_refuse_without_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = self.executable(root)
            alias = root / 'alias'
            alias.symlink_to(executable)
            fifo = root / 'fifo'
            os.mkfifo(fifo)
            for invalid in (alias, fifo):
                with self.subTest(invalid=invalid.name), self.assertRaises((OSError, ValueError)):
                    self.run_example(root, invalid)
            self.assertFalse((root / 'target/quality/handover-bin/probe').exists())

    def test_compiler_output_change_during_execution_refuses_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.executable(root, 'open(' + repr(str(root / 'cargo-output')) + ', "a").write("# changed\\n")')
            with self.assertRaisesRegex(ValueError, 'changed during execution'):
                self.run_example(root, root / 'cargo-output')

    def test_executed_copy_change_during_execution_refuses_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = self.executable(root, 'open(sys.argv[0], "a").write("# changed\\n")')
            original = executable.read_bytes()
            with self.assertRaisesRegex(ValueError, 'changed during execution'):
                self.run_example(root, executable)
            self.assertEqual(executable.read_bytes(), original)
