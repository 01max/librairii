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
if (root/'databases/provenance.json').exists():
    import sqlite3
    prefixes=read('databases/provenance.json')['prefixes']
    assert [r['prefix'] for r in prefixes]==list(range(1,19))
    for r in prefixes:
        db=sqlite3.connect(f"file:{root/r['file']}?mode=ro",uri=True)
        assert [v[0] for v in db.execute('select version from schema_migrations order by version')]==list(range(1,r['prefix']+1))
        assert db.execute('pragma integrity_check').fetchone()[0]=='ok'
        assert not db.execute('pragma foreign_key_check').fetchall()
        db.close()
        expected=read(r['expected'])
        assert len(expected['afterGoOpen']['tables']['schema_migrations'])==18
    representative=read('libraries/schema018/state.json')['tables']
    assert len(representative['shelf_exclusions'])==1 and len(representative['shelf_stories'])==3
    assert len(representative['removal_intents'])==1
    assert {r['status'] for r in representative['file_operations']}=={'queued','running','succeeded'}
    db=sqlite3.connect(f"file:{root/'libraries/schema018/db/librairii.sqlite3'}?mode=ro",uri=True)
    for rel,checksum in db.execute('select managed_path,sha256 from story_archives order by id'):
        assert hashlib.sha256((root/'libraries/schema018'/rel).read_bytes()).hexdigest()==checksum
    assert not db.execute('pragma foreign_key_check').fetchall()
    db.close()
if (root/'limits/archive-headers.json').exists():
    boundaries={r['id']:r for r in read('limits/archive-headers.json')}
    assert boundaries['path-bytes-512']['error']=='' and boundaries['path-bytes-513']['error']=='unsafe_path'
    assert boundaries['entry-count-10000']['error']=='' and boundaries['entry-count-10001']['error']=='entry_limit'
    assert boundaries['aggregate-ratio-200']['error']=='' and boundaries['aggregate-ratio-201']['error']=='compression_ratio_limit'
    queries={r['id']:r for r in read('limits/queries.json')}
    assert queries['search-runes-200']['error']=='' and queries['search-runes-201']['error']=='invalid_story_query'
    assert queries['page-size-100']['error']=='' and queries['page-size-101']['error']=='invalid_story_query'
    lines=(root/'performance/stories.jsonl').read_text().splitlines()
    assert len(lines)==5000 and len({json.loads(r)['uuid'] for r in lines})==5000
    large=read('performance/review5000.json')
    assert len(large['initialAdditions'])==5000 and len({r['uuid'] for r in large['initialAdditions']})==5000
    assert sum(r['broken'] for r in large['initialAdditions'])==1000
    assert len(large['decisions'])==50 and large['remainingAdditions']==4950
    assert large['approved']['count']==50
print(f'verified {len(manifest["files"])} hashed corpus files and semantic witnesses'+(' with identical second export'if args.compare else ''))
