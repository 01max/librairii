"""Linux observer diagnostics; never replaces the method2 acceptance samples."""
import json
import time


def observe(post, read, predicate, before, kind, index, wait):
    """Measure the existing post-to-observation interval and retain its boundaries."""
    key = 'search' if kind == 'search' else 'browser_focus'
    sequence = before['dispatch'][key + '_sequence']
    start = time.monotonic_ns()
    post()
    posted = time.monotonic_ns()
    end = wait(lambda: (s := read()) and predicate(s) and s, 1)
    observed = time.monotonic_ns()
    ack = end['dispatch'][key]
    if ack['kind'] != kind or ack['sequence'] != sequence + 1:
        raise ValueError('one new actual Linux callback per input required')
    raw = {'n': index, 'before_sequence': sequence, 'ack': ack,
           'start_ns': start, 'posted_ns': posted, 'observed_ns': observed}
    return end, (observed - start) / 1e6, raw


def calibrate(output, publish, wait):
    """Bound one offset with all64 handshakes after all acceptance inputs."""
    rows = []
    for sequence in range(1, 65):
        before = time.monotonic_ns()
        publish(output / 'dispatch-clock-request', str(sequence))
        def response():
            try:
                value = json.loads((output / 'dispatch-clock-response').read_text())
            except FileNotFoundError:
                return None
            return value if value['sequence'] == sequence else None
        value = wait(response, 2)
        rows.append({'sequence': sequence, 'before_ns': before,
                     'native_ns': value['native_ns'], 'after_ns': time.monotonic_ns()})
    return rows


def summarize(measured, protocol):
    """Conservative callback and publication bounds, with no gate rescore."""
    result = {'version': 1, 'acceptance_evidence': False,
              'method': 'pre-xdotool launch, process return, actual callback bounds, state observation; all64 clock intervals',
              'cohorts': {}}
    for prefix, kind in [('input', 'search'), ('pointer', 'browser-focus')]:
        samples = measured[prefix + '_dispatch_samples']
        bound = protocol.bounds(samples, measured['clock_calibrations'], kind)
        rows = []
        for sample, limits, wall, state in zip(samples, bound['samples'], measured[prefix + '_ms'], measured[prefix + '_states'], strict=True):
            key = 'search' if kind == 'search' else 'browser_focus'
            if state['dispatch'][key] != sample['ack'] or abs(wall - limits['wall_ms']) > 1e-9:
                raise ValueError('Linux diagnostic and acceptance witnesses differ')
            rows.append({'sequence': limits['sequence'],
                         'process_return_ms': (sample['posted_ns'] - sample['start_ns']) / 1e6,
                         'callback_lower_ms': limits['lower_ms'], 'callback_upper_ms': limits['upper_ms'],
                         'callback_to_observation_lower_ms': max(0, wall - limits['upper_ms']),
                         'callback_to_observation_upper_ms': wall - limits['lower_ms'],
                         'observer_wall_ms': wall})
        result['cohorts'][prefix] = {'bounds': bound, 'samples': rows}
    return result
