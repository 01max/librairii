#!/usr/bin/env python3
"""Assemble source-backed inventories from pinned-source AST captures.

Usage: assemble-inventory.py SPEC_ROOT GO_AST_JSON UI_AST_JSON
Run inventory.go against a clean pinned oracle and ui-inventory.cjs against its
independent dependency-equipped clone. No application code runs during parsing.
"""
from pathlib import Path
import hashlib
import json
import re
import sys

HERE = Path(__file__).resolve().parent.parent
SPEC = Path(sys.argv[1])
source = json.loads((HERE / 'oracle-source.json').read_text())
register = []
for line in (SPEC / 'changes/rewrite-in-rust-native-desktop/parity-matrix.md').read_text().splitlines():
    if not re.match(r'^\| P\d\d ·', line):
        continue
    ident, old, contract, proposed, status = line.strip('| ').split(' | ')
    pid, stream = ident.split(' · ')
    register.append({'id': pid, 'workstream': stream, 'baseline_owner': 'librairii-3',
                     'implementation_accountable': 'librairii-1 (assign section owner before implementation)',
                     'existing_evidence': old, 'retained_behavior': contract,
                     'proposed_rust_evidence': proposed, 'targets': {x: 'unimplemented/unverified' for x in ['A','I','L','W']}})
assert [r['id'] for r in register] == [f'P{i:02}' for i in range(1,61)]
module = {'storage': [1,3,55], 'database': [2,3], 'archive': [4,7,47], 'inspection': [5,6,7],
          'catalog': [8,41], 'importer': [8], 'operations': [9,10,11,44,46,48,50],
          'diagnostics': [12], 'library': [14,15,17,18,19,24,44], 'searchtext': [14],
          'collectionquery': [17,25,26], 'collections': [17,25,26], 'shelves': list(range(27,39)),
          'tagging': [20,21,22,23], 'lunii': [39], 'metadata': [40,41,43], 'artwork': [42],
          'exporter': list(range(44,49)), 'removal': [49], 'preferences': [52,53,54,55],
          'platform': [5,48,52,53,55,57], 'localization': [53], 'performancefixture': [58],
          'app': [8,9,10,11,24,39,44,49,54,60], 'testsupport': [60]}
def ids(path, name=''):
    found = {row['id'] for row in register if f'`{path}`' in row['existing_evidence']}
    parts = path.split('/')
    if len(parts)>2 and parts[0]=='internal': found.update(f'P{i:02}'for i in module.get(parts[1], [60]))
    if path.startswith('frontend/'):
        stem = Path(path).stem.lower()
        nums = [13,18,19,56]
        if 'shelf' in stem: nums = list(range(27,39))
        elif 'collection' in stem: nums = [25,26]
        elif 'query' in stem: nums = [14,15,16,17]
        elif 'tag' in stem: nums = [20,21,22,23]
        elif 'broken' in stem or 'flag' in stem: nums = [24]
        elif 'export' in stem: nums = [44,45,46,47,48]
        elif 'settings' in stem or 'language' in stem or 'i18n' in stem: nums=[52,53,54,55]
        elif 'history' in stem: nums=[50,51]
        elif 'notice' in stem or 'library-view' in stem: nums=[13,43,50,51]
        elif 'sync' in stem or 'metadata' in stem: nums=[39,40,41,43]
        elif 'performance' in path: nums=[58]
        found.update(f'P{i:02}' for i in nums)
    elif path.startswith(('build/','scripts/','.github/')) or path in ['wails.json','release_config.go']: found.add('P59')
    elif path.startswith('settings'): found.update(['P52','P53','P54'])
    elif path=='shelves.go': found.update(f'P{i:02}'for i in range(27,39))
    elif path in ['main.go','app.go']: found.update(['P09','P11','P57','P60'])
    elif path.startswith('cmd/library-performance') or path.startswith('docs/performance'):found.add('P58')
    elif path.startswith('cmd/'):found.add('P60')
    if name:
        if 'Collection' in name: found.update(['P25','P26','P37'])
        if 'Broken' in name or 'StoryFlags' in name: found.add('P24')
        if 'Tag' in name: found.update(['P20','P21','P22','P23'])
        if 'Export' in name: found.update(['P44','P45','P46','P48'])
        if 'Operation' in name:found.update(['P09','P10','P50'])
        if 'Metadata' in name:found.update(['P39','P40','P41','P43'])
    return sorted(found or {'P60'})
def save(name, data): (HERE/name).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
files=[]
for entry in source['files']:
    p=entry['path']; role='asset/config'
    if p.endswith(('_test.go','.test.ts','.test.tsx')):role='test'
    elif p.endswith(('.go','.ts','.tsx','.m','.c','.sql','.css')):role='source'
    elif p.startswith('docs/') or p.endswith('.md'):role='documentation'
    elif 'testdata/' in p or 'fixture' in p:role='fixture'
    files.append({**entry,'role':role,'parity_ids':ids(p),'owner':'librairii-3 (baseline preservation)'})
save('source-inventory.json',{'version':1,'commit':source['commit'],'coverage':'all 436 tracked baseline files, including source/tests/docs/assets/build; broad module associations are discovery routes, not executable coverage claims','files':files})
go=json.loads(Path(sys.argv[2]).read_text());ui=json.loads(Path(sys.argv[3]).read_text())
for row in go+ui:
    row['parity_ids']=ids(row['path'],row['name']);row['owner']='librairii-3 (baseline contract)'
    row['rust_evidence']=[f'parity::{pid}'for pid in row['parity_ids']]
    if row['name']=='SetShelfStoryProtected':row['contract_note']='Retained compatibility bridge/internal historical field; current UI exposes no protection toggle. Do not revive a public control.'
    if row['name'] in ['PackagedAcceptanceMode','RecordPackagedAcceptance','FrontendRendered']:row['contract_note']='Shell/acceptance instrumentation; replace with native evidence rather than preserve Wails API shape.'
save('command-inventory.json',{'version':1,'commands':[r for r in go if r['kind']=='application-command']})
save('payload-inventory.json',{'version':1,'scope':'Every top-level Go struct field carrying a JSON tag, including transient command responses. Consult schema/query/profile inventories to distinguish persisted authority from transient models. A field list does not imply every transient field must become a persistent native preference.','fields':[r for r in go if r['kind']=='serialized-field']})
save('test-inventory.json',{'version':1,'scope':'All named Go Test entrypoints; subtests and frontend assertions remain in their complete pinned files in source-inventory.json. Names are discovery evidence, not test execution results.','go_tests':[r for r in go if r['kind']=='go-test']})
save('ui-inventory.json',{'version':1,'scope':'Every JSX onX event binding, bridge import and explicit browser-state call parsed with TypeScript 6.0.3; delegated/component-level handlers retained verbatim. Full frontend source/test inventory covers helpers, keyboard listeners, accessibility and CSS that have no JSX handler.','entries':ui})
save('parity-register.json',{'version':1,'planning_source':'changes/rewrite-in-rust-native-desktop/parity-matrix.md in separate OpenSpec repository','sha256':hashlib.sha256((SPEC/'changes/rewrite-in-rust-native-desktop/parity-matrix.md').read_bytes()).hexdigest(),'rows':register})
print(f'{len(files)} files, {len(go)} Go AST contracts/tests, {len(ui)} UI entries, {len(register)} parity rows')
