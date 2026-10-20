#!/usr/bin/env python3
"""Verify corpus provenance, hashes and independent semantic witnesses."""
import argparse
import hashlib
import json
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('corpus',type=Path)
parser.add_argument('--compare',type=Path)
args=parser.parse_args()
root=args.corpus
read=lambda name:json.loads((root/name).read_text())
manifest=read('manifest.json')
assert manifest['oracle_commit']=='73a93303a9a44cf0a92eaf525331561ed8c04460'
expected={e['path'] for e in manifest['files']}|{'manifest.json'}
actual={p.relative_to(root).as_posix()for p in root.rglob('*')if p.is_file()}
assert actual==expected,('file inventory mismatch',actual^expected)
for entry in manifest['files']:
    name=entry['path']
    assert not Path(name).is_absolute() and '..' not in Path(name).parts
    p=root/name
    assert not p.is_symlink() and len(p.read_bytes())==entry['bytes']
    assert hashlib.sha256(p.read_bytes()).hexdigest()==entry['sha256'],name
if args.compare:
    other={p.relative_to(args.compare).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()for p in args.compare.rglob('*')if p.is_file()}
    ours={p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()for p in root.rglob('*')if p.is_file()}
    assert ours==other,('nondeterminism',{p for p in ours.keys()|other.keys()if ours.get(p)!=other.get(p)})
assert dict((r['input'],r['normalized'])for r in read('unicode.json'))['Straße']=='strasse'
saved={r['input']['id']:r for r in read('saved-queries.json')}
assert json.loads(saved['v5-reused-user-tag']['canonicalPayload'])['booleanFilters']==[{'definitionId':7,'state':'true'}]
assert saved['v5-retired-filter-rejected']['error']=='invalid_saved_query'
assert saved['unknown-version']['error']=='unsupported_saved_query_version'
assert 'brokenState' not in saved['v4-retired-visibility']['canonicalPayload']
inspected=read('inspection.json')
assert {r['format']for r in inspected if not r['error']}=={'plain_pk','v1_pk','v2_pk','generic_pk','zip','seven_zip','studio_zip'}
assert next(r for r in inspected if r['file'].endswith('studio-disguised.pk'))['error']=='unsupported_format'
imports=read('imports.json')
assert [r['result']['Code']for r in imports[:3]]==['imported','duplicate_checksum','uuid_conflict']
assert imports[0]['result']['Checksum']==imports[1]['result']['Checksum']
review=read('review-complete.json')
assert len(review['additions'])==len(review['removals'])==1
assert review['additions'][0]['story']['broken'] is True and len(review['additions'][0]['sources'])==2
assert review['warnings'][0]['code']=='story_unavailable'
assert read('review-replay.json')['error']=='stale_review'
incomplete=read('review-incomplete.json')
assert incomplete['incomplete'] is True and incomplete['upToDate'] is False and not incomplete['removals']
removed=read('ordinary-removal.json');excluded=read('explicit-exclusion.json');allowed=read('unexclude.json')
assert len(removed['review']['additions'])==1
assert len(excluded['exclusions'])==len(removed['exclusions'])+1 and not excluded['review']['additions']
assert len(allowed['review']['additions'])==1 and len(allowed['members'])==len(removed['members'])
plans=read('export-plans.json')
assert plans[-1]['report']['readyCount']==0 and not plans[-1]['report']['canExport']
assert plans[-2]['report']['items'][0]['issue']['code']=='story_unavailable'
skips=[i for p in plans for i in p['operationItems'] if i['PlannedStatus']=='skipped']
assert skips and all(i['Item']['TotalBytes']==0 for i in skips)
assert read('review-stale.json')['error']=='stale_review'
assert read('review-invalid-decision.json')['error']=='invalid_shelf_input'
assert read('review-deferred.json')['unchanged'] is True
print(f'verified {len(manifest["files"])} hashed corpus files and semantic witnesses'+(' with identical second export'if args.compare else ''))
