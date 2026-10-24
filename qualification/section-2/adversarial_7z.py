"""Bounded original fixtures. NEVER parse these outside verified containment.

The producer allocates at most 16MiB per file; declarations can be adversarial.
No upstream fixture, production library, or product-classification rule is used.
"""
import hashlib
import struct
import zlib

from synthetic_7z import archive, crc32, signature, uint64


def fixtures(root):
    result = []

    def add(name, data, mode="guarded", expected=None, note="", extra=()):
        assert len(data) <= 16 << 20, "trusted producer ceiling"
        file = name + ".7z"
        (root / file).write_bytes(data)
        result.append({"name": name, "file": file, "mode": mode, "extra": extra,
                       "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                       "expected": expected, "scope": note})

    def pad(data, size):
        assert len(data) <= size <= 16 << 20
        return data + bytes(size - len(data))

    def bad_start(offset, size):
        start = struct.pack("<QQI", offset, size, 0)
        return b"7z\xbc\xaf\x27\x1c\x00\x04" + crc32(start) + start

    add("header-size-u64max", bad_start(0, (1 << 64) - 1), expected="library_error")
    add("header-offset-u64max", bad_start((1 << 64) - 1, 0), expected="library_error")
    add("files-count-u64max", signature(b"", b"\x01\x05" + uint64((1 << 64) - 1) + b"\x00\x00"), expected="library_error")
    # Counts below the library's header-byte bound can still imply much larger
    # ArchiveEntry/Coder vectors. Pad only the header, never allocate that vector here.
    header = b"\x01\x05" + uint64(2_000_000) + b"\x00\x00"
    add("files-count-allocation", signature(b"", pad(header, 2 << 20)),
        expected="contained_process_refusal", note="before entry callback; no product refusal code")
    header = b"\x01\x04\x07\x0b\x01\x00" + uint64(2_000_000)
    add("coders-count-allocation", signature(b"", pad(header, 2 << 20)),
        expected="contained_process_refusal", note="before entry callback; no product refusal code")
    header = b"\x01\x04\x07\x0b\x01\x00" + uint64(8_000_000)
    add("coders-count-allocation-larger", signature(b"", pad(header, 8 << 20)),
        expected="contained_process_refusal", note="larger pre-callback reserve; independently contained")

    entries = [("first.bin", b"first solid payload\n" * 200),
               ("second.bin", b"second solid payload\x00\xff\n" * 200)]
    solid = archive(entries, "lzma2")
    add("solid-ordered", solid, expected="decoded", note="both members fully read in order with CRC")
    result[-1]["expectedDecoded"] = [{"name": n, "bytes": len(p), "crc32": zlib.crc32(p)} for n, p in entries]
    add("cancel-before-header", solid, mode="cancel-before", expected="qualification_cancelled")
    add("cancel-after-first-solid", solid, mode="raw-cancel", expected="qualification_cancelled",
        note="callback cancellation; not proof of interrupting header or a long Read")
    result[-1]["expectedDecoded"] = [{"name": entries[0][0], "bytes": len(entries[0][1]), "crc32": zlib.crc32(entries[0][1])}]
    corrupt = bytearray(solid)
    offset = struct.unpack_from("<Q", corrupt, 12)[0]
    corrupt[32 + offset // 2] ^= 1
    add("solid-corrupt-packed", corrupt, mode="raw", expected="library_or_probe_read_error")
    copy = archive(entries)
    add("copy-read-baseline", copy, expected="decoded")
    add("delivered-exact", copy, extra=(64 << 20, sum(len(p) for _, p in entries)), expected="decoded")
    add("delivered-one-short", copy, extra=(64 << 20, sum(len(p) for _, p in entries) - 1),
        expected="library_or_probe_read_error", note="one-byte detection sentinel; not exact hard output cap")
    add("header-source-ceiling", copy, extra=(31,), expected="library_error")

    for count in (10_000, 10_001):
        data = archive([(f"e{i}", b"x") for i in range(count)])
        add(f"entries-{count}", data, mode="metadata-only",
            expected="qualification_metadata_allowed_only" if count == 10_000 else "qualification_metadata_refused",
            note="metadata-only; not a complete 10000-entry decode")
    for expanded in (2 << 30, (2 << 30) + 1, (1 << 64) - 1):
        data = archive([("declared.bin", b"x")], declared_sizes=[expanded])
        add(f"expanded-{expanded}", pad(data, 11 << 20), mode="metadata-only",
            expected="qualification_metadata_allowed_only" if expanded == 2 << 30 else "qualification_metadata_refused",
            note="declared size only; trailing padding isolates aggregate ratio; no multi-GiB decode")
    for ratio in (200, 201):
        data = archive([("declared.bin", b"x")], declared_sizes=[1024 * ratio])
        add(f"ratio-{ratio}", pad(data, 1024), mode="metadata-only",
            expected="qualification_metadata_allowed_only" if ratio == 200 else "qualification_metadata_refused",
            note="aggregate declared bytes / whole container size, as frozen Go; no ratio waiver")
    high_ratio = archive([("expanded.bin", b"A" * (512 << 10))], "lzma2")
    add("actual-expansion-ratio-refused", high_ratio, expected="qualification_metadata_refused",
        note="real 512KiB repeated payload; frozen aggregate ratio prevents decode")
    add("actual-expansion-delivery-probe", high_ratio, mode="raw", extra=(64 << 20, 64 << 10),
        expected="library_or_probe_read_error",
        note="raw diagnostic bypasses policy inside containment only; stops at 64KiB plus one-byte detection sentinel")

    for method, property_value, dictionary_bytes in (
            ("lzma2", 22, 8 << 20), ("lzma2", 23, 12 << 20),
            ("lzma2", 40, (1 << 32) - 1), ("lzma1", 1 << 20, 1 << 20),
            ("lzma1", 1 << 31, 1 << 31)):
        data = archive([("dictionary.bin", b"synthetic dictionary probe\n" * 20)], method,
                       dictionary_property=property_value)
        add(f"{method}-dictionary-{dictionary_bytes}-guarded", data,
            expected="decoded" if dictionary_bytes <= 8 << 20 else "qualification_metadata_refused",
            note="8MiB guard is experimental, absent from frozen product contract")
        if dictionary_bytes >= 1 << 31:
            add(f"{method}-dictionary-{dictionary_bytes}-raw", data, mode="raw",
                expected="contained_process_refusal", note="unmodified decoder; kernel limits cover allocation")
            data = archive([("dictionary.bin", b"synthetic dictionary probe\n" * 20)], method,
                           dictionary_property=property_value, declared_sizes=[2 << 30])
            add(f"{method}-dictionary-{dictionary_bytes}-large-size-raw", data, mode="raw",
                expected="contained_process_refusal",
                note="large declared output forces dictionary allocation path; not a valid multi-GiB payload")
    return result
