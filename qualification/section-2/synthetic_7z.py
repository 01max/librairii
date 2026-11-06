"""Original qualification-only 7z writer: one block, no extraction or product policy.

Format reference: ip7z/7zip commit9128b80e3a471108678e2b3ec3c985861c8d7b0f,
DOC/7zFormat.txt SHA256c5aba210bcc35e0506fa06bed771cf1807cbd0098dad8916b3b65ae819f5b077.
Zstd method ID is the pinned sevenz-rust2 0.24.1 ID_ZSTD, 04f71101.
No upstream fixture bytes or writer implementation are copied.
"""
import hashlib
import lzma
from pathlib import Path
import struct
import sys
import zlib


def uint64(value):
    if not 0 <= value <= (1 << 64) - 1:
        raise ValueError("outside7z UINT64")
    for count in range(8):
        if value < 1 << (7 * (count + 1)):
            prefix = (0xFF << (8 - count)) & 0xFF
            return bytes([prefix | (value >> (8 * count))]) + (value & ((1 << (8 * count)) - 1)).to_bytes(count, "little")
    return b"\xff" + value.to_bytes(8, "little")


def crc32(data):
    return struct.pack("<I", zlib.crc32(data))


def signature(packed, header):
    start = struct.pack("<QQ", len(packed), len(header)) + crc32(header)
    return b"7z\xbc\xaf\x27\x1c\x00\x04" + crc32(start) + start + packed + header


def archive(entries, method="copy", dictionary_property=16, declared_sizes=None):
    """Known synthetic inputs only. Mutated metadata is for contained probes only."""
    payload = b"".join(data for _, data in entries)
    if len(payload) > 4 << 20:
        raise ValueError("fixture producer ceiling4MiB")
    sizes = declared_sizes if declared_sizes is not None else [len(data) for _, data in entries]
    if len(sizes) != len(entries) or not entries:
        raise ValueError("nonempty matching entry/size lists required")
    properties = b""
    if method == "copy":
        packed, method_id = payload, b"\x00"
    elif method == "zstd":
        from compression import zstd  # Optional CPython3.14+; independent of candidate decoder.
        packed, method_id = zstd.compress(payload, level=3), b"\x04\xf7\x11\x01"
    elif method == "lzma2":
        packed = lzma.compress(payload, format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2, "dict_size": 1 << 20}])
        method_id, properties = b"\x21", bytes([dictionary_property])
    elif method == "lzma1":
        packed = lzma.compress(payload, format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA1, "dict_size": 1 << 20}])
        method_id = b"\x03\x01\x01"
        properties = b"\x5d" + dictionary_property.to_bytes(4, "little")
    else:
        raise ValueError("unsupported synthetic method")
    pack_info = b"\x06" + uint64(0) + uint64(1) + b"\x09" + uint64(len(packed)) + b"\x0a\x01" + crc32(packed) + b"\x00"
    coder = bytes([len(method_id) | (0x20 if properties else 0)]) + method_id
    if properties:
        coder += uint64(len(properties)) + properties
    unpack_info = b"\x07\x0b" + uint64(1) + b"\x00" + uint64(1) + coder
    unpack_info += b"\x0c" + uint64(sum(sizes)) + b"\x0a\x01" + crc32(payload) + b"\x00"
    substreams = b"\x08\x0d" + uint64(len(entries))
    if len(entries) > 1:
        substreams += b"\x09" + b"".join(uint64(n) for n in sizes[:-1])
        substreams += b"\x0a\x01" + b"".join(crc32(data) for _, data in entries)
    substreams += b"\x00"
    names = b"\x00" + b"".join((name + "\x00").encode("utf-16le") for name, _ in entries)
    files = b"\x05" + uint64(len(entries)) + b"\x11" + uint64(len(names)) + names + b"\x00"
    return signature(packed, b"\x01\x04" + pack_info + unpack_info + substreams + b"\x00" + files + b"\x00")


def zstd_fixture(root):
    from compression import zstd
    entries = [("preceding.bin", b"synthetic independent Zstd preceding\n" * 1000),
               ("Été 日本語.bin", b"synthetic independent Zstd target\x00\xff\n" * 40)]
    file = "independent-codecs/zstd-independent.7z"
    data = archive(entries, method="zstd")
    path = root / file
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return {"file": file, "methodRequested": "zstd-independent", "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "producer": {"python": sys.version, "libzstd": zstd.zstd_version,
                         "compressionLevel": 3, "writer": "synthetic_7z.py", "writerSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                         "formatSource": "https://github.com/ip7z/7zip/blob/9128b80e3a471108678e2b3ec3c985861c8d7b0f/DOC/7zFormat.txt",
                         "compressionSource": "https://github.com/python/cpython/tree/v3.14.7/Lib/compression/zstd",
                         "licenseSources": ["https://github.com/python/cpython/blob/v3.14.7/LICENSE",
                                            "https://github.com/facebook/zstd/blob/v1.5.7/LICENSE",
                                            "https://github.com/facebook/zstd/blob/v1.5.7/COPYING"],
                         "toolTerms": "CPython PSF-2.0 with historical notices; libzstd1.5.7 BSD or GPLv2 alternatives; no upstream fixture copied; no product license selected"},
            "expectedEntries": [{"name": name, "size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()} for name, payload in entries]}
