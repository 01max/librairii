"""Preserve actual ABI requirements and refuse inspection of substituted bytes."""
import hashlib
import importlib.util
import pathlib
import json
import platform
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('runtime_inspection', pathlib.Path(__file__).parents[1] / 'inspect-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimeInspection(unittest.TestCase):
    def test_linux_numeric_versions_missing_libraries_and_loader_paths(self):
        report = runtime.linux_requirements(
            '(NEEDED) Shared library: [libc.so.6]\n(RUNPATH) Library runpath: [$ORIGIN/lib]',
            'Name: GLIBC_2.9\nName: GLIBC_2.34\nName: GLIBCXX_3.4.30\nName: GLIBC_ABI_DT_RELR',
            '\tlibc.so.6 => /lib/libc.so.6 (0x01)\n\tlibmissing.so => not found\n')
        self.assertEqual(report['maximum_numeric_symbol_versions'], {'GLIBC': '2.34', 'GLIBCXX': '3.4.30'})
        self.assertIn('GLIBC_ABI_DT_RELR', report['required_version_names'])
        self.assertEqual(report['unresolved_libraries'], ['libmissing.so'])
        self.assertEqual(report['needed_sonames'], ['libc.so.6'])
        self.assertEqual(report['loader_paths'], ['$ORIGIN/lib'])

    def test_macos_external_paths_and_minos_are_not_confused_with_sdk_or_source_version(self):
        report = runtime.mac_requirements(
            'probe:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n\t@rpath/vendor.dylib (compatibility version 1.0.0)',
            'Load command 1\n cmd LC_BUILD_VERSION\n minos 15.0\n sdk 26.2\n'
            'Load command 2\n cmd LC_SOURCE_VERSION\n version 1.2.3\n'
            'Load command 3\n cmd LC_RPATH\n path /external/lib (offset 12)')
        self.assertEqual(report['minimum_os_load_commands'], ['15.0'])
        self.assertEqual(report['non_system_libraries'], ['@rpath/vendor.dylib'])
        self.assertEqual(report['rpaths'], ['/external/lib'])

    def test_macos_legacy_version_command(self):
        report = runtime.mac_requirements('probe:\n', 'Load command 1\n cmd LC_VERSION_MIN_MACOSX\n version 15.0\n sdk 26.2')
        self.assertEqual(report['minimum_os_load_commands'], ['15.0'])

    def test_modified_executable_is_refused_before_native_tools(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            binary = root / 'probe'
            binary.write_bytes(b'original bytes')
            digest = hashlib.sha256(binary.read_bytes()).hexdigest()
            manifest = {'executable': 'probe', 'binary_sha256': digest, 'files': {'probe': digest}}
            self.assertEqual(runtime.verified_binary(root, manifest), binary.resolve())
            binary.write_bytes(b'changed bytes')
            with self.assertRaisesRegex(ValueError, 'bytes changed'):
                runtime.verified_binary(root, manifest)

    def test_executable_cannot_alias_a_file_outside_package(self):
        with tempfile.TemporaryDirectory() as directory:
            base = pathlib.Path(directory)
            root = base / 'package'
            root.mkdir()
            external = base / 'external'
            external.write_bytes(b'outside')
            (root / 'probe').symlink_to(external)
            digest = hashlib.sha256(external.read_bytes()).hexdigest()
            manifest = {'executable': 'probe', 'binary_sha256': digest, 'files': {'probe': digest}}
            with self.assertRaisesRegex(ValueError, 'escapes package'):
                runtime.verified_binary(root, manifest)

    def test_tool_timeout_retains_partial_output_without_qualifying_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            binary = root / 'probe'
            binary.write_bytes(b'synthetic bytes; never executed')
            digest = hashlib.sha256(binary.read_bytes()).hexdigest()
            target = next(name for name, host in runtime.TARGETS.items()
                          if host == (platform.system(), platform.machine()))
            manifest = {'target': target, 'source': 'synthetic', 'source_dirty': False,
                        'executable': 'probe', 'binary_sha256': digest, 'files': {'probe': digest}}
            (root / 'manifest.json').write_text(json.dumps(manifest))
            with patch.object(runtime.subprocess, 'run', side_effect=subprocess.TimeoutExpired('native tool', 30, output=b'partial output')):
                report = runtime.inspect(root)
            self.assertEqual(report['immediate_link_inspection'], 'fail')
            self.assertTrue(all(value['timed_out'] for value in report['tool_outputs'].values()))
            self.assertTrue(all(value['stdout'] == 'partial output' for value in report['tool_outputs'].values()))
            self.assertFalse(report['minimum_supported_os_qualified'])
            self.assertFalse(report['toolkit_selected'])

    def test_missing_native_tool_is_a_retained_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            binary = root / 'probe'
            binary.write_bytes(b'synthetic bytes; never executed')
            digest = hashlib.sha256(binary.read_bytes()).hexdigest()
            target = next(name for name, host in runtime.TARGETS.items()
                          if host == (platform.system(), platform.machine()))
            manifest = {'target': target, 'source': 'synthetic', 'source_dirty': False,
                        'executable': 'probe', 'binary_sha256': digest, 'files': {'probe': digest}}
            (root / 'manifest.json').write_text(json.dumps(manifest))
            with patch.object(runtime.subprocess, 'run', side_effect=FileNotFoundError('native tool unavailable')):
                report = runtime.inspect(root)
            self.assertEqual(report['immediate_link_inspection'], 'fail')
            self.assertTrue(all(value['tool_error'] for value in report['tool_outputs'].values()))
            self.assertTrue(all('native tool unavailable' in value['stderr'] for value in report['tool_outputs'].values()))


if __name__ == '__main__':
    unittest.main()
