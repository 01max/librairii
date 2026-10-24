#!/usr/bin/env python3
"""Launch the minimal app archive with fresh HOME and denied development access."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import shutil
import socket
import subprocess
import tempfile
import traceback

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('clean_mac_native', HERE / 'check-native-macos.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def denied(result):
    return (type(result.get('returncode')) is int and result['returncode'] > 0
        and result.get('errors') == [] and result.get('timed_out') is False
        and 'Operation not permitted' in result.get('output', ''))


def inspect(package, archive, helper, output):
    manifest = json.loads((package / 'manifest.json').read_text())
    if (platform.system(), platform.machine()) != native.common.wayland.runtime.TARGETS[manifest['target']]:
        raise ValueError('matching native Mac package required')
    if platform.system() != 'Darwin' or os.environ.get('GITHUB_ACTIONS') != 'true':
        raise ValueError('disposable native macOS CI required')
    native.common.wayland.runtime.verified_binary(package, manifest)
    native.common.clean.resources.package_bytes(package, manifest)
    archive_id = native.common.clean.resources.archive_identity(archive, package, manifest)
    output.mkdir(parents=True)
    report = {'schema': 1, 'source': manifest['source'], 'target': manifest['target'],
        'binary_sha256': manifest['binary_sha256'], 'archive': archive_id, 'errors': [],
        'checks': {}, 'runs': {}, 'toolkit_selected': False,
        'scope': 'Process-isolated clean-runtime proof on identified native CI OS; fresh HOME/caches, '
            'denied source/toolchain/development-runtime reads and subprocess execution, denied network. '
            'Not an erased OS installation, Gatekeeper/notarization or product installer test.'}
    try:
        with tempfile.TemporaryDirectory(prefix='librairii-clean-mac-') as temporary:
            root = pathlib.Path(temporary).resolve()
            installed = root / package.name
            shutil.copytree(package, installed, symlinks=False)
            native.common.clean.resources.package_bytes(installed, manifest)
            binary = native.common.wayland.runtime.verified_binary(installed, manifest)
            home, cache = root / 'home', root / 'tmp'
            home.mkdir(); cache.mkdir()
            forbidden = {str(HERE.parent.parent.resolve()), str(pathlib.Path.home() / '.cargo'),
                str(pathlib.Path.home() / '.rustup'), '/Applications/Xcode.app', '/Library/Developer',
                '/opt/homebrew', '/usr/local/lib', '/usr/local/opt'}
            for key in ('CARGO_HOME', 'RUSTUP_HOME', 'RUNNER_TOOL_CACHE', 'DEVELOPER_DIR'):
                if os.environ.get(key):
                    forbidden.add(str(pathlib.Path(os.environ[key]).resolve()))
            # Parameterized paths are data, never interpolated into SBPL syntax.
            profile = output / 'runtime.sb'
            profile.write_text('(version 1)\n(allow default)\n(deny network*)\n'
                '(deny process-exec)\n(allow process-exec (literal (param "APP")) '
                '(literal "/bin/cat") (literal "/usr/bin/true") (literal "/usr/bin/nc"))\n'
                + ''.join('(deny file-read* (subpath (param "DENY' + str(i) + '")))\n'
                    for i in range(len(forbidden))))
            parameters = ['/usr/bin/sandbox-exec', '-f', str(profile), '-D', 'APP=' + str(binary)]
            parameters += [part for i, path in enumerate(sorted(forbidden)) for part in ('-D', f'DENY{i}={path}')]
            env = {'HOME': str(home), 'TMPDIR': str(cache) + '/', 'PATH': '/usr/bin:/bin',
                'LANG': 'en_US.UTF-8', 'SLINT_BACKEND': 'winit-skia-software',
                'SLINT_DEBUG_PERFORMANCE': 'refresh_lazy,console'}
            report['isolation'] = {'forbidden_read_roots': sorted(forbidden), 'environment': env,
                'allowed_executables': [str(binary), '/bin/cat', '/usr/bin/true', '/usr/bin/nc'],
                'all_other_exec_denied': True, 'network_denied': True,
                'profile_sha256': hashlib.sha256(profile.read_bytes()).hexdigest()}
            controls = {}
            for name, command in {
                'system-read': ['/bin/cat', '/System/Library/CoreServices/SystemVersion.plist'],
                'source-read': ['/bin/cat', str(HERE / 'Cargo.toml')],
                'developer-exec': ['/usr/bin/clang', '--version'],
            }.items():
                controls[name] = native.common.wayland.command(parameters + command, env,
                    output / (name + '.log'), timeout=5)
            report['controls'] = controls
            report['checks']['positive_system_read'] = native.common.wayland.passed(controls['system-read'])
            report['checks']['source_read_denied'] = denied(controls['source-read'])
            report['checks']['developer_execution_denied'] = denied(controls['developer-exec'])
            with socket.socket() as listener:
                listener.bind(('127.0.0.1', 0)); listener.listen(2)
                connect = ['/usr/bin/nc', '-z', '-G', '1', '127.0.0.1', str(listener.getsockname()[1])]
                controls['network-positive'] = native.common.wayland.command(connect, env, output / 'network-positive.log', timeout=5)
                controls['network-denied'] = native.common.wayland.command(parameters + connect, env, output / 'network-denied.log', timeout=5)
            report['checks']['actual_network_denied'] = (native.common.wayland.passed(controls['network-positive'])
                and denied(controls['network-denied']))
            assert all(report['checks'].values()), 'isolation controls failed'
            for backend in ('winit-skia', 'winit-skia-software'):
                for repetition in range(2):
                    name = backend + '-' + str(repetition)
                    evidence = output / name
                    evidence.mkdir()
                    images = evidence / 'frames'
                    images.mkdir()
                    child = subprocess.Popen(parameters + [str(binary), '--renderer-smoke=' + str(images),
                        '--clean-runtime-probe'], env={**env, 'SLINT_BACKEND': backend},
                        cwd=home, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
                    capture = native.capture_output(child, evidence / 'application.log')
                    observer = native.Observer(helper, evidence, child.pid)
                    run = report['runs'][name] = {'returncode': None, 'errors': []}
                    try:
                        def window():
                            if child.poll() is not None:
                                raise RuntimeError('isolated app exited before a native window was observed')
                            value = observer.status()
                            return value if any(w.get('kCGWindowAlpha', 0) > 0 for w in value['windows']) else None
                        run['native_window'] = observer.wait(window)
                        observer.capture('native-window', run['native_window'])
                        run['returncode'] = child.wait(timeout=10)
                    except Exception:
                        run['errors'].append(traceback.format_exc())
                    finally:
                        run['errors'] += native.retire(child, capture)
                    text = (evidence / 'application.log').read_text()
                    run['checks'] = {'normal_exit': run['returncode'] == 0 and not run['errors'],
                        'actual_native_window': bool(run.get('native_window')),
                        'component_render_and_input': 'renderer_smoke=passed;' in text,
                        'fixture_retired': 'clean_runtime_complete; workers_joined=true; fixture_retired=true' in text,
                        'two_distinct_frames': all((images / p).is_file() for p in ('multilingual.png', 'filtered.png'))
                            and (images / 'multilingual.png').read_bytes() != (images / 'filtered.png').read_bytes()}
                    if backend.endswith('-software'):
                        run['checks']['actual_cpu_surface'] = 'skia backend software;' in text
                    report['checks'][name] = all(run['checks'].values())
                    assert report['checks'][name], 'isolated native package launch/relaunch failed'
            # Fixture roots live under this empty per-run TMPDIR and must retire.
            report['checks']['no_remaining_fixture'] = not list(cache.glob('librairii-qualification-*'))
            report['checks']['fresh_home_no_development_cache'] = not any((home / p).exists() for p in ('.cargo', '.rustup'))
        report['checks']['isolation_root_retired'] = not root.exists()
    except Exception:
        report['errors'].append(traceback.format_exc())
    report['result'] = 'pass' if len(report['runs']) == 4 and report['checks'] and all(report['checks'].values()) and not report['errors'] else 'fail'
    files = [p for p in output.rglob('*') if p.is_file()]
    if len(files) > 128 or any(p.stat().st_size > native.LIMIT for p in files):
        report['errors'].append('bounded evidence limit')
        report['result'] = 'fail'
    report['file_hashes'] = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    (output / 'clean-macos.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('package', 'archive', 'helper', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    args = parser.parse_args()
    result = inspect(args.package.resolve(strict=True), args.archive.resolve(strict=True), args.helper.resolve(strict=True), args.output.resolve())
    print(json.dumps({'result': result['result'], 'errors': result['errors']}))
    raise SystemExit(0 if result['result'] == 'pass' else 1)
