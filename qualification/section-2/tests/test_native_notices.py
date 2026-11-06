"""Packaging must carry the pinned native terms, including the IJG binary acknowledgment."""
import hashlib
import importlib.util
import json
import pathlib
import shutil
import tempfile
import unittest

HERE = pathlib.Path(__file__).parents[1]
spec = importlib.util.spec_from_file_location('native_notices', HERE / 'native-notices.py')
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class NativeNotices(unittest.TestCase):
    def fixture(self, root):
        source = root / 'source'
        shutil.copytree(HERE / 'notices/native-skia', source)
        inventory = json.loads((source / 'inventory.json').read_text())
        bindings = [{'version': inventory['skia_bindings_version'], 'metadata': {'skia': inventory['skia_source_ref']}}]
        native = root / 'LICENSE_SKIA'
        shutil.copy2(source / 'LICENSE_SKIA', native)
        destination = root / 'notices'
        destination.mkdir()
        return source, inventory, bindings, native, destination

    def test_complete_real_supplement_is_copied_with_exact_terms_and_acknowledgment(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, inventory, bindings, native, destination = self.fixture(pathlib.Path(temporary))
            record = notices.include(source, destination, bindings, [native])
            for name, digest in inventory['files'].items():
                self.assertEqual(notices.sha(destination / 'native-skia' / name), digest)
            self.assertEqual(record['inventory_sha256'], notices.sha(destination / 'native-skia/inventory.json'))
            self.assertIn(notices.IJG_ACKNOWLEDGMENT, (destination / 'native-skia/ATTRIBUTIONS.md').read_text())
            self.assertFalse(record['final_native_distribution_audit_complete'])

    def test_missing_changed_or_aliased_payload_refuses_before_copying_any_supplement(self):
        for change in ('missing', 'changed', 'alias'):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary)
                source, _, bindings, native, destination = self.fixture(root)
                path = source / 'VMA-LICENSE'
                path.unlink()
                if change == 'changed':
                    path.write_text('replacement terms')
                elif change == 'alias':
                    outside = root / 'outside'
                    shutil.copy2(HERE / 'notices/native-skia/VMA-LICENSE', outside)
                    path.symlink_to(outside)
                with self.assertRaises(ValueError):
                    notices.include(source, destination, bindings, [native])
                self.assertEqual(list(destination.iterdir()), [])

    def test_a_new_source_graph_or_conflicting_build_notice_cannot_reuse_old_approval(self):
        for change in ('version', 'revision', 'native', 'missing-native', 'duplicate-bindings'):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                source, _, bindings, native, destination = self.fixture(pathlib.Path(temporary))
                if change == 'version':
                    bindings[0]['version'] = '0.0.0'
                elif change == 'revision':
                    bindings[0]['metadata']['skia'] = 'different-source'
                elif change == 'native':
                    native.write_text('different native terms')
                elif change == 'duplicate-bindings':
                    bindings *= 2
                with self.assertRaises(ValueError):
                    notices.include(source, destination, bindings, [] if change == 'missing-native' else [native])
                self.assertEqual(list(destination.iterdir()), [])

    def test_rehashing_a_removed_binary_acknowledgment_or_component_still_refuses(self):
        for change in ('acknowledgment', 'component', 'outside-path'):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                source, inventory, bindings, native, destination = self.fixture(pathlib.Path(temporary))
                if change == 'acknowledgment':
                    path = source / 'ATTRIBUTIONS.md'
                    path.write_text(path.read_text().replace(notices.IJG_ACKNOWLEDGMENT, ''))
                    inventory['files'][path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
                elif change == 'component':
                    inventory['components'].pop()
                else:
                    inventory['files']['../outside'] = '0' * 64
                (source / 'inventory.json').write_text(json.dumps(inventory))
                with self.assertRaises(ValueError):
                    notices.include(source, destination, bindings, [native])
                self.assertEqual(list(destination.iterdir()), [])
