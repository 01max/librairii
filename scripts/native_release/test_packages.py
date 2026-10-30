"""Package contents, final-byte binding and native dependency policy regressions."""
import json
import io
import plistlib
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release.files import digest, inventory
from native_release.archive import seal_artifact, tar_gzip
from native_release.linkage import linux_report, macos_report
from native_release.identity import runtime_host
from native_release import install, macos, package


class PlatformPackages(unittest.TestCase):
    def test_installed_intel_host_check_refuses_rosetta_and_needs_no_development_tools(self):
        from subprocess import CompletedProcess
        with patch('native_release.identity.platform.system', return_value='Darwin'), \
             patch('native_release.identity.platform.machine', return_value='x86_64'), \
             patch('native_release.identity.platform.mac_ver', return_value=('15.7', (), '')), \
             patch('native_release.identity.output', return_value='Intel(R) Core(TM) processor') as output, \
             patch('native_release.identity.subprocess.run', return_value=CompletedProcess([], 1, stdout='', stderr='')) as run:
            observed = runtime_host(Path('/unused'), 'x86_64-apple-darwin')
            self.assertIn('Intel', observed['cpu'])
            self.assertEqual(output.call_args.args[1], ['sysctl', '-n', 'machdep.cpu.brand_string'])
            self.assertEqual(run.call_args.args[0], ['sysctl', '-in', 'sysctl.proc_translated'])
            run.return_value = CompletedProcess([], 0, stdout='1\n', stderr='')
            with self.assertRaisesRegex(ValueError, 'actual Intel hardware'):
                runtime_host(Path('/unused'), 'x86_64-apple-darwin')

    def test_installer_refuses_wrong_source_changed_final_bytes_and_escaping_archive_members(self):
        with tempfile.TemporaryDirectory(prefix='librairii-installed-test-') as temporary:
            root = Path(temporary).resolve()
            payload = root / 'payload'
            payload.mkdir()
            (payload / 'app').write_bytes(b'owned product')
            artifact = root / 'candidate.tar.gz'
            release = {key: 'synthetic' for key in ['source_tree', 'rust_version', 'native_toolkit_pin_sha256', 'minimum_os', 'cargo_lock_sha256']}
            release.update(version='0.5.0', source_commit='a'*40, target='x86_64-unknown-linux-gnu', features=[], source_date_epoch=1234)
            prefix = 'Librairii-0.5.0-x86_64-unknown-linux-gnu'
            tar_gzip(payload, artifact, prefix, 1234)
            record = seal_artifact(root, artifact, payload, release, 'tar.gz', {'state': 'synthetic'})
            self.assertEqual(install.artifact_record(root, 'a'*40, release['target'])[1], record)
            with self.assertRaisesRegex(ValueError, 'source/target'):
                install.artifact_record(root, 'b'*40, release['target'])
            valid = root / 'valid'
            valid.mkdir()
            install.extract(artifact, valid, record)
            self.assertEqual(inventory(valid), inventory(payload))
            with artifact.open('ab') as stream:
                stream.write(b'tampering')
            with self.assertRaisesRegex(ValueError, 'artifact bytes'):
                install.artifact_record(root, 'a'*40, release['target'])
            canary = root / 'preserved'
            canary.write_bytes(b'never replace external state')
            for index, name in enumerate(['../preserved', prefix+'/../preserved', prefix+'/app']):
                hostile = root / f'hostile-{index}.tar.gz'
                with tarfile.open(hostile, 'w:gz') as archive:
                    entry = tarfile.TarInfo(name)
                    if index == 2:
                        entry.type, entry.linkname = tarfile.SYMTYPE, str(canary)
                        archive.addfile(entry)
                    else:
                        entry.size, entry.mode = 12, 0o644
                        archive.addfile(entry, io.BytesIO(b'foreign data'))
                destination = root / f'refused-{index}'
                destination.mkdir()
                with self.assertRaises(ValueError):
                    install.extract(hostile, destination, record)
                self.assertEqual(canary.read_bytes(), b'never replace external state')

    def test_macos_bundle_identity_has_native_version_baseline_and_embedded_catalogs(self):
        for target in ('aarch64-apple-darwin', 'x86_64-apple-darwin'):
            observed = plistlib.loads(plistlib.dumps(macos.app_info({'version': '0.5.0', 'target': target})))
            self.assertEqual(observed['CFBundleShortVersionString'], '0.5.0')
            self.assertEqual(observed['CFBundleVersion'], '0.5.0')
            self.assertEqual(observed['LSMinimumSystemVersion'], '15.0')
            self.assertEqual(observed['CFBundleExecutable'], 'librairii-desktop')
            self.assertEqual(observed['CFBundleLocalizations'], ['en', 'fr'])
            self.assertTrue(observed['LSRequiresNativeExecution'])

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
            self.assertEqual(report['payload']['bin/librairii-desktop']['mode'], 0o755)
            self.assertFalse(report['public_release_qualified'])
            self.assertEqual(report['qualification']['state'], 'not performed')
            self.assertEqual(run.call_args.args[0][0], 'desktop-file-validate')
            self.assertIn('share/applications/io.github.librairii.native.desktop', report['payload'])
            self.assertIn('share/icons/hicolor/1024x1024/apps/io.github.librairii.native.png', report['payload'])
            with tarfile.open(artifact) as archive:
                prefix = 'Librairii-0.5.0-x86_64-unknown-linux-gnu/'
                self.assertEqual(archive.extractfile(prefix+'bin/librairii-desktop').read(), b'synthetic production bytes')
                self.assertEqual(archive.getmember(prefix+'bin/librairii-desktop').mode, 0o755)
                self.assertIn(b'Exec=librairii-desktop\n', archive.extractfile(prefix+'share/applications/io.github.librairii.native.desktop').read())
                prerequisites = json.load(archive.extractfile(prefix+'share/librairii/runtime-requirements.json'))
                self.assertIn('xdg-desktop-portal', prerequisites['packages'])
            self.assertEqual((stage / 'payload/bin/librairii-desktop').read_bytes(), b'synthetic production bytes')


if __name__ == '__main__':
    unittest.main()
