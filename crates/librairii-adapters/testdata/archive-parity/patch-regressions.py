from pathlib import Path
import sys,hashlib,json
root=Path(__file__).resolve().parent
sys.path.insert(0,str(root));from synthetic_7z import signature,uint64
cases={
'encoded-header-pack-mismatch.7z':b'\x14\x03\x03\x01\x1b\x04\x01',
}
# BCJ2: four unbound input streams, but PackInfo describes only one.
pack=b'\x06\x00\x01\x09\x01\x00'
block=b'\x01\x14\x03\x03\x01\x1b\x04\x01\x00\x01\x02\x03'
unpack=b'\x07\x0b\x01\x00'+block+b'\x0c\x01\x00'
(root/'encoded-header-pack-mismatch.7z').write_bytes(signature(b'\x00',b'\x17'+pack+unpack+b'\x00'))
# A valid two-coder chain declares two 160MiB Zstd memory models. The single
# packed byte is deliberately not decoded: finite aggregate admission must fail.
block=b'\x02'+b'\x04\x04\xf7\x11\x01'*2+b'\x01\x00'
unpack=b'\x07\x0b\x01\x00'+block+b'\x0c\x01\x01\x00'
(root/'encoded-header-memory-chain.7z').write_bytes(signature(b'\x00',b'\x17'+pack+unpack+b'\x00'))
