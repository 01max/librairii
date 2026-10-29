#!/usr/bin/env python3
"""[DEBUG-task17-init] Private in-memory startup phases; no attached profiler."""
import difflib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def replace(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def patched_sources():
    menus = 'crates/librairii-desktop/src/menus.rs'
    main = 'crates/librairii-desktop/src/main.rs'
    originals = {name: (ROOT / name).read_text() for name in [menus, main]}
    sources = dict(originals)
    sources[menus] = replace(sources[menus], 'type Result<T = ()>', '''// [DEBUG-task17-init] Main-thread memory only, printed after the ordinary marker.
pub fn startup_phase(stage: &'static str) {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let ns = ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_nanos();
    INIT_PHASES.with(|events| {
        let mut events = events.borrow_mut();
        assert!(events.len() < 64, "bounded private startup phase count");
        events.push((stage, ns));
    });
}
thread_local! {
    static INIT_PHASES: std::cell::RefCell<Vec<(&'static str, u128)>> = const { std::cell::RefCell::new(Vec::new()) };
}
pub fn startup_report() -> String {
    INIT_PHASES.with(|events| serde_json::to_string(&*events.borrow()).expect("bounded startup phases"))
}
type Result<T = ()>''')
    sources[menus] = replace(sources[menus], 'pub fn create_window() -> Result<(DesktopWindow, Menus)> {',
                            'pub fn create_window() -> Result<(DesktopWindow, Menus)> {\n    startup_phase("window-enter");')
    sources[menus] = replace(sources[menus], '''        slint::platform::set_platform(Box::new(
            i_slint_backend_winit::Backend::builder()
                .with_renderer_name(renderer)
                .with_default_menu_bar(false)
                .build()?,
        ))?;''', '''        startup_phase("backend-before");
        let backend = i_slint_backend_winit::Backend::builder()
            .with_renderer_name(renderer)
            .with_default_menu_bar(false)
            .build()?;
        startup_phase("backend-built");
        slint::platform::set_platform(Box::new(backend))?;
        startup_phase("platform-installed");''')
    sources[menus] = replace(sources[menus], '    let ui = DesktopWindow::new()?;',
                            '    let ui = DesktopWindow::new()?;\n    startup_phase("component-ready");')
    sources[menus] = replace(sources[menus], '    ui.set_platform_menu(cfg!(target_os = "macos"));',
                            '    ui.set_platform_menu(cfg!(target_os = "macos"));\n    startup_phase("menu-policy-ready");')
    sources[menus] = replace(sources[menus], '    let menus = Menus::install(ui.as_weak())?;',
                            '    let menus = Menus::install(ui.as_weak())?;\n    startup_phase("menus-ready");')
    sources[main] = replace(sources[main], '''            if let Some(ui) = weak.upgrade() {
                view.borrow_mut().resize(&ui);''', '''            if let Some(ui) = weak.upgrade() {
                if first_tick { menus::startup_phase("first-timer-before-work"); }
                view.borrow_mut().resize(&ui);''')
    sources[main] = replace(sources[main], '                if first_tick {\n                    let size',
                           '                if first_tick {\n                    menus::startup_phase("first-timer-after-work");\n                    let size')
    sources[main] = replace(sources[main], '                    first_tick = false;',
                           '                    eprintln!("native_init_phases; {}", menus::startup_report());\n                    first_tick = false;')
    sources[main] = replace(sources[main], '    let result = ui.run();',
                           '    menus::startup_phase("eventloop-before");\n    let result = ui.run();')
    return originals, sources


def build(destination):
    destination.mkdir(mode=0o700)
    originals, sources = patched_sources()
    patch = ''.join(''.join(difflib.unified_diff(originals[name].splitlines(keepends=True), text.splitlines(keepends=True),
                    fromfile='a/' + name, tofile='b/' + name)) for name, text in sources.items())
    (destination / 'private-source.patch').write_text(patch)
    receipt = {'scope': 'diagnostic only; in-memory phase pushes; print after ordinary first timer marker',
               'sources': {name: {'before': sha(originals[name].encode()), 'after': sha(text.encode())} for name, text in sources.items()},
               'patch_sha256': sha(patch.encode()), 'restored': False,
               'compiler': subprocess.check_output(['rustc', '-vV'], text=True)}
    metadata = destination / 'build.json'
    metadata.write_text(json.dumps(receipt, indent=2) + '\n')
    try:
        for name, text in sources.items():
            (ROOT / name).write_text(text)
        with (destination / 'compiler.jsonl').open('w') as events, (destination / 'build.log').open('w') as errors:
            subprocess.run(['cargo', 'build', '--locked', '--release', '--target', 'x86_64-apple-darwin',
                            '-p', 'librairii-desktop', '--bin', 'librairii-desktop', '--message-format=json'],
                           cwd=ROOT, stdout=events, stderr=errors, check=True, timeout=2400)
    finally:
        for name, text in originals.items():
            (ROOT / name).write_text(text)
        receipt['restored'] = all((ROOT / name).read_text() == text for name, text in originals.items())
        metadata.write_text(json.dumps(receipt, indent=2) + '\n')
    assert receipt['restored'] and not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT)
    events = [json.loads(line) for line in (destination / 'compiler.jsonl').read_text().splitlines()]
    artifacts = [e for e in events if e.get('reason') == 'compiler-artifact' and e.get('target', {}).get('name') == 'librairii-desktop'
                 and e.get('target', {}).get('kind') == ['bin'] and e.get('executable')]
    assert len(artifacts) == 1
    artifact = artifacts[0]
    profile = artifact['profile']
    assert profile['opt_level'] == '3' and not profile['debug_assertions'] and not profile['overflow_checks']
    source = Path(artifact['executable'])
    path = destination / 'librairii-desktop'
    expected = sha(source.read_bytes())
    with path.open('xb') as stream:
        stream.write(source.read_bytes())
        stream.flush()
        os.fsync(stream.fileno())
    path.chmod(0o700)
    assert path.stat().st_nlink == 1 and sha(path.read_bytes()) == sha(source.read_bytes()) == expected
    receipt.update(executable=str(source), profile=profile, binary_sha256=expected)
    metadata.write_text(json.dumps(receipt, indent=2) + '\n')
    return path


def main():
    assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
    spec = importlib.util.spec_from_file_location('startup', Path(__file__).with_name('native-startup-diagnostic.py'))
    startup = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(startup)
    startup.OUT = ROOT / 'target/intel-startup-phases'
    original_load = startup.load

    def load(name, path):
        module = original_load(name, path)
        if name != 'resources':
            return module
        # All retained artifacts have already been downloaded by this point.
        os.environ.pop('GH_TOKEN', None)
        os.environ.pop('GITHUB_TOKEN', None)
        traced = build(startup.OUT / 'phase-build')
        product_run = module.product_run
        index = 0
        plan = {'scope': 'diagnostic only; same eight treatment/renderer cohorts with alternating phase/control product',
                'phase_indices': [0, 2, 4, 6], 'control_indices': [1, 3, 5, 7],
                'phase_binary_sha256': sha(traced.read_bytes()), 'control_binary_sha256': startup.PINS['librairii-desktop']}
        (startup.OUT / 'phase-plan.json').write_text(json.dumps(plan, indent=2) + '\n')

        def run(binary, observer, destination, backend, helper, native):
            nonlocal index
            phase = index in plan['phase_indices']
            index += 1
            try:
                return product_run(traced if phase else binary, observer, destination, backend, helper, native)
            finally:
                assert sha(traced.read_bytes()) == plan['phase_binary_sha256']

        module.product_run = run
        return module

    startup.load = load
    startup.main()


if __name__ == '__main__':
    main()
