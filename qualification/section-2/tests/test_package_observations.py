"""The operator recorder must reject aliases/tampering and never pre-fill passes."""
import contextlib
import hashlib
import importlib.util
import io
import json
import pathlib
import platform
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('observe', pathlib.Path(__file__).parents[1] / 'observe.py')
observe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observe)

class Observations(unittest.TestCase):
    def fixture(self, root):
        package = root / 'package'
        package.mkdir()
        binary = package / 'qualification-desktop'
        binary.write_bytes(b'synthetic fixture, never executable')
        target = {('Darwin','arm64'):'aarch64-apple-darwin',
                  ('Darwin','x86_64'):'x86_64-apple-darwin',
                  ('Linux','x86_64'):'x86_64-unknown-linux-gnu'}[(platform.system(), platform.machine())]
        sha = hashlib.sha256(binary.read_bytes()).hexdigest()
        manifest = {'target':target,'source':'synthetic','source_dirty':False,
                    'executable':binary.name,'binary_sha256':sha,'files':{binary.name:sha}}
        (package/'manifest.json').write_text(json.dumps(manifest))
        return package, manifest

    def run_recorder(self, package, output, signature_valid=True):
        def run(command, **kwargs):
            code = 1 if command[0] == '/usr/bin/codesign' and not signature_valid else 0
            return subprocess.CompletedProcess(command, code, '0', '')
        with patch.object(sys, 'argv', ['observe.py','--package',str(package),'--output',str(output),'--operator','test']), \
             patch.object(observe.subprocess, 'run', side_effect=run), \
             patch.object(observe.subprocess, 'check_output', return_value='synthetic OS'), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            observe.main()

    def test_verified_package_starts_every_observation_as_not_run(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            package, _ = self.fixture(root)
            output = root/'observations.json'
            self.run_recorder(package, output)
            record = json.loads(output.read_text())
            self.assertEqual(set(record['checks']), set(observe.CHECKS))
            self.assertTrue(all(check['status']=='not_run' for check in record['checks'].values()))
            self.assertFalse(record['toolkit_selected'])

    def test_changed_binary_cannot_create_observation_record(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            package, _ = self.fixture(root)
            (package/'qualification-desktop').write_bytes(b'changed')
            with self.assertRaises(SystemExit):
                self.run_recorder(package, root/'observations.json')
            self.assertFalse((root/'observations.json').exists())

    def test_matching_bytes_with_invalid_macos_signature_cannot_create_record(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(observe.platform, 'system', return_value='Darwin'), \
             patch.object(observe.platform, 'machine', return_value='arm64'):
            root = pathlib.Path(directory)
            package, manifest = self.fixture(root)
            executable = 'Probe.app/Contents/MacOS/probe'
            binary = package / executable
            binary.parent.mkdir(parents=True)
            (package / manifest['executable']).rename(binary)
            manifest['executable'] = executable
            manifest['files'] = {executable: manifest['binary_sha256']}
            (package / 'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaises(SystemExit):
                self.run_recorder(package, root/'observations.json', signature_valid=False)
            self.assertFalse((root/'observations.json').exists())

    def test_manifest_cannot_follow_an_outside_alias_or_unlisted_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            package, manifest = self.fixture(root)
            external = root/'outside'
            (package/'qualification-desktop').rename(external)
            (package/'qualification-desktop').symlink_to(external)
            with self.assertRaises(SystemExit):
                self.run_recorder(package, root/'observations.json')
            (package/'qualification-desktop').unlink()
            external.rename(package/'qualification-desktop')
            manifest['executable'] = str(external)
            (package/'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaises(SystemExit):
                self.run_recorder(package, root/'observations.json')
            self.assertFalse((root/'observations.json').exists())

if __name__ == '__main__':
    unittest.main()
