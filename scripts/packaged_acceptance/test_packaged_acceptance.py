"""Admission and custody regressions; never runs a native GUI."""
import copy
import gc
from contextlib import closing
import importlib.util
import io
from http.client import HTTPConnection
import json
import os
from pathlib import Path
import sys
import sqlite3
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release.files import digest, inventory
from native_release.files import write_json
from packaged_acceptance.fixture import Fixture
from packaged_acceptance.run import admit, qualified, run
from packaged_acceptance.workflow import REQUIRED


class PackagedAdmission(unittest.TestCase):
    def test_native_graphics_cache_fixture_repeated_launch_and_retention_receipts_are_immutable(self):
        from packaged_acceptance.native_cache import BUNDLE
        source = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory(prefix='librairii-cache-lifecycle-') as temporary:
            root = Path(temporary).resolve()
            evidence, parent = root/'evidence', root/'synthetic-os-cache'
            evidence.mkdir()
            parent.mkdir()
            fixture = Fixture(source, evidence, False)
            try:
                with patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}), \
                     patch('packaged_acceptance.fixture.platform.system', return_value='Darwin'), \
                     patch('packaged_acceptance.fixture.subprocess.check_output', return_value=str(parent)+'\n'):
                    for _ in range(5):
                        fixture.prepare_graphics_cache()
                    self.assertTrue((parent/BUNDLE).is_symlink())
                    (fixture.graphics_cache.target/'shader').write_bytes(b'owned fixture cache')
                fixture.retain()
                self.assertFalse((parent/BUNDLE).exists())
                self.assertFalse(fixture.root.exists())
                launches = sorted(evidence.glob('native-graphics-cache-launch-*.json'))
                self.assertEqual(len(launches), 5)
                self.assertEqual([json.loads(p.read_text())['launch_index'] for p in launches], list(range(1, 6)))
                retired = json.loads((evidence/'native-graphics-cache-retirement.json').read_text())
                self.assertTrue(retired['retired'])
                self.assertFalse(retired['reserved'])
                retained = json.loads((evidence/'fixture-final.json').read_text())
                self.assertIn('native-graphics-cache/shader', retained)
            finally:
                # Own and retire the red implementation's resources as well.
                if fixture.root.exists():
                    if fixture.graphics_cache is not None:
                        fixture.graphics_cache.retire(True)
                        fixture.graphics_cache = None
                    if not (evidence/'local-network.json').exists():
                        fixture.retain()

    def test_native_graphics_cache_reserves_only_fresh_owned_aliases_and_retains_uncertainty(self):
        from packaged_acceptance.native_cache import BUNDLE, MARKER, NativeGraphicsCache
        import shutil
        def layout(temporary):
            root = Path(temporary).resolve()
            fixture, parent = root/'fixture', root/'synthetic-os-cache'
            fixture.mkdir()
            parent.mkdir()
            (fixture/'.librairii-packaged-fixture').write_bytes(MARKER)
            return NativeGraphicsCache(fixture, parent), parent/BUNDLE
        for existing in ('file', 'directory', 'dangling-link'):
            with self.subTest(existing=existing), tempfile.TemporaryDirectory() as temporary:
                owner, anchor = layout(temporary)
                if existing == 'file':
                    anchor.write_bytes(b'foreign sentinel')
                elif existing == 'directory':
                    anchor.mkdir()
                    (anchor/'sentinel').write_bytes(b'foreign sentinel')
                else:
                    anchor.symlink_to(anchor.parent/'missing')
                before = (anchor.lstat(), os.readlink(anchor) if anchor.is_symlink() else None)
                with self.assertRaises(FileExistsError):
                    owner.prepare()
                owner.retire(True)
                after = anchor.lstat()
                self.assertEqual((after.st_dev, after.st_ino, after.st_mode, after.st_size),
                                 (before[0].st_dev, before[0].st_ino, before[0].st_mode, before[0].st_size))
                if existing == 'dangling-link':
                    self.assertEqual(os.readlink(anchor), before[1])
                else:
                    self.assertEqual((anchor/'sentinel' if existing == 'directory' else anchor).read_bytes(),
                                     b'foreign sentinel')
        for substitution in ('none', 'anchor', 'target', 'parent'):
            with self.subTest(substitution=substitution), tempfile.TemporaryDirectory() as temporary:
                owner, anchor = layout(temporary)
                owner.prepare()
                parent_fd, target_fd = owner.fd, owner.target_fd
                with self.assertRaisesRegex(ValueError, 'uncertain'):
                    owner.retire(False)
                self.assertTrue(anchor.is_symlink())
                owner.verify()
                (anchor/'shader-cache').write_bytes(b'owned synthetic cache')
                self.assertEqual((owner.target/'shader-cache').read_bytes(), b'owned synthetic cache')
                if substitution != 'none':
                    original = {'anchor': anchor, 'target': owner.target, 'parent': owner.parent}[substitution]
                    saved = original.with_name(original.name+'-saved')
                    original.rename(saved)
                    if substitution == 'anchor':
                        original.write_bytes(b'foreign sentinel')
                    else:
                        original.mkdir()
                        (original/'sentinel').write_bytes(b'foreign sentinel')
                    with self.assertRaises(ValueError):
                        owner.retire(True)
                    self.assertTrue(owner.reserved)
                    if original.is_dir():
                        self.assertEqual((original/'sentinel').read_bytes(), b'foreign sentinel')
                        shutil.rmtree(original)  # The test owns this synthetic substitution.
                    else:
                        self.assertEqual(original.read_bytes(), b'foreign sentinel')
                        original.unlink()
                    saved.rename(original)
                owner.retire(True)
                self.assertFalse(anchor.exists())
                self.assertTrue(owner.receipt()['retired'])
                self.assertEqual((owner.target/'shader-cache').read_bytes(), b'owned synthetic cache')
                for descriptor in (parent_fd, target_fd):
                    with self.assertRaises(OSError):
                        os.fstat(descriptor)
        fixture = Fixture.__new__(Fixture)
        with patch.dict(os.environ, {'GITHUB_ACTIONS': 'false'}), \
             patch('packaged_acceptance.fixture.platform.system', return_value='Darwin'), \
             patch('packaged_acceptance.fixture.subprocess.check_output') as lookup:
            with self.assertRaisesRegex(ValueError, 'disposable macOS CI'):
                fixture.prepare_graphics_cache()
            lookup.assert_not_called()

    def test_failed_mac_session_samples_only_its_unreaped_child_before_retirement(self):
        from packaged_acceptance.session import Session
        import subprocess
        cases = [('Darwin', 'true', True, False), ('Darwin', 'true', True, 'sample'),
                 ('Darwin', 'true', True, 'log'), ('Darwin', 'true', True, 'both'),
                 ('Darwin', 'false', True, False), ('Linux', 'true', True, False),
                 ('Darwin', 'true', False, False)]
        for system, ci, live, failed in cases:
            with self.subTest(system=system, ci=ci, live=live, sample_failed=failed), tempfile.TemporaryDirectory() as temporary:
                session = Session.__new__(Session)
                session.output, session.errors = Path(temporary), []
                from datetime import datetime, timezone
                session.started_utc = datetime.now(timezone.utc)
                child = session.process = Mock(pid=123)
                child.poll.return_value = None if live else 0
                child.kill.side_effect = lambda: setattr(child.poll, 'return_value', -9)
                child.wait.return_value = -9 if live else 0
                session.reader = Mock(ident=1)
                session.reader.is_alive.return_value = False
                def sample(command, **options):
                    child.kill.assert_not_called()
                    if command[0] == '/usr/bin/sample':
                        self.assertEqual(command, ['/usr/bin/sample', '123', '1', '1', '-file',
                                                    str(session.output/'failure.sample.txt')])
                        self.assertEqual(options['timeout'], 30)
                    else:
                        self.assertEqual(command[:3], ['/usr/bin/log', 'show', '--style'])
                        self.assertEqual(options['timeout'], 5)
                        self.assertEqual(command[-1], '(processIdentifier == 123) OR '
                                         '((process == "kernel" OR process == "sandboxd") AND '
                                         'eventMessage CONTAINS "(123)")')
                        self.assertIn('--start', command)
                        self.assertIn('--end', command)
                    if failed in ('both', 'sample' if command[0] == '/usr/bin/sample' else 'log'):
                        raise subprocess.TimeoutExpired(command, options['timeout'])
                    return SimpleNamespace(returncode=0)
                with patch.dict(os.environ, {'GITHUB_ACTIONS': ci}), \
                     patch('packaged_acceptance.session.platform.system', return_value=system), \
                     patch('packaged_acceptance.session.subprocess.run', side_effect=sample) as sampled:
                    receipt = session.retire_failure()
                if system == 'Darwin' and ci == 'true' and live:
                    self.assertEqual(sampled.call_count, 2)
                    self.assertIn('error' if failed in ('sample', 'both') else 'returncode',
                                  receipt['failure_sample'])
                    system_log = receipt['failure_system_log']
                    self.assertIn('error' if failed in ('log', 'both') else 'returncode', system_log)
                    self.assertEqual(system_log['end_utc'], receipt['failure_sample']['started_utc'])
                    start = datetime.fromisoformat(system_log['start_utc'])
                    end = datetime.fromisoformat(system_log['end_utc'])
                    self.assertGreaterEqual(start, session.started_utc)
                    self.assertGreaterEqual((end-start).total_seconds(), 0)
                    self.assertLessEqual((end-start).total_seconds(), 120)
                else:
                    sampled.assert_not_called()
                if live:
                    child.kill.assert_called_once()
                else:
                    child.kill.assert_not_called()
                self.assertTrue(receipt['retired'])
                self.assertEqual(receipt['returncode'], -9 if live else 0)

    def test_mac_activation_waits_for_the_exact_owned_visible_window(self):
        from packaged_acceptance.macos import Driver
        pid = 123
        state = {'launch_reads': 0, 'window_reads': 0, 'activated': False}
        def status():
            if not state['activated']:
                raise RuntimeError('transparent startup window needs activation')
            state['window_reads'] += 1
            count = state['window_reads']
            windows = ([] if count == 1 else
                       [{'kCGWindowOwnerPID': 456 if count == 2 else pid,
                         'kCGWindowLayer': 0, 'kCGWindowIsOnscreen': True,
                         'kCGWindowAlpha': 0 if count == 3 else 1}])
            return {'pid': pid, 'frontmost_pid': pid if count >= 5 else 456,
                    'windows': windows}
        def call(action):
            if action == 'launch-status':
                state['launch_reads'] += 1
                return {'pid': pid, 'registered': state['launch_reads'] >= 2,
                        'finished_launching': state['launch_reads'] >= 3,
                        'regular_activation': True, 'terminated': False}
            self.assertEqual(action, 'activate')
            if state['launch_reads'] < 3:
                raise RuntimeError('activation preceded AppKit launch readiness')
            state['activated'] = True
            return {'activated': True}
        def wait(predicate):
            for _ in range(6):
                if value := predicate():
                    return value
            raise TimeoutError('owned native window never appeared')
        observer = SimpleNamespace(status=status, call=Mock(side_effect=call), wait=wait)
        module = SimpleNamespace(Observer=Mock(return_value=observer))
        with tempfile.TemporaryDirectory() as temporary, \
             patch.dict(os.environ, {'GITHUB_ACTIONS': 'true'}), \
             patch('packaged_acceptance.macos.importlib.util.spec_from_file_location'), \
             patch('packaged_acceptance.macos.importlib.util.module_from_spec', return_value=module):
            root = Path(temporary)
            Driver(root, root, pid, root/'unused-test-helper')
            self.assertEqual(observer.call.call_args_list,
                             [unittest.mock.call('launch-status')]*3+[unittest.mock.call('activate')])
            self.assertEqual(state['window_reads'], 5)
            # Even two diagnostic failures must preserve the actual readiness
            # failure, rather than turn a storage error into its replacement.
            timeout = TimeoutError('original positive-window deadline')
            observer.wait = Mock(side_effect=[{'pid': pid}, timeout])
            write = Path.write_text
            def failed_log(path, *args, **kwargs):
                if path.name == 'initial-window-failure.log':
                    raise OSError('diagnostic output is full')
                return write(path, *args, **kwargs)
            with patch.object(Driver, 'snapshot', side_effect=RuntimeError('snapshot failed')), \
                 patch.object(Path, 'write_text', failed_log), self.assertRaises(TimeoutError) as raised:
                Driver(root, root, pid, root/'unused-test-helper')
            self.assertIs(raised.exception, timeout)

    def test_observer_closes_database_readers_before_product_relocation(self):
        # A connection context commits/rolls back; it does not close SQLite.
        # Keep collection out of this lifecycle check so ownership is explicit.
        enabled = gc.isenabled()
        gc.disable()
        try:
            for invalid in (False, True):
                with self.subTest(invalid=invalid), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    (root/'db').mkdir()
                    path = root/'db/librairii.sqlite3'
                    fixture = Fixture.__new__(Fixture)
                    fixture.current_root = lambda: root
                    with closing(sqlite3.connect(path)) as writer:
                        writer.execute('PRAGMA journal_mode=WAL')
                        writer.execute('CREATE TABLE sample(data BLOB)')
                        writer.execute("INSERT INTO sample VALUES (X'0102')")
                        writer.commit()
                        if invalid:
                            with self.assertRaises(sqlite3.OperationalError):
                                fixture.rows('SELECT * FROM missing_table')
                        else:
                            self.assertEqual(fixture.rows('SELECT * FROM sample'),
                                             [{'data': {'blob_hex': '0102'}}])
                    self.assertFalse(path.with_name(path.name+'-wal').exists(),
                                     'observer reader retained WAL after product closure')
                    self.assertFalse(path.with_name(path.name+'-shm').exists(),
                                     'observer reader retained shared memory after product closure')
        finally:
            if enabled:
                gc.enable()

    def test_runtime_requires_one_observed_renderer_matching_the_requested_route(self):
        from packaged_acceptance.session import Session
        with tempfile.TemporaryDirectory(prefix='librairii-renderer-witness-') as temporary:
            session = Session.__new__(Session)
            session.pid, session.output = 123, Path(temporary)
            with patch('packaged_acceptance.session.platform.system', return_value='Darwin'), \
                 patch('packaged_acceptance.session.subprocess.check_output', return_value='n/usr/lib/libSystem.B.dylib\n'):
                for backend, surfaces, accepted in [
                        ('winit-skia', [], False),
                        ('winit-skia', ['software'], False),
                        ('winit-skia-software', ['OpenGL'], False),
                        ('winit-skia', ['OpenGL', 'software'], False),
                        ('winit-skia', ['OpenGL'], True),
                        ('winit-skia-software', ['software'], True)]:
                    with self.subTest(backend=backend, surfaces=surfaces):
                        session.launch_receipt = {'backend': backend}
                        (session.output/'product.log').write_text(''.join(
                            'Skia renderer (skia backend '+surface+'; test-only diagnostic)\n' for surface in surfaces))
                        if accepted:
                            self.assertEqual(session.runtime()['active_surface'], surfaces)
                        else:
                            with self.assertRaisesRegex(ValueError, 'actual installed renderer'):
                                session.runtime()

    def test_mac_press_refreshes_only_a_proven_pre_action_stale_path_once(self):
        from packaged_acceptance.macos import Driver
        driver=Driver.__new__(Driver)
        nodes=[{'path':1},{'path':2}]
        driver.find=Mock(side_effect=nodes)
        driver.wait=lambda predicate:predicate()
        driver.module=SimpleNamespace(selector=lambda node:node)
        stale=RuntimeError('press failed: {"returncode": 1, "errors": []}; invalid("native control changed")\n')
        driver.observer=SimpleNamespace(call=Mock(side_effect=[stale,{'performed':True}]))
        with patch('packaged_acceptance.macos.time.sleep'):
            self.assertIs(driver.press('Close'),nodes[1])
        self.assertEqual(driver.observer.call.call_count,2)
        for error in (RuntimeError('native control action failed'),stale):
            driver.find=Mock(side_effect=nodes)
            driver.observer.call=Mock(side_effect=error)
            with self.assertRaises(RuntimeError),patch('packaged_acceptance.macos.time.sleep'):
                driver.press('Close')
            self.assertEqual(driver.observer.call.call_count,2 if error is stale else 1)

    def test_picker_location_removes_only_selected_inline_filename_completion(self):
        from packaged_acceptance.linux import picker_location
        driver=Mock()
        driver.desktop.a.STATE_FOCUSED='focused'
        driver.read_errors=(RuntimeError,)
        directory=Path('/tmp/owned/review-inputs')
        expected=str(directory)+'/'
        state={'value':expected+'Nouvelle','selection':(len(expected),len(expected+'Nouvelle'))}
        text=Mock()
        text.getText.side_effect=lambda start,end:state['value']
        text.getNSelections.return_value=1
        text.getSelection.side_effect=lambda index:state['selection']
        node=SimpleNamespace(name='Location',getRoleName=lambda:'text',getState=Mock(),queryText=lambda:text)
        node.getState.return_value.contains.return_value=True
        driver.nodes.return_value=[node]
        def wait(predicate):
            if value:=predicate():return value
            raise TimeoutError('exact picker directory was not observed')
        driver.wait.side_effect=wait
        def key(value):
            self.assertEqual(value,'BackSpace')
            state['value']=expected
        driver.key.side_effect=key
        observed=picker_location(driver,object(),directory)
        self.assertEqual(observed['value'],expected)
        driver.key.assert_called_once_with('BackSpace')
        for selection in ((0,len(expected+'Nouvelle')),(len(expected)-1,len(expected+'Nouvelle'))):
            driver.key.reset_mock()
            state.update(value=expected+'Nouvelle',selection=selection)
            with self.assertRaises(TimeoutError):picker_location(driver,object(),directory)
            driver.key.assert_not_called()

    def test_native_reveal_traversal_observes_target_and_stops_at_its_input_bound(self):
        from packaged_acceptance.workflow import reveal
        driver=Mock()
        target=object()
        predicate=Mock(side_effect=[None,None,target])
        self.assertIs(reveal(driver,predicate),target)
        self.assertEqual(driver.key.call_args_list,[unittest.mock.call('Tab')]*2)
        driver.key.reset_mock()
        with self.assertRaisesRegex(TimeoutError,'32 Tab'):
            reveal(driver,lambda:None)
        self.assertEqual(driver.key.call_count,32)
        driver.key.reset_mock()
        self.assertIs(reveal(driver,lambda:target),target)
        driver.key.assert_not_called()

    def test_empty_native_text_replacement_deletes_selection_instead_of_typing_nothing(self):
        from packaged_acceptance.macos import Driver as Mac
        from packaged_acceptance.linux import Driver as Linux
        for cls in (Mac,Linux):
            with self.subTest(platform=cls.__module__):
                driver=cls.__new__(cls)
                state={'value':'Clockwork','selected':False}
                driver.focus=Mock()
                def key(value):
                    if value in ('ctrl+a','cmd+a'):
                        state['selected']=True
                    elif value in ('BackSpace','delete') and state['selected']:
                        state['value']=''
                driver.key=key
                def wait(predicate):
                    value=predicate()
                    if not value:
                        raise TimeoutError('search still contains selected text')
                    return value
                driver.wait=wait
                driver.snapshot=lambda:{'nodes':[{'name':'Search stories','title':'Search stories','value':state['value']}]}
                driver.desktop=SimpleNamespace(text=Mock())
                driver.observer=SimpleNamespace(call=Mock())
                driver.text('Search stories','')
                self.assertEqual(state['value'],'')
                driver.desktop.text.assert_not_called()
                driver.observer.call.assert_not_called()

    def test_picker_root_uses_focused_native_list_row_and_refuses_failed_focus(self):
        from packaged_acceptance.linux import open_picker_root
        driver = Mock()
        driver.desktop.a.STATE_FOCUSED = 'focused'
        driver.desktop.a.STATE_SELECTED = 'selected'
        def wait(predicate):
            value=predicate()
            if not value:
                raise TimeoutError('native focus was not observed')
            return value
        driver.wait.side_effect=wait
        selection=Mock()
        state={'selected':False,'focused':True}
        def select(index):
            self.assertEqual(index,3)
            state['selected']=True
            return True
        selection.selectChild.side_effect=select
        box=SimpleNamespace(querySelection=Mock(return_value=selection),getRoleName=Mock(return_value='list box'))
        row=SimpleNamespace(parent=box,name='',getIndexInParent=Mock(return_value=3),getRoleName=Mock(return_value='list item'),
                            queryAction=Mock(side_effect=NotImplementedError),queryComponent=Mock(),getState=Mock())
        row.getState.return_value.contains.side_effect=lambda flag:state[flag]
        driver.key.side_effect=lambda key:self.assertTrue(state['selected'],'Return would reopen the previously selected Recent row')
        label=SimpleNamespace(parent=row,name='/',getRoleName=Mock(return_value='label'),
                              queryAction=Mock(side_effect=NotImplementedError))
        driver.find.return_value=label
        observed=open_picker_root(driver,object())
        self.assertEqual(observed[-1]['role'],'list item')
        row.queryComponent().grabFocus.assert_called_once()
        driver.key.assert_called_once_with('Return')
        driver.action.assert_not_called()
        selection.selectChild.assert_called_once_with(3)
        for accepted,focused in ((False,True),(True,False)):
            driver.key.reset_mock()
            row.queryComponent().grabFocus.return_value=accepted
            state['focused']=focused
            with self.assertRaises((ValueError,TimeoutError)):
                open_picker_root(driver,object())
            driver.key.assert_not_called()
        state['focused']=True
        driver.key.reset_mock()
        selection.selectChild.side_effect=None
        selection.selectChild.return_value=False
        with self.assertRaisesRegex(ValueError,'selection refused'):
            open_picker_root(driver,object())
        driver.key.assert_not_called()

    def test_mac_wait_retries_pending_profile_publication_without_hiding_other_errors(self):
        from packaged_acceptance.macos import Driver
        driver = Driver.__new__(Driver)
        def native_wait(predicate,timeout):
            for _ in range(2):
                if value:=predicate():
                    return value
            raise TimeoutError('unpublished fixture')
        driver.observer = SimpleNamespace(wait=native_wait)
        published = Mock(side_effect=[FileNotFoundError('worker has not published profile'),'owned-profile'])
        self.assertEqual(driver.wait(published),'owned-profile')
        self.assertEqual(published.call_count,2)
        with self.assertRaisesRegex(ValueError,'invalid profile'):
            driver.wait(Mock(side_effect=ValueError('invalid profile')))
        with self.assertRaises(TimeoutError):
            driver.wait(Mock(side_effect=FileNotFoundError('unpublished')))

    def test_linux_press_keeps_observed_identity_when_action_replaces_native_node(self):
        from packaged_acceptance.linux import Driver
        driver = Driver.__new__(Driver)
        state = {'replaced':False}
        class Node:
            @property
            def name(self):
                if state['replaced']:
                    raise RuntimeError('native node was replaced')
                return 'Sync official metadata'
            def getRoleName(self):
                if state['replaced']:
                    raise RuntimeError('native node was replaced')
                return 'push button'
        node = Node()
        driver.application = object()
        driver.desktop = SimpleNamespace(nodes=lambda root:iter([None,node]))
        driver.wait = lambda predicate:predicate()
        driver.find = lambda *args,**kwargs:node
        driver.action = lambda selected:state.update(replaced=True)
        self.assertEqual(driver.press('Sync official metadata'),{'name':'Sync official metadata','role':'push button'})
        self.assertEqual(list(driver.nodes()),[node])
        driver.action = Mock(side_effect=ValueError('native action refused'))
        state['replaced'] = False
        with self.assertRaisesRegex(ValueError,'native action refused'):
            driver.press('Sync official metadata')

    def test_picker_selects_exact_named_native_rows_and_rejects_ambiguous_or_refused_selection(self):
        from packaged_acceptance.linux import select_picker_rows
        driver = Mock()
        driver.desktop.a.ROLE_TABLE,driver.desktop.a.ROLE_TREE_TABLE = 1,2
        driver.desktop.visible.return_value = True
        def wait(predicate):
            value = predicate()
            if not value:
                raise TimeoutError('unobserved native selection')
            return value
        driver.wait.side_effect = wait
        table = Mock(nRows=3,nColumns=1)
        node = Mock()
        node.getRole.return_value = 1
        node.queryTable.return_value = table
        names = [{'unrelated'}, {'Été 日本語 故事.zip'}, {'Deuxième archive 02.pk'}]
        table.getAccessibleAt.side_effect = lambda row,column:row
        driver.desktop.nodes.side_effect = lambda row:[None,*[SimpleNamespace(name=name) for name in names[row]]]
        driver.desktop.find.side_effect = lambda predicate,root:node if predicate(node) else None
        selected = {0}
        table.isRowSelected.side_effect = lambda row:row in selected
        table.addRowSelection.side_effect = lambda row:(selected.add(row) or True)
        table.removeRowSelection.side_effect = lambda row:(selected.remove(row) or True)
        table.getSelectedRows.side_effect = lambda:list(sorted(selected))
        wanted = {'Été 日本語 故事.zip','Deuxième archive 02.pk'}
        self.assertEqual(select_picker_rows(driver,object(),wanted)['selected_rows'],[1,2])
        self.assertEqual(selected,{1,2})
        for bad in ([{'Été 日本語 故事.zip'},{'Été 日本語 故事.zip'},{'Deuxième archive 02.pk'}],
                    [{'unrelated'},wanted,set()], [{'unrelated'},{'Été 日本語 故事.zip'},set()]):
            names[:] = bad
            with self.assertRaises(TimeoutError):
                select_picker_rows(driver,object(),wanted)
        names[:] = [{'unrelated'}, {'Été 日本語 故事.zip'}, {'Deuxième archive 02.pk'}]
        selected.clear()
        table.addRowSelection.side_effect = lambda row:False
        with self.assertRaisesRegex(ValueError,'selection refused'):
            select_picker_rows(driver,object(),wanted)
        node.queryComponent.return_value.grabFocus.return_value = False
        with self.assertRaisesRegex(ValueError,'focus refused'):
            select_picker_rows(driver,object(),wanted)

    def test_managed_imports_must_match_independent_original_identity_name_and_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root/'archives').mkdir()
            archive = root/'archives/managed'
            archive.write_bytes(b'original immutable input')
            row = {'id':1,'uuid':'original','is_broken':0,'original_filename':'original.zip',
                   'managed_path':'archives/managed','sha256':digest(archive),'byte_size':archive.stat().st_size,
                   'validation_state':'valid'}
            fixture = Fixture.__new__(Fixture)
            fixture.initial_uuids,fixture.review_uuid = {'original'},'review'
            fixture.original_archives = {'original':{('original.zip',row['sha256'],row['byte_size'])}}
            fixture.current_root = lambda:root
            fixture.rows = lambda _: [row]
            self.assertEqual(fixture.archives(reviewed=False),[row])
            with self.assertRaises(ValueError):
                fixture.archives(reviewed=True)
            for key,value in [('uuid','unexpected'),('original_filename','wrong.zip'),('sha256','f'*64),('byte_size',0)]:
                changed = dict(row,**{key:value})
                fixture.rows = lambda _,value=changed:[value]
                with self.assertRaises(ValueError):
                    fixture.archives(reviewed=False)
            # A wrong input cannot become the oracle merely by agreeing with DB.
            archive.write_bytes(b'other internally consistent bytes')
            changed = dict(row,sha256=digest(archive),byte_size=archive.stat().st_size)
            fixture.rows = lambda _:[changed]
            with self.assertRaisesRegex(ValueError,'independent original'):
                fixture.archives(reviewed=False)

    def test_combo_keyboard_selection_clamps_from_any_prior_choice_and_closes_popup(self):
        from packaged_acceptance.macos import Driver as Mac
        from packaged_acceptance.linux import Driver as Linux
        for cls in (Mac,Linux):
            for initial in (0,3,8):
                for selected in (0,1,7):
                    with self.subTest(platform=cls.__module__,initial=initial,selected=selected):
                        state = {'index':initial,'popup':False}
                        driver = cls.__new__(cls)
                        driver.focus = Mock()
                        def native_key(key):
                            # Pinned native ComboBox semantics: Return toggles
                            # popup custody; arrows clamp and commit selection.
                            if key.lower() == 'return':
                                state['popup'] = not state['popup']
                            elif key.lower() == 'up':
                                state['index'] = max(0,state['index']-1)
                            elif key.lower() == 'down':
                                state['index'] = min(8,state['index']+1)
                        driver.key = native_key
                        driver.choose('actual native combo',selected)
                        self.assertEqual(state,{'index':selected,'popup':False})

    def test_display_witness_requires_actual_requested_scale_and_consistent_native_geometry(self):
        from packaged_acceptance.session import Session
        with tempfile.TemporaryDirectory() as temporary:
            session = Session.__new__(Session)
            session.output, session.pid = Path(temporary), 123
            session.launch_receipt = {'requested_scale':'1.25'}
            log = session.output/'product.log'
            line = 'native_acceptance; stage=display; pid=123; scale_factor=1.250; physical=1000x750; logical=800.000x600.000\n'
            log.write_text('startup only\n')
            self.assertIsNone(session.display())
            log.write_text(line)
            self.assertEqual(session.display()['observed_scale'], 1.25)
            for changed in (line.replace('pid=123','pid=124'), line.replace('1.250','1.000'),
                            line.replace('1000x750','9999x750'), line.replace('1000x750','0x750'), line+line):
                log.write_text(changed)
                with self.assertRaises(ValueError):
                    session.display()

    def test_native_press_selectors_exclude_duplicate_static_labels(self):
        from packaged_acceptance.macos import Driver
        driver = Driver.__new__(Driver)
        label = {'title':'same story','role':'AXStaticText','enabled':True,'actions':[]}
        button = {'title':'same story','role':'AXButton','enabled':True,'actions':['AXPress']}
        driver.snapshot = lambda: {'nodes':[label,button]}
        self.assertEqual(driver.find('same story'), label)
        self.assertEqual(driver.find('same story', actionable=True), button)

    def test_exports_require_exact_frozen_scope_items_outcomes_and_original_bytes(self):
        from packaged_acceptance.exports import freeze, verify
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root/'archives').mkdir()
            rows = []
            for index in range(3):
                path = root/'archives'/str(index)
                path.write_bytes(('archive '+str(index)).encode())
                rows.append({'id':index+1,'uuid':'uuid-'+str(index),'managed_path':'archives/'+str(index),
                             'validation_state':'valid','byte_size':path.stat().st_size,'sha256':digest(path),
                             'original_filename':str(index)+'.zip','is_broken':int(index==2)})
            expected = freeze(rows, root, {row['uuid'] for row in rows}, 'curated_shelf',
                              [{'id':7,'name':'approved'}], root/'destination')
            operation = {'id':'new-operation','kind':'export','status':'partially_succeeded','error_code':'partial_export',
                         'export_source_type':'curated_shelf','export_destination':str(root/'destination'),
                         'completed_items':3,'total_items':3,'total_bytes':expected['total_bytes'],'cancel_requested':0,
                         'finished_at':'2000-01-01T00:00:00Z','export_source_shelf_ids':'[7]',
                         'export_source_shelf_names':'["approved"]'}
            items = [dict(value, story_uuid=uuid, operation_id=operation['id']) for uuid,value in expected['items'].items()]
            files = copy.deepcopy(expected['files'])
            self.assertEqual(verify(expected, operation, items, files)['expected'], expected)
            for change in ({'status':'failed'},{'status':'succeeded'},{'export_source_type':'selection'},
                           {'export_source_shelf_ids':'[8]'},{'export_source_shelf_names':'["other"]'},
                           {'total_items':2},{'completed_items':2},{'total_bytes':0},{'finished_at':None},
                           {'export_destination':str(root/'wrong')},{'error_code':'export_failed'}):
                with self.subTest(change=change), self.assertRaises(ValueError):
                    verify(expected, dict(operation, **change), items, files)
            for changed in (items[:-1], items+[items[0]],
                            [dict(items[0],status='failed')]+items[1:],
                            [dict(items[0],archive_sha256='f'*64)]+items[1:],
                            [dict(items[0],completed_bytes=0)]+items[1:],
                            [dict(items[0],operation_id='old-operation')]+items[1:]):
                with self.assertRaises(ValueError):
                    verify(expected, operation, changed, files)
            for changed in ({'0.zip':files['0.zip']}, {'renamed.zip':files['0.zip'],'1.zip':files['1.zip']},
                            dict(files, **{'extra.zip':files['0.zip']}),
                            dict(files, **{'0.zip':dict(files['0.zip'],bytes=0)})):
                with self.assertRaises(ValueError):
                    verify(expected, operation, items, changed)
            (root/'archives/0').write_bytes(b'changed bytes')
            with self.assertRaisesRegex(ValueError, 'stored bytes'):
                freeze(rows, root, {'uuid-0'}, 'selection', [], root/'destination')

    def test_launch_receipt_failure_retains_child_and_cleanup_always_stops_fixture(self):
        from packaged_acceptance.session import Session
        from packaged_acceptance.run import retire_owned
        with tempfile.TemporaryDirectory() as temporary:
            session = Session.__new__(Session)
            session.output = Path(temporary)
            session.command, session.environment, session.working = ['/unused'], {}, temporary
            session.launch_receipt, session.errors = {}, []
            session.process, session.reader = None, None
            child = Mock()
            child.pid = 123
            child.stdout = io.BytesIO(b'')
            child.poll.return_value = None
            child.kill.side_effect = lambda: setattr(child.poll, 'return_value', -9)
            child.wait.return_value = -9
            reader = Mock()
            reader.ident = 1
            reader.is_alive.return_value = False
            with patch('packaged_acceptance.session.subprocess.Popen', return_value=child), \
                 patch('packaged_acceptance.session.threading.Thread', return_value=reader) as thread, \
                 patch('packaged_acceptance.session.write_json', side_effect=OSError('receipt failed')):
                reader.start.side_effect = lambda: thread.call_args.kwargs['target']()
                with self.assertRaises(OSError):
                    session.start()
            self.assertIs(session.process, child)
            with patch('packaged_acceptance.session.subprocess.run', return_value=SimpleNamespace(returncode=0)):
                self.assertTrue(session.retire_failure()['retired'])
            child.kill.assert_called_once()
            reader.join.assert_called_once()
        application, fixture = Mock(), Mock()
        application.retire_failure.side_effect = ValueError('unresolved process')
        processes = [{'retired':True}]
        errors = retire_owned(application, fixture, processes)
        self.assertEqual(len(errors), 1)
        self.assertFalse(processes[0]['retired'])
        fixture.retain.assert_called_once_with(process_retired=False)

    def test_interruption_requires_exact_signal_and_retired_error_free_capture(self):
        from packaged_acceptance.session import Session
        import signal
        for french,compact in ((False,False),(False,True),(True,False),(True,True)):
            for exited, code, alive, errors in [(None,-signal.SIGKILL,False,[]),
                                            (0,-signal.SIGKILL,False,[]),
                                            (None,0,False,[]), (None,-signal.SIGKILL,True,[]),
                                            (None,-signal.SIGKILL,False,['capture failed'])]:
                with self.subTest(exited=exited, code=code, alive=alive, errors=errors), tempfile.TemporaryDirectory() as temporary:
                    session = Session.__new__(Session)
                    session.output = Path(temporary)
                    session.process = Mock()
                    session.process.poll.return_value = exited
                    session.process.wait.return_value = code
                    session.reader = Mock()
                    session.reader.is_alive.return_value = alive
                    session.errors = errors
                    fixture = Mock()
                    fixture.catalog_waiting.wait.return_value = True
                    fixture.rows.return_value = [{'id':'actual-running-id','status':'running'}]
                    valid = exited is None and code == -signal.SIGKILL and not alive and not errors
                    driver = Mock()
                    label='Synchroniser les métadonnées' if french else 'Sync official metadata'
                    driver.find.side_effect=lambda name,**kwargs: object() if name==label or (name=='Navigation' and compact) else None
                    def press(name):
                        if name not in (label,'Navigation'):
                            raise ValueError('retired first-sync banner')
                    driver.press.side_effect=press
                    if valid:
                        self.assertTrue(session.interrupt_catalog(driver, fixture, french)['retired'])
                    else:
                        with self.assertRaises(ValueError):
                            session.interrupt_catalog(driver, fixture, french)
                    expected=[unittest.mock.call('Navigation'),unittest.mock.call(label)] if compact else [unittest.mock.call(label)]
                    self.assertEqual(driver.press.call_args_list,expected)
                    if compact:
                        driver.key.assert_called_once_with('shift+Tab')
                    else:
                        driver.key.assert_not_called()
                        driver.focus.assert_called_once_with('Historique' if french else 'History',roles=('push button','AXButton'))
                    receipt = json.loads((session.output/'interruption.json').read_text())
                    self.assertEqual(receipt['retired'], valid)
                    fixture.catalog_release.set.assert_called_once()
                    self.assertFalse(fixture.pause_catalog)
                    if exited is not None:
                        session.process.kill.assert_not_called()

    def orchestration(self):
        spec = importlib.util.spec_from_file_location('packaged_orchestration',
                    Path(__file__).resolve().parents[1]/'verify-native-packages.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_container_retirement_requires_exact_creation_image_and_invocation_receipts(self):
        from subprocess import CompletedProcess
        module = self.orchestration()
        with tempfile.TemporaryDirectory(prefix='librairii-container-custody-') as temporary:
            receipt = Path(temporary)/'container-id'
            receipt.write_text('a'*64)
            observed = {'Id': 'a'*64, 'Name': '/owned', 'Image': 'sha256:image',
                        'Config': {'Labels': {'librairii.packaged-observer': 'nonce'}},
                        'State': {'Running': False}}
            with patch.object(module.subprocess, 'check_output', return_value=json.dumps([observed])), \
                 patch.object(module.subprocess, 'run', return_value=CompletedProcess([], 0)) as remove:
                self.assertEqual(module.retire(receipt, 'sha256:image', 'owned', 'nonce', False), 'a'*64)
                self.assertEqual(remove.call_args.args[0], ['docker', 'rm', 'a'*64])
            for key, value in [('Id','b'*64), ('Name','/foreign'), ('Image','sha256:foreign'),
                               ('State',{'Running':True}), ('Config',{'Labels':{'librairii.packaged-observer':'foreign'}})]:
                changed = dict(observed, **{key:value})
                with patch.object(module.subprocess, 'check_output', return_value=json.dumps([changed])), \
                     patch.object(module.subprocess, 'run') as remove:
                    with self.assertRaises(ValueError):
                        module.retire(receipt, 'sha256:image', 'owned', 'nonce', False)
                    remove.assert_not_called()

    def test_gui_receipt_requires_matching_candidate_lane_and_unchanged_raw_files(self):
        module = self.orchestration()
        with tempfile.TemporaryDirectory(prefix='librairii-gui-receipt-') as temporary:
            root = Path(temporary)
            raw = root/'native-observation.txt'
            raw.write_bytes(b'synthetic unit-test observation')
            report = {'format':'librairii.packaged-gui','schema':1,'complete':True,'errors':[],
                      'method':{'format':'librairii.packaged-gui-method','files':{'observer.py':{'sha256':'c'*64}}},
                      'checks':{name:{'observed':True} for name in REQUIRED},
                      'processes':[{'retired':True} for _ in range(5)],
                      'source_commit':'a'*40,'target':'x86_64-unknown-linux-gnu',
                      'artifact':{'sha256':'b'*64,'name':'unit.tar.gz','bytes':123},
                      'backend':'winit-skia-software','case':'fresh','session':'x11','files':inventory(root)}
            path = root/'report.json'
            write_json(path, report)
            args = [path,'a'*40,report['target'],report['artifact'],report['backend'],'fresh','x11',report['method']]
            self.assertTrue(module.verify_report(*args)['passed'])
            for position, value in [(1,'c'*40),(2,'x86_64-apple-darwin'),
                                    (3,dict(report['artifact'],sha256='c'*64)),(4,'winit-skia'),(5,'upgraded'),(6,'wayland'),
                                    (7,dict(report['method'],source_commit='d'*40))]:
                changed = list(args)
                changed[position] = value
                with self.assertRaises(ValueError):
                    module.verify_report(*changed)
            raw.write_bytes(b'tampered raw observation')
            with self.assertRaisesRegex(ValueError, 'retained GUI files'):
                module.verify_report(*args)

    def test_fixture_preserves_source_inputs_and_uses_only_explicit_roots_and_loopback(self):
        source = Path(__file__).resolve().parents[2]
        original = inventory(source/'preservation/corpus/libraries/schema018')
        with tempfile.TemporaryDirectory(prefix='librairii-acceptance-evidence-') as temporary:
            evidence = Path(temporary)
            fixture = Fixture(source, evidence, True)
            owned = fixture.root
            try:
                # The copied PK keeps the format suffix required by the frozen
                # passthrough policy; generated review archives are actual ZIPs.
                pk = [path for path in fixture.inputs.iterdir()
                      if path.read_bytes() == (source/'preservation/corpus/archives/clockwork-forest.plain.pk').read_bytes()]
                self.assertEqual(len(pk), 1)
                self.assertTrue(pk[0].name.endswith('.plain.pk'))
                self.assertEqual(len(list(fixture.review_inputs.iterdir())), 2)
                self.assertTrue(all(path.name.endswith('.zip') for path in fixture.review_inputs.iterdir()))
                environment = fixture.environment('winit-skia-software')
                self.assertEqual(environment.get('HOME'), os.environ.get('HOME'))
                self.assertEqual(environment['LIBRAIRII_ACCEPTANCE_LIBRARY'], str(owned/'library'))
                self.assertEqual(environment['LIBRAIRII_ACCEPTANCE_PROFILE'], str(owned/'profile'))
                self.assertNotIn('LIBRAIRII_DATA_ROOT', environment)
                connection = HTTPConnection('127.0.0.1', fixture.server.server_port, timeout=2)
                try:
                    connection.request('GET', '/v2/packs')
                    response = connection.getresponse()
                    self.assertEqual(response.status, 200)
                    data = json.loads(response.read())
                    self.assertEqual(data['response']['fixture-pack-001']['uuid'], '00112233-4455-4677-8899-aabbccddeeff')
                finally:
                    connection.close()
                import socket
                fixture.stop_catalog()
                self.assertFalse(fixture.online)
                self.assertFalse(fixture.thread.is_alive())
                with self.assertRaises(ConnectionRefusedError):
                    socket.create_connection(('127.0.0.1', fixture.port), timeout=2)
                fixture.scale = '1.25'
                self.assertEqual(fixture.environment('winit-skia')['SLINT_SCALE_FACTOR'], '1.25')
                fixture.start_catalog()
                connection = HTTPConnection('127.0.0.1', fixture.port, timeout=2)
                try:
                    connection.request('GET', '/guest/create')
                    response = connection.getresponse()
                    self.assertEqual(response.status, 200)
                    response.read()
                finally:
                    connection.close()
            finally:
                fixture.retain()
            self.assertFalse(owned.exists())
            self.assertEqual(inventory(source/'preservation/corpus/libraries/schema018'), original)
            self.assertTrue((evidence/'fixture-final.tar.gz').is_file())
            self.assertEqual(json.loads((evidence/'local-network.json').read_text())['errors'], [])

    def test_gui_admission_refuses_stale_source_or_changed_installed_resources_before_execution(self):
        with tempfile.TemporaryDirectory(prefix='librairii-gui-admission-') as temporary:
            installed = Path(temporary).resolve()
            binary = installed/'bin/librairii-desktop'
            binary.parent.mkdir()
            binary.write_bytes(b'synthetic executable')
            binary.chmod(0o755)
            asset = installed/'asset'
            asset.write_bytes(b'sealed resource')
            report = {'format': 'librairii.native-package-inspection', 'schema': 1,
                      'source_commit': 'a'*40, 'target': 'x86_64-unknown-linux-gnu',
                      'installed_payload_verified': True, 'public_release_qualified': False,
                      'installed': str(installed), 'binary': str(binary),
                      'binary_sha256': digest(binary), 'installed_payload': inventory(installed),
                      'build_info': {'source_commit': 'a'*40}}
            with patch('packaged_acceptance.run.verify_embedded') as execute:
                self.assertEqual(admit(report, 'a'*40, report['target']), (installed, Path('bin/librairii-desktop')))
                execute.assert_called_once()
                execute.reset_mock()
                with self.assertRaises(ValueError):
                    admit(report, 'b'*40, report['target'])
                asset.write_bytes(b'changed resource')
                with self.assertRaises(ValueError):
                    admit(report, 'a'*40, report['target'])
                execute.assert_not_called()

    def test_missing_ci_or_incomplete_gui_checkpoint_and_process_inventory_cannot_qualify(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, 'disposable CI host'):
                run(Path('/unused'), Path('/missing'), 'a'*40, 'x86_64-unknown-linux-gnu',
                    'target/native-release/refused', 'fresh', 'winit-skia-software')
        complete = {'format': 'librairii.packaged-gui', 'schema': 1, 'complete': True, 'errors': [],
                    'method':{'format':'librairii.packaged-gui-method','files':{'observer.py':{'sha256':'c'*64}}},
                    'checks': {name: {'observed': True} for name in REQUIRED},
                    'processes': [{'retired': True} for _ in range(5)]}
        self.assertTrue(qualified(complete))
        for name in REQUIRED:
            changed = copy.deepcopy(complete)
            del changed['checks'][name]
            self.assertFalse(qualified(changed))
        for change in ({'complete': False}, {'errors': ['native picker failed']}, {'processes': []}, {'method':{}},
                       {'processes': [{'retired': True}]*4+[{'retired': False}]}):
            self.assertFalse(qualified(dict(complete, **change)))


if __name__ == '__main__':
    unittest.main()
