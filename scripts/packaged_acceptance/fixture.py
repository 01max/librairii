"""Owned temporary profiles/libraries and local catalog for installed GUI runs."""
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import platform
import shutil
import sqlite3
import tempfile
import threading
import zipfile

from native_release.archive import tar_gzip
from native_release.files import copy_tree, digest, inventory, write_json


class Fixture:
    def __init__(self, source, evidence, upgraded):
        # Explicit custody: unresolved product ownership must never trigger an
        # automatic TemporaryDirectory finalizer under a still-running child.
        self.root = Path(tempfile.mkdtemp(prefix='librairii-packaged-')).resolve()
        self.profile, self.library = self.root/'profile', self.root/'library'
        self.profile.mkdir()
        self.library.mkdir()
        (self.root/'.librairii-packaged-fixture').write_bytes(b'librairii-packaged-acceptance-v1\n')
        self.inputs, self.exports = self.root/'inputs', self.root/'exports'
        self.inputs.mkdir()
        self.exports.mkdir()
        for old, new in [('clockwork-forest.zip', 'Été 日本語 故事.zip'),
                         ('clockwork-forest.plain.pk', 'Deuxième archive 02.pk')]:
            shutil.copyfile(source/'preservation/corpus/archives'/old, self.inputs/new)
        self.input_bytes = inventory(self.inputs)
        self.review_inputs = self.root/'review-inputs'
        self.review_inputs.mkdir()
        self.review_uuid = '90000000-0000-4000-8000-000000000018'
        with zipfile.ZipFile(source/'preservation/corpus/archives/clockwork-forest.zip') as original:
            for filename in ('Nouvelle histoire 03.zip', 'Nouvelle archive 04.pk'):
                with zipfile.ZipFile(self.review_inputs/filename, 'x', compression=zipfile.ZIP_DEFLATED) as changed:
                    for entry in original.infolist():
                        relative = entry.filename.split('/', 1)[1]
                        changed.writestr(self.review_uuid+'/'+relative, original.read(entry))
        self.review_bytes = inventory(self.review_inputs)
        self.candidate, self.moved = self.library/'candidate', self.library/'moved'
        self.candidate.mkdir()
        self.moved.mkdir()
        if platform.system() == 'Darwin':
            base = self.profile/'Library/Preferences'
            self.native_profile = base/'io.github.librairii.native'
            legacy_profile = base/'io.github.librairii.app'
        else:
            base = self.profile/'.config'
            self.native_profile = base/'librairii-native'
            legacy_profile = base/'librairii'
        self.legacy = None
        if upgraded:
            self.legacy = self.library/'legacy-source'
            copy_tree(source/'preservation/corpus/libraries/schema018', self.legacy)
            self.legacy_before = inventory(self.legacy)
            legacy_profile.mkdir(parents=True)
            write_json(legacy_profile/'settings.json', {
                'version': 1, 'language': 'en', 'root': str(self.legacy),
                'libraryId': (self.legacy/'.librairii-library').read_text().strip(), 'complete': True})
        self.requests, self.errors = [], []
        self.pause_catalog = False
        self.catalog_waiting = threading.Event()
        self.catalog_release = threading.Event()
        fixture = self
        catalog_fixture = json.loads((source/'crates/librairii-app/tests/fixtures/lunii-catalog.json').read_text())
        catalog_fixture['response']['fixture-pack-001']['uuid'] = '00112233-4455-4677-8899-aabbccddeeff'
        catalog = json.dumps(catalog_fixture).encode()
        icon = (source/'build/appicon.png').read_bytes()
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                paused = False
                if len(fixture.requests) >= 128:
                    fixture.errors.append('local request bound exceeded')
                    self.send_error(429)
                    return
                fixture.requests.append({'method': 'GET', 'path': self.path, 'client': self.client_address[0]})
                if self.path == '/guest/create':
                    body, media = b'{"response":{"token":{"server":"fixture-only-guest"}}}', 'application/json'
                elif self.path == '/v2/packs':
                    if fixture.pause_catalog:
                        paused = True
                        fixture.catalog_waiting.set()
                        if not fixture.catalog_release.wait(15):
                            fixture.errors.append('paused catalog fixture exceeded deadline')
                    body, media = catalog, 'application/json'
                elif self.path.startswith('/lunii-data-prod/fixture/'):
                    body, media = icon, 'image/png'
                else:
                    fixture.errors.append('unexpected local request: '+self.path)
                    self.send_error(404)
                    return
                self.send_response(200)
                self.send_header('Content-Type', media)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                try:
                    self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError):
                    if not paused:
                        fixture.errors.append('unexpected disconnected fixture client')
            def log_message(self, *_):
                pass
        self.server = HTTPServer(('127.0.0.1', 0), Handler)
        self.server.timeout = 1
        self.thread = threading.Thread(target=self.server.serve_forever)
        self.evidence = evidence
        try:
            write_json(evidence/'fixture.json', {'owner': str(self.root), 'upgraded': upgraded,
                       'profile': str(self.profile), 'library': str(self.library),
                       'input_files': self.input_bytes, 'legacy_before': self.legacy_before if upgraded else None})
        except Exception:
            self.server.server_close()
            raise
        self.thread.start()

    def environment(self, backend):
        environment = dict(os.environ)
        environment.pop('LIBRAIRII_DATA_ROOT', None)
        environment.update({
            'PATH': '/usr/bin:/bin:/usr/sbin:/sbin',
            'LIBRAIRII_PACKAGED_ACCEPTANCE': '1',
            'LIBRAIRII_ACCEPTANCE_ROOT': str(self.root),
            'LIBRAIRII_ACCEPTANCE_PROFILE': str(self.profile),
            'LIBRAIRII_ACCEPTANCE_LIBRARY': str(self.library),
            'LIBRAIRII_ACCEPTANCE_CATALOG': 'http://127.0.0.1:'+str(self.server.server_port)+'/',
            'SLINT_BACKEND': backend,
            'SLINT_SCALE_FACTOR': '1',
        })
        return environment

    def current_root(self):
        profile = json.loads((self.native_profile/'settings.json').read_text())
        root = Path(profile['root'])
        if not root.is_relative_to(self.library) or root.resolve() != root:
            raise ValueError('native profile escaped owned library')
        return root

    def rows(self, statement):
        root = self.current_root()
        uri = (root/'db/librairii.sqlite3').as_uri()+'?mode=ro'
        with sqlite3.connect(uri, uri=True) as connection:
            connection.execute('PRAGMA query_only=ON')
            connection.row_factory = sqlite3.Row
            return [{key: ({'blob_hex': value.hex()} if isinstance(value, bytes) else value)
                     for key,value in dict(row).items()} for row in connection.execute(statement)]

    def retain(self, *, process_retired=True):
        self.catalog_release.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise ValueError('local catalog thread did not stop')
        write_json(self.evidence/'local-network.json', {'requests': self.requests, 'errors': self.errors})
        failures = list(self.errors)
        if inventory(self.inputs) != self.input_bytes:
            failures.append('original import fixtures changed')
        if inventory(self.review_inputs) != self.review_bytes:
            failures.append('generated review import fixtures changed')
        if self.legacy and inventory(self.legacy) != self.legacy_before:
            failures.append('Go-era source changed during independent adoption')
        write_json(self.evidence/'fixture-final.json', inventory(self.root))
        tar_gzip(self.root, self.evidence/'fixture-final.tar.gz', 'owned-fixture', 0)
        if process_retired:
            shutil.rmtree(self.root)
        else:
            # Preserve data still potentially held by an unresolved process.
            failures.append('product retirement uncertain; fixture retained at '+str(self.root))
        if failures:
            raise ValueError('fixture custody/network failure: '+str(failures))
