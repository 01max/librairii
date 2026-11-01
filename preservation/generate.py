#!/usr/bin/env python3
"""Export using frozen Go source in a disposable archive, never a task tree."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--oracle', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path, help='new directory; never an existing library/profile')
args = parser.parse_args()
subprocess.run(['python3', str(HERE/'tools/verify-source.py'), str(args.oracle)], check=True)
out = args.output.absolute()
if out.exists() or out.is_symlink():
    raise SystemExit('output must be a new, explicitly selected directory')
commit = json.loads((HERE/'oracle.json').read_text())['source_commit']
with tempfile.TemporaryDirectory(prefix='librairii-frozen-export-') as temp:
    work = Path(temp)/'source'
    work.mkdir()
    # git archive reads exact committed source, independently of checkout changes.
    archive = subprocess.Popen(['git', '-C', str(args.oracle), 'archive', commit], stdout=subprocess.PIPE)
    subprocess.run(['tar', '-xf', '-', '-C', str(work)], stdin=archive.stdout, check=True)
    archive.stdout.close()
    if archive.wait():
        raise SystemExit('oracle archive failed')
    shutil.copytree(HERE/'generator', work/'cmd/preservation-oracle')
    staged = Path(temp)/'corpus'
    env = dict(os.environ, GOTOOLCHAIN='local')
    # Generator only calls local adapters. It never constructs the production HTTP client.
    subprocess.run(['go', 'run', './cmd/preservation-oracle', '-out', str(staged)], cwd=work, env=env, check=True)
    files = []
    for path in sorted(staged.rglob('*')):
        if path.is_file():
            files.append({'path':path.relative_to(staged).as_posix(), 'bytes':path.stat().st_size, 'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
    generators = [HERE/'generate.py'] + sorted((HERE/'generator').glob('*.go'))
    manifest = {'version':1,'oracle_commit':commit,'generator_files':[{'path':p.relative_to(HERE).as_posix(),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}for p in generators], 'files':files,'normalizations':'../golden-contract.md','network':'No production endpoint called; no HTTP adapter constructed','sqlite_producer':'Go modernc.org/sqlite v1.60.1; see database manifest when present'}
    (staged/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
    shutil.copytree(staged, out)
print(f'generated {len(files)} corpus files from {commit}')
