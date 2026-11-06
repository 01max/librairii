#!/usr/bin/env python3
"""Bind actual Orca/PCM workflow evidence to the retained native CI executable."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time
import uuid

# Independent inventory: dropping a driver case cannot reduce the accepted matrix.
WORKFLOWS=('browse','filters','details','removal','tag-manager','tag-assignment',
    'tag-delete','flag-bulk','collections','collection-union','collection-impact',
    'shelf-members','shelf-create','shelf-exclusions','review-additions',
    'review-removals','review-delete','import','sync','export','export-report',
    'history','history-error','setup','settings','relocation','recovery','schema')

ROOT=Path(__file__).resolve().parents[1]
def run(argv,**kwargs):return subprocess.run(argv,cwd=ROOT,check=True,**kwargs)
def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        while data:=stream.read(1024*1024):h.update(data)
    return h.hexdigest()
def retire_observer(cid_file,invocation,image_id,name,force=False):
    assert cid_file.is_file() and not cid_file.is_symlink(),'missing observer creation receipt'
    cid=cid_file.read_text().strip();assert re.fullmatch('[0-9a-f]{64}',cid)
    observed=json.loads(subprocess.check_output(['docker','inspect',cid],text=True,timeout=10))
    assert len(observed)==1
    container=observed[0]
    assert container['Id']==cid and container['Name']=='/'+name and container['Image']==image_id
    assert container['Config']['Labels']['librairii.workflow-observer']==invocation
    assert force or container['State']['Running'] is False,'observer did not stop'
    run(['docker','rm',*(['--force'] if force else []),cid],timeout=15)
    return cid
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--case',choices=WORKFLOWS);args=parser.parse_args()
    assert sys.platform=='linux','spoken release evidence requires the native Linux CI host'
    assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=normal'],cwd=ROOT,text=True).strip(),'clean committed source required'
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    sealed=ROOT/'target/quality/native-ui';manifest_path=sealed/'manifest.json'
    assert os.environ.get('RUST_TARGET')=='x86_64-unknown-linux-gnu' and platform.machine()=='x86_64'
    binary=sealed/'native-foundations';binary_digest=sha(binary)
    visual_passed=manifest_path.is_file()
    if visual_passed:
        manifest=json.loads(manifest_path.read_text());assert manifest['source_commit']==commit
        assert manifest['target']=='x86_64-unknown-linux-gnu' and binary_digest==manifest['executable']['sha256']
    else:
        # Retain spoken diagnostics after a visual failure. That earlier CI step
        # still fails, and complete remains false without its sealed manifest.
        events=[json.loads(line) for line in (sealed/'compiler-artifacts.jsonl').read_text().splitlines()]
        artifacts=[e for e in events if e.get('reason')=='compiler-artifact' and e.get('target',{}).get('name')=='native-foundations' and e.get('target',{}).get('kind')==['example'] and e.get('executable')]
        assert len(artifacts)==1
        compiled=Path(artifacts[0]['executable']).resolve(strict=True)
        assert compiled.is_relative_to(ROOT/'target') and sha(compiled)==binary_digest
    evidence=ROOT/'target/quality'/('workflow-speech' if args.case is None else 'workflow-speech-'+args.case);evidence.mkdir(exist_ok=False)
    uid,gid=str(os.getuid()),str(os.getgid());image='librairii-workflow-speech:'+commit
    run(['docker','build','--build-arg','QUALIFICATION_UID='+uid,'--build-arg','QUALIFICATION_GID='+gid,'-t',image,'scripts/native-speech'])
    image_id=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',image],text=True).strip()
    invocation=uuid.uuid4().hex
    binding={'source_commit':commit,'binary_sha256':binary_digest,'native_visual_passed':visual_passed,'host':platform.platform(),'machine':platform.machine(),'image_id':image_id,'invocation':invocation,'isolation':'one-private-desktop-reader-per-workflow-language-v1','observer_files':{str(p.relative_to(ROOT)):sha(p) for p in sorted((ROOT/'scripts/native-speech').glob('*')) if p.is_file()},'sessions':{}}
    verified=False
    failures=[]
    try:
        for session in ('x11','wayland'):
            session_root=evidence/session;session_root.mkdir()
            entry={'reports':[],'cases':0,'speech_steps':0};binding['sessions'][session]=entry
            # Retain the original aggregate budget, not a new30min per case.
            deadline=time.monotonic()+1800
            for case in ([args.case] if args.case else WORKFLOWS):
                for language in ('en','fr'):
                    name=case+'-'+language;output=session_root/name;output.mkdir()
                    (output/'.workflow-speech-fixture').write_text('workflow-speech-v1\n')
                    container='librairii-speech-'+invocation+'-'+session+'-'+name
                    cid_file=session_root/(name+'.container-id');assert not cid_file.exists()
                    argv=['docker','run','--name',container,'--cidfile',str(cid_file),
                        '--label','librairii.workflow-observer='+invocation,'--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--network=none',
                        '--user',uid+':'+gid,'--pids-limit=256','--memory=3g','--cpus=2',
                        '--tmpfs','/tmp:rw,nosuid,nodev,mode=1777,size=1g',
                        '--mount','type=bind,src='+str(ROOT)+',dst='+str(ROOT)+',readonly',
                        '--mount','type=bind,src='+str(output)+',dst=/evidence',image_id,
                        '--session',session,'--binary',str(binary),'--output','/evidence',
                        '--case',case,'--language',language]
                    receipt={'name':name,'passed':False};entry['reports'].append(receipt)
                    print('Native speech:',session,name,flush=True)
                    try:
                        remaining=deadline-time.monotonic()
                        assert remaining>0,'native speech aggregate deadline'
                        run(argv,timeout=min(210,remaining))
                    except subprocess.TimeoutExpired:
                        # A name alone grants no authority to remove a container.
                        if cid_file.is_file():
                            receipt['timeout_retired_container']=retire_observer(cid_file,invocation,image_id,container,force=True)
                        else:
                            receipt['timeout_retirement']='no completed creation receipt'
                        raise
                    except subprocess.CalledProcessError as error:
                        failures.append(session+'/'+name+': '+str(error))
                    report_path=output/'report.json'
                    if cid_file.is_file():
                        receipt['container_id']=retire_observer(cid_file,invocation,image_id,container)
                        receipt['container_retired']=True
                    if report_path.is_file():
                        receipt.update(report=str(report_path.relative_to(evidence)),report_sha256=sha(report_path))
                        report=json.loads(report_path.read_text())
                        if not report['passed']:
                            continue
                        assert not report['overflow'] and report['binary_sha256']==binding['binary_sha256']
                        assert report['method']=='native-workflows-orca-pcm-v5' and report['session']==session
                        assert 'container_id' in receipt,'missing actual observer creation receipt'
                        assert len(report['cases'])==1 and report['cases'][0]['name']==name
                        assert report['cases'][0]['passed'] and report['cases'][0]['closed']=={'actual_worker_joined':True}
                        expected={name+'-focus'}
                        if case=='flag-bulk':expected|={name+'-all',name+'-mixed-restored'}
                        if case=='shelf-members':expected.add(name+'-after-reorder')
                        if case=='tag-assignment':expected.add(name+'-assigned')
                        assert set(report['speech_steps'])==expected
                        assert all(s['audio']['nonzero_samples']>1600 for s in report['speech_steps'].values())
                        for path,facts in report['files'].items():
                            relative=Path(path);assert not relative.is_absolute() and '..' not in relative.parts
                            file=output/relative
                            assert file.stat().st_size==facts['bytes'] and sha(file)==facts['sha256'],path
                        receipt['passed']=True;entry['cases']+=1;entry['speech_steps']+=len(expected)
                    else:
                        failures.append(session+'/'+name+': missing actual report')
            expected_names={case+'-'+language for case in ([args.case] if args.case else WORKFLOWS) for language in ('en','fr')}
            assert len(entry['reports'])==len(expected_names) and {r['name'] for r in entry['reports']}==expected_names
            if entry['cases']!=len(expected_names) or not all(r['passed'] for r in entry['reports']):
                failures.append(session+': incomplete or failed independent cases')
            assert time.monotonic()<=deadline,'native speech aggregate deadline'
        assert not failures,'native speech sessions failed: '+'; '.join(failures)
        assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==commit,'source commit changed'
        assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=normal'],cwd=ROOT,text=True).strip(),'source changed during spoken execution'
        assert sha(binary)==binding['binary_sha256'],'retained executable changed'
        verified=True
    finally:
        binding['spoken_complete']=verified and args.case is None
        binding['complete']=binding['spoken_complete'] and visual_passed
        (evidence/'binding.json').write_text(json.dumps(binding,indent=2)+'\n')
if __name__=='__main__':main()
