"""Source admission failure tests on owned copies of the decoder patch."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('archive_patch', SOURCE / 'xtask/check_archive_patch.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class ArchivePatchPolicy(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='librairii-archive-patch-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name) / 'source'
        shutil.copytree(SOURCE / 'third_party', self.root / 'third_party')
        for name in ['Cargo.toml', 'fuzz/Cargo.toml', '.config/sevenz-turbo-patch.json']:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(SOURCE / name, path)

    def snapshot(self):
        return {p.relative_to(self.root).as_posix(): p.read_bytes()
                for p in self.root.rglob('*') if p.is_file()}

    def test_pinned_tree_and_both_graph_overrides_pass_without_writes(self):
        before = self.snapshot()
        report = gate.check(self.root)
        self.assertEqual(report['upstream_checksum'], gate.ARCHIVE_SHA)
        self.assertEqual(set(report['patched_files']), {'src/reader.rs', 'src/codec/brotli.rs'})
        self.assertEqual(before, self.snapshot())

    def test_missing_extra_and_modified_sources_fail(self):
        path = self.root / 'third_party/sevenz-turbo/src/codec/brotli.rs'
        original = path.read_bytes()
        for data in [None, original + b'\n// changed\n']:
            with self.subTest(data=data is None):
                if data is None:
                    path.unlink()
                else:
                    path.write_bytes(data)
                with self.assertRaises((OSError, ValueError)):
                    gate.check(self.root)
                path.write_bytes(original)
        (path.parent / 'extra.rs').write_bytes(b'additional source')
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_original_source_and_upstream_identity_cannot_self_repin(self):
        manifest = self.root / '.config/sevenz-turbo-patch.json'
        raw = manifest.read_bytes()
        original = json.loads(raw)
        for field, value in [('archive_sha256', '0' * 64), ('git_revision', '0' * 40),
                             ('source', 'https://example.invalid/crate')]:
            with self.subTest(field=field):
                pin = dict(original, **{field: value})
                manifest.write_text(json.dumps(pin))
                with self.assertRaises(ValueError):
                    gate.check(self.root)
        pin = json.loads(raw)
        source = self.root / 'third_party/sevenz-turbo/src/lib.rs'
        data = source.read_bytes() + b'\n// changed upstream\n'
        source.write_bytes(data)
        pin['files']['src/lib.rs'] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
        manifest.write_text(json.dumps(pin))
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_new_patched_file_and_duplicate_manifest_fields_fail(self):
        manifest = self.root / '.config/sevenz-turbo-patch.json'
        raw = manifest.read_text()
        pin = json.loads(raw)
        pin['patched_files']['src/lib.rs'] = pin['files']['src/lib.rs']
        manifest.write_text(json.dumps(pin))
        with self.assertRaises(ValueError):
            gate.check(self.root)
        manifest.write_text('{"name":"sevenz-turbo",' + raw[1:])
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_root_nested_and_source_aliases_fail(self):
        for relative in ['third_party', 'third_party/sevenz-turbo',
                         'third_party/sevenz-turbo/src', 'third_party/sevenz-turbo/src/lib.rs']:
            with self.subTest(relative=relative):
                path = self.root / relative
                original = path.with_name(path.name + '-original')
                path.rename(original)
                path.symlink_to(original, target_is_directory=original.is_dir())
                with self.assertRaises(ValueError):
                    gate.check(self.root)
                path.unlink()
                original.rename(path)
        path = self.root / 'third_party/sevenz-turbo/src/lib.rs'
        os.link(path, self.root / 'peer')
        with self.assertRaises(ValueError):
            gate.check(self.root)

    def test_either_dependency_graph_cannot_bypass_patch(self):
        for relative in ['Cargo.toml', 'fuzz/Cargo.toml']:
            path = self.root / relative
            original = path.read_text()
            path.write_text(original.replace('third_party/sevenz-turbo', 'third_party/unreviewed'))
            with self.assertRaises(ValueError):
                gate.check(self.root)
            path.write_text(original)
