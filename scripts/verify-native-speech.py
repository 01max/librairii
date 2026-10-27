#!/usr/bin/env python3
"""Bind actual Orca/PCM workflow evidence to the retained native CI executable."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
def run(argv,**kwargs):return subprocess.run(argv,cwd=ROOT,check=True,**kwargs)
def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        while data:=stream.read(1024*1024):h.update(data)
    return h.hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--case');args=parser.parse_args()
    assert sys.platform=='linux','spoken release evidence requires the native Linux CI host'
    assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=normal'],cwd=ROOT,text=True).strip(),'clean committed source required'
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    sealed=ROOT/'target/quality/native-ui';manifest=json.loads((sealed/'manifest.json').read_text())
    assert manifest['source_commit']==commit
    assert manifest['target']=='x86_64-unknown-linux-gnu' and platform.machine()=='x86_64'
    binary=sealed/'native-foundations';binary_digest=sha(binary);assert binary_digest==manifest['executable']['sha256']
    evidence=ROOT/'target/quality/workflow-speech';evidence.mkdir(exist_ok=False)
    uid,gid=str(os.getuid()),str(os.getgid());image='librairii-workflow-speech:'+commit
    run(['docker','build','--build-arg','QUALIFICATION_UID='+uid,'--build-arg','QUALIFICATION_GID='+gid,'-t',image,'scripts/native-speech'])
    image_id=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',image],text=True).strip()
    binding={'source_commit':commit,'binary_sha256':binary_digest,'host':platform.platform(),'machine':platform.machine(),'image_id':image_id,'observer_files':{str(p.relative_to(ROOT)):sha(p) for p in sorted((ROOT/'scripts/native-speech').glob('*')) if p.is_file()},'sessions':{}}
    verified=False
    try:
        for session in ('x11','wayland'):
            output=evidence/session;output.mkdir();(output/'.workflow-speech-fixture').write_text('workflow-speech-v1\n')
            argv=['docker','run','--rm','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--network=none',
                '--user',uid+':'+gid,'--pids-limit=256','--memory=3g','--cpus=2',
                '--tmpfs','/tmp:rw,nosuid,nodev,mode=1777,size=1g',
                '--mount','type=bind,src='+str(ROOT)+',dst='+str(ROOT)+',readonly',
                '--mount','type=bind,src='+str(output)+',dst=/evidence',image,
                '--session',session,'--binary',str(binary),'--output','/evidence']
            if args.case:argv+=['--case',args.case]
            run(argv,timeout=1800)
            report=json.loads((output/'report.json').read_text())
            assert report['passed'] and not report['overflow'] and report['binary_sha256']==binding['binary_sha256']
            expected=2 if args.case else 56
            assert len(report['cases'])==expected and all(c['passed'] and c['closed']['actual_worker_joined'] for c in report['cases'])
            assert report['speech_steps'] and all(s['audio']['nonzero_samples']>1600 for s in report['speech_steps'].values())
            binding['sessions'][session]={'report_sha256':sha(output/'report.json'),'cases':len(report['cases']),'speech_steps':len(report['speech_steps'])}
        assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==commit,'source commit changed'
        assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=normal'],cwd=ROOT,text=True).strip(),'source changed during spoken execution'
        assert sha(binary)==binding['binary_sha256'],'retained executable changed'
        verified=True
    finally:
        binding['complete']=verified and args.case is None
        (evidence/'binding.json').write_text(json.dumps(binding,indent=2)+'\n')
if __name__=='__main__':main()
