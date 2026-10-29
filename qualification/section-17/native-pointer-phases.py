#!/usr/bin/env python3
"""[DEBUG-task17-pointer] Actual browser-focus callback timing, diagnostic only."""
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def load_base():
    spec = importlib.util.spec_from_file_location('phases', Path(__file__).with_name('native-input-phases.py'))
    base = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(base)
    return base


def patched_sources(base, original):
    originals, sources = original()
    replace = base.replace
    ui = 'crates/librairii-ui/ui/desktop.slint'
    originals[ui] = (ROOT / ui).read_text()
    sources[ui] = replace(originals[ui], '    callback search(string);',
                          '    callback search(string);\n    callback qualification-pointer-focus(int);')
    sources[ui] = replace(sources[ui], 'focus-entered => { FocusState.key = 31; }',
                          'focus-entered => { FocusState.key = 31; root.qualification-pointer-focus(31); }')
    view = 'crates/librairii-desktop/src/view.rs'
    sources[view] = replace(sources[view], 'thread_local! {\n    static INPUT_PHASES:',
                            'thread_local! {\n    static POINTER_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };\n    static INPUT_PHASES:')
    sources[view] = replace(sources[view], 'pub(crate) fn input_phase_save', '''pub(crate) fn pointer_count() -> u64 {
    POINTER_COUNT.with(std::cell::Cell::get)
}
pub(crate) fn pointer_focus(key: i32) {
    let sequence = POINTER_COUNT.with(|count| { let next=count.get()+1; count.set(next); next });
    input_phase("pointer-focus", &format!("{sequence}:{key}"));
}
pub(crate) fn input_phase_save''')
    example = 'crates/librairii-desktop/examples/scale-native.rs'
    sources[example] = replace(sources[example], '    let view = view::install(&ui, worker.clone(), first)?;',
                               '    let view = view::install(&ui, worker.clone(), first)?;\n    ui.on_qualification_pointer_focus(view::pointer_focus);')
    sources[example] = replace(sources[example], '"query":ui.get_query().as_str()',
                               '"pointer_focus_count":view::pointer_count(),"query":ui.get_query().as_str()')
    return originals, sources


def helper_source(base, original):
    text = original()
    replace = base.replace
    text = replace(text, 'func phaseCalibrate() throws {',
                   'var phasePointerInputs: [[String: Any]] = []\nfunc phaseCalibrate() throws {')
    text = replace(text, '"samples": phaseInputs]',
                   '"samples": phaseInputs, "pointer_samples": phasePointerInputs]')
    text = replace(text, '    try phaseFinish()\n    let target = try firstStoryPoint()', '    let target = try firstStoryPoint()')
    text = replace(text, '''        try focusSearch(); _ = try wait { active($0) }
        let start = DispatchTime.now().uptimeNanoseconds
        for type in [CGEventType.leftMouseDown, CGEventType.leftMouseUp] {''', '''        try focusSearch(); let phaseBefore = try wait { active($0) }
        guard let phaseBeforeCount = phaseBefore["pointer_focus_count"] as? UInt64,
            phaseBefore["query_focused"] as? Bool == true, phaseBefore["focus_key"] as? Int == 30 else {
            throw Refusal(message: "pointer sample requires observed actual Search focus")
        }
        let start = DispatchTime.now().uptimeNanoseconds
        for type in [CGEventType.leftMouseDown, CGEventType.leftMouseUp] {''')
    text = replace(text, '        let end = try wait({ $0["focus_key"] as? Int == 31 }, seconds: 1)',
                   '        let phasePointerPosted = DispatchTime.now().uptimeNanoseconds\n        let end = try wait({ $0["focus_key"] as? Int == 31 }, seconds: 1)')
    text = replace(text, '        pointers.append(elapsed(start)); pointerStates.append(end)', '''        pointers.append(elapsed(start)); pointerStates.append(end)
        guard let phaseAfterCount = end["pointer_focus_count"] as? UInt64, phaseAfterCount == phaseBeforeCount+1 else {
            throw Refusal(message: "pointer focus callback count is ambiguous")
        }
        phasePointerInputs.append(["n": phasePointerInputs.count, "before_count": phaseBeforeCount, "after_count": phaseAfterCount,
            "start_ns": start, "posted_ns": phasePointerPosted, "observed_ns": DispatchTime.now().uptimeNanoseconds])''')
    text = replace(text, '    var navigation: [Double] = []; var endpoints:',
                   '    try phaseFinish()\n    var navigation: [Double] = []; var endpoints:')
    return text


def pointer_complete(output):
    path = output / 'diagnostic.json'
    if not path.exists():
        return False
    receipt = json.loads(path.read_text())
    complete = [c['traced'] for c in receipt['cohorts']] == [False, True, True]
    for cohort in receipt['cohorts']:
        if not cohort['traced']:
            continue
        destination = output / cohort['name']
        try:
            helper = json.loads((destination / 'helper-phases.json').read_text())
            events = json.loads((destination / 'native-phases.json').read_text())
            focus = [e for e in events if e['stage'] == 'pointer-focus']
            counts = [sample['after_count'] for sample in helper['pointer_samples']]
            valid = (len(helper['pointer_samples']) == len(set(counts)) == 64
                     and len({e['query'] for e in focus}) == len(focus)
                     and all(sample['n'] == i and sample['after_count'] == sample['before_count']+1
                             and f"{sample['after_count']}:31" in {e['query'] for e in focus}
                             for i, sample in enumerate(helper['pointer_samples'])))
        except (OSError, ValueError, KeyError, TypeError) as error:
            valid = False
            cohort['pointer_phase_error'] = f'{type(error).__name__}: {error}'
        cohort['pointer_phases_complete'] = valid
        complete = complete and valid
    complete = complete and len(receipt['cohorts']) == 3
    receipt['pointer_phases_complete'] = complete
    receipt['complete'] = receipt['complete'] and complete
    path.write_text(json.dumps(receipt, indent=2) + '\n')
    return complete


def main():
    base = load_base()
    original_patch, original_helper = base.patched_sources, base.helper_source
    base.patched_sources = lambda: patched_sources(base, original_patch)
    base.helper_source = lambda: helper_source(base, original_helper)
    base.OUT = ROOT / 'target/intel-pointer-phases'
    complete = False
    try:
        base.main()
    finally:
        complete = pointer_complete(base.OUT)
    if not complete:
        raise ValueError('actual pointer-focus diagnostic incomplete')


if __name__ == '__main__':
    main()
