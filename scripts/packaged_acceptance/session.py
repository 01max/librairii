"""Own the actual installed process, bounded raw logs and native shutdown."""
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import threading

from native_release.files import digest, write_json


class Session:
    def __init__(self, binary, fixture, backend, output):
        if os.environ.get('GITHUB_ACTIONS') != 'true':
            raise ValueError('installed GUI execution requires a disposable CI host')
        self.output, self.binary = output, binary
        output.mkdir()
        self.errors = []
        environment = fixture.environment(backend)
        working = fixture.root/'working-directory'
        working.mkdir(exist_ok=True)
        command = [str(binary)]
        if platform.system() == 'Darwin':
            # The application may execute only its installed binary and Apple's
            # reveal launcher. Its qualification observers run outside this
            # sandbox; Go/Rust/Node/archive tools cannot service product work.
            quote = lambda value: '"'+str(value).replace('\\','\\\\').replace('"','\\"')+'"'
            policy = ('(version 1)(allow default)(deny network-outbound)'
                      '(allow network-outbound (remote ip "localhost:*"))'
                      '(deny file-write*)(allow file-write* (subpath '+quote(fixture.root)+')'
                      '(subpath '+quote(Path(os.environ.get('TMPDIR','/private/tmp')).resolve())+'))'
                      '(deny process-exec)(allow process-exec (literal '+quote(binary)+')'
                      '(literal "/usr/bin/open"))')
            checkout = os.environ.get('GITHUB_WORKSPACE')
            if checkout:
                policy += '(deny file-read* (subpath '+quote(Path(checkout).resolve())+'))'
            (output/'product.sb').write_text(policy+'\n')
            command = ['/usr/bin/sandbox-exec', '-p', policy, str(binary)]
        self.process = subprocess.Popen(command, cwd=working, env=environment,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        self.pid = self.process.pid
        write_json(output/'launch.json', {'pid': self.pid, 'binary': str(binary),
                   'binary_sha256': digest(binary), 'cwd': str(working), 'backend': backend,
                   'path': environment['PATH'], 'qualification_root': str(fixture.root),
                   'profile': str(fixture.profile), 'library': str(fixture.library)})
        def capture():
            total = 0
            with self.process.stdout as source, (output/'product.log').open('xb') as log:
                while data := source.read1(65536):
                    total += len(data)
                    log.write(data[:max(0, 4194304-(total-len(data)))])
                    log.flush()
                    if total > 4194304:
                        self.errors.append('product output exceeded4MiB')
                        self.process.kill()
                        return
        self.reader = threading.Thread(target=capture)
        self.reader.start()

    def runtime(self):
        if platform.system() == 'Linux':
            mappings = Path('/proc')/str(self.pid)/'maps'
            observed = mappings.read_text()
        else:
            observed = subprocess.check_output(['/usr/sbin/lsof', '-p', str(self.pid), '-Fn'], text=True, timeout=15)
        if len(observed.encode()) > 4194304:
            raise ValueError('runtime mapping inventory exceeds bound')
        (self.output/'runtime-mappings.txt').write_text(observed)
        if re.search(r'(?i)(webkit|javascriptcore|webview2|libcef|chromium|/node(?:$|[./]))', observed):
            raise ValueError('installed product loaded an undeclared browser/Node runtime')
        return {'pid': self.pid, 'mappings_sha256': digest(self.output/'runtime-mappings.txt'),
                'method': 'proc-maps' if platform.system() == 'Linux' else 'lsof',
                'forbidden_runtime_found': False}

    def close(self, driver, french):
        driver.quit(french)
        code = self.process.wait(timeout=20)
        self.reader.join(timeout=5)
        text = (self.output/'product.log').read_text(errors='replace')
        if code != 0 or self.reader.is_alive() or self.errors or 'native_shutdown; stage=controller_joined; ok=true' not in text:
            raise ValueError('installed product did not complete native worker shutdown')
        return {'returncode': code, 'actual_worker_joined': True}

    def interrupt_catalog(self, driver, fixture, french):
        fixture.pause_catalog = True
        driver.press('Synchroniser les métadonnées' if french else 'Sync official metadata')
        if not fixture.catalog_waiting.wait(10):
            raise ValueError('no actual in-flight local catalog request before interruption')
        running = fixture.rows("SELECT id,status FROM file_operations WHERE kind='metadata_sync' AND status='running'")
        if not running:
            raise ValueError('metadata job was not durably running before interruption')
        receipt = {'actual_local_request_in_flight': True, 'running': running, 'retired': False}
        try:
            if self.process.poll() is not None:
                raise ValueError('product exited before the requested interruption')
            self.process.kill()
            receipt['returncode'] = self.process.wait(timeout=10)
            self.reader.join(timeout=5)
            receipt['reader_retired'] = not self.reader.is_alive()
            receipt['capture_errors'] = list(self.errors)
            if receipt['returncode'] != -signal.SIGKILL or not receipt['reader_retired'] or self.errors:
                raise ValueError('interruption lacks an exact SIGKILL and retired error-free capture')
            receipt.update(signal='SIGKILL', retired=True)
            return receipt
        except Exception as error:
            receipt['error'] = str(error)
            raise
        finally:
            fixture.catalog_release.set()
            fixture.pause_catalog = False
            write_json(self.output/'interruption.json', receipt)

    def retire_failure(self):
        # Popen owns this unreaped child; no name/PID search authorizes killing.
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.reader.join(timeout=5)
        if self.reader.is_alive():
            raise ValueError('product output reader survived owned retirement')
