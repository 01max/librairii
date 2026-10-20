"""Safe qualification prerequisite/protocol tests; no oracle/toolchain execution."""
import importlib.util
import base64
import hashlib
import json
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SOURCE = Path(__file__).resolve().parents[1] / "probe.py"
SPEC = importlib.util.spec_from_file_location("unicode_qualification_probe", SOURCE)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class QualificationSafety(unittest.TestCase):
    def test_untrusted_or_nonregular_zip_cannot_create_owned_sources(self):
        for kind in ["wrong-checksum", "symlink", "unsafe-path"]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                cache = root / "mod/golang.org/x/text@v0.43.0"
                archive = root / "mod/cache/download/golang.org/x/text/@v/v0.43.0.zip"
                archive.parent.mkdir(parents=True)
                with zipfile.ZipFile(archive, "w") as z:
                    z.writestr("golang.org/x/text@v0.43.0/go.mod", b"synthetic bytes")
                    if kind == "symlink":
                        link = zipfile.ZipInfo("golang.org/x/text@v0.43.0/link.go")
                        link.create_system = 3
                        link.external_attr = (stat.S_IFLNK | 0o777) << 16
                        z.writestr(link, "outside")
                    elif kind == "unsafe-path":
                        z.writestr("golang.org/x/text@v0.43.0/../escape.go", "outside")
                target = root / "owned/golang.org/x/text@v0.43.0"
                expected = {"wrong-checksum": "checksum differs", "symlink": "nonregular",
                            "unsafe-path": "unsafe module ZIP path"}[kind]
                with self.assertRaisesRegex(RuntimeError, expected):
                    probe.materialize_text(cache, target)
                self.assertFalse(target.exists(), "unauthenticated bytes must not be materialized")

    def test_owned_module_excludes_unverified_extra_go_before_build(self):
        # All bytes are synthetic and owned. Exercise actual main orchestration
        # through the Go-build boundary; no Git/Go/Rust command executes.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            cache = root / "mod/golang.org/x/text@v0.43.0"
            archive = root / "mod/cache/download/golang.org/x/text/@v/v0.43.0.zip"
            archive.parent.mkdir(parents=True)
            members = {"go.mod": b"module golang.org/x/text\n",
                       "language/language.go": b"package language\n"}
            summary = hashlib.sha256()
            with zipfile.ZipFile(archive, "w") as z:
                for name, data in sorted(members.items()):
                    full = "golang.org/x/text@v0.43.0/" + name
                    z.writestr(full, data)
                    summary.update(f"{probe.digest(data)}  {full}\n".encode())
                    target = cache / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(data)
            checksum = "h1:" + base64.b64encode(summary.digest()).decode()
            extra = cache / "language/extra.go"
            extra.write_bytes(b"package language // unverified extra source\n")
            downloads = archive.parent
            for suffix, data in {"mod": members["go.mod"], "info": b'{"Version":"v0.43.0"}',
                                 "ziphash": checksum.encode()}.items():
                (downloads / ("v0.43.0." + suffix)).write_bytes(data)
            oracle = root / "oracle"
            oracle.mkdir()
            output = root / "report.json"
            build_boundaries = []

            def boundary(argv, cwd, env=None, timeout=120):
                args = list(map(str, argv))
                if args[:2] == ["git", "clone"]:
                    Path(args[-1]).mkdir()
                    return b""
                if args[0] == "synthetic-rustup":
                    if "rustc" in args:
                        return b"release: 1.99.1\nhost: aarch64-apple-darwin\n"
                    return b"synthetic tool version\n"
                if args[0] == str(root / "synthetic-go") and args[1] == "build":
                    owned = Path(env["GOMODCACHE"]) / "golang.org/x/text@v0.43.0"
                    build_boundaries.append(owned)
                    self.assertFalse((owned / "language/extra.go").exists(),
                                     "unverified extra.go reached the oracle Go-build boundary")
                    actual = {str(p.relative_to(owned)): p.read_bytes()
                              for p in owned.rglob("*") if p.is_file()}
                    self.assertEqual(actual, members)
                    raise RuntimeError("synthetic stop before actual Go build")
                self.fail("unexpected runtime boundary: " + repr(args))

            args = [str(SOURCE), "--oracle", str(oracle), "--text-cache", str(cache),
                    "--go", str(root / "synthetic-go"), "--output", str(output)]
            with patch.object(probe, "TEXT_SUM", checksum), \
                    patch.object(probe, "source_files", return_value=[]), \
                    patch.object(probe, "check_oracle", return_value=436), \
                    patch.object(probe, "command", side_effect=boundary), \
                    patch.object(probe.shutil, "which", return_value="synthetic-rustup"), \
                    patch.object(probe.os, "environ", {}), patch.object(sys, "argv", args):
                with self.assertRaisesRegex(RuntimeError, "stop before actual Go build"):
                    probe.main()
            self.assertEqual(len(build_boundaries), 1)
            self.assertTrue(extra.is_file(), "the synthetic source cache must not be modified")
            self.assertIn("synthetic stop", json.loads(output.read_bytes())["toolingError"])

    def test_changed_frozen_file_is_rejected_and_restored_bytes_verify(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rows = []
            for i in range(436):
                p = root / str(i)
                p.write_bytes(b"synthetic")
                rows.append({"path": str(i), "bytes": 9, "sha256": probe.digest(b"synthetic")})
            self.assertEqual(probe.identities(root, rows), 436)
            (root / "17").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "identity mismatch"):
                probe.identities(root, rows)
            (root / "17").write_bytes(b"synthetic")
            self.assertEqual(probe.identities(root, rows), 436)

    def test_duplicate_or_oversized_synthetic_cases_fail_closed(self):
        row = {"name": "same", "input": "é"}
        for rows in [[], [row, row], [{"name": "large", "input": "é" * 3000}]]:
            with self.assertRaises(RuntimeError):
                probe.bounded_cases(rows)
        self.assertEqual(probe.bounded_cases([row]), [row])

    def test_source_path_escape_is_refused_before_reading_outside_the_oracle(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "owned"
            root.mkdir()
            outside = Path(temporary) / "outside"
            outside.write_bytes(b"synthetic")
            row = {"path": "../outside", "bytes": 9, "sha256": probe.digest(b"synthetic")}
            with self.assertRaisesRegex(RuntimeError, "unsafe oracle source path"):
                probe.identities(root, [row] * 436)

    def test_tooling_failure_cannot_be_reported_as_a_compatibility_result(self):
        with patch.object(probe.subprocess, "run", return_value=subprocess.CompletedProcess(
                ["not-executed"], 9, stdout=b"PASS", stderr=b"synthetic tooling failure")):
            with self.assertRaisesRegex(RuntimeError, "not compatibility result"):
                probe.command(["not-executed"], Path("."))


if __name__ == "__main__":
    unittest.main()
