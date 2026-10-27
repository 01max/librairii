#!/usr/bin/env python3
"""Shown native component evidence; separate from installed-workflow acceptance."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import shutil
import sys
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'target/quality/native-ui'
def run(arguments, **kwargs):
    return subprocess.run(arguments, cwd=ROOT, check=True, **kwargs)
def main():
    if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT, text=True).strip():
        raise ValueError('native evidence requires a clean committed source tree')
    reference = json.loads((ROOT / 'docs/rewrite/native-reference.json').read_text())
    source = ROOT / 'docs/rewrite/ui-reference'
    if hashlib.sha256((source / 'manifest.json').read_bytes()).hexdigest() != reference['manifest_sha256']:
        raise ValueError('approved native reference manifest changed')
    original = json.loads((source / 'manifest.json').read_text())
    if any(reference[key] != value for key, value in original.items()):
        raise ValueError('native reference diverges from the original approval')
    for entry in original['files']:
        if hashlib.sha256((source / entry['path']).read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError('approved native editable/export source changed')
    source_commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    target = os.environ.get('RUST_TARGET')
    if not target:
        target = subprocess.check_output(['rustc', '-vV'], text=True).split('host: ')[1].splitlines()[0]
    allowed = {'aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu'}
    if target not in allowed:
        raise ValueError('unsupported native fixture host')
    host = subprocess.check_output(['rustc', '-vV'], text=True).split('host: ')[1].splitlines()[0]
    if host != target:
        raise ValueError('native evidence requires matching execution architecture')
    if target == 'x86_64-apple-darwin':
        brand = subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True)
        translated = subprocess.run(['sysctl', '-n', 'sysctl.proc_translated'], text=True, capture_output=True)
        if 'Intel' not in brand or translated.stdout.strip() == '1':
            raise ValueError('Intel native evidence requires physical Intel hardware')
    if DESTINATION.exists():
        if DESTINATION.is_symlink() or not (DESTINATION / '.native-fixture-root').is_file() or (DESTINATION / '.native-fixture-root').read_bytes() != b'native-foundations-14\n':
            raise ValueError('refuse to replace an unowned native evidence directory')
        shutil.rmtree(DESTINATION)
    DESTINATION.mkdir(parents=True)
    (DESTINATION / '.native-fixture-root').write_text('native-foundations-14\n')
    with (DESTINATION / 'build.log').open('w') as log, (DESTINATION / 'compiler-artifacts.jsonl').open('w') as events:
        run(['cargo', 'build', '--locked', '--target', target, '-p', 'librairii-desktop', '--example', 'native-foundations', '--message-format=json'], stdout=events, stderr=log)
    artifacts = [json.loads(line) for line in (DESTINATION / 'compiler-artifacts.jsonl').read_text().splitlines()]
    artifacts = [event for event in artifacts if event.get('reason') == 'compiler-artifact' and event.get('target', {}).get('name') == 'native-foundations' and event['target']['kind'] == ['example'] and event.get('executable')]
    if len(artifacts) != 1:
        raise ValueError('Cargo must report exactly one native fixture executable')
    cargo_binary = Path(artifacts[0]['executable']).resolve(strict=True)
    binary = DESTINATION / 'native-foundations'
    shutil.copy2(cargo_binary, binary)
    if not binary.is_file():
        raise ValueError('Cargo native fixture executable is not a regular file')
    binary_digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    executable = {'path': str(binary), 'cargo_path': str(cargo_binary), 'retained_path': 'native-foundations', 'sha256': binary_digest, 'bytes': binary.stat().st_size, 'compiler_fresh': artifacts[0]['fresh']}
    reports = []
    for scale in ['1', '2']:
        output = DESTINATION / ('scale-' + scale)
        output.mkdir(exist_ok=True)
        (output / '.librairii-native-fixture').write_text('native-foundations-14\n')
        environment = dict(os.environ, SLINT_BACKEND='winit-skia-software', SLINT_SCALE_FACTOR=scale)
        with (output / 'execution.log').open('w') as log:
            run([str(binary), str(output)], env=environment, stdout=log, stderr=subprocess.STDOUT, timeout=90)
        report = json.loads((output / 'report.json').read_text())
        if report['macos_programmatic_fixed_window'] != (sys.platform == 'darwin'):
            raise ValueError('native fixture window policy differs from the host')
        if report['native_menu_composition'] != ('passed' if sys.platform == 'darwin' else 'not-applicable'):
            raise ValueError('native production menu composition checks incomplete')
        if report['search_paging_callbacks'] != 'passed':
            raise ValueError('production search/paging callback checks incomplete')
        if report['history_restoration_callbacks'] != 'passed':
            raise ValueError('production history restoration callback checks incomplete')
        if report['failed_query_history_callbacks'] != 'passed':
            raise ValueError('production failed-query history callback checks incomplete')
        frames = report['frames']
        expected = {(concept, density, mode, french) for concept in ['Story shelves', 'Desktop workspace'] for density in range(3) for mode in (range(3) if concept == 'Story shelves' else [0]) for french in [False, True]}
        if {(f['concept'], f['density'], f['mode'], f['french']) for f in frames} != expected or len(frames) != len(expected):
            raise ValueError('native reference/language/density/mode coverage incomplete')
        for frame in frames:
            native = frame['native_window']
            if native['width'] != frame['width'] or native['height'] != frame['height'] or min(native['monitor_width'], native['monitor_height'], native['monitor_scale']) <= 0:
                raise ValueError('native window/monitor observations are inconsistent')
            if sys.platform == 'darwin' and native['resizable']:
                raise ValueError('macOS content fixture permits pointer resizing')
            density = frame['density']
            logical_width, logical_height = [(1180, 760), (980, 600), (560, 400)][density]
            if abs(frame['width'] / frame['scale'] - logical_width) > 1 or abs(frame['height'] / frame['scale'] - logical_height) > 1:
                raise ValueError('native frame observed bounds differ from the requested render density')
            if frame['compact'] != (density == 2) or frame['wide'] != (density == 0) or frame['items'] != 24 or abs(frame['scale'] - float(scale)) > 0.01:
                raise ValueError('native frame did not exercise its claimed geometry/scale/model')
            if frame['concept'] == 'Story shelves' and frame['story_viewport_height'] + 0.5 < frame['story_row_height']:
                raise ValueError('story viewport cannot display its row and external captions')
            if frame['concept'] == 'Desktop workspace' and (frame['approved_viewport_height'] < 76 or frame['review_viewport_height'] < 80):
                raise ValueError('workspace panes cannot display full interaction targets')
            if frame['concept'] == 'Desktop workspace' and frame['decisions'] != 0:
                raise ValueError('fixture approved a review decision implicitly')
        if report['keyboard_checks'] != 'passed':
            raise ValueError('native keyboard checks incomplete')
        reports.append({'scale': scale, 'report': str((output / 'report.json').relative_to(DESTINATION))})
    if sys.platform.startswith('linux'):
        run(['/usr/bin/python3', '-I', '-B', 'scripts/check-native-accessibility.py', str(binary), str(DESTINATION / 'accessibility')], timeout=90)
    if subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() != source_commit or subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT, text=True).strip():
        raise ValueError('native evidence source changed during execution')
    if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_digest:
        raise ValueError('native fixture executable changed during execution')
    files = {str(path.relative_to(DESTINATION)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(DESTINATION.rglob('*')) if path.is_file()}
    value = {'schema': 1, 'source_commit': source_commit, 'executable': executable, 'target': target, 'host': platform.platform(), 'hardware': subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip() if sys.platform == 'darwin' else platform.machine(), 'reference': json.loads((ROOT / 'docs/rewrite/native-reference.json').read_text()), 'runs': reports, 'files': files, 'scope': 'shown native UI/public component keyboard events, observed scales and Skia snapshots; physical HID, full workflow acceptance and packages remain tasks 15/18'}
    (DESTINATION / 'manifest.json').write_text(json.dumps(value, indent=2) + '\n')
    print('Native foundations passed: both approved concepts, English/French, three densities, all story modes, two observed scales and keyboard/modal checks.')
if __name__ == '__main__':
    main()
