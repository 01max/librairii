"""A build or partial macOS summary cannot qualify missing native observations."""
import copy
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('macos', pathlib.Path(__file__).parents[1] / 'check-native-macos.py')
macos = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macos)


class NativeMacOS(unittest.TestCase):
    def test_pending_root_can_only_continue_a_read_only_wait_within_its_original_deadline(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        pending = {'returncode': 75, 'errors': [], 'output': json.dumps({
            'format': 'librairii.native-snapshot-unavailable', 'schema': 1,
            'pid': 123, 'attribute': 'AXWindows', 'status': -25204})}
        ready = {'returncode': 0, 'errors': [], 'output': '{"pid":123,"nodes":[{"role":"AXSheet"}]}'}
        now, calls = [0.0], []
        def command(argv, env, log, timeout):
            calls.append((argv[1], timeout))
            now[0] += min(2, timeout)
            return pending if len(calls) == 1 else ready
        def sleep(duration):
            now[0] += duration
        with mock.patch.object(macos.common.wayland, 'command', side_effect=command), \
                mock.patch.object(macos.time, 'monotonic', side_effect=lambda: now[0]), \
                mock.patch.object(macos.time, 'sleep', side_effect=sleep):
            self.assertEqual(observer.wait(lambda: observer.wait(
                lambda: observer.snapshot(summary=True), timeout=30))['nodes'], [{'role': 'AXSheet'}])
        self.assertEqual([action for action, _ in calls], ['snapshot', 'snapshot'])
        self.assertEqual(calls[0][1], 12)
        self.assertAlmostEqual(calls[1][1], 9.95)
        self.assertLess(now[0], 12)
        self.assertIsNone(macos._ACTIVE_WAIT.get())

    def test_persistent_pending_root_expires_without_a_successful_empty_snapshot(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        pending = {'returncode': 75, 'errors': [], 'output': json.dumps({
            'format': 'librairii.native-snapshot-unavailable', 'schema': 1,
            'pid': 123, 'attribute': 'AXWindows', 'status': -25204})}
        now, timeouts = [0.0], []
        def command(argv, env, log, timeout):
            self.assertEqual(argv[1], 'snapshot')
            self.assertGreater(timeout, 0)
            self.assertLessEqual(timeout, 12 - now[0])
            timeouts.append(timeout)
            now[0] += min(2, timeout)
            return pending
        with mock.patch.object(macos.common.wayland, 'command', side_effect=command), \
                mock.patch.object(macos.time, 'monotonic', side_effect=lambda: now[0]), \
                mock.patch.object(macos.time, 'sleep', side_effect=lambda duration: now.__setitem__(0, now[0] + duration)), \
                self.assertRaises(TimeoutError):
            observer.wait(lambda: observer.snapshot(summary=True))
        self.assertEqual(len(timeouts), 6)
        self.assertEqual(now[0], 12)
        self.assertIsNone(macos._ACTIVE_WAIT.get())

    def test_unfiltered_window_snapshot_uses_the_same_pending_read_protocol(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        pending = {'returncode': 75, 'errors': [], 'output': json.dumps({
            'format': 'librairii.native-snapshot-unavailable', 'schema': 1,
            'pid': 123, 'attribute': 'AXWindows', 'status': -25204})}
        ready = {'returncode': 0, 'errors': [], 'output': '{"pid":123,"nodes":[{"role":"AXWindow"}]}'}
        with mock.patch.object(macos.common.wayland, 'command', side_effect=[pending, ready]) as command, \
                mock.patch.object(macos.time, 'sleep'):
            self.assertEqual(observer.wait(lambda: observer.call('snapshot-window'))['nodes'], [{'role': 'AXWindow'}])
        self.assertEqual([call.args[0][1] for call in command.call_args_list], ['snapshot-window', 'snapshot-window'])

    def test_pending_root_protocol_refuses_wrong_identity_actions_filtered_reads_and_command_errors(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        reply = {'format': 'librairii.native-snapshot-unavailable', 'schema': 1,
                 'pid': 123, 'attribute': 'AXWindows', 'status': -25204}
        result = {'returncode': 75, 'errors': [], 'output': json.dumps(reply)}
        changes = [{'returncode': 1}, {'returncode': -9}, {'errors': ['deadline']},
            {'output': '{}'}, {'output': 'not JSON'},
            *({'output': json.dumps({**reply, **change})} for change in [
                {'pid': 456}, {'pid': True}, {'schema': True}, {'status': -25202},
                {'attribute': 'AXChildren'}, {'nodes': []}])]
        for change in changes:
            with self.subTest(change=change), mock.patch.object(macos.common.wayland, 'command',
                    return_value={**result, **change}) as command, self.assertRaises(RuntimeError):
                observer.wait(lambda: observer.snapshot(summary=True))
            self.assertEqual(command.call_count, 1)
        for action, request in [('press', {}), ('status', {}),
                                ('snapshot', {'document': '/owned/folder'}),
                                ('snapshot-window', {'document': '/owned/folder'})]:
            with self.subTest(action=action), mock.patch.object(macos.common.wayland, 'command',
                    return_value=result) as command, self.assertRaises(RuntimeError):
                observer.wait(lambda: observer.call(action, **request))
            self.assertEqual(command.call_count, 1)
        with mock.patch.object(macos.common.wayland, 'command', return_value=result), self.assertRaises(RuntimeError):
            observer.snapshot(summary=True)

    def test_wait_never_repeats_an_action_before_a_pending_root_read(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        pending = {'returncode': 75, 'errors': [], 'output': json.dumps({
            'format': 'librairii.native-snapshot-unavailable', 'schema': 1,
            'pid': 123, 'attribute': 'AXWindows', 'status': -25204})}
        success = {'returncode': 0, 'errors': [], 'output': '{"performed":true}'}
        def predicate():
            observer.call('press', selector={'window': 0, 'path': [1]})
            return observer.snapshot(summary=True)
        with mock.patch.object(macos.common.wayland, 'command', side_effect=[success, pending]) as command, \
                self.assertRaises(RuntimeError):
            observer.wait(predicate)
        self.assertEqual(command.call_count, 2)

    def test_wait_never_repeats_an_action_from_another_observer(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        other = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/other'), 456)
        pending = {'returncode': 75, 'errors': [], 'output': json.dumps({
            'format': 'librairii.native-snapshot-unavailable', 'schema': 1,
            'pid': 123, 'attribute': 'AXWindows', 'status': -25204})}
        success = {'returncode': 0, 'errors': [], 'output': '{"performed":true}'}
        def predicate():
            other.call('press', selector={'window': 0, 'path': [1]})
            return observer.snapshot(summary=True)
        with mock.patch.object(macos.common.wayland, 'command', side_effect=[success, pending]) as command, \
                self.assertRaises(RuntimeError):
            observer.wait(predicate)
        self.assertEqual(command.call_count, 2)

    def test_nested_wait_from_another_observer_inherits_the_outer_deadline(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        other = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/other'), 456)
        now, budgets = [0.0], []
        def command(argv, env, log, timeout):
            budgets.append(timeout)
            now[0] += timeout
            return {'returncode': -9, 'errors': ['deadline'], 'output': ''}
        with mock.patch.object(macos.common.wayland, 'command', side_effect=command), \
                mock.patch.object(macos.time, 'monotonic', side_effect=lambda: now[0]), self.assertRaises(RuntimeError):
            observer.wait(lambda: other.wait(lambda: other.snapshot(summary=True), timeout=30), timeout=1)
        self.assertEqual(budgets, [1])
        self.assertEqual(now[0], 1)

    def test_go_folder_uses_native_editing_and_exact_readback_before_one_return(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = pathlib.Path(temporary)
            observer = macos.Observer(directory / 'helper', directory, 123)
            root = {'role': 'AXSheet', 'identifier': 'open-panel',
                'selector': {'window': 0, 'sheet': 0, 'path': []}}
            sheet = {**root, 'identifier': 'GoToWindow',
                'selector': {'window': 0, 'sheet': 0, 'path': [9]}}
            field = {'role': 'AXTextField', 'identifier': 'PathTextField',
                'selector': {'window': 0, 'sheet': 0, 'path': [9, 2]},
                'value_settable': True, 'enabled': True, 'focused': True, 'value': 'previous path'}
            opened = {'nodes': [root, sheet, field]}
            edited = {'nodes': [root, sheet, {**field, 'value': str(directory)}]}
            with mock.patch.object(observer, 'snapshot', side_effect=[opened, edited, {'nodes': [root]}]), \
                    mock.patch.object(observer, 'call', return_value={'value': str(directory)}) as call:
                observer.go_folder(directory, root)
            self.assertNotIn('set', [v.args[0] for v in call.call_args_list])
            self.assertEqual([v.kwargs['key'] for v in call.call_args_list if v.args[0] == 'key'],
                ['cmd+shift+g', 'cmd+a', 'cmd+v', 'return'])
            self.assertEqual([v.kwargs['text'] for v in call.call_args_list if v.args[0] == 'clipboard-set'],
                [str(directory)])

    def test_explicit_batch_deadline_does_not_enter_native_request_payload(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        with mock.patch.object(macos.common.wayland, 'command', return_value={
                'returncode': 0, 'errors': [], 'output': '{}'}) as command:
            observer.call('performance-navigation', command_timeout=30, directory='/owned/app')
        self.assertEqual(command.call_args.kwargs['timeout'], 30)
        self.assertNotIn('command_timeout', command.call_args.args[0][-1])

    def test_busy_finder_read_is_neither_a_ready_window_nor_retirement(self):
        pending = {'pid': 123, 'nodes': [], 'windows': [], 'document_window_query_status': -25204,
            'document_window_list_observed': False, 'document_windows_observed': False}
        with tempfile.TemporaryDirectory() as temporary:
            self.assertIsNone(macos.ready_finder_snapshot(pending, pathlib.Path(temporary), 123))
        absent = {'observed': True, 'pid': 123, 'window_id': 184, 'present': False}
        self.assertFalse(macos.finder_window_retired(pending, absent, 123, 184))

    def test_retirement_requires_the_verified_window_id_to_be_absent_even_offscreen(self):
        snapshot = {'document_window_list_observed': True, 'document_windows_observed': False,
            'unresolved_title_window_count': 1, 'owned_document_window_present': False,
            'nodes': [], 'owned_title_candidates': []}
        status = {'observed': True, 'pid': 123, 'window_id': 184, 'present': False}
        self.assertTrue(macos.finder_window_retired(snapshot, status, 123, 184))
        for changed in ({}, {**status, 'observed': False}, {**status, 'present': True},
                {**status, 'pid': 456}, {**status, 'window_id': 185}):
            self.assertFalse(macos.finder_window_retired(snapshot, changed, 123, 184))
        for changed in ({}, {**snapshot, 'document_window_list_observed': False},
                {**snapshot, 'owned_document_window_present': True},
                {**snapshot, 'owned_title_candidates': [{'title': 'owned'}]},
                {**snapshot, 'nodes': [{'role': 'AXWindow'}]}):
            self.assertFalse(macos.finder_window_retired(changed, status, 123, 184))

    def test_only_one_verified_owned_window_can_supply_the_retirement_identity(self):
        window = {'kCGWindowOwnerPID': 123, 'kCGWindowNumber': 184}
        self.assertEqual(macos.finder_window_id({'windows': [window]}, 123), 184)
        for windows in ([], [window, window], [{**window, 'kCGWindowOwnerPID': 456}],
                [{**window, 'kCGWindowNumber': 0}], [{**window, 'kCGWindowNumber': True}],
                [{**window, 'kCGWindowNumber': '184'}]):
            with self.assertRaises(ValueError):
                macos.finder_window_id({'windows': windows}, 123)

    def test_picker_button_waits_for_enabled_state_but_never_retries_an_ambiguous_action(self):
        root = {'selector': {'window': 0, 'sheet': 0, 'path': []}}
        button = {'role': 'AXButton', 'title': 'Open', 'enabled': False,
                  'selector': {'window': 0, 'sheet': 0, 'path': [9]}}
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        with mock.patch.object(observer, 'snapshot', return_value={'nodes': [{**button, 'enabled': True}]}), \
                mock.patch.object(observer, 'call') as call:
            observer.button({'nodes': [button]}, root, ('Open',))
            self.assertEqual(call.call_count, 1)
            self.assertEqual(call.call_args.args[0], 'press')
        with mock.patch.object(observer, 'call') as call, self.assertRaises(ValueError):
            observer.button({'nodes': [button, button]}, root, ('Open',))
        call.assert_not_called()

    def test_finder_icon_view_requires_both_exact_existing_file_urls_in_the_owned_window(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = pathlib.Path(temporary).resolve()
            def node(role, path, **values):
                return {'role': role, 'selector': {'window': 0, 'path': path}, **values}
            window = node('AXWindow', [], identifier='FinderWindow', title=fixture.name)
            view = node('AXList', [0, 2], identifier='IconView', subrole='AXCollectionList')
            files = []
            for i, name in enumerate(('Été 日本語 故事.zip', 'Deuxième archive 02.pk')):
                path = fixture / name
                path.touch()
                files.append(node('AXImage', [0, 2, i, 0], title=name, url=path.as_uri()))
            snapshot = {'nodes': [], 'windows': [], 'owned_title_candidates': [window],
                'owned_candidate_diagnostic_tree': [window, view, *files], 'owned_candidate_cg_windows': []}
            result = macos.verified_finder_snapshot(snapshot, fixture)
            self.assertIsNotNone(result)
            self.assertEqual(result[2]['method'], 'exact-native-icon-view-file-urls')
            self.assertEqual(result[2]['directory'], str(fixture))
            self.assertIsNone(macos.ready_finder_snapshot(snapshot, fixture, 123))
            ready = {**snapshot, 'owned_candidate_cg_windows': [
                {'kCGWindowOwnerPID': 123, 'kCGWindowNumber': 184}]}
            self.assertEqual(macos.ready_finder_snapshot(ready, fixture, 123)[3], 184)
            with self.assertRaises(ValueError):
                macos.ready_finder_snapshot({**ready,
                    'owned_candidate_cg_windows': ready['owned_candidate_cg_windows'] * 2}, fixture, 123)
            for nodes in ([window], [window, view, files[0]], [window, view, files[0], files[0]],
                    [window, view, *[{**f, 'selector': {'window': 1, 'path': f['selector']['path']}} for f in files]],
                    [window, view, files[0], {**files[1], 'url': fixture.parent.as_uri()}],
                    [window, view, files[0], {**files[1], 'selector': {'window': 0, 'path': [9, 1]}}]):
                self.assertIsNone(macos.verified_finder_snapshot(
                    {**snapshot, 'owned_candidate_diagnostic_tree': nodes}, fixture))
            (fixture / files[0]['title']).unlink()
            self.assertIsNone(macos.verified_finder_snapshot(snapshot, fixture))

    def test_finder_requires_an_actual_local_file_url_for_the_exact_owned_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = pathlib.Path(temporary).resolve() / 'Été 日本語'
            fixture.mkdir()
            window = {'role': 'AXWindow', 'title': fixture.name, 'document': '', 'url': fixture.as_uri()}
            self.assertEqual(macos.finder_fixture_window({'nodes': [window]}, fixture), window)
            self.assertEqual(macos.finder_fixture_window({'nodes': [{**window,
                'document': fixture.as_uri(), 'url': ''}]}, fixture)['document'], fixture.as_uri())
            for document in ((fixture / 'missing-old-document').as_uri(), 'file://[invalid',
                    fixture.as_uri() + '%00'):
                alternate = {**window, 'document': document}
                self.assertEqual(macos.finder_fixture_window({'nodes': [alternate]}, fixture), alternate)
            for change in ({'url': ''}, {'url': fixture.parent.as_uri()}, {'role': 'AXTextField'},
                    {'url': 'https://example.invalid' + str(fixture)},
                    {'url': fixture.as_uri().replace('file:///', 'file://other-host/')}):
                self.assertIsNone(macos.finder_fixture_window({'nodes': [{**window, **change}]}, fixture))

    def test_callback_identity_resolves_existing_aliases_without_accepting_other_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            fixture = root / 'fixture'
            fixture.mkdir()
            selected = fixture / 'Été 日本語.zip'
            selected.touch()
            alias = root / 'alias'
            alias.symlink_to(fixture, target_is_directory=True)
            self.assertEqual(macos.canonical_paths([str(alias)]), macos.canonical_paths([str(fixture)]))
            self.assertEqual(macos.canonical_paths([str(alias / selected.name)]),
                             macos.canonical_paths([str(selected)]))
            self.assertNotEqual(macos.canonical_paths([str(fixture.parent)]),
                                macos.canonical_paths([str(fixture)]))
            with self.assertRaises(FileNotFoundError):
                macos.canonical_paths([str(alias / 'missing')])
            with self.assertRaises(ValueError):
                macos.canonical_paths(['fixture'])

    def test_go_folder_cannot_edit_search_or_native_filename_cells(self):
        def node(role, identifier, path, **values):
            return {'role': role, 'identifier': identifier, 'selector':
                {'window': 0, 'sheet': 0, 'path': path}, **values}
        root = node('AXSheet', 'open-panel', [])
        sheet = node('AXSheet', 'GoToWindow', [9])
        field = node('AXTextField', 'PathTextField', [9, 2], value_settable=True)
        other = [node('AXTextField', 'Search', [6], value_settable=True),
                 node('AXTextField', '', [0, 2, 1, 1], value_settable=True),
                 node('AXTextField', 'PathTextField', [8], value_settable=True)]
        self.assertEqual(macos.go_folder_fields({'nodes': [root, sheet, field, *other]}, root), [field])
        self.assertEqual(macos.go_folder_fields({'nodes': [root, field, *other]}, root), [])
        self.assertEqual(macos.go_folder_fields({'nodes': [sheet, {**field, 'enabled': False}]}, root), [])

    def test_diagnostics_cannot_turn_a_failed_native_command_into_a_pass(self):
        observer = macos.Observer(pathlib.Path('/owned/helper'), pathlib.Path('/owned/output'), 123)
        result = {'returncode': 0, 'errors': [],
                  'output': 'observer-stage: reading AXWindows\n{"pid":123,"nodes":[]}\n'}
        with mock.patch.object(macos.common.wayland, 'command', return_value=result):
            self.assertEqual(observer.call('snapshot')['pid'], 123)
        for change in ({'returncode': -9}, {'errors': ['process deadline exceeded']}):
            with self.subTest(change=change), mock.patch.object(macos.common.wayland, 'command',
                    return_value={**result, **change}), self.assertRaises(RuntimeError):
                observer.call('snapshot')

    def test_exited_parent_cannot_leave_a_pipe_reader_or_owned_descendant(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = pathlib.Path(temporary) / 'app.log'
            source = ('import subprocess, sys; '
                'subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"]); '
                'print("parent exited", flush=True)')
            app = subprocess.Popen([sys.executable, '-c', source], stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, start_new_session=True)
            capture = macos.capture_output(app, output)
            started = time.monotonic()
            try:
                self.assertEqual(app.wait(timeout=5), 0)
                errors = macos.retire(app, capture)
                self.assertIn('descendant survived exited app', errors)
                self.assertFalse(capture[1].is_alive())
                self.assertIn('parent exited', output.read_text())
                self.assertLess(time.monotonic() - started, 6)
            finally:
                macos.common.wayland.kill_group(app.pid)
                capture[0].set()
                capture[1].join(timeout=2)

    def fixture(self):
        cases = {}
        for name, kind, _ in macos.CASES:
            checks = {'native_settings_accelerator', 'native_settings_menu', 'native_search_input',
                      'owned_shutdown', 'fixture_removed'}
            checks |= ({'real_parented_modal_sheet', 'exact_native_callback', 'unforced_parent_focus_restored'}
                if kind in ('files', 'folder') else {'actual_finder_path', 'verified_finder_window_closed', 'explicit_return_from_finder'}
                if kind == 'reveal' else {'early_settings_delivered', 'native_unicode_clipboard_and_edit_menu'})
            cases[name] = {'errors': [], 'returncode': 0, 'checks': dict.fromkeys(checks, True)}
        return {'binary_sha256': 'expected', 'errors': [], 'cases': cases}

    def test_every_case_and_required_native_witness_is_mandatory(self):
        report = self.fixture()
        self.assertTrue(macos.session_passed(report, 'expected'))
        self.assertFalse(macos.session_passed(report, 'changed'))
        for name, case in report['cases'].items():
            changed = copy.deepcopy(report)
            del changed['cases'][name]
            self.assertFalse(macos.session_passed(changed, 'expected'))
            for key in case['checks']:
                changed = copy.deepcopy(report)
                del changed['cases'][name]['checks'][key]
                self.assertFalse(macos.session_passed(changed, 'expected'))
            for field, value in [('returncode', 1), ('errors', ['failed capture'])]:
                changed = copy.deepcopy(report)
                changed['cases'][name][field] = value
                self.assertFalse(macos.session_passed(changed, 'expected'))

    def test_parenting_requires_public_attached_sheet_not_a_standalone_dialog(self):
        sheet = {'role': 'AXSheet', 'selector': {'window': 0, 'sheet': 0, 'path': []}}
        self.assertEqual(macos.picker_root({'nodes': [sheet]}), sheet)
        for changed in ({**sheet, 'modal': False}, {**sheet, 'role': 'AXWindow'},
                        {**sheet, 'selector': {'window': 0, 'path': []}},
                        {**sheet, 'selector': {'window': 0, 'sheet': 0, 'path': [1]}}):
            self.assertIsNone(macos.picker_root({'nodes': [changed]}))

    def test_selector_fingerprints_do_not_depend_on_mutating_text_values(self):
        node = {'role': 'AXTextField', 'title': 'Go to Folder', 'description': 'Path', 'identifier': 'owned',
                'value': '/tmp/before', 'selector': {'window': 0, 'sheet': 0, 'path': [2, 1]}}
        selected = macos.selector(node)
        self.assertNotIn('value', selected['match'])
        self.assertEqual(selected['match']['identifier'], 'owned')
        self.assertTrue(macos.same_root(node, {**node, 'selector': {'window': 0, 'sheet': 0, 'path': []}}))
        self.assertFalse(macos.same_root(node, {**node, 'selector': {'window': 1, 'sheet': 0, 'path': []}}))


if __name__ == '__main__':
    unittest.main()
