"""Headless checks of explicit execution and human signing handoff boundaries."""
from contextlib import redirect_stderr, redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


def load(name):
    path = Path(__file__).resolve().parents[1] / (name + '.py')
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ManualTools(unittest.TestCase):
    def test_package_preview_has_no_output_or_build_side_effect(self):
        tool = load('build-native-package')
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(tool, 'ROOT', Path(temporary).resolve()), \
                mock.patch.object(tool, 'plan', return_value={}) as planning, \
                mock.patch.object(tool.subprocess, 'run') as run, redirect_stdout(io.StringIO()) as output:
            tool.main(['--target', 'aarch64-apple-darwin', '--name', 'preview'])
            run.assert_not_called()
            self.assertEqual(list(Path(temporary).resolve().iterdir()), [])
            planning.assert_called_once_with(Path(temporary).resolve(), 'aarch64-apple-darwin', require_clean=False)
            self.assertFalse(json.loads(output.getvalue())['execute'])

    def test_package_execution_keeps_verified_order_for_each_target(self):
        tool = load('build-native-package')
        for target in tool.TARGETS:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary, \
                    mock.patch.object(tool, 'ROOT', Path(temporary).resolve()), \
                    mock.patch.object(tool, 'plan', return_value={}) as planning, \
                    mock.patch.object(tool.subprocess, 'run') as run, redirect_stdout(io.StringIO()):
                tool.main(['--target', target, '--name', 'chosen', '--execute'])
                expected = ['build', 'verify-stage', 'package-macos' if target.endswith('apple-darwin') else 'package-linux']
                self.assertEqual([call.args[0][4] for call in run.call_args_list], expected)
                self.assertTrue(all(call.kwargs == {'cwd': Path(temporary).resolve(), 'check': True} for call in run.call_args_list))
                planning.assert_called_once_with(Path(temporary).resolve(), target, require_clean=True)

    def test_existing_outputs_and_traversal_never_start_build(self):
        tool = load('build-native-package')
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(tool, 'ROOT', Path(temporary).resolve()), \
                mock.patch.object(tool, 'plan', return_value={}), \
                mock.patch.object(tool.subprocess, 'run') as run, redirect_stdout(io.StringIO()):
            directory = Path(temporary).resolve() / 'target/native-release/existing'
            directory.mkdir(parents=True)
            marker = directory / 'retained'
            marker.write_bytes(b'previous evidence')
            with self.assertRaises(FileExistsError):
                tool.main(['--target', 'aarch64-apple-darwin', '--name', 'existing', '--execute'])
            with self.assertRaises(ValueError):
                tool.main(['--target', 'aarch64-apple-darwin', '--name', '../escape', '--execute'])
            run.assert_not_called()
            self.assertEqual(marker.read_bytes(), b'previous evidence')

    def test_failure_stops_later_steps_and_retains_output(self):
        tool = load('build-native-package')
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(tool, 'ROOT', Path(temporary).resolve()), \
                mock.patch.object(tool, 'plan', return_value={}), \
                mock.patch.object(tool.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'build')) as run, \
                redirect_stdout(io.StringIO()):
            with self.assertRaises(subprocess.CalledProcessError):
                tool.main(['--target', 'aarch64-apple-darwin', '--name', 'failed', '--execute'])
            self.assertEqual(run.call_count, 1)
            self.assertTrue((Path(temporary).resolve() / 'target/native-release/failed').is_dir())

    def test_signing_preview_preserves_selection_without_dispatch(self):
        tool = load('prepare-native-signing')
        with redirect_stdout(io.StringIO()) as output:
            tool.main(['--source', 'a' * 40, '--candidate-run', '123', '--target', 'x86_64-apple-darwin',
                       '--unsigned-sha256', 'b' * 64])
        receipt = json.loads(output.getvalue())
        self.assertFalse(receipt['dispatched'])
        self.assertFalse(receipt['inputs_verified'])
        self.assertFalse(receipt['public_release_qualified'])
        command = receipt['argv']
        self.assertEqual(command[:4], ['gh', 'workflow', 'run', 'rust-sign-macos.yml'])
        for value in ['source=' + 'a' * 40, 'candidate_run=123', 'target=x86_64-apple-darwin',
                      'unsigned_sha256=' + 'b' * 64, 'rewrite-in-rust-native-desktop']:
            self.assertIn(value, command)

    def test_signing_refuses_invalid_selection_before_handoff(self):
        tool = load('prepare-native-signing')
        valid = ['--source', 'a' * 40, '--candidate-run', '123', '--target', 'aarch64-apple-darwin',
                 '--unsigned-sha256', 'b' * 64]
        for index, value in [(1, 'HEAD'), (3, '0'), (5, 'x86_64-unknown-linux-gnu'), (7, 'not-a-hash')]:
            invalid = valid.copy()
            invalid[index] = value
            with self.subTest(value=value), redirect_stdout(io.StringIO()) as output, redirect_stderr(io.StringIO()), \
                    self.assertRaises(SystemExit):
                tool.main(invalid)
            self.assertEqual(output.getvalue(), '')
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            tool.main(valid + ['--trusted-ref', 'unreviewed-branch'])
