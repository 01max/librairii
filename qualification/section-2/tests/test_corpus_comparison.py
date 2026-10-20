"""Protect semantic mismatch reporting; these tests do not assert product parity."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "corpus_probe", Path(__file__).resolve().parents[1] / "probe-corpus.py"
)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class SemanticDifferences(unittest.TestCase):
    def test_array_order_and_identity_changes_are_reported(self):
        differences = probe.differences(
            [{"id": 7, "position": 0}, {"id": 8, "position": 1}],
            [{"id": 8, "position": 0}, {"id": 7, "position": 1}],
        )
        self.assertEqual([d["path"] for d in differences], ["/0/id", "/1/id"])
        self.assertEqual(differences[0]["expected"], 7)
        self.assertEqual(differences[0]["actual"], 8)

    def test_missing_field_is_distinct_from_null_and_boolean_from_integer(self):
        differences = probe.differences({"nullable": None, "flag": True}, {"flag": 1})
        self.assertEqual(len(differences), 2)
        self.assertEqual(differences[1]["path"], "/nullable")
        self.assertTrue(differences[1]["expectedPresent"])
        self.assertFalse(differences[1]["actualPresent"])

    def test_unicode_filename_forms_are_not_normalized_away(self):
        differences = probe.differences({"name": "\u00c9t\u00e9"}, {"name": "E\u0301te\u0301"})
        self.assertEqual(len(differences), 1)
        self.assertEqual(differences[0]["expected"].encode().hex(), "c38974c3a9")
        self.assertEqual(differences[0]["actual"].encode().hex(), "45cc817465cc81")

    def test_rejected_suffix_is_never_relabelled_acceptance(self):
        expected = [{"file": "archives/unsupported.txt", "error": "unsupported_format"}]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "inspection.json").write_text(json.dumps(expected))
            report = probe.compare(root, {"sqliteVersion": "test", "databases": [], "archives": [
                {"file": expected[0]["file"], "observation": {"container": "zip", "entries": []}}
            ]})
        self.assertEqual(report["archives"][0]["oracle"], expected[0])
        self.assertIn("not implemented", report["archives"][0]["classificationComparison"])
        self.assertIn("partial", report["status"])


if __name__ == "__main__":
    unittest.main()
