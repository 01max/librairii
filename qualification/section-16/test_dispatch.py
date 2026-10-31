"""Keep dispatch gates sensitive to queue/callback delay, independently of transport."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('dispatch', Path(__file__).with_name('dispatch.py'))
dispatch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dispatch)


def observations(dispatch_ms=5, publication_ms=200, posting_ms=1):
    offset = 10**12
    calibration = [{'sequence': n+1, 'before_ns': offset+n*10**8, 'native_ns': n*10**8,
                    'after_ns': offset+n*10**8+1_000_000} for n in range(64)]
    samples = []
    for n in range(64):
        start = offset+(n+100)*10**9
        native = start-offset+int(dispatch_ms*10**6)
        samples.append({'n': n, 'before_sequence': n,
                        'ack': {'kind': 'search', 'sequence': n+1, 'query': 'x' if n % 2 == 0 else '', 'native_ns': native},
                        'start_ns': start, 'posted_ns': start+int(posting_ms*10**6),
                        'observed_ns': native+offset+int(publication_ms*10**6)})
    return samples, calibration


class DispatchTests(unittest.TestCase):
    def test_captured_failed_keyboard_wall_cohort_keeps_the_callback_boundary(self):
        path = Path(__file__).parents[1] / 'section-17/fixtures/keyboard-publication-delay.json'
        data = json.loads(path.read_text())
        result = dispatch.bounds(data['samples'], data['calibrations'], 'search')
        self.assertGreater(sorted(data['original_wall_ms'])[60], 100)
        self.assertLess(result['p95_upper_ms'], 100)
        self.assertAlmostEqual(result['p95_upper_ms'], 87.300910)

    def test_late_publication_is_retained_without_becoming_dispatch(self):
        samples, calibration = observations()
        result = dispatch.bounds(samples, calibration, 'search')
        self.assertLess(result['p95_upper_ms'], 100)
        self.assertGreater(result['wall_p95_ms'], 100)
        self.assertEqual(result['p95_upper_ms'], 6)
        self.assertEqual(len(result['samples']), 64)

    def test_real_queue_or_callback_delay_still_fails(self):
        samples, calibration = observations(dispatch_ms=205)
        result = dispatch.bounds(samples, calibration, 'search')
        self.assertGreater(result['p95_upper_ms'], 100)
        self.assertGreater(result['p95_lower_ms'], 100)

    def test_slow_event_posting_is_part_of_the_dispatch_gate(self):
        samples, calibration = observations(dispatch_ms=125, posting_ms=120)
        self.assertGreater(dispatch.bounds(samples, calibration, 'search')['p95_upper_ms'], 100)

    def test_all_handshakes_constrain_the_clock_without_a_favorable_offset(self):
        samples, calibration = observations()
        calibration[0]['before_ns'] += 500_000
        result = dispatch.bounds(samples, calibration, 'search')
        self.assertEqual(result['offset_lower_ns'], 10**12+500_000)
        self.assertEqual(result['offset_upper_ns'], 10**12+1_000_000)
        self.assertEqual(result['p95_upper_ms'], 6)

    def test_missing_ambiguous_wrong_query_and_incompatible_clocks_refuse(self):
        samples, calibration = observations()
        broken = []
        missing = copy.deepcopy(samples); missing.pop(); broken.append((missing, calibration))
        duplicate = copy.deepcopy(samples); duplicate[7]['ack']['sequence'] -= 1; broken.append((duplicate, calibration))
        query = copy.deepcopy(samples); query[7]['ack']['query'] = 'x'; broken.append((query, calibration))
        wrong_kind = copy.deepcopy(samples); wrong_kind[7]['ack']['kind'] = 'browser-focus'; broken.append((wrong_kind, calibration))
        disjoint = copy.deepcopy(calibration); disjoint[2]['before_ns'] += 2_000_000; disjoint[2]['after_ns'] += 2_000_000
        broken.append((samples, disjoint))
        for values, clocks in broken:
            with self.subTest(values=values[0]['n'], clocks=clocks[0]['sequence']):
                with self.assertRaises(ValueError):
                    dispatch.bounds(values, clocks, 'search')

    def test_acknowledgement_cannot_precede_post_or_follow_observation(self):
        for native in [-10_000_000, 300_000_000]:
            samples, calibration = observations()
            samples[0]['ack']['native_ns'] = samples[0]['start_ns']-10**12+native
            with self.assertRaises(ValueError):
                dispatch.bounds(samples, calibration, 'search')

    def test_pointer_focus_counts_need_one_unique_actual_event(self):
        samples, calibration = observations()
        for sample in samples:
            sample['ack']['kind'] = 'browser-focus'
            sample['ack']['query'] = ''
            sample['before_sequence'] += 10
            sample['ack']['sequence'] += 10
        self.assertLess(dispatch.bounds(samples, calibration, 'browser-focus')['p95_upper_ms'], 100)
        samples[3]['ack']['sequence'] += 1
        with self.assertRaises(ValueError):
            dispatch.bounds(samples, calibration, 'browser-focus')

    def helper(self):
        samples, calibration = observations()
        pointers = copy.deepcopy(samples)
        for sample in pointers:
            sample['ack']['kind'] = 'browser-focus'
            sample['ack']['query'] = ''
            for key in ['start_ns', 'posted_ns', 'observed_ns']:
                sample[key] += 100*10**9
            sample['ack']['native_ns'] += 100*10**9
        helper = {'clock_calibrations': calibration}
        for prefix, values, key in [('input', samples, 'search'), ('pointer', pointers, 'browser_focus')]:
            helper[prefix+'_dispatch_samples'] = values
            helper[prefix+'_ms'] = [(sample['observed_ns']-sample['start_ns'])/1e6 for sample in values]
            helper[prefix+'_p95_ms'] = sorted(helper[prefix+'_ms'])[60]
            helper[prefix+'_states'] = [{'dispatch': {key: sample['ack']}, 'query': sample['ack']['query'], 'focus_key': 31}
                                       for sample in values]
        return helper

    def test_helper_conversion_and_joined_event_binding_preserve_both_metrics(self):
        helper = self.helper()
        result = dispatch.measure(copy.deepcopy(helper))
        self.assertEqual(result['input_method_version'], 3)
        self.assertGreater(result['input_observer_wall_p95_ms'], 100)
        self.assertLess(result['input_p95_ms'], 100)
        self.assertEqual(result['input_observer_wall_ms'], helper['input_ms'])
        events = [sample['ack'] for key in ['input', 'pointer'] for sample in helper[key+'_dispatch_samples']]
        for n in range(42):
            events.append({'kind': 'search', 'sequence': n+65, 'query': 'x' if n % 2 == 0 else '',
                           'native_ns': events[-1]['native_ns']+10**9})
        dispatch.verify_events(result, events)
        events[2]['native_ns'] += 1
        with self.assertRaises(ValueError):
            dispatch.verify_events(result, events)

    def test_helper_tampering_cannot_replace_or_hide_its_original_wall(self):
        helper = self.helper()
        for mutate in [lambda h: h['input_ms'].__setitem__(0, float('nan')),
                       lambda h: h['input_ms'].__setitem__(0, 0),
                       lambda h: h['input_states'][0].__setitem__('query', 'unexpected'),
                       lambda h: h['pointer_states'][0].__setitem__('focus_key', 30)]:
            altered = copy.deepcopy(helper)
            mutate(altered)
            with self.assertRaises(ValueError):
                dispatch.measure(altered)


if __name__ == '__main__':
    unittest.main()
