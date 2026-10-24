"""A build or partial macOS summary cannot qualify missing native observations."""
import copy
import importlib.util
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
    def test_failed_or_ambiguous_window_queries_cannot_prove_finder_closed(self):
        absent = {'document_windows_observed': True, 'owned_document_window_present': False,
                  'nodes': [], 'owned_title_candidates': []}
        self.assertTrue(macos.finder_window_absent(absent))
        for value in ({}, {**absent, 'document_windows_observed': False},
                {**absent, 'owned_document_window_present': True},
                {**absent, 'owned_title_candidates': [{'title': 'owned'}]},
                {**absent, 'nodes': [{'role': 'AXWindow'}]}):
            self.assertFalse(macos.finder_window_absent(value))

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
