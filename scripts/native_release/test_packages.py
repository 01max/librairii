"""Package contents, final-byte binding and native dependency policy regressions."""
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release.files import digest
from native_release.linkage import linux_report, macos_report
from native_release import package


class PlatformPackages(unittest.TestCase):
    def test_linkage_refuses_browser_foreign_paths_newer_glibc_and_wrong_deployment(self):
        dynamic = '0x1 (NEEDED) Shared library: [libc.so.6]\n'
        versions = 'Name: GLIBC_2.39\nName: GLIBC_2.17\n'
        interpreter = '[Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]'
        loaded = 'libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x1234)'
        self.assertEqual(linux_report(dynamic, versions, interpreter, loaded)['maximum_glibc'], '2.39')
        for changed in [dict(dynamic=dynamic+'(NEEDED) [libwebkit.so]\n'),
                        dict(versions=versions+'Name: GLIBC_2.40\n'),
                        dict(interpreter='/foreign/ld.so'), dict(loaded='libc.so.6 => not found'),
                        dict(loaded='libc.so.6 => /checkout/libc.so.6 (0x1234)')]:
            values = dict(dynamic=dynamic, versions=versions, interpreter=interpreter, loaded=loaded)
            values.update(changed)
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                linux_report(**values)
        libraries = 'app:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n'
        commands = 'cmd LC_BUILD_VERSION\n minos 15.0\n'
        self.assertEqual(macos_report(libraries, commands)['minimum_os'], '15.0')
        for libraries, commands in [(libraries, commands.replace('15.0', '14.0')),
                                    (libraries+'\t@rpath/developer.dylib\n', commands),
                                    (libraries+'\t/System/Library/Frameworks/WebKit.framework/WebKit\n', commands)]:
            with self.assertRaises(ValueError):
                macos_report(libraries, commands)

    def test_linux_archive_contains_desktop_icons_notices_and_exact_final_checksums(self):
        with tempfile.TemporaryDirectory(prefix='librairii-package-test-') as temporary:
            root = Path(temporary).resolve()
            stage = root / 'stage'
            shared = stage / 'payload/share/librairii'
            shared.mkdir(parents=True)
            (stage / 'payload/bin').mkdir()
            (stage / 'payload/bin/librairii-desktop').write_bytes(b'synthetic production bytes')
            (stage / 'payload/bin/librairii-desktop').chmod(0o755)
            for name in ('appicon.png', 'appicon.svg', 'sbom.spdx.json', 'LICENSE'):
                (shared / name).write_bytes(b'synthetic retained '+name.encode())
            (root / 'Cargo.lock').write_bytes(b'synthetic locked graph')
            release = {key: 'synthetic' for key in ['source_commit', 'source_tree', 'rust_version', 'native_toolkit_pin_sha256', 'minimum_os']}
            release.update(version='0.5.0', target='x86_64-unknown-linux-gnu', features=[], source_date_epoch=1234,
                           cargo_lock_sha256=digest(root / 'Cargo.lock'))
            with patch('native_release.package.inputs', return_value=(stage, release, {'host': 'synthetic'})), \
                 patch('native_release.package.linux_baseline', return_value={'baseline': 'synthetic'}), \
                 patch('native_release.package.linkage.inspect', return_value={'needed': ['libc.so.6']}), \
                 patch('native_release.package.clean_source', return_value='synthetic'), \
                 patch('native_release.package.subprocess.run') as run:
                report = package.linux(root, stage, 'target/native-release/linux')
            output = root / 'target/native-release/linux'
            artifact = output / report['artifact']['name']
            self.assertEqual(report['artifact']['sha256'], digest(artifact))
            self.assertFalse(report['public_release_qualified'])
            self.assertEqual(report['qualification']['state'], 'not performed')
            self.assertEqual(run.call_args.args[0][0], 'desktop-file-validate')
            self.assertIn('share/applications/io.github.librairii.native.desktop', report['payload'])
            self.assertIn('share/icons/hicolor/1024x1024/apps/io.github.librairii.native.png', report['payload'])
            with tarfile.open(artifact) as archive:
                prefix = 'Librairii-0.5.0-x86_64-unknown-linux-gnu/'
                self.assertEqual(archive.extractfile(prefix+'bin/librairii-desktop').read(), b'synthetic production bytes')
                self.assertIn(b'Exec=librairii-desktop\n', archive.extractfile(prefix+'share/applications/io.github.librairii.native.desktop').read())
                prerequisites = json.load(archive.extractfile(prefix+'share/librairii/runtime-requirements.json'))
                self.assertIn('xdg-desktop-portal', prerequisites['packages'])
            self.assertEqual((stage / 'payload/bin/librairii-desktop').read_bytes(), b'synthetic production bytes')


if __name__ == '__main__':
    unittest.main()
