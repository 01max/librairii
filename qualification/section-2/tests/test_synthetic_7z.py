"""Independent producer checks, separate from Rust decoder/acceptance evidence."""
import hashlib
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zlib

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from synthetic_7z import zstd_fixture

try:
    from compression import zstd
except ImportError:
    zstd = None


@unittest.skipIf(zstd is None, "optional CPython compression.zstd producer unavailable")
class IndependentZstd(unittest.TestCase):
    def test_repeated_production_has_valid_headers_and_ordered_exact_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = zstd_fixture(root)
            data = (root / first["file"]).read_bytes()
            second = zstd_fixture(root)
            self.assertEqual(first, second)
            self.assertEqual(data, (root / first["file"]).read_bytes())
        self.assertEqual(hashlib.sha256(data).hexdigest(), first["sha256"])
        self.assertEqual(zlib.crc32(data[12:32]), struct.unpack_from("<I", data, 8)[0])
        offset, size, crc = struct.unpack_from("<QQI", data, 12)
        self.assertEqual(zlib.crc32(data[32 + offset:]), crc)
        self.assertEqual(len(data), 32 + offset + size)
        payload = zstd.decompress(data[32:32 + offset])
        position = 0
        for entry in first["expectedEntries"]:
            end = position + entry["size"]
            self.assertEqual(hashlib.sha256(payload[position:end]).hexdigest(), entry["sha256"])
            position = end
        self.assertEqual(position, len(payload))
        self.assertEqual([e["name"] for e in first["expectedEntries"]],
                         ["preceding.bin", "Été 日本語.bin"])
        if sys.version_info[:3] == (3, 14, 7) and zstd.zstd_version == "1.5.7":
            self.assertEqual(first["sha256"], "54de17c1dcaef5862816409eda41ff4cc6e57a303ef98eb182b2aa2a42013d39")
            self.assertEqual(data, (Path(__file__).resolve().parents[1] / "fixtures/zstd-independent.7z").read_bytes())


if __name__ == "__main__":
    unittest.main()
