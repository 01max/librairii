"""Private clean Linux desktop: no source checkout, build tools or network."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys

sys.path.insert(0, '/checks')
from packaged_acceptance.run import qualified, run


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--inspection', type=Path, required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--case', choices=('fresh', 'upgraded'), required=True)
    parser.add_argument('--backend', choices=('winit-skia', 'winit-skia-software'), required=True)
    parser.add_argument('--session', choices=('x11', 'wayland'), required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise ValueError('disposable CI host required')
    tools = {name: shutil.which(name) for name in ('rustc', 'cargo', 'go', 'node', 'gcc', 'g++', 'clang', '7z')}
    if any(tools.values()):
        raise ValueError('clean runtime image contains a development/archive tool: '+str(tools))
    work = Path('/work/target/native-release')
    desktop_output = work/'desktop'
    desktop_output.mkdir()
    spec = importlib.util.spec_from_file_location('packaged_private_desktop', '/checks/desktop.py')
    desktop_module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(desktop_module)
    desktop = desktop_module.Desktop(args.session, desktop_output)
    try:
        # AccessKit registers only after the private session enables semantic
        # accessibility. Match the existing native accessibility qualification.
        status = desktop.dbus.Interface(desktop.bus.get_object('org.a11y.Bus', '/org/a11y/bus'),
                                        'org.freedesktop.DBus.Properties')
        status.Set('org.a11y.Status', 'IsEnabled', desktop.dbus.Boolean(True))
        enabled = bool(status.Get('org.a11y.Status', 'IsEnabled'))
        (desktop_output/'accessibility-activation.json').write_text(json.dumps(
            {'private_session': True, 'semantic_accessibility_enabled': enabled})+'\n')
        if not enabled:
            raise ValueError('private native accessibility did not activate')
        portal = Path(os.environ['XDG_CONFIG_HOME'])/'xdg-desktop-portal/portals.conf'
        portal.parent.mkdir(parents=True)
        portal.write_text('[preferred]\ndefault=gtk\norg.freedesktop.impl.portal.FileChooser=gtk\n')
        desktop.run(['gio', 'mime', 'inode/directory', 'thunar.desktop'])
        report = run(Path('/work'), args.inspection, args.source, 'x86_64-unknown-linux-gnu',
                     'target/native-release/case', args.case, args.backend, desktop=desktop,
                     fixture_source=Path('/fixtures'), observer_source=Path('/checks'),
                     method=json.loads(Path('/checks/method.json').read_text()))
        if not qualified(report):
            raise ValueError('installed GUI report is incomplete')
    finally:
        desktop.close()
        (work/'clean-host.json').write_text(json.dumps({'development_tools': tools,
            'network': 'Docker network=none, numeric loopback fixture only',
            'desktop_processes_retired': all(child.poll() is not None for child in desktop.children),
            'observer_overflow': desktop.overflow}, indent=2)+'\n')
    if desktop.overflow:
        raise ValueError('private desktop observer overflow or failed retirement')


if __name__ == '__main__':
    main()
