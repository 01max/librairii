"""Additional original bounded regressions for the reviewed decoder patch.

Never regenerates the retained base corpus. Compressors run only on known tiny
synthetic inputs. The Zstd counterexample is a raw-block frame with one explicitly
bounded 256MiB window, not an arbitrary declaration/mutation probe.
"""
from pathlib import Path
import hashlib, json, os, struct, subprocess, tempfile, zipfile
from synthetic_7z import archive, signature, uint64, crc32
root=Path(__file__).resolve().parent;repo=root.parents[3]
manifest=json.loads((root/'manifest.json').read_text())
def add(name,data,category):
 (root/name).write_bytes(data)
 record={'file':name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'category':category}
 manifest['fixtures']=[r for r in manifest['fixtures'] if r['file']!=name]+[record]
with zipfile.ZipFile(repo/'preservation/corpus/archives/clockwork-forest-studio.zip') as z:png=z.read('thumbnail.png')
entries=[('story.json',b'{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[{"audio":"assets/track"}]}'),('thumbnail.png',png),('assets/track',bytes(range(256))*8)]
payload=b''.join(p for _,p in entries);wire=b'\x04\xf7\x11\x02';large=b'\x11\x1e'+bytes(14)
add('large-window-brotli.7z',archive(entries,external=(large,wire)),'rejected codec')
pack=[('00112233445546778899aabbccddeeff/'+n,b'x') for n in ['ni','nm','li','ri','si','rf/a','sf/a']]
base=archive(pack);offset,size=struct.unpack_from('<QQ',base,12);decoded=base[32+offset:]
with tempfile.TemporaryDirectory(prefix='librairii-brotli-regressions-') as tmp:
 tmp=Path(tmp)
 def compress(data):
  (tmp/'payload').write_bytes(data)
  subprocess.run([os.environ.get('TASK5_GO','go'),'run',str(repo/'target/task5-codec-producer/main.go'),str(tmp/'payload'),str(tmp)],cwd=repo,check=True,stdout=subprocess.DEVNULL)
  return (tmp/'brotli.packed').read_bytes()
 packed=compress(payload)
 frame=lambda data:struct.pack('<IIIHH',0x184d2a50,8,len(data),0x5242,1)+data
 add('brotli-envelope.7z',archive(entries,external=(frame(packed),wire)),'codec')
 # The frozen Go wrapper strips one envelope and decodes one Brotli stream.
 first=compress(entries[0][1]);second=compress(b''.join(p for _,p in entries[1:]))
 add('brotli-concatenated.7z',archive(entries,external=(frame(first)+frame(second),wire)),'rejected codec')
 for name,packed in [('encoded-header-brotli',compress(decoded)),('large-window-encoded-brotli',large)]:
  pi=b'\x06'+uint64(offset)+b'\x01\x09'+uint64(len(packed))+b'\x0a\x01'+crc32(packed)+b'\x00'
  ui=b'\x07\x0b\x01\x00\x01\x04'+wire+b'\x0c'+uint64(len(decoded))+b'\x0a\x01'+crc32(decoded)+b'\x00'
  add(name+'.7z',signature(base[32:32+offset]+packed,b'\x17'+pi+ui+b'\x00'),'codec' if name.startswith('encoded-header-') else 'rejected codec')
# No decoder is needed to produce this valid Zstd raw-block frame.
assert 256<=len(payload)<65536
zstd=b'\x28\xb5\x2f\xfd\x40\x90'+struct.pack('<H',len(payload)-256)+((len(payload)<<3)|1).to_bytes(3,'little')+payload
add('zstd-window-256m.7z',archive(entries,external=(zstd,b'\x04\xf7\x11\x01')),'hardening')
# 3000 valid BCJ2 blocks have 12000 packed streams but only 3000 files/coders.
items=pack+[(f'opaque/e{i}',b'x') for i in range(3000-len(pack))]
with tempfile.TemporaryDirectory(prefix='librairii-many-pack-streams-') as tmp:
 tmp=Path(tmp)
 for name,data in items:
  path=tmp/name;path.parent.mkdir(exist_ok=True,parents=True);path.write_bytes(data)
 out=tmp/'many.7z'
 subprocess.run([os.environ['TASK5_7ZZ'],'a','-t7z','-mhc=off','-mtm=off','-mqs=off','-ms=off','-m0=BCJ2',str(out),'00112233445546778899aabbccddeeff','opaque'],cwd=tmp,check=True,stdout=subprocess.DEVNULL)
 data=out.read_bytes();o,z=struct.unpack_from('<QQ',data,12);header=data[32+o:]
 assert header[:4]==b'\x01\x04\x06\x00'
 method=header.index(b'\x01\x14\x03\x03\x01\x1b')+2
 # Frozen Go readCoder uses Read rather than ReadFull. Align 4-byte method
 # IDs so this valid large header does not trip its unrelated bufio short-read.
 padding=[0,128,16384,2097152][(-method)%4]
 header=header[:3]+uint64(padding)+header[4:]
 add('pack-streams-12000.7z',signature(bytes(padding)+data[32:32+o],header),'hardening')
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
