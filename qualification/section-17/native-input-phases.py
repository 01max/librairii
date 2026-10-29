#!/usr/bin/env python3
"""[DEBUG-task17-phases] Intrinsic timing in disposable patched example builds."""
import difflib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'target/intel-input-phases'
TARGET = 'x86_64-apple-darwin'
HELPER_SOURCE = '2279dee1c5e4eead74d3ac2ff88192866c5f81fad08e2473751bf3a9df93dea2'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def replace(source, old, new):
    assert source.count(old) == 1, old
    return source.replace(old, new)


def patched_sources():
    view = 'crates/librairii-desktop/src/view.rs'
    filters = 'crates/librairii-desktop/src/view/filters.rs'
    example = 'crates/librairii-desktop/examples/scale-native.rs'
    originals = {name: (ROOT / name).read_text() for name in [view, filters, example]}
    sources = dict(originals)
    sources[view] = replace(sources[view], 'use std::{cell::RefCell, rc::Rc};', '''use std::{cell::RefCell, rc::Rc};
// [DEBUG-task17-phases] This private patch is never applied to a product build.
pub(crate) fn input_clock() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_nanos().try_into().expect("bounded diagnostic clock")
}
thread_local! {
    static INPUT_PHASES: RefCell<Vec<serde_json::Value>> = const { RefCell::new(Vec::new()) };
}
pub(crate) fn input_phase(stage: &str, query: &str) {
    let ns = input_clock();
    INPUT_PHASES.with(|events| {
        let mut events = events.borrow_mut();
        assert!(events.len() < 100_000, "bounded diagnostic phase count");
        events.push(serde_json::json!({"stage":stage,"query":query,"native_ns":ns}));
    });
}
pub(crate) fn input_phase_save(path: &std::path::Path) -> std::io::Result<()> {
    let bytes = INPUT_PHASES.with(|events| serde_json::to_vec(&*events.borrow())).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
}''')
    sources[filters] = replace(sources[filters], '''    ui.on_search(move |text| {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();''', '''    ui.on_search(move |text| {
        if let Some(ui) = weak.upgrade() {
            super::input_phase("search-enter", text.as_str());
            let mut state = state.borrow_mut();''')
    sources[filters] = replace(sources[filters], '''            state.filters.project(&ui, &state.snapshot, &state.catalogs);
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_filter_search''', '''            state.filters.project(&ui, &state.snapshot, &state.catalogs);
            super::input_phase("search-exit", ui.get_query().as_str());
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_filter_search''')
    sources[example] = replace(sources[example], '    let worker = Rc::new(RefCell::new(Worker::start_with(config.clone())?));',
                               '    let _ = view::input_clock();\n    let worker = Rc::new(RefCell::new(Worker::start_with(config.clone())?));')
    sources[example] = replace(sources[example], '    let mut last_projection = None;', '    let mut last_projection = None;\n    let mut clock_sequence = 0u64;')
    sources[example] = replace(sources[example], '        stage="projection";', '''        if let Ok(text)=fs::read_to_string(output.join("clock-request")) {
            let sequence=text.parse::<u64>()?;
            if sequence>clock_sequence {
                let response=json!({"sequence":sequence,"native_ns":view::input_clock()});
                atomic(&output.join("clock-response"),serde_json::to_vec(&response)?)?;
                clock_sequence=sequence;
            }
        }
        view::input_phase("timer-before-projection",ui.get_query().as_str());
        stage="projection";''')
    sources[example] = replace(sources[example], '        stage="state";', '        view::input_phase("state-before",ui.get_query().as_str());\n        stage="state";')
    sources[example] = replace(sources[example], 'atomic(&output.join("state.json"),&encoded)?;previous_state=encoded;',
        'view::input_phase("publish-before",ui.get_query().as_str());atomic(&output.join("state.json"),&encoded)?;view::input_phase("publish-after",ui.get_query().as_str());previous_state=encoded;')
    sources[example] = replace(sources[example], '    let joined = worker.borrow_mut().finish();',
        '    let joined = worker.borrow_mut().finish();\n    view::input_phase_save(&failure_output.join("native-phases.json"))?;')
    return originals, sources


def helper_source():
    original = (ROOT / 'qualification/section-16/input-macos.swift').read_text()
    assert sha(original.encode()) == HELPER_SOURCE
    snippet = Path(__file__).with_name('input-phases-macos.swift').read_text()
    substitutions = [
        ('func state() throws -> [String: Any] {', snippet + '\nfunc state() throws -> [String: Any] {'),
        ('    var input: [Double] = []; var inputStates: [[String: Any]] = []',
         '    try phaseCalibrate()\n    var input: [Double] = []; var inputStates: [[String: Any]] = []'),
        ('        try key(n % 2 == 0 ? 7 : 51)', '        try key(n % 2 == 0 ? 7 : 51)\n        let phasePosted = DispatchTime.now().uptimeNanoseconds'),
        ('        input.append(elapsed(start)); inputStates.append(end)', '''        input.append(elapsed(start)); inputStates.append(end)
        phaseInputs.append(["n": n, "wanted": wanted, "start_ns": start, "posted_ns": phasePosted,
            "observed_ns": DispatchTime.now().uptimeNanoseconds, "key_phases": phaseKeys.last ?? [:]])'''),
        ('    let target = try firstStoryPoint()', '    try phaseFinish()\n    let target = try firstStoryPoint()'),
        ('    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "lost owned foreground PID") }',
         '''    let phaseKeyStart = DispatchTime.now().uptimeNanoseconds
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "lost owned foreground PID") }
    let phaseForeground = DispatchTime.now().uptimeNanoseconds
    defer { if phaseEnabled { phaseKeys.append(["start_ns": phaseKeyStart, "foreground_checked_ns": phaseForeground,
        "posted_ns": DispatchTime.now().uptimeNanoseconds]) } }'''),
        ('} catch { fputs("\\(error)\\n", stderr); exit(1) }', '} catch { try? phaseFinish(); fputs("\\(error)\\n", stderr); exit(1) }'),
    ]
    for old, new in substitutions:
        original = replace(original, old, new)
    return original


def build(label):
    with (OUT / f'{label}-compiler.jsonl').open('w') as events, (OUT / f'{label}-build.log').open('w') as errors:
        subprocess.run(['cargo', 'build', '--locked', '--release', '--target', TARGET,
                        '-p', 'librairii-desktop', '--example', 'scale-native', '--message-format=json'],
                       cwd=ROOT, stdout=events, stderr=errors, check=True, timeout=2400)
    events = [json.loads(line) for line in (OUT / f'{label}-compiler.jsonl').read_text().splitlines()]
    artifacts = [e for e in events if e.get('reason') == 'compiler-artifact' and e.get('target', {}).get('name') == 'scale-native' and e.get('executable')]
    assert len(artifacts) == 1
    a = artifacts[0]
    assert a['profile']['opt_level'] == '3' and not a['profile']['debug_assertions'] and not a['profile']['overflow_checks']
    path = OUT / label
    shutil.copyfile(a['executable'], path)
    path.chmod(0o700)
    return {'sha256': sha(path.read_bytes()), 'profile': a['profile'], 'compiler_executable': a['executable']}


def main():
    assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
    assert platform.system() == 'Darwin' and platform.machine() == 'x86_64'
    assert not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT)
    OUT.mkdir(mode=0o700, parents=True)
    receipt = {'scope': 'diagnostic only; private source patch; never qualification', 'complete': False,
               'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
               'target': TARGET, 'platform': platform.platform(),
               'hardware': {key: subprocess.check_output(['sysctl', '-n', value], text=True).strip()
                            for key, value in [('cpu', 'machdep.cpu.brand_string'), ('memory', 'hw.memsize'),
                                               ('logical_cpus', 'hw.logicalcpu'), ('model', 'hw.model')]},
               'compiler': subprocess.check_output(['rustc', '-vV'], text=True), 'cohorts': [],
               'plan': [['control-software', False, 'winit-skia-software'],
                        ['traced-software', True, 'winit-skia-software'], ['traced-gpu', True, 'winit-skia']]}
    def publish():
        (OUT / 'diagnostic.json').write_text(json.dumps(receipt, indent=2) + '\n')
    publish()
    receipt['control'] = build('control')
    originals, sources = patched_sources()
    receipt['sources'] = {name: {'before': sha(originals[name].encode()), 'after': sha(text.encode())} for name, text in sources.items()}
    patch = ''.join(''.join(difflib.unified_diff(originals[name].splitlines(keepends=True), text.splitlines(keepends=True),
                    fromfile='a/' + name, tofile='b/' + name)) for name, text in sources.items())
    (OUT / 'private-source.patch').write_text(patch)
    receipt['patch_sha256'] = sha(patch.encode())
    publish()
    try:
        for name, text in sources.items():
            (ROOT / name).write_text(text)
        receipt['traced'] = build('traced')
    finally:
        for name, text in originals.items():
            (ROOT / name).write_text(text)
    assert not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT)
    original_helper = OUT / 'original-input'
    subprocess.run(['xcrun', 'swiftc', str(ROOT / 'qualification/section-16/input-macos.swift'), '-o', str(original_helper)], check=True)
    source = OUT / 'input-phases.swift'
    source.write_text(helper_source())
    traced_helper = OUT / 'traced-input'
    subprocess.run(['xcrun', 'swiftc', str(source), '-o', str(traced_helper)], check=True)
    receipt['helpers'] = {p.name: sha(p.read_bytes()) for p in [original_helper, traced_helper, source]}
    publish()
    spec = importlib.util.spec_from_file_location('native', ROOT / 'qualification/section-16/native.py')
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    for name, traced, backend in receipt['plan']:
        destination = OUT / name
        cohort = {'name': name, 'traced': traced, 'error': None}
        try:
            native.inspect(OUT / ('traced' if traced else 'control'), destination, backend,
                           traced_helper if traced else original_helper)
        except Exception as error:
            cohort['error'] = f'{type(error).__name__}: {error}'
        try:
            report = json.loads((destination / 'report.json').read_text())
        except (OSError, ValueError) as error:
            report = {}
            cohort['report_error'] = f'{type(error).__name__}: {error}'
        cohort.update(workers_joined=report.get('workers_joined', False), fixture_retired=report.get('fixture_retired', False),
                      gate_complete=report.get('complete', False),
                      workload_complete=cohort['error'] in [None, 'ValueError: retained native input/completion wall gate failed'],
                      input_p95_ms=report.get('input', {}).get('input_p95_ms'))
        if traced:
            phases = destination / 'native-phases.json'
            helper_phases = destination / 'helper-phases.json'
            cohort['phases_complete'] = phases.exists() and helper_phases.exists()
            if cohort['phases_complete']:
                try:
                    measured = json.loads(helper_phases.read_text())
                    cohort['phases_complete'] = measured['clock_bounds_valid'] and len(measured['samples']) == 64
                    cohort['clock_uncertainty_ns'] = measured['offset_upper_ns'] - measured['offset_lower_ns'] if measured['clock_bounds_valid'] else None
                except (OSError, ValueError, KeyError, TypeError) as error:
                    cohort['phases_complete'] = False
                    cohort['phase_error'] = f'{type(error).__name__}: {error}'
        receipt['cohorts'].append(cohort)
        publish()
        print(json.dumps(cohort), flush=True)
        if not cohort['workers_joined'] or not cohort['fixture_retired']:
            raise ValueError('owned retirement required before another cohort')
    for label in ['control', 'traced']:
        assert sha((OUT / label).read_bytes()) == receipt[label]['sha256']
    receipt['complete'] = all(c['workload_complete'] and c.get('phases_complete', not c['traced']) for c in receipt['cohorts'])
    publish()
    if not receipt['complete']:
        raise ValueError('intrinsic diagnostic incomplete')


if __name__ == '__main__':
    main()
