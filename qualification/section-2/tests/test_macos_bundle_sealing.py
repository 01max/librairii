"""Verify the actual macOS resource seal, independently of GUI runtime checks."""
import importlib.util
import pathlib
import plistlib
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'qualification_package', pathlib.Path(__file__).parents[1] / 'package.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)

# These exercise native macOS tooling; Linux still runs the archive byte checks.
if sys.platform == 'darwin':
    class MacBundleSealing(unittest.TestCase):
        def fixture(self, root):
            bundle = root / 'Probe.app'
            contents = bundle / 'Contents'
            (contents / 'MacOS').mkdir(parents=True)
            (contents / 'Resources').mkdir()
            info = contents / 'Info.plist'
            info.write_bytes(plistlib.dumps({'CFBundleIdentifier': 'dev.librairii.seal-test',
                'CFBundleExecutable': 'probe', 'CFBundlePackageType': 'APPL'}))
            notice = contents / 'Resources' / 'NOTICE.txt'
            notice.write_text('Original synthetic notice\n')
            subprocess.run(['/usr/bin/clang', '-x', 'c', '-', '-o',
                str(contents / 'MacOS' / 'probe')], input='int main(void) { return 0; }\n',
                text=True, capture_output=True, check=True)
            return bundle, info, notice

        def verify(self, bundle):
            return subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict',
                str(bundle)], capture_output=True, text=True)

        def test_completed_bundle_seal_rejects_notice_tampering(self):
            with tempfile.TemporaryDirectory() as directory:
                bundle, _, notice = self.fixture(pathlib.Path(directory))
                self.assertNotEqual(self.verify(bundle).returncode, 0)
                package.seal_macos_bundle(bundle)
                self.assertEqual(self.verify(bundle).returncode, 0)
                self.assertTrue((bundle / 'Contents/_CodeSignature/CodeResources').is_file())
                notice.write_text('Changed after signing\n')
                self.assertNotEqual(self.verify(bundle).returncode, 0)

        def test_completed_bundle_seal_rejects_plist_tampering(self):
            with tempfile.TemporaryDirectory() as directory:
                bundle, info, _ = self.fixture(pathlib.Path(directory))
                package.seal_macos_bundle(bundle)
                self.assertEqual(self.verify(bundle).returncode, 0)
                values = plistlib.loads(info.read_bytes())
                values['CFBundleIdentifier'] = 'dev.librairii.changed'
                info.write_bytes(plistlib.dumps(values))
                self.assertNotEqual(self.verify(bundle).returncode, 0)

if __name__ == '__main__':
    unittest.main()
