"""Actual Orca/eSpeakNG/Pulse PCM witnesses, qualified per-utterance method v5."""
import array

import hashlib

import json

import os

import pathlib

import re

import shutil

import signal

import subprocess

import threading

import time

import traceback

import wave

import desktop as native

EVIDENCE_LIMITS = {'speech.pcm': 64 * 1024 * 1024, 'speech.wav': 64 * 1024 * 1024, 'orca-debug.txt': 64 * 1024 * 1024}

def evidence_limit(path):
    return EVIDENCE_LIMITS.get(path.name, native.LIMIT)

def start_audio(desktop, argv):
    # stderr is diagnostic text, never PCM. Keep independent bounded streams.
    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    desktop.children.append(child)
    def capture(stream, name):
        try:
            size = 0
            with stream, (desktop.root / name).open('xb') as output:
                while data := stream.read1(65536):
                    limit = evidence_limit(desktop.root / name)
                    output.write(data[:max(0, limit - size)])
                    output.flush()
                    size += len(data)
                    if size >= limit:
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
                    for name in ('focused', 'focusable', 'enabled', 'sensitive', 'showing', 'selected', 'checked', 'editable')}}
            if states.contains(desktop.a.STATE_EDITABLE):
                row['text'] = node.queryText().getText(0, -1)
            if row['role'] == 'progress bar':
                value = node.queryValue()
                row['numeric_value'] = value.currentValue
                row['minimum_value'] = value.minimumValue
                row['maximum_value'] = value.maximumValue
            result.append(row)
        except Exception as error:
            result.append({'observation_error': type(error).__name__ + ': ' + str(error)})
    return result

def focused(nodes):
    return [n for n in nodes if n.get('states', {}).get('focused')]

def utterance_observed(log, expected):
    utterances = re.findall(r"SPEECH OUTPUT: '([^']*)' \{", log)
    if isinstance(expected, str):
        return expected in utterances
    text = ' '.join(utterances)
    return (isinstance(expected, dict) and bool(expected.get('contains') or expected.get('words'))
        and all(part in text for part in expected.get('contains', []))
        and all(re.search(r'\b' + re.escape(word) + r'\b', text, re.IGNORECASE)
            for word in expected.get('words', []))
        and not any(part.casefold() in text.casefold() for part in expected.get('excludes', [])))

def reader_locus_acknowledgement(log, target):
    """Observe Orca processing this current locus, never a name in the old locus."""
    compact = lambda value: ' '.join(value.replace('\\n', '\n').split())
    identity = compact('[' + target['role'] + ": '" + target['name'] + "']")
    requested = False
    acknowledgement = None
    for raw in log.splitlines():
        line = compact(raw)
        request = 'FOCUS MANAGER: Request to set locus of focus to '
        if request in line:
            current = line.split(request, 1)[1]
            requested = current == identity or current.startswith(identity + ' ')
            acknowledgement = None
        changing = 'FOCUS MANAGER: Changing locus of focus from '
        if changing in line:
            # Match the complete current object suffix; either label can contain ' to '.
            acknowledgement = raw if line.endswith(' to ' + identity + '. Notify: True') else None
            requested = False
        if requested and 'FOCUS MANAGER: Setting locus of focus to existing locus of focus' in line:
            acknowledgement = raw
    return acknowledgement

def await_reader_locus(desktop, target, marker):
    path = desktop.root / 'orca-debug.txt'
    def observed():
        data = path.read_bytes()
        acknowledgement = reader_locus_acknowledgement(
            data[marker['debug_offset']:].decode('utf-8', errors='replace'), target)
        if acknowledgement:
            return {'debug_offset': marker['debug_offset'], 'debug_end_offset': len(data),
                'name': target['name'], 'role': target['role'], 'acknowledgement': acknowledgement}
        return None
    return native.wait_for(observed)

def start(desktop, report):
    root, runtime = desktop.root, desktop.runtime
    os.environ['HOME'] = str(runtime / 'outside')
    report['versions'] = desktop.run(['dpkg-query', '-W', '-f=${Package} ${Version}\n',
        'orca', 'at-spi2-core', 'speech-dispatcher', 'speech-dispatcher-espeak-ng', 'pulseaudio', 'ca-certificates'])
    certificates = pathlib.Path('/etc/ssl/certs/ca-certificates.crt')
    assert certificates.is_file() and certificates.stat().st_size > 0
    report['platform_certificate_bundle'] = {'path': str(certificates), 'bytes': certificates.stat().st_size,
        'sha256': hashlib.sha256(certificates.read_bytes()).hexdigest()}
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
    (speech / 'speechd.conf').write_text('LogLevel 2\nLogDir "stdout"\n'
        'CommunicationMethod "unix_socket"\nSocketPath "' + str(speech / 'speechd.sock') + '"\n'
        'Timeout 0\nAudioOutputMethod "pulse"\nAudioPulseDevice "qualification"\n'
        'AddModule "espeak-ng" "sd_espeak-ng" "espeak-ng.conf"\nDefaultModule "espeak-ng"\n')
    os.environ['SPEECHD_ADDRESS'] = 'unix_socket:' + str(speech / 'speechd.sock')
    dispatcher = desktop.start('speech-dispatcher', ['speech-dispatcher', '--run-single',
        '--config-dir', str(speech), '--log-dir', 'stdout', '--pid-file', str(speech / 'speechd.pid')])
    desktop.dispatcher = dispatcher
    native.wait_for(lambda: (speech / 'speechd.sock').is_socket())
    # Capture the actual synthesizer sink, without a host audio device or service.
    audio = start_audio(desktop, ['parec', '--device=qualification.monitor',
        '--format=s16le', '--rate=16000', '--channels=1', '--raw'])
    prefs = runtime / 'orca'
    prefs.mkdir()
    trace_ready = root / 'orca-stack-handler-ready.txt'
    (prefs / 'orca-customizations.py').write_text('from orca import debug, settings\n'
        'import faulthandler, json, pathlib, signal, sys\n'
        'faulthandler.register(signal.SIGUSR1, file=sys.stderr, all_threads=True)\n'
        'pathlib.Path(' + repr(str(trace_ready)) + ').write_text("ready\\n")\n'
        'assert debug.debugFile is not None\n'
        'debug.debugFile.reconfigure(line_buffering=True, write_through=True)\n'
        'pathlib.Path(' + repr(str(root / 'orca-log-mode.json')) + ').write_text(json.dumps('
        '{"line_buffering": debug.debugFile.line_buffering, "write_through": debug.debugFile.write_through}))\n'
        'debug.debugLevel = debug.LEVEL_INFO\nsettings.enableMouseReview = False\n'
        'settings.speakProgressBarUpdates = True\nsettings.progressBarUpdateInterval = 1\n'
        + ('settings.enableActionKeys = False\n' if desktop.session == 'x11' else '')
        + 'from gi.repository import GLib\n'
        'def qualification_record_reader_preferences():\n'
        '    path = pathlib.Path(' + repr(str(root / 'orca-reader-preferences.json')) + ')\n'
        '    temporary = path.with_suffix(".tmp")\n'
        '    temporary.write_text(json.dumps({"enable_action_keys": settings.enableActionKeys}))\n'
        '    temporary.replace(path)\n'
        '    return False\n'
        'GLib.idle_add(qualification_record_reader_preferences)\n')
    orca_argv = ['orca', '--user-prefs', str(prefs), '--enable=speech', '--disable=braille',
        '--debug-file', str(root / 'orca-debug.txt')]
    if desktop.session == 'wayland':
        # Ubuntu's Orca46.1 unconditionally invokes xkbcomp using DISPLAY.
        # Only the reader receives this isolated XKB compatibility display;
        # the application still has no DISPLAY and Sway disables Xwayland.
        desktop.start('orca-xkb-display', ['Xvfb', ':92', '-screen', '0', '640x480x24', '-nolisten', 'tcp'])
        native.wait_for(lambda: pathlib.Path('/tmp/.X11-unix/X92').is_socket())
        orca_argv = ['env', 'DISPLAY=:92', 'GDK_BACKEND=x11', *orca_argv]
        report['orca_xkb_compatibility_display'] = ':92'
    orca = desktop.start('orca', orca_argv)
    desktop.reader = orca
    debug = root / 'orca-debug.txt'
    native.wait_for(lambda: debug.exists() and 'Starting Atspi main event loop' in debug.read_text())
    report['debug_log_configuration'] = json.loads((root / 'orca-log-mode.json').read_text())
    assert report['debug_log_configuration'] == {'line_buffering': True, 'write_through': True}
    preferences = root / 'orca-reader-preferences.json'
    native.wait_for(preferences.is_file)
    report['reader_preferences'] = json.loads(preferences.read_text())
    assert report['reader_preferences'] == {'enable_action_keys': desktop.session != 'x11'}
    pcm = root / 'speech.pcm'
    def startup_speech_finished():
        data = pcm.read_bytes() if pcm.exists() else b''
        return (len(data) >= 16000 and audio_facts(data)['nonzero_samples'] > 0
            and audio_facts(data[-16000:])['nonzero_samples'] == 0)
    native.wait_for(startup_speech_finished)
    report['application_audio_start_offset'] = pcm.stat().st_size
    before = debug.stat().st_size
    report['speech_steps'] = {}
    def speech_marker():
        return {'debug_offset': debug.stat().st_size, 'audio_offset': pcm.stat().st_size}
    def announced(name, text, marker):
        try:
            native.wait_for(lambda: utterance_observed(
                debug.read_bytes()[marker['debug_offset']:].decode('utf-8', errors='replace'), text))
        except Exception:
            # Failure-only diagnostics in this owned reader. They never replace
            # an utterance, change focus or extend the announcement deadline.
            diagnostic = {'stage': name, 'expected_utterance': text,
                          'handler_ready': trace_ready.is_file(), 'stack_observed': False}
            report['announcement_failure'] = diagnostic
            try:
                if diagnostic['handler_ready'] and orca.poll() is None:
                    log = root / 'orca.log'
                    offset = log.stat().st_size if log.exists() else 0
                    orca.send_signal(signal.SIGUSR1)
                    deadline = time.monotonic() + 1
                    while time.monotonic() < deadline:
                        if log.exists() and b'Current thread' in log.read_bytes()[offset:]:
                            diagnostic['stack_observed'] = True
                            break
                        time.sleep(0.01)
            except Exception as error:
                diagnostic['diagnostic_error'] = type(error).__name__ + ': ' + str(error)
            raise
        def completed():
            data = pcm.read_bytes()[marker['audio_offset']:]
            return (len(data) >= 16000 and audio_facts(data)['nonzero_samples'] > 1600
                and audio_facts(data[-16000:])['nonzero_samples'] == 0)
        native.wait_for(completed)
        end = pcm.stat().st_size
        report['speech_steps'][name] = {**marker, 'expected_utterance': text,
            'debug_end_offset': debug.stat().st_size, 'audio_end_offset': end,
            'audio': audio_facts(pcm.read_bytes()[marker['audio_offset']:end])}
    return speech_marker, announced, pcm, audio
