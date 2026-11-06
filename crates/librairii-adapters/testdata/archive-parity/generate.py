"""Original bounded fixture producer. 7zz is a test producer, never a runtime dependency."""
import hashlib, io, json, os, struct, subprocess, sys, tempfile, zipfile, zlib
from pathlib import Path
from synthetic_7z import archive, signature, uint64, crc32
root=Path(__file__).resolve().parent
repo=root.parents[3]
source=repo/'preservation/corpus/archives/clockwork-forest-studio.zip'
entries=[(n,zipfile.ZipFile(source).read(n)) for n in ['story.json','thumbnail.png','assets/scene.png','assets/intro.mp3'] if n in zipfile.ZipFile(source).namelist()]
# Keep an original small Studio document and referenced opaque asset.
entries=[('story.json',b'{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[{"audio":"assets/track"}]}'),('thumbnail.png',zipfile.ZipFile(source).read('thumbnail.png')),('assets/track',bytes(range(256))*8)]
records=[]
# Compressor output is produced independently of the Rust decoder.
with tempfile.TemporaryDirectory(prefix='librairii-task5-frames-') as frame_tmp:
 frame_tmp=Path(frame_tmp);(frame_tmp/'payload').write_bytes(b''.join(p for _,p in entries))
 subprocess.run([os.environ.get('TASK5_GO','go'),'run',str(repo/'target/task5-codec-producer/main.go'),str(frame_tmp/'payload'),str(frame_tmp)],cwd=repo,check=True)
 external={m:(frame_tmp/(m+'.packed')).read_bytes() for m in ['brotli','lz4','zstd']}
def add(name,data,category='parity'):
 (root/name).write_bytes(data)
 records.append({'file':name,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),'category':category})
for method,wire in [('brotli',b'\x04\xf7\x11\x02'),('lz4',b'\x04\xf7\x11\x04'),('zstd',b'\x04\xf7\x11\x01')]:
 add(method+'.7z',archive(entries,external=(external[method],wire)),'codec')
with tempfile.TemporaryDirectory(prefix='librairii-task5-codecs-') as tmp:
 tmp=Path(tmp)
 for name,data in entries:
  p=tmp/name;p.parent.mkdir(exist_ok=True);p.write_bytes(data)
 seven=os.environ['TASK5_7ZZ']
 for name,methods in [('copy',['Copy']),('lzma',['LZMA:d=1m']),('lzma2',['LZMA2:d=1m']),('ppmd',['PPMd:mem=4m']),('bzip2',['BZip2']),('deflate',['Deflate']),('bcj',['BCJ','LZMA2:d=1m']),('ppc',['PPC','LZMA2:d=1m']),('arm',['ARM','LZMA2:d=1m']),('arm64',['ARM64','LZMA2:d=1m']),('sparc',['SPARC','LZMA2:d=1m']),('delta',['Delta','LZMA2:d=1m']),('bcj2',['BCJ2','LZMA2:d=1m']),('unsupported-ia64',['IA64','LZMA2:d=1m']),('unsupported-thumb',['ARMT','LZMA2:d=1m']),('unsupported-riscv',['RISCV','LZMA2:d=1m'])]:
  out=tmp/(name+'.7z')
  args=[seven,'a','-t7z','-mhc=off','-mtc=off','-mta=off','-mtm=off','-mqs=off','-ms=on',*[f'-m{i}={v}' for i,v in enumerate(methods)],str(out),'story.json','thumbnail.png','assets/track']
  subprocess.run(args,cwd=tmp,check=True,stdout=subprocess.DEVNULL)
  add(name+'.7z',out.read_bytes(),'rejected codec' if name.startswith('unsupported') else 'codec')
 # Non-solid distinct blocks: corrupt unused audio while selected metadata stays valid.
 out=tmp/'non-solid.7z'
 subprocess.run([seven,'a','-t7z','-mhc=off','-mtm=off','-mqs=off','-ms=off','-m0=Copy',str(out),'story.json','thumbnail.png','assets/track'],cwd=tmp,check=True,stdout=subprocess.DEVNULL)
 data=bytearray(out.read_bytes());marker=bytes(range(256))*8;pos=data.index(marker);data[pos]^=1
 add('non-solid-unused-corrupt.7z',data)
# Independent solid Copy block, exercising earlier work and per-file CRCs.
solid=archive([('before.bin',bytes(range(256))*32),*entries,('after.bin',b'later unused member')])
add('solid-copy.7z',solid)
for name,entry in [('selected-crc','story.json'),('preceding-crc','before.bin')]:
 data=bytearray(solid);payload=dict([('before.bin',bytes(range(256))*32),*entries])[entry]
 # Change the stored per-file CRC only and recompute next/start header CRCs.
 offset,size=struct.unpack_from('<QQ',data,12);start=32+offset;header=bytearray(data[start:start+size]);needle=crc32(payload);pos=header.index(needle);header[pos]^=1
 add(name+'.7z',signature(bytes(data[32:start]),bytes(header)),'hardening')
# Native path boundary includes UTF16 terminators in the total names property.
add('path-512.7z',archive([('a'*512,b'x')]))
# Safe tiny valid UUID-root pack: payloads are never requested by the Go inspector.
pack=[('00112233445546778899aabbccddeeff/'+n,b'x') for n in ['ni','nm','li','ri','si','rf/a','sf/a']]
add('dictionary-512m.7z',archive(pack,'lzma1',dictionary_property=512<<20),'hardening')
# Filtered encoded-header parity: only the selected header decoder differs.
base=archive(pack);off,sz=struct.unpack_from('<QQ',base,12);decoded=base[32+off:]
with tempfile.TemporaryDirectory(prefix='librairii-task5-header-') as temp:
 temp=Path(temp);(temp/'header').write_bytes(decoded)
 for name,methods in [('encoded-header-lzma2',['LZMA2:d=1m']),('encoded-header-bcj2',['BCJ2','LZMA2:d=1m']),('encoded-header-ia64',['IA64','LZMA2:d=1m'])]:
  out=temp/(name+'.7z')
  subprocess.run([seven,'a','-t7z','-mhc=off','-mtm=off',*[f'-m{i}={v}' for i,v in enumerate(methods)],str(out),'header'],cwd=temp,check=True,stdout=subprocess.DEVNULL)
  q=out.read_bytes();qo,qs=struct.unpack_from('<QQ',q,12);qh=q[32+qo:];fi=qh.rfind(b'\x05\x01\x19');assert fi>2,qh.hex()
  streams=qh[2:fi];assert streams[:2]==b'\x06\x00'
  # Original pack bytes precede the encoded header stream.
  streams=streams[:1]+uint64(off)+streams[2:]
  add(name+'.7z',signature(base[32:32+off]+q[32:32+qo],b'\x17'+streams),'rejected codec' if name.endswith('ia64') else 'codec')

plain=archive(pack);offset,size=struct.unpack_from('<QQ',plain,12);payload=plain[32:32+offset];header=plain[32+offset:]
# A valid ignored archive property; its bytes exceed the proposed header allocation ceiling.
large=header[:-2]+b'\x19'+uint64((16<<20)+1)+bytes((16<<20)+1)+b'\x00\x00'
add('header-16m.7z',signature(payload,large),'hardening')
import lzma
compressed=lzma.compress(large,format=lzma.FORMAT_RAW,filters=[{'id':lzma.FILTER_LZMA2,'dict_size':1<<20}])
pi=b'\x06'+uint64(len(payload))+b'\x01\x09'+uint64(len(compressed))+b'\x0a\x01'+crc32(compressed)+b'\x00'
ui=b'\x07\x0b\x01\x00\x01\x21\x21\x01\x10\x0c'+uint64(len(large))+b'\x0a\x01'+crc32(large)+b'\x00'
add('decoded-header-16m.7z',signature(payload+compressed,b'\x17'+pi+ui+b'\x00'),'hardening')
# Nine identity coders are a well-formed chain; Go index-only inspection accepts it.
def coder_chain(count):
 sizes=[len(p) for _,p in pack];data=b''.join(p for _,p in pack)
 pi=b'\x06\x00\x01\x09'+uint64(len(data))+b'\x0a\x01'+crc32(data)+b'\x00'
 folder=uint64(count)+(b'\x01\x00'*count)+b''.join(uint64(i+1)+uint64(i) for i in range(count-1))
 ui=b'\x07\x0b\x01\x00'+folder+b'\x0c'+b''.join(uint64(len(data)) for _ in range(count))+b'\x0a\x01'+crc32(data)+b'\x00'
 sub=b'\x08\x0d'+uint64(len(pack))+b'\x09'+b''.join(uint64(n) for n in sizes[:-1])+b'\x0a\x01'+b''.join(crc32(p) for _,p in pack)+b'\x00'
 names=b'\x00'+b''.join((n+'\x00').encode('utf-16le') for n,_ in pack)
 files=b'\x05'+uint64(len(pack))+b'\x11'+uint64(len(names))+names+b'\x00'
 return signature(data,b'\x01\x04'+pi+ui+sub+b'\x00'+files+b'\x00')
add('coders-nine.7z',coder_chain(9),'hardening')
# Index-only graph with nine packed input streams; no codec is instantiated by Go.
sizes=[len(p) for _,p in pack];data=b''.join(p for _,p in pack)
pi=b'\x06\x00\x09\x09'+uint64(len(data))+b'\x00'*8+b'\x00'
folder=b'\x01\x11\x00\x09\x01'+bytes(range(9))
ui=b'\x07\x0b\x01\x00'+folder+b'\x0c'+uint64(len(data))+b'\x00'
sub=b'\x08\x0d'+uint64(len(pack))+b'\x09'+b''.join(uint64(n) for n in sizes[:-1])+b'\x00'
names=b'\x00'+b''.join((n+'\x00').encode('utf-16le') for n,_ in pack)
files=b'\x05'+uint64(len(pack))+b'\x11'+uint64(len(names))+names+b'\x00'
add('streams-nine.7z',signature(data,b'\x01\x04'+pi+ui+sub+b'\x00'+files+b'\x00'),'hardening')
# 5001 independent two-Copy blocks exceed the 10000-coder aggregate ceiling.
items=pack+[(f'opaque/e{i}',b'x') for i in range(5001-len(pack))];data=b''.join(p for _,p in items)
pi=b'\x06\x00'+uint64(len(items))+b'\x09'+b''.join(uint64(len(p)) for _,p in items)+b'\x00'
ui=b'\x07\x0b'+uint64(len(items))+b'\x00'+b'\x02\x01\x00\x01\x00\x01\x00'*len(items)+b'\x0c'+b''.join(uint64(len(p))*2 for _,p in items)+b'\x00'
names=b'\x00'+b''.join((n+'\x00').encode('utf-16le') for n,_ in items)
files=b'\x05'+uint64(len(items))+b'\x11'+uint64(len(names))+names+b'\x00'
add('total-coders-10002.7z',signature(data,b'\x01\x04'+pi+ui+b'\x00'+files+b'\x00'),'hardening')
# Unsupported unused coder model: structurally valid index-only payload.
unknown=bytearray(plain);off,sz=struct.unpack_from('<QQ',unknown,12);h=bytearray(unknown[32+off:]);idx=h.index(b'\x07\x0b\x01\x00\x01\x01\x00')+6;h[idx]=0x7f
add('unknown-unused-coder.7z',signature(unknown[32:32+off],h),'hardening')
# Identical unused corrupt payload distinction for ZIP: full CRC must fail.
out=io.BytesIO()
with zipfile.ZipFile(out,'w') as z:
 for n,p in entries:z.writestr(n,p)
bad=bytearray(out.getvalue());pos=bad.index(bytes(range(256))*8);bad[pos]^=1
add('zip-unused-corrupt.zip',bad)


# Original malformed constructor declarations; never submit these to the unbounded Go parser.
for name,h in [('files-u64max',b'\x01\x05'+uint64((1<<64)-1)+b'\x00\x00'),('coders-u64max',b'\x01\x04\x07\x0b\x01\x00'+uint64((1<<64)-1))]:add(name+'.7z',signature(b'',h),'malformed')
# DecodeConfig accepted RGBA header-only image; Rust requires additional framing.
png=dict(entries)['thumbnail.png'][:33]
out=io.BytesIO()
with zipfile.ZipFile(out,'w') as z:
 for n,p in entries:z.writestr(n,png if n=='thumbnail.png' else p)
add('png-header-only.zip',out.getvalue(),'hardening')
original=dict(entries)['thumbnail.png']
ancillary=original[:33]+struct.pack('>I',1)+b'sRGB'+b'\x00'+bytes(4)+original[33:]
out=io.BytesIO()
with zipfile.ZipFile(out,'w') as z:
 for n,p in entries:z.writestr(n,ancillary if n=='thumbnail.png' else p)
add('png-ancillary-crc.zip',out.getvalue(),'parity')

(root/'manifest.json').write_text(json.dumps({'producer':'generate.py; original synthetic_7z.py from qualification branch41e8854; independent official7-Zip26.05 for codec streams','tool_url':'https://github.com/ip7z/7zip/releases/download/26.05/7z2605-mac.tar.xz','tool_archive_sha256':hashlib.sha256(Path(seven).parent.joinpath('archive.tar.xz').read_bytes()).hexdigest(),'fixtures':records},indent=2)+'\n')
