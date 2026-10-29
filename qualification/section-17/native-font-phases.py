#!/usr/bin/env python3
"""[DEBUG-task17-fonts] Private pinned font-discovery phases, never qualification."""
import difflib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def load_base():
    spec = importlib.util.spec_from_file_location('init_phases', Path(__file__).with_name('native-startup-phases.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def patch_fontique(base, source):
    originals = {name: (source / name).read_text() for name in ['src/lib.rs', 'src/backend/mod.rs', 'src/backend/coretext.rs']}
    sources = dict(originals)
    sources['src/lib.rs'] += '''
// [DEBUG-task17-fonts] Read only after the ordinary startup marker.
#[cfg(all(feature = "system", target_vendor = "apple"))]
pub fn librairii_startup_phases() -> alloc::vec::Vec<(&'static str, u128)> {
    backend::librairii_startup_phases()
}
'''
    sources['src/backend/mod.rs'] += '''
#[cfg(all(feature = "system", target_vendor = "apple"))]
pub(crate) use system::librairii_startup_phases;
'''
    text = sources['src/backend/coretext.rs']
    text = base.replace(text, 'const DEFAULT_GENERIC_FAMILIES:', '''// [DEBUG-task17-fonts] Main-thread memory only; no I/O during discovery.
std::thread_local! {
    static FONT_PHASES: std::cell::RefCell<Vec<(&'static str, u128)>> = const { std::cell::RefCell::new(Vec::new()) };
}
pub(crate) fn librairii_startup_phases() -> Vec<(&'static str, u128)> {
    FONT_PHASES.with(|events| events.borrow().clone())
}
fn font_phase(stage: &'static str, start: std::time::Instant) {
    FONT_PHASES.with(|events| {
        let mut events = events.borrow_mut();
        assert!(events.len() < 64, "bounded private font phases");
        events.push((stage, start.elapsed().as_nanos()));
    });
}
const DEFAULT_GENERIC_FAMILIES:''')
    text = base.replace(text, '    // SAFETY: Calls into CoreText.', '    let start = std::time::Instant::now();\n    font_phase("scan-enter", start);\n    // SAFETY: Calls into CoreText.')
    text = base.replace(text, '    let descriptors = unsafe { collection.matching_font_descriptors()? };',
                        '    font_phase("collection-ready", start);\n    let descriptors = unsafe { collection.matching_font_descriptors()? };\n    font_phase("descriptors-ready", start);')
    text = base.replace(text, '    paths.extend(library_font_files());',
                        '    font_phase("descriptor-paths-ready", start);\n    paths.extend(library_font_files());\n    font_phase("directory-paths-ready", start);')
    text = base.replace(text, '    Some(scan::ScannedCollection::from_paths(paths.iter(), 0))',
                        '    let result = scan::ScannedCollection::from_paths(paths.iter(), 0);\n    font_phase("font-index-ready", start);\n    Some(result)')
    sources['src/backend/coretext.rs'] = text
    patch = ''.join(''.join(difflib.unified_diff(originals[name].splitlines(keepends=True), text.splitlines(keepends=True),
                    fromfile='a/' + name, tofile='b/' + name)) for name, text in sources.items())
    return sources, patch


def build(base, destination):
    # Materialize the exact archive pinned by Cargo.lock; never edit the shared
    # registry or a production vendor. Extract regular contained files only.
    cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    packages = [p for p in lock['package'] if p['name'] == 'fontique' and p['version'] == '0.11.1']
    assert len(packages) == 1
    archives = list((cargo_home / 'registry/cache').glob('*/fontique-0.11.1.crate'))
    assert len(archives) == 1 and base.sha(archives[0].read_bytes()) == packages[0]['checksum']
    vendor = destination.parent / 'fontique-source'
    vendor.mkdir(mode=0o700)
    checksum = {'package': packages[0]['checksum'], 'files': {}}
    with tarfile.open(archives[0], 'r:gz') as archive:
        for member in archive.getmembers():
            parts = Path(member.name).parts
            assert parts[0] == 'fontique-0.11.1' and len(parts) > 1 and '..' not in parts
            assert member.isfile() and not Path(member.name).is_absolute()
            relative = Path(*parts[1:])
            assert relative.as_posix() not in checksum['files']
            data = archive.extractfile(member).read()
            checksum['files'][relative.as_posix()] = base.sha(data)
            path = vendor / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    font_sources, font_patch = patch_fontique(base, vendor)
    (destination.parent / 'fontique-private.patch').write_text(font_patch)
    (destination.parent / 'fontique-registry-checksum.json').write_text(json.dumps(checksum, indent=2) + '\n')
    shutil.copyfile(archives[0], destination.parent / 'fontique-0.11.1.crate')
    for name, text in font_sources.items():
        (vendor / name).write_text(text)
    original_patch = base.patched_sources

    def patched_sources():
        originals, sources = original_patch()
        root_manifest = 'Cargo.toml'
        desktop_manifest = 'crates/librairii-desktop/Cargo.toml'
        for name in [root_manifest, desktop_manifest, 'Cargo.lock']:
            originals[name] = (ROOT / name).read_text()
            sources[name] = originals[name]
        sources[root_manifest] = base.replace(sources[root_manifest], '[patch.crates-io]',
            '[patch.crates-io]\nfontique = { path = "' + vendor.relative_to(ROOT).as_posix() + '" }')
        sources[desktop_manifest] = base.replace(sources[desktop_manifest], 'slint.workspace = true',
            'slint = { workspace = true, features = ["unstable-fontique-011"] }')
        main = 'crates/librairii-desktop/src/main.rs'
        sources[main] = base.replace(sources[main], '                    first_tick = false;',
            '                    eprintln!("native_font_phases; {}", serde_json::to_string(&slint::fontique_011::fontique::librairii_startup_phases()).expect("private font phases"));\n                    first_tick = false;')
        # Resolve only the local path replacement, then restore before returning
        # the complete patch. The actual build remains --locked.
        try:
            for name, text in sources.items():
                (ROOT / name).write_text(text)
            subprocess.run(['cargo', 'update', '--offline', '-p', 'fontique'], cwd=ROOT, check=True, timeout=120)
            sources['Cargo.lock'] = (ROOT / 'Cargo.lock').read_text()
            resolved = tomllib.loads(sources['Cargo.lock'])
            assert ([p for p in resolved['package'] if p['name'] != 'fontique']
                    == [p for p in lock['package'] if p['name'] != 'fontique'])
            replacements = [p for p in resolved['package'] if p['name'] == 'fontique']
            assert replacements == [{k: v for k, v in packages[0].items() if k not in ['source', 'checksum']}]
        finally:
            for name, text in originals.items():
                (ROOT / name).write_text(text)
        return originals, sources

    base.patched_sources = patched_sources
    try:
        return base.build(destination)
    finally:
        base.patched_sources = original_patch


def main():
    base = load_base()
    original_build = base.build
    # Preserve the reviewed eight-cohort execution and cleanup protocol.
    def font_build(destination):
        base.build = original_build
        try:
            return build(base, destination)
        finally:
            base.build = font_build
    base.build = font_build
    base.main()


if __name__ == '__main__':
    main()
