"""Record an Orca baseline in a private desktop; never infer full accessibility."""
import argparse
import array
import hashlib
import importlib.util
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

spec = importlib.util.spec_from_file_location('native', pathlib.Path(__file__).with_name('native-drive.py'))
native = importlib.util.module_from_spec(spec)
# Unit tests import the source tree, while the container has the copied context.
if not pathlib.Path(spec.origin).exists():
    spec = importlib.util.spec_from_file_location('native', pathlib.Path(__file__).parents[1] / 'native-linux/drive.py')
    native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
EVIDENCE_LIMITS = {'speech.pcm': 8 * 1024 * 1024, 'speech.wav': 8 * 1024 * 1024,
    'orca-debug.txt': 16 * 1024 * 1024}


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


def named(nodes, name, role=None):
    found = [n for n in nodes if n.get('name') == name and (role is None or n.get('role') == role)]
    return found[0] if len(found) == 1 else None


def selected_story(nodes, name):
    selected = [n for n in nodes if n.get('role') == 'list item'
        and n.get('states', {}).get('selected') and n.get('states', {}).get('showing')]
    return selected[0] if len(selected) == 1 and selected[0].get('name') == name else None


def active_progress(nodes):
    return next((n for n in nodes if n.get('role') == 'progress bar'
        and n.get('minimum_value') == 0 and n.get('maximum_value') == 1
        and 0 < n.get('numeric_value', -1) < 1), None)


def modal_controls_confined(nodes):
    buttons = [n for n in nodes if n.get('role') in ('push button', 'check box')
        and n.get('states', {}).get('showing')]
    dialog_names = {'English', 'Français', 'Close'}
    dialog = [n for n in buttons if n['name'] in dialog_names]
    background = [n for n in buttons if n['name'] not in dialog_names]
    return (set(n['name'] for n in dialog) == dialog_names
        and all(n['states'].get('enabled') is True and n['states'].get('sensitive') is True for n in dialog)
        and {'Settings', 'List'} <= {n['name'] for n in background}
        and all(n['states'].get('enabled') is False and n['states'].get('sensitive') is False for n in background)
        and len(focused(nodes)) == 1 and focused(nodes)[0]['name'] == 'Close')


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
    frame_speech = speech_marker()
    before_fixtures = set(pathlib.Path('/tmp').glob('librairii-qualification-*'))
    report['application_environment'] = {key: os.environ.get(key) for key in
        ('DISPLAY', 'WAYLAND_DISPLAY', 'GDK_BACKEND', 'SLINT_BACKEND')}
    app = desktop.start('application', [str(binary), '--size=1280x900', '--slow-synthetic-work'], debug=True)
    report['launch_focus_observations'] = []
    def foreground_ready():
        if app.poll() is not None:
            raise RuntimeError('application exited before acquiring a native window')
        try:
            return desktop.active_pid() == app.pid
        except subprocess.CalledProcessError as error:
            # Before the first X11 window, _NET_ACTIVE_WINDOW may not exist.
            # Poll only this launch-readiness witness within its existing bound;
            # every later key still requires an immediate exact foreground PID.
            if len(report['launch_focus_observations']) < 16:
                report['launch_focus_observations'].append(str(error) + ': ' + str(error.output)[:1000])
            return False
    native.wait_for(foreground_ready)
    fixtures = native.wait_for(lambda: set(pathlib.Path('/tmp').glob('librairii-qualification-*')) - before_fixtures)
    assert len(fixtures) == 1, 'owned fixture is not unique'
    report['fixtures'] = [str(path) for path in fixtures]
    accessible = native.wait_for(lambda: desktop.find(lambda n: n.getRole() == desktop.a.ROLE_APPLICATION
        and n.get_process_id() == app.pid))
    report['application_pid'] = app.pid
    announced('frame', 'Librairii · qualification frame.', frame_speech)
    report['snapshots'] = {}
    def snapshot(label):
        nodes = tree(desktop, accessible)
        path = root / (label + '.json')
        path.write_text(json.dumps(nodes, ensure_ascii=False, indent=2) + '\n')
        report['snapshots'][label] = {'nodes': len(nodes), 'focused': focused(nodes),
            'observation_errors': [n['observation_error'] for n in nodes if 'observation_error' in n],
            'audio_byte_offset': pcm.stat().st_size}
        return nodes
    initial = snapshot('initial')
    assert not any(n.get('name', '').startswith('0999 ·') for n in initial), 'last story was not initially offscreen'
    # Observe keyboard navigation only. Do not call an accessibility action or
    # programmatically set focus/text to make a missing semantic witness pass.
    navigation = []
    search_speech = speech_marker()
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
    announced('search', 'Search stories entry.', search_speech)
    assert desktop.active_pid() == app.pid, 'text target lost foreground'
    desktop.text('0042')
    native.wait_for(lambda: any(n.get('text') == '0042' for n in tree(desktop, accessible)))
    snapshot('filtered')
    desktop.screenshot(root / 'filtered.png')
    settings_speech = speech_marker()
    key('Tab')
    settings_focus = snapshot('settings-invoker')
    assert any(n['name'] == 'Settings' for n in focused(settings_focus)), 'Settings invoker not keyboard-focused'
    announced('settings', 'Settings push button.', settings_speech)
    close_speech = speech_marker()
    key('space')
    native.wait_for(lambda: any(n['name'] == 'Close' for n in focused(tree(desktop, accessible))))
    modal = snapshot('settings-open')
    assert modal_controls_confined(modal), 'modal background controls are not disabled in AT-SPI'
    report['baseline_checks']['modal_background_disabled'] = True
    announced('close', 'Close push button.', close_speech)
    english_speech = speech_marker()
    key('Tab')
    snapshot('settings-tab')
    announced('english', 'English push button.', english_speech)
    restored_speech = speech_marker()
    key('Escape')
    native.wait_for(lambda: focused(tree(desktop, accessible)) == focused(settings_focus))
    snapshot('settings-restored')
    announced('restored', 'Settings push button.', restored_speech)
    def read():
        return tree(desktop, accessible)
    def tab_to(predicate, reverse=False, limit=80):
        navigation = []
        for _ in range(limit):
            current = focused(read())
            navigation.append(current)
            if len(current) == 1 and predicate(current[0]):
                report.setdefault('workflow_navigation', []).append(navigation)
                return current[0]
            key('shift+Tab' if reverse else 'Tab')
        report.setdefault('workflow_navigation', []).append(navigation)
        raise AssertionError('bounded native keyboard target not reached')
    def focus_name(name, reverse=False):
        return tab_to(lambda n: n.get('name') == name, reverse)
    def wait_named(name, role=None, **states):
        return native.wait_for(lambda: (n := named(read(), name, role))
            and all(n.get('states', {}).get(k) is v for k,v in states.items()) and n)

    # Native End must scroll to and select a row absent from the initial tree.
    focus_name('Search stories', reverse=True)
    key('ctrl+a')
    key('BackSpace')
    native.wait_for(lambda: (n := named(read(), 'Search stories')) and n.get('text') == '')
    tab_to(lambda n: n.get('role') == 'list item')
    end_speech = speech_marker()
    key('End')
    def end_selected():
        nodes = read()
        report['end_selection_nodes'] = nodes
        return next((n for n in focused(nodes) if n.get('name', '').startswith('0999 ·')
            and n['states'].get('selected')), None)
    selected = native.wait_for(end_selected)
    report['offscreen_selected_story'] = selected
    snapshot('offscreen-selected')
    desktop.screenshot(root / 'offscreen-selected.png')
    announced('story-end', {'contains': ['0999', 'Étoiles']}, end_speech)
    report['baseline_checks']['virtualized_offscreen_selection'] = True
    key('Home')
    key('Down')
    visible = native.wait_for(lambda: next((n for n in focused(read())
        if n.get('name', '').startswith('0001 ·') and n['states'].get('selected')), None))
    report['view_selected_story'] = visible
    assert selected_story(read(), visible['name'])
    snapshot('visible-selected')
    focus_name('Small covers', reverse=True)
    key('space')
    native.wait_for(lambda: selected_story(read(), visible['name']))
    snapshot('covers-selection-preserved')
    focus_name('List', reverse=True)
    key('space')
    native.wait_for(lambda: selected_story(read(), visible['name']))
    snapshot('list-selection-preserved')
    report['baseline_checks']['view_preserved_selection'] = True

    # Default unchecked, two pages, explicit Apply, then reset: no implicit Add.
    focus_name('Review')
    key('space')
    first = 'Add 0 · Otherwise: Not now'
    second = 'Add 25 · Otherwise: Not now'
    wait_named(first, 'check box', checked=False)
    wait_named('Apply 0', 'push button', enabled=False)
    focus_name(first)
    check_speech = speech_marker()
    key('space')
    wait_named(first, 'check box', checked=True)
    wait_named('Apply 1', 'push button', enabled=True)
    snapshot('review-page-one-checked')
    announced('review-checked-0', {'words': ['checked'], 'excludes': ['not checked', 'unchecked']}, check_speech)
    focus_name('Next', reverse=True)
    key('space')
    wait_named(second, 'check box', checked=False)
    focus_name(second)
    check_speech = speech_marker()
    key('space')
    wait_named(second, 'check box', checked=True)
    wait_named('Apply 2', 'push button', enabled=True)
    snapshot('review-page-two-checked')
    announced('review-checked-25', {'words': ['checked'], 'excludes': ['not checked', 'unchecked']}, check_speech)
    focus_name('Previous', reverse=True)
    key('space')
    wait_named(first, 'check box', checked=True)
    snapshot('review-page-one-restored')
    focus_name('Apply 2')
    apply_speech = speech_marker()
    key('space')
    wait_named(first, 'check box', checked=False)
    wait_named('Apply 0', 'push button', enabled=False)
    snapshot('review-applied')
    announced('review-applied', {'contains': ['2 synthetic decisions']}, apply_speech)
    report['baseline_checks']['review_checked_paging_apply'] = True

    # Actual progress, enabled/disabled actions, Cancel, completion, error/retry.
    focus_name('Synthetic work')
    cancel_speech = speech_marker()
    key('space')
    progress = native.wait_for(lambda: active_progress(read()))
    report['cancel_progress'] = progress
    wait_named('Synthetic work', 'push button', enabled=False)
    wait_named('Cancel', 'push button', enabled=True)
    focus_name('Cancel')
    snapshot('work-before-cancel')
    key('space')
    wait_named('Synthetic work', 'push button', enabled=True)
    wait_named('Cancel', 'push button', enabled=False)
    snapshot('work-cancelled')
    announced('work-cancelled', {'contains': ['Synthetic work cancelled']}, cancel_speech)
    report['baseline_checks']['work_progress_and_cancel'] = True
    focus_name('Synthetic work', reverse=True)
    complete_speech = speech_marker()
    key('space')
    native.wait_for(lambda: active_progress(read()))
    announced('work-completed', {'contains': ['Synthetic work completed']}, complete_speech)
    snapshot('work-completed')
    completed_output = debug.read_bytes()[complete_speech['debug_offset']:].decode('utf-8', errors='replace')
    spoken = re.findall(r"SPEECH OUTPUT: '([^']*)' \{", completed_output)
    report['progress_utterances'] = [s for s in spoken if re.search(r'\d+\s*(?:percent|%)', s)]
    assert report['progress_utterances'], 'no actual Orca percentage announcement'
    assert sum('Synthetic work completed' in s for s in spoken) == 1, 'terminal speech flooded'
    focus_name('Synthetic error')
    error_speech = speech_marker()
    key('space')
    announced('synthetic-error', {'contains': ['Synthetic failure', 'retry']}, error_speech)
    snapshot('synthetic-error')
    focus_name('Synthetic work', reverse=True)
    retry_speech = speech_marker()
    key('space')
    native.wait_for(lambda: active_progress(read()))
    announced('retry-completed', {'contains': ['Synthetic work completed']}, retry_speech)
    snapshot('retry-completed')
    report['baseline_checks']['work_completion_error_retry'] = True
    # Give real speech delivery time to finish before stopping its sink recorder.
    time.sleep(2)
    report['orca_application_output'] = debug.read_bytes()[before:].decode('utf-8', errors='replace')
    report['baseline_checks'].update({'app_accessibility_tree': all(not s['observation_errors']
            for s in report['snapshots'].values()), 'keyboard_search': True,
        'keyboard_settings_and_focus_return': True,
        'orca_search_label_completed': 'search' in report['speech_steps'],
        'orca_settings_labels_completed': {'settings', 'close', 'english', 'restored'} <= report['speech_steps'].keys(),
        'orca_spoke_search_or_settings': any('SPEECH OUTPUT:' in line and
            any(word in line.lower() for word in ('search', 'settings', 'close'))
            for line in report['orca_application_output'].splitlines()),
        'speech_services_alive': orca.poll() is None and dispatcher.poll() is None and audio.poll() is None})
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
        'observation_method': 'native-keyboard-semantic-workflows-and-completed-audio-v5',
        'scope': 'Private Orca/eSpeak-NG speech and AT-SPI baseline; keyboard Search and Settings only. '
            'X11 action-key echo is disabled in this private reader profile; its default-echo route stays unqualified. '
            'Native virtualized End/selection, view preservation, review paging/Apply, modal state, '
            'progress/cancel/completion/error/retry are required. Final toolkit review remains separate.'}
    desktop, guard = None, None
    stop = threading.Event()
    try:
        desktop = native.Desktop(args.session, root)
        def watch():
            while not stop.wait(0.1):
                try:
                    files = list(root.rglob('*'))
                    if len(files) <= 512 and all(not p.is_file() or p.stat().st_size <= evidence_limit(p) for p in files):
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
