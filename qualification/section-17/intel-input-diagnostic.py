#!/usr/bin/env python3
"""[DEBUG-task17-input] Bounded retained-binary comparison, never qualification."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'target/intel-input-diagnostic'
PINS = {
    'baseline': (12881354229, 109806928,
                 'dd91b5011667c49a190d67d139758a8939491eaa462768ad7bd0f5e84a77e3c1',
                 '81909365ba3d0ed04853d50e4e6381117b0b8c2e',
                 '096bfb009443ce85011ed9d306f3906d0bfff4935229a7b5813b9f8b88b5e010'),
    'current': (12927381614, 109820130,
                '4c25490e071a4ae6fe8957fa863055ffce86e14722891d95bb93f36f16cb248f',
                'ef1985ca09abdbdcee27ed5cf1f3b23634db5c8a',
                'c3b940cafc5dd972d23acdde65bf7f45cb7e4f6b5ca1eb42c932c8f9223beea1'),
}
HELPER = '7038b979bb74a946c603122c28bb6df37e47abedb9665eb31f43eef384ea0230'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def retain(label):
    artifact, size, sha, source, binary_sha = PINS[label]
    archive = OUT / f'{label}.zip'
    endpoint = f'repos/01max/librairii/actions/artifacts/{artifact}'
    metadata = subprocess.check_output(['gh', 'api', endpoint])
    (OUT / f'{label}-provider.json').write_bytes(metadata)
    assert json.loads(metadata)['id'] == artifact
    with archive.open('xb') as stream:
        subprocess.run(['gh', 'api', endpoint + '/zip'], stdout=stream, check=True)
    assert archive.stat().st_size == size and digest(archive.read_bytes()) == sha
    with zipfile.ZipFile(archive) as z:
        assert len(z.namelist()) == len(set(z.namelist()))
        manifest = json.loads(z.read('desktop/manifest.json'))
        assert manifest['source_commit'] == source and manifest['target'] == 'x86_64-apple-darwin'
        assert manifest['executable']['profile']['opt_level'] == '3'
        assert not manifest['executable']['profile']['debug_assertions']
        assert not manifest['executable']['profile']['overflow_checks']
        root = json.loads(z.read('manifest.json'))
        assert root['cargo_lock_sha256'] == digest((ROOT / 'Cargo.lock').read_bytes())
        for name, expected in root['fixtures'].items():
            assert digest((ROOT / name).read_bytes()) == expected
        for member, expected in [('scale-native', binary_sha), ('input-macos', HELPER)]:
            data = z.read('desktop/' + member)
            assert digest(data) == expected
            path = OUT / f'{label}-{member}'
            with path.open('xb') as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            path.chmod(0o700)
            assert path.stat().st_nlink == 1
            assert 'x86_64' in subprocess.check_output(['file', str(path)], text=True)
    return {'artifact': artifact, 'zip_sha256': sha, 'binary_sha256': binary_sha,
            'source': source, 'hardware': root['hardware']}


def probe_source():
    original = (ROOT / 'qualification/section-16/input-macos.swift').read_text()
    probe = Path(__file__).with_name('input-probe-macos.swift').read_text()
    substitutions = [
        ('func state() throws -> [String: Any] {', probe + '\nfunc state() throws -> [String: Any] {'),
        ('    try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("state.json"))) as! [String: Any]',
         '''    let before = DispatchTime.now().uptimeNanoseconds
    let cpu = probeEnabled ? probeCPU() : 0
    let value = try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("state.json"))) as! [String: Any]
    if probeEnabled { probeReads.append(["before_ns": before, "after_ns": DispatchTime.now().uptimeNanoseconds,
        "cpu_ns": probeCPU() - cpu, "query": value["query"] ?? NSNull()]) }
    return value'''),
        ('    var input: [Double] = []; var inputStates: [[String: Any]] = []',
         '    probeStart()\n    var input: [Double] = []; var inputStates: [[String: Any]] = []'),
        ('        try key(n % 2 == 0 ? 7 : 51)',
         '''        let cpuStart = probeCPU(); let readsStart = probeReads.count
        try key(n % 2 == 0 ? 7 : 51)
        let posted = DispatchTime.now().uptimeNanoseconds'''),
        ('        input.append(elapsed(start)); inputStates.append(end)',
         '''        input.append(elapsed(start)); inputStates.append(end)
        probeSamples.append(["n": n, "wanted": wanted, "start_ns": start, "posted_ns": posted,
            "observed_ns": DispatchTime.now().uptimeNanoseconds, "thread_cpu_ns": probeCPU() - cpuStart,
            "key_phases": probeKeys.last ?? [:], "reads": Array(probeReads[readsStart...])])'''),
        ('    let target = try firstStoryPoint()', '    try probeEnd()\n    let target = try firstStoryPoint()'),
        ('    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "lost owned foreground PID") }',
         '''    let probeKeyStart = DispatchTime.now().uptimeNanoseconds
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "lost owned foreground PID") }
    let probeForeground = DispatchTime.now().uptimeNanoseconds
    defer { if probeEnabled { probeKeys.append(["start_ns": probeKeyStart, "foreground_checked_ns": probeForeground,
        "posted_ns": DispatchTime.now().uptimeNanoseconds]) } }'''),
        ('} catch { fputs("\\(error)\\n", stderr); exit(1) }',
         '} catch { try? probeEnd(); fputs("\\(error)\\n", stderr); exit(1) }'),
    ]
    for old, new in substitutions:
        assert original.count(old) == 1, old
        original = original.replace(old, new)
    return original


def main():
    assert os.environ.get('GITHUB_ACTIONS') == 'true'
    assert platform.system() == 'Darwin' and platform.machine() == 'x86_64'
    assert os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
    OUT.mkdir(mode=0o700, parents=True)
    receipt = {'scope': 'diagnostic only; never substitutes for or erases qualification',
               'complete': False, 'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
               'platform': platform.platform(), 'hardware': {
                   key: subprocess.check_output(['sysctl', '-n', value], text=True).strip()
                   for key, value in [('cpu', 'machdep.cpu.brand_string'), ('memory', 'hw.memsize'),
                                      ('logical_cpus', 'hw.logicalcpu'), ('model', 'hw.model')]},
               'binaries': {}, 'cohorts': []}
    path = OUT / 'diagnostic.json'
    def publish():
        path.write_text(json.dumps(receipt, indent=2) + '\n')
    publish()
    for label in PINS:
        receipt['binaries'][label] = retain(label)
        publish()
    source = OUT / 'input-probe.swift'
    source.write_text(probe_source())
    helper = OUT / 'input-probe'
    subprocess.run(['xcrun', 'swiftc', str(source), '-o', str(helper)], check=True)
    receipt['probe_sha256'] = digest(helper.read_bytes())
    # All six cohorts are declared before observing any result. Each uses the
    # unchanged full 1000-cover/64-key/64-pointer/20-completion workload. The last
    # two use the exact original Swift bytes, with no AX/CPU/stack probes.
    plan = [('baseline', 'software', True), ('current', 'software', True),
            ('current', 'gpu-api', True), ('baseline', 'gpu-api', True),
            ('current', 'software', False), ('baseline', 'software', False)]
    receipt['plan'] = plan
    publish()
    spec = importlib.util.spec_from_file_location('native', ROOT / 'qualification/section-16/native.py')
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    # Event-driving children have no reason to inherit the read-only API token.
    os.environ.pop('GH_TOKEN', None)
    os.environ.pop('GITHUB_TOKEN', None)
    for index, (label, renderer, probes) in enumerate(plan):
        name = f'{index}-{label}-{renderer}-' + ('probe' if probes else 'original')
        cohort = {'name': name, 'probes': probes, 'error': None}
        destination = OUT / name
        try:
            native.inspect(OUT / f'{label}-scale-native', destination,
                           'winit-skia-software' if renderer == 'software' else 'winit-skia',
                           helper if probes else OUT / f'{label}-input-macos')
        except Exception as error:
            cohort['error'] = f'{type(error).__name__}: {error}'
        report = json.loads((destination / 'report.json').read_text()) if (destination / 'report.json').exists() else {}
        if probes:
            probe_path = destination / 'keyboard-probe.json'
            probe = json.loads(probe_path.read_text()) if probe_path.exists() else {}
            cohort['probe_cleanup_complete'] = probe.get('cleanup_complete', False)
            cohort['probes_complete'] = probe.get('probes_complete', False)
        cohort.update(workers_joined=report.get('workers_joined', False),
                      fixture_retired=report.get('fixture_retired', False), gate_complete=report.get('complete', False))
        for key in ['input_p95_ms', 'pointer_p95_ms', 'navigation_p95_ms']:
            cohort[key] = report.get('input', {}).get(key)
        receipt['cohorts'].append(cohort)
        publish()
        print(json.dumps(cohort), flush=True)
        if not cohort['workers_joined'] or not cohort['fixture_retired'] or (probes and not cohort['probe_cleanup_complete']):
            raise ValueError('owned process/fixture retirement required before another cohort')
    for label, pin in PINS.items():
        assert digest((OUT / f'{label}-scale-native').read_bytes()) == pin[4]
        assert digest((OUT / f'{label}-input-macos').read_bytes()) == HELPER
    receipt['complete'] = True  # completion of the declared diagnostic, not admission
    receipt['probes_complete'] = all(c.get('probes_complete', not c['probes']) for c in receipt['cohorts'])
    publish()
    if not receipt['probes_complete']:
        raise ValueError('declared diagnostic completed, but one or more probes are unavailable')


if __name__ == '__main__':
    main()
