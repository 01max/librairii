"""CI-only, nonqualifying stack sample of the exact owned loaded-workload PID."""
import json
import os
from pathlib import Path
import platform
import subprocess
import threading
import time

class PointerProfile:
    def __init__(self, directory, binary):
        self.directory = directory
        self.binary = binary
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.receipt = {'release_qualified': False, 'started': False, 'finished': False,
                        'purpose': 'App main-thread stacks during original pointer cohort; sampling perturbs timings'}

    def start(self):
        if os.environ.get('GITHUB_ACTIONS') != 'true' or (platform.system(), platform.machine()) != ('Darwin', 'x86_64'):
            raise ValueError('sampling restricted to disposable Intel GitHub Actions')
        self.thread.start()

    def run(self):
        task = None
        try:
            deadline = time.monotonic()+240
            while not self.stop.wait(0.02):
                if time.monotonic() > deadline:
                    raise ValueError('owned pointer cohort was not observed within diagnostic deadline')
                try:
                    state = json.loads((self.directory/'state.json').read_text())
                except (FileNotFoundError, json.JSONDecodeError):
                    continue
                dispatch = state.get('dispatch', {})
                if (not (self.directory/'pointer-target.json').is_file() or
                        (self.directory/'dispatch-clock-request').exists() or
                        dispatch.get('search_sequence') != 64 or
                        dispatch.get('browser_focus_sequence', 0) < 1 or
                        state.get('focus_key') != 31 or not state.get('running_import')):
                    continue
                pid = state['pid']
                if type(pid) is not int or pid <= 1:
                    raise ValueError('positive owned PID required')
                command = subprocess.check_output(['/bin/ps', '-p', str(pid), '-o', 'command='],
                                                  text=True, timeout=3).strip()
                if not command.startswith(str(self.binary)+' ') or str(self.directory/'fixture.json') not in command:
                    raise ValueError('sample target differs from exact owned workload command')
                self.receipt.update(started=True, pid=pid, command=command,
                                    pointer_sequence=state['dispatch']['browser_focus_sequence'],
                                    start_phase={'search_sequence': dispatch['search_sequence'],
                                                 'pointer_sequence': dispatch['browser_focus_sequence'],
                                                 'focus_key': state['focus_key']},
                                    start_monotonic=time.monotonic(), duration_seconds=5, interval_ms=5)
                with (self.directory/'sample-tool.log').open('wb') as log:
                    task = subprocess.Popen(['/usr/bin/sample', str(pid), '5', '5', '-mayDie',
                                             '-file', str(self.directory/'pointer-stacks.txt')],
                                            stdout=log, stderr=subprocess.STDOUT)
                    self.receipt['sampler_pid'] = task.pid
                    self.receipt['returncode'] = task.wait(timeout=35)
                self.receipt['end_monotonic'] = time.monotonic()
                end_state = json.loads((self.directory/'state.json').read_text())
                self.receipt['end_phase'] = {
                    'pid': end_state['pid'], 'focus_key': end_state['focus_key'],
                    'search_sequence': end_state['dispatch']['search_sequence'],
                    'pointer_sequence': end_state['dispatch']['browser_focus_sequence'],
                    'calibration_request_present': (self.directory/'dispatch-clock-request').exists()}
                self.receipt['same_search_focus_at_boundaries'] = (
                    end_state['pid'] == pid and end_state['focus_key'] == 31 and
                    end_state['dispatch']['search_sequence'] == 64)
                self.receipt['finished'] = True
                return
        except Exception as error:
            self.receipt['error'] = str(error)
        finally:
            if task is not None and task.poll() is None:
                task.terminate()
                try:
                    task.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    task.kill()
                    task.wait(timeout=5)
            if task is not None:
                self.receipt['sampler_retired'] = task.poll() is not None

    def finish(self):
        self.stop.set()
        self.thread.join(timeout=55)
        if self.thread.is_alive():
            raise ValueError('owned sampler thread did not finish')
        stack = self.directory/'pointer-stacks.txt'
        self.receipt['stack_file_present'] = stack.is_file() and stack.stat().st_size > 0
        (self.directory/'pointer-profile.json').write_text(json.dumps(self.receipt, indent=2)+'\n')
        return (self.receipt['started'] and self.receipt['finished'] and
                'error' not in self.receipt and 'end_phase' in self.receipt and
                self.receipt.get('returncode') == 0 and self.receipt.get('sampler_retired') is True and
                self.receipt['stack_file_present'])
