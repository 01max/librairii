"""Assemble native distributions from a verified stage of the current source."""
from pathlib import Path
import subprocess

from .archive import seal_artifact, tar_gzip
from .build import verify_stage
from .files import copy_file, copy_tree, digest, fresh_output, write_json
from .identity import clean_source, native_host, plan, verify_embedded
from . import linkage

BUNDLE_ID = 'io.github.librairii.native'


def inputs(root, stage):
    stage = Path(stage).resolve(strict=True)
    release = verify_stage(stage)
    current = plan(root, release['target'])
    if any(release.get(key) != value for key, value in current.items()):
        raise ValueError('package stage is not from the exact clean current source and release inputs')
    host = native_host(root, release)
    verify_embedded(root, stage / 'payload/bin/librairii-desktop', release)
    return stage, release, host


def linux_baseline(root):
    observed = dict(line.split('=', 1) for line in Path('/etc/os-release').read_text().splitlines()
                    if '=' in line and not line.startswith('#'))
    if observed.get('ID', '').strip('"') != 'ubuntu' or observed.get('VERSION_ID', '').strip('"') != '24.04':
        raise ValueError('Linux packages require the Ubuntu24.04 native build baseline')
    libc = subprocess.check_output(['getconf', 'GNU_LIBC_VERSION'], cwd=root, text=True).strip()
    if libc != 'glibc 2.39':
        raise ValueError('Linux packages require glibc2.39')
    return {'distribution': observed, 'libc': libc}


def requirements(target):
    if target.endswith('linux-gnu'):
        return {
            'baseline': 'Ubuntu 24.04 LTS x86-64; glibc 2.39',
            'packages': ['ca-certificates', 'libgcc-s1', 'libstdc++6', 'libfontconfig1', 'libfreetype6', 'libx11-6', 'libx11-xcb1',
                         'libxcursor1', 'libxi6', 'libxrandr2', 'libxkbcommon0', 'libxkbcommon-x11-0',
                         'libwayland-client0', 'libegl1', 'libgl1', 'libvulkan1', 'mesa-vulkan-drivers',
                         'libdbus-1-3', 'xdg-desktop-portal', 'xdg-desktop-portal-gtk', 'libglib2.0-bin'],
            'session': 'A running X11 or Wayland desktop, D-Bus session and working file-chooser portal are required.',
            'graphics': 'The default Winit/Skia backend uses the installed graphics stack; SLINT_BACKEND=winit-skia-software selects qualified CPU rendering.',
            'scope': 'These are declared prerequisites; installed GUI reports separately qualify each session and renderer.',
        }
    return {'baseline': 'macOS 15.0 or later on matching native hardware',
            'system_dependencies': 'AppKit, Foundation, CoreGraphics, Metal and system graphics/font services',
            'scope': 'OS frameworks are supplied by macOS. ARM and Intel packages are separate.'}


def finish(root, destination, artifact, payload, release, kind, signing):
    if clean_source(root) != release['source_commit'] or digest(root / 'Cargo.lock') != release['cargo_lock_sha256']:
        raise ValueError('source or locked graph changed during packaging')
    return seal_artifact(destination, artifact, payload, release, kind, signing)


def linux(root, stage, requested_output):
    stage, release, host = inputs(root, stage)
    if release['target'] != 'x86_64-unknown-linux-gnu':
        raise ValueError('Linux package requires the Linux stage')
    baseline = linux_baseline(root)
    destination = fresh_output(root, requested_output)
    payload = destination / 'package'
    copy_tree(stage / 'payload', payload)
    shared = payload / 'share/librairii'
    write_json(shared / 'runtime-requirements.json', requirements(release['target']))
    report = linkage.inspect(root, payload / 'bin/librairii-desktop', release['target'])
    write_json(shared / 'linkage.json', dict(report, host=host, baseline=baseline))
    entry = payload / 'share/applications' / (BUNDLE_ID + '.desktop')
    entry.parent.mkdir(parents=True)
    with entry.open('x') as stream:
        stream.write('[Desktop Entry]\nType=Application\nVersion=1.0\nName=Librairii\n'
                     'Comment=Manage and export your story library\nComment[fr]=Gérer et exporter votre bibliothèque\n'
                     'Exec=librairii-desktop\nTryExec=librairii-desktop\nIcon=' + BUNDLE_ID + '\n'
                     'Terminal=false\nCategories=AudioVideo;Education;\nStartupNotify=true\n')
    copy_file(shared / 'appicon.png', payload / 'share/icons/hicolor/1024x1024/apps' / (BUNDLE_ID + '.png'))
    copy_file(shared / 'appicon.svg', payload / 'share/icons/hicolor/scalable/apps' / (BUNDLE_ID + '.svg'))
    subprocess.run(['desktop-file-validate', str(entry)], cwd=root, check=True)
    with (payload / 'INSTALL.txt').open('x') as stream:
        stream.write('Librairii ' + release['version'] + ' — Ubuntu24.04 x86-64\n\n'
                     'Extract this versioned directory to a permanent location. Launch bin/librairii-desktop.\n'
                     'For desktop integration, copy bin/ and share/ into your chosen application prefix,\n'
                     'make its bin directory available in PATH and its share directory available in XDG_DATA_DIRS.\n'
                     'See share/librairii/runtime-requirements.json for required system packages and sessions.\n'
                     'UI, English/French catalogs and artwork fallbacks are embedded in the executable.\n'
                     'Update by replacing application files only. Uninstall removes this application directory\n'
                     'and its explicitly installed desktop/icon entries; retain your independent native profile,\n'
                     'library, backups and recovery journals. Reinstall and select the same library to reopen it.\n'
                     'This candidate is unsigned and requires separate installed GUI qualification.\n')
    name = 'Librairii-' + release['version'] + '-' + release['target']
    artifact = destination / (name + '.tar.gz')
    tar_gzip(payload, artifact, name, release['source_date_epoch'])
    return finish(root, destination, artifact, payload, release, 'tar.gz', {'state': 'not applicable (Linux portable archive)'})
