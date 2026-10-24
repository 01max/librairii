"""Record an Orca baseline in a private desktop; never infer full accessibility."""
import argparse
import array
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess
import threading
import time
import traceback
import wave

spec = importlib.util.spec_from_file_location('native', pathlib.Path(__file__).with_name('native-drive.py'))
native = importlib.util.module_from_spec(spec)
# Unit tests import the source tree, while the container has the copied context.
if not pathlib.Path(spec.origin).exists():
    spec = importlib.util.spec_from_file_location('native', pathlib.Path(__file__).parents[1] / 'native-linux/drive.py')
    native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def start_audio(desktop, argv):
    # stderr is diagnostic text, never PCM. Keep independent bounded streams.
    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    desktop.children.append(child)
    def capture(stream, name):
        try:
            size = 0
            with stream, (desktop.root / name).open('xb') as output:
                while data := stream.read1(65536):
                    output.write(data[:max(0, native.LIMIT - size)])
                    output.flush()
                    size += len(data)
                    if size >= native.LIMIT:
                        desktop.overflow.append(name + ': output limit')
                        child.kill()
                        break
        except Exception as error:
            desktop.overflow.append(name + ': capture failure: ' + str(error))
            if child.poll() is None:
                child.kill()
    for stream, name in ((child.stdout, 'speech.pcm'), (child.stderr, 'speech-audio-stderr.txt')):
        reader = threading.Thread(target=capture, args=(stream, name))
        reader.start()
        desktop.logs.append(reader)
    return child


def audio_facts(data):
    if len(data) % 2:
        raise ValueError('partial PCM sample')
    samples = array.array('h')
    samples.frombytes(data)
    if os.sys.byteorder != 'little':
        samples.byteswap()
    return {'format': 's16le', 'sample_rate': 16000, 'channels': 1,
        'samples': len(samples), 'nonzero_samples': sum(value != 0 for value in samples),
        'peak': max((abs(value) for value in samples), default=0),
        'duration_seconds': len(samples) / 16000}


def tree(desktop, app):
    result = []
    for node in desktop.nodes(app):
        try:
            states = node.getState()
            row = {'name': str(node.name), 'role': node.getRoleName(),
                'states': {name: states.contains(getattr(desktop.a, 'STATE_' + name.upper()))
                    for name in ('focused', 'focusable', 'enabled', 'showing', 'selected', 'checked', 'editable')}}
            if states.contains(desktop.a.STATE_EDITABLE):
                row['text'] = node.queryText().getText(0, -1)
            result.append(row)
        except Exception as error:
            result.append({'observation_error': type(error).__name__ + ': ' + str(error)})
    return result


def focused(nodes):
    return [n for n in nodes if n.get('states', {}).get('focused')]


def record(desktop, binary, report):
    root, runtime = desktop.root, desktop.runtime
    os.environ['HOME'] = str(runtime / 'outside')
    report['versions'] = desktop.run(['dpkg-query', '-W', '-f=${Package} ${Version}\n',
        'orca', 'at-spi2-core', 'speech-dispatcher', 'speech-dispatcher-espeak-ng', 'pulseaudio'])
    pulse = runtime / 'pulse'
    pulse.mkdir(mode=0o700)
    os.environ['PULSE_SERVER'] = 'unix:' + str(pulse / 'native')
    desktop.start('pulse', ['pulseaudio', '-n', '--daemonize=no', '--exit-idle-time=-1',
        '--load=module-native-protocol-unix socket=' + str(pulse / 'native') + ' auth-anonymous=1',
        '--load=module-null-sink sink_name=qualification rate=16000 channels=1'])
    native.wait_for(lambda: (pulse / 'native').is_socket())
    report['pulse_sinks'] = desktop.run(['pactl', 'list', 'short', 'sinks'])
    assert 'qualification' in report['pulse_sinks']
    speech = runtime / 'speech'
    (speech / 'modules').mkdir(parents=True)
    shutil.copyfile('/etc/speech-dispatcher/modules/espeak-ng.conf', speech / 'modules/espeak-ng.conf')
    (speech / 'speechd.conf').write_text('LogLevel 4\nLogDir "stdout"\n'
        'CommunicationMethod "unix_socket"\nSocketPath "' + str(speech / 'speechd.sock') + '"\n'
        'Timeout 0\nAudioOutputMethod "pulse"\nAudioPulseDevice "qualification"\n'
        'AddModule "espeak-ng" "sd_espeak-ng" "espeak-ng.conf"\nDefaultModule "espeak-ng"\n')
    os.environ['SPEECHD_ADDRESS'] = 'unix_socket:' + str(speech / 'speechd.sock')
    dispatcher = desktop.start('speech-dispatcher', ['speech-dispatcher', '--run-single',
        '--config-dir', str(speech), '--log-dir', 'stdout', '--pid-file', str(speech / 'speechd.pid')])
    native.wait_for(lambda: (speech / 'speechd.sock').is_socket())
    # Capture the actual synthesizer sink, without a host audio device or service.
    audio = start_audio(desktop, ['parec', '--device=qualification.monitor',
        '--format=s16le', '--rate=16000', '--channels=1', '--raw'])
    prefs = runtime / 'orca'
    prefs.mkdir()
    (prefs / 'orca-customizations.py').write_text('from orca import debug, settings\n'
        'debug.debugLevel = debug.LEVEL_INFO\nsettings.enableMouseReview = False\n'
        'settings.speakProgressBarUpdates = True\nsettings.progressBarUpdateInterval = 1\n')
    orca = desktop.start('orca', ['orca', '--user-prefs', str(prefs), '--enable=speech', '--disable=braille',
        '--debug-file', str(root / 'orca-debug.txt')])
    debug = root / 'orca-debug.txt'
    native.wait_for(lambda: debug.exists() and 'Starting Atspi main event loop' in debug.read_text())
    pcm = root / 'speech.pcm'
    def startup_speech_finished():
        data = pcm.read_bytes() if pcm.exists() else b''
        return (len(data) >= 16000 and audio_facts(data)['nonzero_samples'] > 0
            and audio_facts(data[-16000:])['nonzero_samples'] == 0)
    native.wait_for(startup_speech_finished)
    report['application_audio_start_offset'] = pcm.stat().st_size
    before = debug.stat().st_size
    before_fixtures = set(pathlib.Path('/tmp').glob('librairii-qualification-*'))
    app = desktop.start('application', [str(binary), '--size=1280x900'])
    native.wait_for(lambda: desktop.active_pid() == app.pid)
    fixtures = native.wait_for(lambda: set(pathlib.Path('/tmp').glob('librairii-qualification-*')) - before_fixtures)
    assert len(fixtures) == 1, 'owned fixture is not unique'
    report['fixtures'] = [str(path) for path in fixtures]
    accessible = native.wait_for(lambda: desktop.find(lambda n: n.getRole() == desktop.a.ROLE_APPLICATION
        and n.get_process_id() == app.pid))
    report['application_pid'] = app.pid
    report['snapshots'] = {}
    def snapshot(label):
        nodes = tree(desktop, accessible)
        path = root / (label + '.json')
        path.write_text(json.dumps(nodes, ensure_ascii=False, indent=2) + '\n')
        report['snapshots'][label] = {'nodes': len(nodes), 'focused': focused(nodes),
            'observation_errors': [n['observation_error'] for n in nodes if 'observation_error' in n],
            'audio_byte_offset': pcm.stat().st_size}
        return nodes
    snapshot('initial')
    # Observe keyboard navigation only. Do not call an accessibility action or
    # programmatically set focus/text to make a missing semantic witness pass.
    navigation = []
    def key(value):
        assert desktop.active_pid() == app.pid, 'keyboard target lost foreground'
        desktop.key(value)
    for _ in range(12):
        key('Tab')
        nodes = tree(desktop, accessible)
        current = focused(nodes)
        navigation.append(current)
        if any(n.get('states', {}).get('editable') for n in current):
            break
    report['tab_navigation'] = navigation
    assert navigation and any(n.get('states', {}).get('editable') for n in navigation[-1]), 'no keyboard-focused editable control'
    assert desktop.active_pid() == app.pid, 'text target lost foreground'
    desktop.text('0042')
    native.wait_for(lambda: any(n.get('text') == '0042' for n in tree(desktop, accessible)))
    snapshot('filtered')
    desktop.screenshot(root / 'filtered.png')
    key('Tab')
    settings_focus = snapshot('settings-invoker')
    assert any(n['name'] == 'Settings' for n in focused(settings_focus)), 'Settings invoker not keyboard-focused'
    key('space')
    native.wait_for(lambda: any(n['name'] == 'Close' for n in focused(tree(desktop, accessible))))
    snapshot('settings-open')
    key('Tab')
    snapshot('settings-tab')
    key('Escape')
    native.wait_for(lambda: focused(tree(desktop, accessible)) == focused(settings_focus))
    snapshot('settings-restored')
    # Give real speech delivery time to finish before stopping its sink recorder.
    time.sleep(2)
    report['orca_application_output'] = debug.read_bytes()[before:].decode('utf-8', errors='replace')
    report['baseline_checks'] = {'app_accessibility_tree': all(not s['observation_errors']
            for s in report['snapshots'].values()), 'keyboard_search': True,
        'keyboard_settings_and_focus_return': True,
        'orca_spoke_search_or_settings': any('SPEECH OUTPUT:' in line and
            any(word in line.lower() for word in ('search', 'settings', 'close'))
            for line in report['orca_application_output'].splitlines()),
        'speech_services_alive': orca.poll() is None and dispatcher.poll() is None and audio.poll() is None}
    key('ctrl+q')
    report['application_returncode'] = app.wait(timeout=10)
    report['baseline_checks']['normal_application_quit'] = app.returncode == 0
    report['baseline_checks']['owned_fixture_removed'] = all(not path.exists() for path in fixtures)
    audio.terminate()
    audio.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--session', choices=('x11', 'wayland'), required=True)
    args = parser.parse_args()
    root = pathlib.Path('/evidence')
    binary = pathlib.Path('/opt/librairii/qualification-desktop')
    assert os.uname().machine == 'x86_64' and os.geteuid() != 0
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    assert binary_sha == os.environ['EXPECTED_BINARY_SHA256']
    report = {'schema': 1, 'session': args.session, 'binary_sha256': binary_sha, 'errors': [],
        'baseline_checks': {}, 'baseline': 'fail', 'full_accessibility_qualified': False,
        'scope': 'Private Orca/eSpeak-NG speech and AT-SPI baseline; keyboard Search and Settings only. '
            'Virtualized selection, review, progress/error speech and complete pointer-free flows remain unqualified.'}
    desktop, guard = None, None
    stop = threading.Event()
    try:
        desktop = native.Desktop(args.session, root)
        def watch():
            while not stop.wait(0.1):
                try:
                    files = list(root.rglob('*'))
                    if len(files) <= 512 and all(not p.is_file() or p.stat().st_size <= native.LIMIT for p in files):
                        continue
                    reason = 'direct evidence output limit'
                except Exception as error:
                    reason = 'evidence guard failure: ' + str(error)
                desktop.overflow.append(reason)
                for child in desktop.children:
                    if child.poll() is None:
                        try:
                            child.kill()
                        except ProcessLookupError:
                            pass
                return
        guard = threading.Thread(target=watch)
        guard.start()
        record(desktop, binary, report)
    except Exception:
        report['errors'].append(traceback.format_exc())
    finally:
        if desktop:
            desktop.close()
            if desktop.overflow:
                report['errors'].append('bounded output failure: ' + repr(desktop.overflow))
        stop.set()
        if guard:
            guard.join(timeout=2)
            if guard.is_alive():
                report['errors'].append('unretired evidence guard')
        pcm = root / 'speech.pcm'
        if pcm.is_file():
            try:
                # The WAV and pass criterion exclude Orca's startup greeting.
                # Raw PCM and byte offsets retain the complete original capture.
                offset = report.get('application_audio_start_offset')
                data = pcm.read_bytes()[offset:] if offset is not None else b''
                report['audio'] = audio_facts(data)
                with wave.open(str(root / 'speech.wav'), 'wb') as wav:
                    wav.setnchannels(1)
                    wav.setsampwidth(2)
                    wav.setframerate(16000)
                    wav.writeframes(data)
                report['baseline_checks']['actual_synthesized_audio'] = report['audio']['nonzero_samples'] > 1600
            except (OSError, ValueError) as error:
                report['errors'].append('audio capture: ' + str(error))
        if (not report['errors'] and report['baseline_checks'] and all(report['baseline_checks'].values())):
            report['baseline'] = 'pass'
        (root / 'accessibility-session.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'session': args.session, 'baseline': report['baseline'], 'errors': report['errors']}))
    return 0 if report['baseline'] == 'pass' else 1


if __name__ == '__main__':
    raise SystemExit(main())
