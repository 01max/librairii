"""Admission and custody regressions; never runs a native GUI."""
import copy
import importlib.util
import io
from http.client import HTTPConnection
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release.files import digest, inventory
from native_release.files import write_json
from packaged_acceptance.fixture import Fixture
from packaged_acceptance.run import admit, qualified, run
from packaged_acceptance.workflow import REQUIRED


class PackagedAdmission(unittest.TestCase):
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
                if valid:
                    self.assertTrue(session.interrupt_catalog(Mock(), fixture, False)['retired'])
                else:
                    with self.assertRaises(ValueError):
                        session.interrupt_catalog(Mock(), fixture, False)
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
                      'checks':{name:{'observed':True} for name in REQUIRED},
                      'processes':[{'retired':True} for _ in range(5)],
                      'source_commit':'a'*40,'target':'x86_64-unknown-linux-gnu',
                      'artifact':{'sha256':'b'*64,'name':'unit.tar.gz','bytes':123},
                      'backend':'winit-skia-software','case':'fresh','session':'x11','files':inventory(root)}
            path = root/'report.json'
            write_json(path, report)
            args = [path,'a'*40,report['target'],report['artifact'],report['backend'],'fresh','x11']
            self.assertTrue(module.verify_report(*args)['passed'])
            for position, value in [(1,'c'*40),(2,'x86_64-apple-darwin'),
                                    (3,dict(report['artifact'],sha256='c'*64)),(4,'winit-skia'),(5,'upgraded'),(6,'wayland')]:
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
                    'checks': {name: {'observed': True} for name in REQUIRED},
                    'processes': [{'retired': True} for _ in range(5)]}
        self.assertTrue(qualified(complete))
        for name in REQUIRED:
            changed = copy.deepcopy(complete)
            del changed['checks'][name]
            self.assertFalse(qualified(changed))
        for change in ({'complete': False}, {'errors': ['native picker failed']}, {'processes': []},
                       {'processes': [{'retired': True}]*4+[{'retired': False}]}):
            self.assertFalse(qualified(dict(complete, **change)))


if __name__ == '__main__':
    unittest.main()
