#!/usr/bin/env python3
"""[DEBUG-task17-startup-stack] Profile two owned startup processes only."""
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]


class Sampler:
    def __init__(self, destination):
        self.destination = destination
        self.process = None
        self.receipt = {'scope': 'diagnostic only; profiler changes timing; never qualification',
                        'duration_seconds': 3, 'interval_ms': 1, 'joined': False}

    def publish(self):
        (self.destination / 'startup-sampler.json').write_text(json.dumps(self.receipt, indent=2) + '\n')

    def start(self, pid, parent):
        assert self.process is None and os.getpgid(pid) == parent
        self.receipt.update(product_pid=pid, owned_product_group=parent, requested_ns=time.monotonic_ns())
        log = self.destination / 'startup-sampler.log'
        self.log = log.open('wb')
        try:
            self.process = subprocess.Popen(['/usr/bin/sample', str(pid), '3', '1', '-file',
                                             str(self.destination / 'startup-stacks.txt')],
                                            stdin=subprocess.DEVNULL, stdout=self.log, stderr=self.log,
                                            start_new_session=True)
        except BaseException:
            self.log.close()
            raise
        self.receipt.update(sampler_pid=self.process.pid, launched_ns=time.monotonic_ns())
        self.publish()

    def finish(self, allow_full_capture):
        if self.receipt['joined']:
            return
        process = self.process
        if process is None:
            self.receipt['error'] = 'owned product PID was never sampled'
            self.publish()
            raise ValueError(self.receipt['error'])
        try:
            if allow_full_capture:
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    self.receipt['capture_timeout'] = True
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGINT)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
            self.receipt.update(returncode=process.returncode, ended_ns=time.monotonic_ns())
            try:
                os.killpg(process.pid, 0)
            except OSError as error:
                if error.errno != errno.ESRCH:
                    raise
                self.receipt['joined'] = True
            else:
                raise ValueError('owned sampler process group still exists')
            path = self.destination / 'startup-stacks.txt'
            size = path.stat().st_size if path.exists() else 0
            self.receipt['stack_bytes'] = size
            if not 0 < size <= 16 * 1024 * 1024:
                raise ValueError('bounded native startup stack file missing')
            self.receipt['stack_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
            self.receipt['complete'] = (allow_full_capture and process.returncode == 0
                                        and not self.receipt.get('capture_timeout', False))
        finally:
            self.log.close()
            self.publish()
        if not self.receipt.get('complete', False):
            raise ValueError('startup sampler did not complete normally')


def main():
    assert sys.argv[1:] in ([], ['--late-first'])
    assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('GITHUB_REPOSITORY') == '01max/librairii'
    spec = importlib.util.spec_from_file_location('startup', Path(__file__).with_name('native-startup-diagnostic.py'))
    startup = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(startup)
    original_load = startup.load

    def load(name, path):
        module = original_load(name, path)
        if name != 'resources':
            return module
        product_run = module.product_run
        index = 0

        def profiled(binary, observer, destination, backend, helper, native):
            nonlocal index
            sampled = index < 2
            index += 1
            if not sampled:
                return product_run(binary, observer, destination, backend, helper, native)
            sampler = Sampler(destination)
            child_pid = module.child_pid
            receipt = module.StartupCapture.receipt

            def owned_child(parent):
                pid = child_pid(parent)
                if pid is not None and sampler.process is None:
                    sampler.start(pid, parent)
                return pid

            def quiet_receipt(capture, pid):
                marker = receipt(capture, pid)
                # The original parent-delivery timestamp is already captured.
                # Join the profiler before the separate15-second quiet phase.
                sampler.finish(True)
                return marker

            module.child_pid = owned_child
            module.StartupCapture.receipt = quiet_receipt
            try:
                return product_run(binary, observer, destination, backend, helper, native)
            finally:
                module.child_pid = child_pid
                module.StartupCapture.receipt = receipt
                try:
                    sampler.finish(False)
                except Exception as error:
                    # Unknown observer retirement must stop the whole plan,
                    # rather than becoming a recoverable per-cohort gate error.
                    raise SystemExit(f'startup profiler incomplete: {error}') from error

        module.product_run = profiled
        return module

    startup.load = load
    startup.main()


if __name__ == '__main__':
    main()
