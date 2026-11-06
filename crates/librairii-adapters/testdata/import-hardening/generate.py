"""Original deterministic image-bound counterexamples; no user/archive payload."""
from pathlib import Path
import binascii, hashlib, io, json, struct, zipfile, zlib
root=Path(__file__).resolve().parent

def chunk(kind,payload):
 return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',binascii.crc32(kind+payload)&0xffffffff)

def png(width,height):
 encoder=zlib.compressobj(9)
 data=bytearray()
 row=bytes(1+width*4)
 for _ in range(height):data.extend(encoder.compress(row))
 data.extend(encoder.flush())
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+chunk(b'IDAT',data)+chunk(b'IEND',b'')
records=[]
for case,width,height in [('dimension',8193,1),('pixels',8192,2049)]:
 image=png(width,height)
 out=io.BytesIO()
 story=b'{"uuid":"00112233-4455-4677-8899-aabbccddeeff","stageNodes":[{"audio":"assets/track"}]}'
 with zipfile.ZipFile(out,'w') as archive:
  for name,data in [('story.json',story),('assets/track',b'synthetic opaque audio'),('thumbnail.png',image)]:
   info=zipfile.ZipInfo(name,date_time=(2026,1,1,0,0,0))
   info.compress_type=zipfile.ZIP_STORED
   info.external_attr=(0o100644 << 16)
   archive.writestr(info,data)
 payload=out.getvalue();name=case+'.zip';(root/name).write_bytes(payload)
 records.append({'file':name,'bytes':len(payload),'sha256':hashlib.sha256(payload).hexdigest(),'width':width,'height':height,'go':'accepted','rust':'artwork_limit'})
(root/'images.json').write_text(json.dumps({'category':'bounded embedded PNG dimensions and pixels','scope':'both mandatory Studio and optional embedded PNG','limits':{'dimension':8192,'pixels':16777216,'decoder_allocation_bytes':134217728},'rationale':'Reject image headers exceeding the existing bounded thumbnail pipeline before raster allocation. Original compressed bytes remain immutable.','review':'pending Task 5.10 independent review','fixtures':records},indent=2)+'\n')
