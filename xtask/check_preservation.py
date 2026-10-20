"""Run the admitted read-only checker, never the Go oracle or fixture generator."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def verify_pins(root, baseline):
    """A missing/replaced baseline fails before any checker is executed."""
    for relative, key in [
        ('preservation/check.py', 'checker_sha256'),
        ('preservation/corpus/manifest.json', 'manifest_sha256'),
    ]:
        path = root / relative
        if path.is_symlink() or not path.is_file():
            raise ValueError(f'missing or aliased admitted input: {relative}')
        if hashlib.sha256(path.read_bytes()).hexdigest() != baseline[key]:
            raise ValueError(f'admitted input changed; requires reviewed baseline update: {relative}')
    manifest = json.loads((root / 'preservation/corpus/manifest.json').read_text(encoding='utf-8'))
    if manifest['oracle_commit'] != baseline['oracle_commit']:
        raise ValueError('wrong preservation oracle')
    if len(manifest['files']) != baseline['hashed_files']:
        raise ValueError('wrong preservation file inventory')


def main():
    root = Path(__file__).resolve().parent.parent
    baseline = json.loads((root / '.config/rust-preservation-baseline.json').read_text(encoding='utf-8'))
    verify_pins(root, baseline)
    # -I preserves assertions despite PYTHONOPTIMIZE/PYTHONPATH. -B avoids source
    # writes; UTF-8 preserves Unicode witnesses on native Windows shells.
    # The frozen checker constructs SQLite URIs literally. Keep the corpus
    # relative to this checkout so drive letters and #/?/% in its absolute
    # parent path never enter those URIs. Do not depend on the caller's cwd.
    result = subprocess.run(
        [sys.executable, '-I', '-B', '-X', 'utf8', 'preservation/check.py',
         'preservation/corpus'],
        cwd=root, check=False, capture_output=True, encoding='utf-8',
    )
    if result.returncode != 0:
        raise RuntimeError(f'preservation checker failed ({result.returncode}):\n{result.stdout}\n{result.stderr}')
    verify_pins(root, baseline)
    print(json.dumps({
        **baseline,
        'python': sys.version,
        'mode': 'read-only integrity and Go-produced semantic witnesses; no Rust feature parity claim',
        'result': result.stdout.strip(),
    }, indent=2))


if __name__ == '__main__':
    main()
