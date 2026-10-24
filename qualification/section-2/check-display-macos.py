#!/usr/bin/env python3
"""Observe real window resizing, palette contrast and localized native frames."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import subprocess
import time
import traceback

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('display_macos_native', HERE / 'check-native-macos.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def inspect(package, archive, helper, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if platform.system() != 'Darwin' or (platform.system(), platform.machine()) != native.common.wayland.runtime.TARGETS[manifest['target']]:
        raise ValueError('matching native Mac package required')
    binary = native.common.wayland.runtime.verified_binary(package, manifest)
    native.common.clean.resources.package_bytes(package, manifest)
    archive_id = native.common.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    app_dir = output / 'app'
    report = {'schema': 1, 'source': manifest['source'], 'target': manifest['target'],
        'binary_sha256': manifest['binary_sha256'], 'archive': archive_id, 'errors': [], 'checks': {}, 'sizes': [],
        'toolkit_selected': False, 'scope': 'Actual native AX window resizing and CG frames, localized Settings, '
            'long French inspector, actual public Palette contrast, stable nonessential-motion-free idle view. '
            'Mixed-scale movement is separately observed on the owned Wayland displays.'}
    child = subprocess.Popen([str(binary), '--native-probe=' + str(app_dir), '--native-case=input',
        '--french', '--display-long-text'], env={**os.environ, 'SLINT_BACKEND': 'winit-skia-software'},
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
    capture = native.capture_output(child, output / 'application.log')
    observer = native.Observer(helper, output, child.pid)
    fixture = None
    try:
        observer.wait(lambda: (app_dir / 'ready.txt').is_file())
        fixture = pathlib.Path((app_dir / 'fixture.txt').read_text())
        observer.call('activate')
        observer.wait(lambda: observer.status()['frontmost_pid'] == child.pid)
        report['native_preferences_and_screens'] = observer.call('display-preferences')
        def geometry():
            return json.loads((app_dir / 'geometry.json').read_text())
        def frame(name):
            request = app_dir / '.capture-request.tmp'
            request.write_text(name)
            request.replace(app_dir / 'capture-request')
            observer.wait(lambda: (app_dir / (name + '.png')).is_file())
            observer.capture(name)
        # AX obeys the actual screen's visible frame. Exercise compact and
        # intermediate sizes that fit even the hosted 1024x768 desktop first.
        report['ax_sizes'] = []
        for width, height in ((560, 400), (900, 600)):
            old = geometry()
            resize = observer.call('resize', width=width, height=height,
                content_width=old['logical_width'], content_height=old['logical_height'])
            value = observer.wait(lambda: (g := geometry()) and abs(g['logical_width'] - width) <= 1
                and abs(g['logical_height'] - height) <= 1 and g)
            report['ax_sizes'].append({'requested': [width, height], 'native_resize': resize, 'geometry': value})
            frame('ax-size-' + str(width))
        report['checks']['native_ax_resize'] = len(report['ax_sizes']) == 2
        # The public Slint Window API requests a real AppKit content size even
        # beyond the hosted screen. Retain full renderer frames and actual CG
        # captures separately: a screen crop does not establish a visible panel.
        for width, height in ((560, 400), (900, 760), (1280, 900)):
            request = app_dir / '.size-request.tmp'
            request.write_text(f'{width}x{height}')
            request.replace(app_dir / 'size-request')
            value = observer.wait(lambda: (g := geometry()) and abs(g['logical_width'] - width) <= 1
                and abs(g['logical_height'] - height) <= 1 and g['compact'] == (width < 850)
                and g['wide'] == (width >= 1180) and g)
            assert len(value['contrast_ratios']) == 3 and min(value['contrast_ratios']) >= 4.5
            report['sizes'].append({'requested': [width, height],
                'method': 'public Window.set_size; actual native geometry readback', 'geometry': value})
            frame('size-' + str(width))
            observer.call('key', key='cmd+comma')
            observer.wait(lambda: (s := native.state(app_dir)) and s[2] == '1' and s[4] == 'true')
            frame('settings-' + str(width))
            observer.call('key', key='escape')
            observer.wait(lambda: (s := native.state(app_dir)) and s[2] == '0')
            if width == 560:
                for _ in range(20):
                    if geometry()['focus_key'] == 10:
                        break
                    observer.call('key', key='tab')
                else:
                    raise AssertionError('compact Review not keyboard reachable')
                observer.call('key', key='space')
                observer.wait(lambda: geometry()['review_visible'])
                frame('compact-review')
                observer.call('key', key='space')
                observer.wait(lambda: not geometry()['review_visible'])
                report['checks']['compact_review_keyboard_reachable'] = True
        report['checks']['native_compact_intermediate_wide'] = len(report['sizes']) == 3
        report['checks']['localized_modal_focus_and_dismiss'] = True
        report['checks']['actual_text_control_selection_contrast'] = True
        # One native Tab removes the text caret; compare settled public-rendered
        # frames without changing focus/data between them. Color transitions may
        # settle, but the idle library has no decorative spatial animation.
        observer.call('key', key='tab')
        time.sleep(0.3)
        frame('motion-first')
        time.sleep(0.4)
        frame('motion-second')
        report['checks']['idle_without_nonessential_motion'] = (
            (app_dir / 'motion-first.png').read_bytes() == (app_dir / 'motion-second.png').read_bytes())
        assert all(report['checks'].values())
    except Exception:
        report['errors'].append(traceback.format_exc())
    finally:
        try:
            (app_dir / 'finish').touch()
            report['returncode'] = child.wait(timeout=10)
            report['checks']['normal_shutdown'] = child.returncode == 0 and (app_dir / 'retired.txt').is_file()
            report['checks']['fixture_retired'] = fixture is not None and not fixture.exists()
        except Exception:
            report['errors'].append(traceback.format_exc())
        report['errors'] += native.retire(child, capture)
    files = [p for p in output.rglob('*') if p.is_file()]
    if len(files) > 160 or any(p.stat().st_size > native.LIMIT for p in files):
        report['errors'].append('bounded display evidence limit')
    report['file_hashes'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    report['result'] = 'pass' if report['checks'] and all(report['checks'].values()) and not report['errors'] else 'fail'
    (output / 'display-macos.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('package', 'archive', 'helper', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    args = parser.parse_args()
    report = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.helper.resolve(strict=True), args.output.resolve())
    print(json.dumps({'result': report['result'], 'errors': report['errors']}))
    raise SystemExit(0 if report['result'] == 'pass' else 1)
