import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release import sbom


class CompiledGraph(unittest.TestCase):
    def setUp(self):
        self.owner = tempfile.TemporaryDirectory(prefix='librairii-sbom-test-')
        self.addCleanup(self.owner.cleanup)
        self.root = Path(self.owner.name)
        (self.root / '.config/upstream-notices').mkdir(parents=True)
        (self.root / '.config/upstream-notices/derivative.txt').write_text('synthetic derivative notice')
        source = self.root / 'qualification/section-2/notices'
        (source / 'native-skia').mkdir(parents=True)
        (source / 'native-skia/native-license').write_bytes(b'synthetic native notice')
        self.native = {'components': [], 'files': {'native-license': hashlib.sha256(b'synthetic native notice').hexdigest()}}
        (source / 'native-skia/inventory.json').write_text(json.dumps(self.native))
        (source / 'Slint-Royalty-free-2.0.md').write_text('synthetic license witness; not release text')
        packages = []
        locks = []
        for name in ['runtime', 'compiler-helper', 'unused-dev']:
            path = self.root / 'registry' / name
            path.mkdir(parents=True)
            (path / 'Cargo.toml').write_text('synthetic')
            (path / 'LICENSE').write_text('synthetic notice ' + name)
            packages.append({'id': name, 'name': name, 'version': '1.0.0',
                             'manifest_path': str(path / 'Cargo.toml'),
                             'source': 'registry+https://example.invalid/index', 'license': 'MIT'})
            locks.append('[[package]]\nname="' + name + '"\nversion="1.0.0"\nsource="registry+https://example.invalid/index"\nchecksum="' + 'a' * 64 + '"\n')
        (self.root / 'Cargo.lock').write_text('\n'.join(locks))
        self.metadata = {'packages': packages}
        self.events = [{'package_id': name, 'features': ['actual-feature']} for name in ['runtime', 'compiler-helper']]
        self.release = {'source_commit': 'b' * 40, 'source_date_epoch': 1234,
                        'version': '0.5.0', 'target': 'synthetic-test', 'cargo_lock_sha256': 'c' * 64}

    def test_sbom_describes_actual_compiler_graph_and_excludes_unused_dev_metadata(self):
        directory = self.root / 'release'
        directory.mkdir()
        sbom.write(self.root, directory, self.release, self.metadata, self.events)
        value = json.loads((directory / 'sbom.spdx.json').read_text())
        self.assertEqual(value['spdxVersion'], 'SPDX-2.3')
        self.assertEqual({p['name'] for p in value['packages']}, {'Librairii', 'runtime', 'compiler-helper'})
        self.assertEqual(value['creationInfo']['created'], '1970-01-01T00:20:34Z')
        self.assertFalse((directory / 'notices/unused-dev-1.0.0').exists())
        self.assertEqual((directory / 'notices/native-skia/native-license').read_bytes(), b'synthetic native notice')
        second = self.root / 'repeat'
        second.mkdir()
        sbom.write(self.root, second, self.release, self.metadata, self.events)
        self.assertEqual((directory / 'sbom.spdx.json').read_bytes(), (second / 'sbom.spdx.json').read_bytes())

    def test_sbom_refuses_missing_compiled_identity_and_changed_native_notices(self):
        destination = self.root / 'missing'
        destination.mkdir()
        with self.assertRaises(ValueError):
            sbom.write(self.root, destination, self.release, self.metadata,
                       [{'package_id': 'unrecorded', 'features': []}])
        (self.root / 'qualification/section-2/notices/native-skia/native-license').write_bytes(b'tampered')
        changed = self.root / 'changed'
        changed.mkdir()
        with self.assertRaises(ValueError):
            sbom.write(self.root, changed, self.release, self.metadata, self.events)
        self.assertFalse((changed / 'sbom.spdx.json').exists())

    def test_pinned_supplemental_license_satisfies_crates_without_packaged_license_files(self):
        (self.root / 'registry/runtime/LICENSE').unlink()
        source = self.root / '.config/upstream-notices/runtime-1.0.0'
        source.mkdir()
        (source / 'LICENSE-MIT').write_bytes(b'synthetic pinned supplemental license')
        directory = self.root / 'supplemental-release'
        directory.mkdir()
        sbom.write(self.root, directory, self.release, self.metadata, self.events)
        self.assertEqual((directory / 'notices/runtime-1.0.0/supplemental/LICENSE-MIT').read_bytes(),
                         b'synthetic pinned supplemental license')
        self.assertTrue((directory / 'sbom.spdx.json').is_file())
