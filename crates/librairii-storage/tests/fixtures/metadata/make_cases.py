"""Deterministic supplied-byte cases; never contacts Lunii or rewrites Go output."""
import json,copy
from pathlib import Path
base=json.loads(Path('crates/librairii-app/tests/fixtures/lunii-catalog.json').read_text())
cases=[]
def add(name,fn=lambda x:None,locale='en-GB',raw=None,hardening=None):
 v=copy.deepcopy(base);fn(v);payload=raw if raw is not None else json.dumps(v,ensure_ascii=False,separators=(',',':')).encode();case=dict(name=name,locale=locale,payload=payload.hex());
 if hardening:case['hardening']=hardening
 cases.append(case)
def row(v):return v['response']['fixture-pack-001']
def local(v):return row(v)['localized_infos']['en_GB']
add('baseline')
for locale in ['fr-FR','en_US','iw_IL','und','not a locale','EN-gb','en-x-private']:
 add('requested-'+locale,locale=locale)
for key,val in [('uuid','123E4567-E89B-42D3-A456-426614174000'),('uuid','00000000-0000-0000-0000-000000000000'),('uuid','bad'),('authors',[' One ',None,' Two ']),('authors',{'z':{'name':'Last'},'a':{'name':' First '}}),('authors',{'x':{'NAME':'Named'}}),('authors',123),('publisher',None),('publisher',{'x':{'name':'Publisher'}}),('updated_at',None),('updated_at','2026-07-20T14:00:00.123456789123+02:00'),('updated_at','2026-07-20T1:00:00,5Z'),('updated_at','2026-07-20T12:00:00+24:60'),('updated_at','2026-07-20T12:00:60Z'),('updated_at','0000-01-01T00:00:00+01:00')]:
 add(f'record-{key}-{len(cases)}',lambda v,k=key,x=val:row(v).__setitem__(k,x))
for key,val in [('title','  ÉTÉ  '),('title',None),('description',None),('title','x'*1025),('image',None),('image',{'IMAGE_URL':'/fixture/cover.png'}),('minimum_age','  +3 '),('maximum_age',-1),('minimum_age',-1),('duration_seconds',1.0),('duration_seconds',True),('duration_seconds','9223372036854775807'),('duration_seconds','9223372036854775808')]:
 add(f'localized-{key}-{len(cases)}',lambda v,k=key,x=val:local(v).__setitem__(k,x))
for path in ['/','/fixture/a b.png','/fixture/été.png','/fixture/\u0085.png','/fixture/../escape','/fixture/%2e%2e/escape','//outside/cover.png','/fixture/cover.png?x','/fixture/'+('x'*4200)+'.png']:
 add('image-path-'+str(len(cases)),lambda v,x=path:local(v).__setitem__('image',{'image_url':x}))
def locales(v):
 r=row(v);r['locales_available']={'fr_FR':True,'en_US':True,'de_DE':False};r['localized_infos']={'fr_FR':{'title':'Français'},'en_US':{'title':'English'},'de_DE':False}
for locale in ['en-GB','fr-FR','it-IT']:add('fallback-'+locale,locales,locale=locale)
add('ambiguous-available',lambda v:row(v).__setitem__('locales_available',{'en_GB':True,'en-GB':True}))
add('availability-not-bool',lambda v:row(v).__setitem__('locales_available',{'en_GB':None}))
add('no-available',lambda v:row(v).__setitem__('locales_available',{'en_GB':False}))
add('duplicate-uuid',lambda v:v['response'].__setitem__('duplicate',copy.deepcopy(row(v))))
add('unavailable-duplicate',lambda v:(v['response'].__setitem__('duplicate',copy.deepcopy(row(v))),v['response']['duplicate'].__setitem__('locales_available',{'en_GB':False})))
add('record-case',lambda v:row(v).__setitem__('UUID',row(v).pop('uuid')))
add('envelope-case',lambda v:v.__setitem__('RESPONSE',v.pop('response')))
add('publisher-fallback',lambda v:(row(v).__setitem__('publisher',''),row(v).__setitem__('publishers',['Fallback'])))
add('unused-publisher-invalid',lambda v:row(v).__setitem__('publishers',123))
add('record-age-alias',lambda v:(local(v).pop('minimum_age'),row(v).__setitem__('min_age','6'),local(v).__setitem__('maximum_age',8)))
raw=json.dumps(base,separators=(',',':')).encode()
add('invalid-utf8-repair',raw=raw.replace(b'A. Example',b'A. \xff'),hardening='invalid UTF-8 rejected before normalization')
add('unpaired-surrogate-repair',raw=raw.replace(b'A. Example',b'A. \\ud800'),hardening='unpaired surrogate rejected before normalization')
add('deep-unknown-json',raw=raw[:-1]+b',"unknown":'+b'['*150+b'0'+b']'*150+b'}',hardening='JSON nesting ceiling retained for resource bounds')
add('duplicate-object-key',raw=raw.replace(b'"uuid":',b'"uuid":"123e4567-e89b-42d3-a456-426614174000","uuid":'),hardening='duplicate JSON keys rejected instead of merged')
add('ambiguous-struct-case',raw=raw.replace(b'"uuid":',b'"UUID":"123e4567-e89b-42d3-a456-426614174000","uuid":'),hardening='ambiguous struct spellings rejected instead of last-wins')
add('unknown-extreme-number',raw=raw[:-1]+b',"unknown":1e10000}',hardening='unrepresentable JSON numbers rejected at the bounded tree edge')
Path('crates/librairii-storage/tests/fixtures/metadata/cases.json').write_text(json.dumps(cases,ensure_ascii=False,indent=2)+'\n')
