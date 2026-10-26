import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[2] / "scripts/frozen_go.py"
SPEC = importlib.util.spec_from_file_location("frozen_go", MODULE)
FROZEN_GO = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FROZEN_GO)


class FrozenGoTests(unittest.TestCase):
    def fixture(self, root):
        path = root / "internal/policy/policy.go"
        path.parent.mkdir(parents=True)
        path.write_bytes(b"package policy\n")
        manifest = {"files": [{"path": "internal/policy/policy.go",
                               "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}]}
        (root / "preservation").mkdir()
        (root / "preservation/oracle-source.json").write_text(json.dumps(manifest))
        return path

    def test_extra_checkout_sources_and_inherited_overlays_cannot_enter_oracle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            admitted = self.fixture(root)
            extra = admitted.with_name("extra.go")
            extra.write_text("package policy\nfunc init() { panic(\"injected\") }\n")
            with patch.dict(os.environ, {"GOFLAGS": "-overlay=/untrusted.json",
                                         "GOWORK": "/untrusted.work", "GOOS": "foreign"}):
                with FROZEN_GO.frozen_module(root, "test-") as (work, environment, _):
                    self.assertEqual((work / admitted.relative_to(root)).read_bytes(), admitted.read_bytes())
                    self.assertFalse((work / extra.relative_to(root)).exists())
                    self.assertEqual(environment["GOFLAGS"], "")
                    self.assertEqual(environment["GOWORK"], "off")
                    self.assertEqual(environment["GOENV"], "off")
                    self.assertNotIn("GOOS", environment)
                self.assertFalse(work.exists())
            self.assertTrue(extra.exists())

    def test_changed_manifest_member_refuses_without_compiling(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            admitted = self.fixture(root)
            admitted.write_bytes(b"package changed\n")
            with self.assertRaisesRegex(ValueError, "frozen source changed"):
                with FROZEN_GO.frozen_module(root, "test-"):
                    self.fail("changed input was admitted")
            self.assertEqual(list((root / "tmp").iterdir()), [])
