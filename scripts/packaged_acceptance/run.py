"""Checksum-bound production GUI, restart, recovery and reinstall qualification."""
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import traceback

from native_release.files import copy_tree, digest, fresh_output, inventory, write_json
from native_release.identity import runtime_host, verify_embedded
from .fixture import Fixture
from .session import Session
from .workflow import REQUIRED, perform

TABLES = ('stories', 'story_archives', 'tag_definitions', 'tag_values', 'story_tag_assignments',
          'collections', 'shelves', 'shelf_stories', 'shelf_sources', 'shelf_story_origins',
          'shelf_exclusions', 'file_operations', 'file_operation_items')


def durable(fixture):
    return {table: fixture.rows('SELECT * FROM '+table+' ORDER BY rowid') for table in TABLES}


def retire_owned(application, fixture, processes):
    errors, retired = [], application is None
    try:
        if application:
            receipt = application.retire_failure()
            retired = receipt['retired']
            if processes:
                processes[-1]['retirement'] = receipt
    except Exception:
        errors.append('owned process cleanup: '+traceback.format_exc())
    if application and processes:
        processes[-1]['retired'] = retired
    try:
        fixture.retain(process_retired=retired)
    except Exception:
        errors.append('owned fixture cleanup: '+traceback.format_exc())
    return errors


def admit(inspection, expected_source, target):
    if (inspection.get('format') != 'librairii.native-package-inspection' or inspection.get('schema') != 1
            or inspection.get('source_commit') != expected_source or inspection.get('target') != target
            or inspection.get('installed_payload_verified') is not True
            or inspection.get('public_release_qualified') is not False):
        raise ValueError('GUI qualification requires the exact externally selected inspected package')
    installed = Path(inspection['installed']).resolve(strict=True)
    if inventory(installed) != inspection['installed_payload']:
        raise ValueError('installed payload changed before GUI qualification')
    binary = Path(inspection['binary'])
    relative = binary.relative_to(installed)
    expected = ('Librairii.app/Contents/MacOS/librairii-desktop'
                if target.endswith('apple-darwin') else 'bin/librairii-desktop')
    if relative.as_posix() != expected or binary.is_symlink() or digest(binary) != inspection['binary_sha256']:
        raise ValueError('GUI executable is not the exact inspected installed production binary')
    verify_embedded(installed, binary, inspection['build_info'])
    return installed, relative


def qualified(report):
    return (report.get('format') == 'librairii.packaged-gui' and report.get('schema') == 1
            and report.get('complete') is True and report.get('errors') == []
            and report.get('method', {}).get('format') == 'librairii.packaged-gui-method'
            and bool(report['method'].get('files'))
            and set(report.get('checks', {})) == set(REQUIRED)
            and all(isinstance(value, dict) and value for value in report['checks'].values())
            and len(report.get('processes', [])) == 5
            and all(process.get('retired') is True for process in report['processes']))


def run(root, inspection_path, source, target, requested_output, case, backend, *, desktop=None, helper=None,
        fixture_source=None, method=None, observer_source=None):
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise ValueError('installed GUI execution requires a disposable CI host')
    if not method or method.get('format') != 'librairii.packaged-gui-method' or not method.get('files'):
        raise ValueError('installed GUI qualification requires independently recorded observer source')
    inspection = json.loads(Path(inspection_path).read_text())
    installed, relative = admit(inspection, source, target)
    host = runtime_host(root, target)
    output = fresh_output(root, requested_output)
    report = {'format': 'librairii.packaged-gui', 'schema': 1, 'source_commit': source,
              'target': target, 'artifact': inspection['artifact'], 'host': host, 'case': case,
              'backend': backend, 'session': desktop.session if desktop else 'macOS',
              'method': method,
              'checks': {}, 'processes': [], 'errors': [], 'complete': False,
              'signing': inspection['signing'], 'public_release_qualified': False}
    fixture = Fixture(fixture_source or root, output, case == 'upgraded')
    application = None
    driver = None
    installation = fixture.root/'installation'
    binary = installation/relative
    def launch(name):
        nonlocal application, driver
        application = Session(binary, fixture, backend, output/name)
        process = {'name': name, 'pid': None, 'binary_sha256': digest(binary), 'retired': False}
        report['processes'].append(process)
        application.start()
        process['pid'] = application.pid
        if desktop:
            from .linux import Driver
            driver = Driver(observer_source or root, output/name, application.pid, desktop)
        else:
            from .macos import Driver
            driver = Driver(observer_source or root, output/name, application.pid, helper)
        return process
    def close(process):
        process['shutdown'] = application.close(driver, french)
        process['retired'] = True
    def observe(name, value):
        if name not in REQUIRED or name in report['checks']:
            raise ValueError('unexpected/duplicate packaged acceptance checkpoint')
        driver.snapshot(name)
        report['checks'][name] = value
    french = case == 'upgraded'
    words = lambda en, fr: fr if french else en
    try:
        copy_tree(installed, installation)
        if inventory(installation) != inspection['installed_payload']:
            raise ValueError('isolated installed copy differs from the final distributed payload')
        first = launch('workflow')
        perform(driver, fixture, report, french)
        first_display = driver.wait(application.display)
        # Remove through the detail dialog, then independently verify managed
        # custody and unchanged original inputs. No direct DB mutation drives UI.
        driver.press(words('Library', 'Bibliothèque'))
        driver.text(words('Search stories', 'Rechercher des histoires'), 'Clockwork')
        driver.press('The Clockwork Mountain', prefix=True)
        driver.press(words('Details…', 'Détails…'))
        managed = fixture.rows("SELECT a.sha256,a.managed_path FROM story_archives a JOIN stories s ON s.id=a.story_id WHERE s.uuid='00112233-4455-4677-8899-aabbccddeeff'")
        driver.press(words('Remove from library…', 'Retirer de la bibliothèque…'))
        driver.press(words('Move to trash', 'Déplacer dans la corbeille'))
        driver.wait(lambda: not fixture.rows("SELECT id FROM stories WHERE uuid='00112233-4455-4677-8899-aabbccddeeff'"))
        trash = inventory(fixture.current_root()/'trash')
        assert any(facts['sha256'] in {row['sha256'] for row in managed} for facts in trash.values())
        observe('trash-removal', {'managed_before': managed, 'retained_trash': trash})
        before = inventory(fixture.current_root()/'archives')
        driver.wait(lambda: driver.find(words('Search stories', 'Rechercher des histoires')))
        driver.settings(french)
        driver.press(words('Change library folder…', 'Changer le dossier de la bibliothèque…'))
        driver.picker(fixture.moved)
        driver.press(words('Move the complete library', 'Déplacer toute la bibliothèque'))
        driver.wait(lambda: fixture.current_root() == fixture.moved, timeout=40)
        assert inventory(fixture.moved/'archives') == before
        observe('relocation', {'destination': str(fixture.moved), 'managed_archives': before})
        driver.press(words('Close', 'Fermer'))
        first['runtime'] = application.runtime()
        close(first)
        saved = durable(fixture)
        fixture.stop_catalog()
        try:
            with socket.create_connection(('127.0.0.1',fixture.port), timeout=2):
                raise ValueError('offline startup still has a listening local catalog')
        except ConnectionRefusedError:
            pass
        offline_requests = len(fixture.requests)
        fixture.scale = '1.25'
        second = launch('reopen')
        driver.wait(lambda: driver.find(words('Search stories', 'Rechercher des histoires')))
        assert durable(fixture) == saved
        assert json.loads((fixture.native_profile/'settings.json').read_text())['language'] == ('fr' if french else 'en')
        observe('reopen', {'root': str(fixture.current_root()), 'durable_state': saved, 'same_package_sha256': inspection['artifact']['sha256']})
        observe('scaling', {'initial': first_display, 'reopen': driver.wait(application.display),
                            'native_window_and_controls_observed': True})
        if fixture.online or fixture.thread.is_alive() or len(fixture.requests) != offline_requests:
            raise ValueError('offline startup unexpectedly contacted a catalog listener')
        observe('offline-startup', {'catalog_connection_refused': True, 'catalog_thread_retired': True,
                                    'retained_catalog_requests': offline_requests, 'durable_state_unchanged': True})
        fixture.start_catalog()
        interruption = application.interrupt_catalog(driver, fixture, french)
        second['interrupted'] = interruption
        second['retired'] = True
        third = launch('interrupted-reopen')
        driver.wait(lambda: driver.find(words('Search stories', 'Rechercher des histoires')))
        ids = {row['id'] for row in interruption['running']}
        settled = driver.wait(lambda: (rows := fixture.rows(
            "SELECT id,status,error_code FROM file_operations WHERE kind='metadata_sync'")) and
            all(any(row['id'] == identity and row['status'] == 'interrupted' for row in rows) for identity in ids) and rows)
        driver.press(words('History', 'Historique'))
        driver.snapshot('interrupted-history')
        driver.press(words('Close', 'Fermer'))
        close(third)
        disconnected = fixture.library/'reconnected-library'
        previous_root = fixture.current_root()
        previous_root.rename(disconnected)
        fourth = launch('location-recovery')
        driver.wait(lambda: driver.find(words('Locate the same library…', 'Retrouver la même bibliothèque…'))
                    or driver.find(words('Recovery…', 'Récupération…')))
        if not driver.find(words('Locate the same library…', 'Retrouver la même bibliothèque…')):
            driver.press(words('Recovery…', 'Récupération…'))
        driver.press(words('Locate the same library…', 'Retrouver la même bibliothèque…'))
        driver.picker(disconnected)
        driver.wait(lambda: fixture.current_root() == disconnected, timeout=30)
        assert inventory(disconnected/'archives') == before
        observe('recovery', {'interruption': interruption, 'settled': settled,
                            'same_library_reconnected': str(disconnected), 'managed_archives': before})
        driver.press(words('Close', 'Fermer'))
        close(fourth)
        preserved = {'library': inventory(fixture.library), 'profile': inventory(fixture.profile)}
        assert inventory(installation) == inspection['installed_payload']
        shutil.rmtree(installation)  # Exact fresh owned copy, after all product processes joined.
        assert {'library': inventory(fixture.library), 'profile': inventory(fixture.profile)} == preserved
        copy_tree(installed, installation)
        assert inventory(installation) == inspection['installed_payload']
        final = launch('reinstall')
        driver.wait(lambda: driver.find(words('Search stories', 'Rechercher des histoires')))
        assert inventory(disconnected/'archives') == before
        observe('uninstall-reinstall', {'independent_data_preserved': preserved,
                                     'reinstalled_payload': inspection['installed_payload']})
        close(final)
        report['complete'] = True
        if not qualified(report):
            raise ValueError('incomplete installed GUI checkpoint/process inventory')
    except Exception:
        report['complete'] = False
        report['errors'].append(traceback.format_exc())
        if driver:
            try:
                driver.snapshot('failure')
            except Exception:
                report['errors'].append('failure snapshot: '+traceback.format_exc())
        raise
    finally:
        cleanup_errors = retire_owned(application, fixture, report['processes'])
        if cleanup_errors:
            report['complete'] = False
            report['errors'].extend(cleanup_errors)
        report['files'] = inventory(output)
        write_json(output/'report.json', report)
    return report
