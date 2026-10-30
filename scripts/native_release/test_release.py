"""Release custody and reproducibility regressions; no GUI or real user state."""
import gzip
import io
import json
from pathlib import Path
import struct
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release.archive import seal_artifact, tar_gzip
from native_release.files import copy_file, copy_tree, fresh_output, inventory, write_json
from native_release.identity import architecture, cargo_executable, verify_embedded


class ReleaseFiles(unittest.TestCase):
    def setUp(self):
        self.owner = tempfile.TemporaryDirectory(prefix='librairii-release-test-')
        self.addCleanup(self.owner.cleanup)
        self.root = Path(self.owner.name).resolve()

    def test_output_refuses_existing_external_parent_and_boundary_links(self):
        destination = fresh_output(self.root, 'target/native-release/new')
        canary = destination / 'canary'
        canary.write_bytes(b'preserved')
        with self.assertRaises(FileExistsError):
            fresh_output(self.root, destination)
        self.assertEqual(canary.read_bytes(), b'preserved')
        for invalid in [self.root / 'outside', self.root / 'target/native-release/../outside']:
            with self.assertRaises(ValueError):
                fresh_output(self.root, invalid)
        (self.root / 'target/native-release/redirect').symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            fresh_output(self.root, 'target/native-release/redirect/child')
        other = self.root / 'other'
        other.mkdir()
        (other / 'target').symlink_to(self.root / 'target', target_is_directory=True)
        with self.assertRaises(ValueError):
            fresh_output(other, 'target/native-release/also-refused')

    def test_copy_preserves_bytes_but_never_follows_or_replaces_links(self):
        source = self.root / 'source'
        source.write_bytes(b'license and product bytes')
        target = self.root / 'target'
        copy_file(source, target, executable=True)
        self.assertEqual(source.read_bytes(), target.read_bytes())
        self.assertEqual(target.stat().st_mode & 0o777, 0o755)
        with self.assertRaises(FileExistsError):
            copy_file(source, target)
        link = self.root / 'link'
        link.symlink_to(source)
        with self.assertRaises(ValueError):
            copy_file(link, self.root / 'copy')
        with self.assertRaises(FileExistsError):
            copy_file(source, link)
        directory = self.root / 'directory'
        directory.mkdir()
        (directory / 'link').symlink_to(source)
        with self.assertRaises(ValueError):
            copy_tree(directory, self.root / 'tree')
        with self.assertRaises(ValueError):
            inventory(directory)
        self.assertEqual(source.read_bytes(), b'license and product bytes')

    def test_archive_is_deterministic_and_normalizes_all_metadata(self):
        payload = self.root / 'payload'
        (payload / 'bin').mkdir(parents=True)
        binary = payload / 'bin/app'
        binary.write_bytes(b'synthetic executable')
        binary.chmod(0o755)
        (payload / 'readme').write_text('synthetic notice\n')
        first, second = self.root / 'first.tar.gz', self.root / 'second.tar.gz'
        expected = tar_gzip(payload, first, 'Librairii-0.5.0-target', 1234)
        import os
        os.utime(binary, (7654321, 7654321))
        self.assertEqual(expected, tar_gzip(payload, second, 'Librairii-0.5.0-target', 1234))
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with tarfile.open(fileobj=io.BytesIO(gzip.decompress(first.read_bytes()))) as archive:
            entries = archive.getmembers()
            self.assertTrue(all(e.uid == e.gid == 0 and e.uname == e.gname == '' and e.mtime == 1234 for e in entries))
            self.assertEqual(archive.extractfile('Librairii-0.5.0-target/bin/app').read(), b'synthetic executable')
        with self.assertRaises(FileExistsError):
            tar_gzip(payload, first, 'safe', 1)
        for invalid in ['../escape', '/absolute', 'a/b']:
            with self.assertRaises(ValueError):
                tar_gzip(payload, self.root / 'refused', invalid, 1)

    def test_json_records_are_no_clobber_and_payload_tampering_is_visible(self):
        root = self.root / 'payload'
        root.mkdir()
        file = root / 'artifact'
        file.write_bytes(b'first')
        before = inventory(root)
        file.write_bytes(b'other')
        self.assertNotEqual(before, inventory(root))
        record = self.root / 'manifest.json'
        write_json(record, {'source': 'observed'})
        with self.assertRaises(FileExistsError):
            write_json(record, {'source': 'forged'})
        self.assertEqual(json.loads(record.read_text()), {'source': 'observed'})

    def test_architecture_rejects_wrong_target_fat_and_foreign_executables(self):
        binary = self.root / 'binary'
        elf = bytearray(32)
        elf[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', elf, 18, 62)
        binary.write_bytes(elf)
        architecture(binary, 'x86_64-unknown-linux-gnu')
        with self.assertRaises(ValueError):
            architecture(binary, 'aarch64-apple-darwin')
        macho = bytearray(32)
        macho[:4] = b'\xcf\xfa\xed\xfe'
        struct.pack_into('<I', macho, 4, 0x100000C)
        binary.write_bytes(macho)
        architecture(binary, 'aarch64-apple-darwin')
        with self.assertRaises(ValueError):
            architecture(binary, 'x86_64-apple-darwin')
        binary.write_bytes(b'\xca\xfe\xba\xbe' + bytes(28))
        with self.assertRaises(ValueError):
            architecture(binary, 'aarch64-apple-darwin')

    def test_actual_cargo_executable_requires_unique_production_profile(self):
        profile = {'opt_level': '3', 'debug_assertions': False, 'overflow_checks': False}
        release = {'profile': profile}
        event = {'reason': 'compiler-artifact', 'target': {'name': 'librairii-desktop', 'kind': ['bin']},
                 'executable': '/synthetic/product', 'profile': dict(profile)}
        path = self.root / 'cargo.jsonl'
        path.write_text(json.dumps(event) + '\n')
        self.assertEqual(cargo_executable(path, release)[0], event)
        for key, value in [('opt_level', '1'), ('debug_assertions', True), ('overflow_checks', True)]:
            changed = json.loads(json.dumps(event))
            changed['profile'][key] = value
            path.write_text(json.dumps(changed) + '\n')
            with self.assertRaises(ValueError):
                cargo_executable(path, release)
        path.write_text((json.dumps(event) + '\n') * 2)
        with self.assertRaises(ValueError):
            cargo_executable(path, release)

    def test_embedded_identity_refuses_missing_or_foreign_source_graph_and_target(self):
        release = {'version': '0.5.0', 'source_commit': 'a' * 40,
                   'source_tree': 'b' * 40, 'cargo_lock_sha256': 'c' * 64,
                   'target': 'aarch64-apple-darwin'}
        product = self.root / 'synthetic identity with spaces'
        observed = dict(release, debug_assertions=False, runtime='Rust/Slint')
        def program(value):
            product.write_text('#!' + sys.executable + '\nprint(' + repr(json.dumps(value)) + ')\n')
            product.chmod(0o755)
        program(observed)
        self.assertEqual(verify_embedded(self.root, product, release), observed)
        for key in release:
            changed = dict(observed)
            changed[key] = None
            program(changed)
            with self.assertRaises(ValueError):
                verify_embedded(self.root, product, release)
        program(dict(observed, debug_assertions=True))
        with self.assertRaises(ValueError):
            verify_embedded(self.root, product, release)

    def test_final_artifact_checksum_binds_actual_bytes_and_never_claims_qualification(self):
        payload = self.root / 'payload'
        payload.mkdir()
        (payload / 'app').write_bytes(b'synthetic product')
        artifact = self.root / 'candidate.tar.gz'
        tar_gzip(payload, artifact, 'candidate', 1234)
        release = {key: 'synthetic' for key in ['version', 'source_commit', 'source_tree',
                   'target', 'rust_version', 'cargo_lock_sha256', 'native_toolkit_pin_sha256', 'minimum_os']}
        release.update(source_date_epoch=1234, features=[])
        result = seal_artifact(self.root, artifact, payload, release, 'synthetic-test', {'state': 'unsigned'})
        self.assertEqual(result['payload'], inventory(payload))
        self.assertEqual(result['artifact']['bytes'], artifact.stat().st_size)
        self.assertFalse(result['public_release_qualified'])
        self.assertEqual(result['qualification']['state'], 'not performed')
        self.assertEqual(artifact.with_name(artifact.name + '.sha256').read_text().split()[0], result['artifact']['sha256'])
        with self.assertRaises(FileExistsError):
            seal_artifact(self.root, artifact, payload, release, 'synthetic-test', {'state': 'unsigned'})


if __name__ == '__main__':
    unittest.main()
