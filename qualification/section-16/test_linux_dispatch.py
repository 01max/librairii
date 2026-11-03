"""Headless boundary/clock regressions; no native application or input is run."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


linux = load('linux_dispatch')
protocol = load('dispatch')
fixtures = load('test_dispatch')


class LinuxDispatchTests(unittest.TestCase):
    def test_actual_call_order_retains_process_return_and_observation(self):
        events = []
        ack = {'kind': 'browser-focus', 'sequence': 8, 'native_ns': 20, 'query': ''}
        end = {'focus_key': 31, 'dispatch': {'browser_focus': ack}}
        def post():
            events.append('post')
        def read():
            events.append('read')
            return end
        def wait(predicate, seconds):
            self.assertEqual(seconds, 1)
            return predicate()
        with patch.object(linux.time, 'monotonic_ns', side_effect=[1_000, 121_001_000, 131_001_000]):
            state, wall, raw = linux.observe(post, read, lambda s: s['focus_key'] == 31,
                {'dispatch': {'browser_focus_sequence': 7}}, 'browser-focus', 0, wait)
        self.assertEqual(events, ['post', 'read'])
        self.assertEqual(wall, 131)
        self.assertEqual(raw['posted_ns'] - raw['start_ns'], 121_000_000)
        self.assertIs(state, end)
        self.assertEqual(raw['ack'], ack)

    def test_stale_or_skipped_callback_is_rejected(self):
        for sequence in [7, 9]:
            end = {'dispatch': {'browser_focus': {'kind': 'browser-focus', 'sequence': sequence}}}
            with self.assertRaisesRegex(ValueError, 'one new actual'):
                linux.observe(lambda: None, lambda: end, lambda s: True,
                    {'dispatch': {'browser_focus_sequence': 7}}, 'browser-focus', 0, lambda p, seconds: p())

    def measured(self, callback=5, publication=200, posting=1):
        samples, clocks = fixtures.observations(callback, publication, posting)
        pointers = copy.deepcopy(samples)
        for row in pointers:
            row['ack'].update(kind='browser-focus', query='')
        result = {'clock_calibrations': clocks}
        for prefix, rows, key in [('input', samples, 'search'), ('pointer', pointers, 'browser_focus')]:
            result[prefix + '_dispatch_samples'] = rows
            result[prefix + '_states'] = [{'dispatch': {key: row['ack']}} for row in rows]
            result[prefix + '_ms'] = [(row['observed_ns']-row['start_ns'])/1e6 for row in rows]
        return result

    def test_diagnostic_partitions_cost_without_rescoring_acceptance(self):
        for callback, publication, posting in [(5, 200, 1), (205, 10, 1), (125, 10, 120)]:
            with self.subTest(callback=callback, publication=publication, posting=posting):
                measured = self.measured(callback, publication, posting)
                before = copy.deepcopy(measured)
                result = linux.summarize(measured, protocol)
                self.assertEqual(measured, before)
                self.assertFalse(result['acceptance_evidence'])
                self.assertGreater(sorted(measured['pointer_ms'])[60], 100)
                row = result['cohorts']['pointer']['samples'][0]
                self.assertEqual(row['process_return_ms'], posting)
                self.assertLessEqual(row['callback_lower_ms'], callback)
                self.assertGreaterEqual(row['callback_upper_ms'], callback)
                self.assertLessEqual(row['callback_to_observation_lower_ms'], publication)
                self.assertGreaterEqual(row['callback_to_observation_upper_ms'], publication)

    def test_wrong_witness_or_wall_is_rejected(self):
        for kind in ['wall', 'ack']:
            m = self.measured()
            if kind == 'wall':
                m['pointer_ms'][0] += 1
            else:
                m['pointer_states'][0] = {'dispatch': {'browser_focus': {}}}
            with self.assertRaisesRegex(ValueError, 'witnesses differ'):
                linux.summarize(m, protocol)

    def test_all64_clock_responses_are_ordered_and_bound(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            ticks = iter(range(100_000, 100_128))
            requests = []
            def publish(path, text):
                requests.append(int(text))
                (directory / 'dispatch-clock-response').write_text(
                    '{"sequence":' + text + ',"native_ns":1}')
            with patch.object(linux.time, 'monotonic_ns', side_effect=lambda: next(ticks)):
                rows = linux.calibrate(directory, publish, lambda p, seconds: p())
            self.assertEqual(requests, list(range(1, 65)))
            self.assertEqual(len(rows), 64)
            self.assertTrue(all(r['after_ns'] == r['before_ns'] + 1 for r in rows))


if __name__ == '__main__':
    unittest.main()
