"""Conservative actual-callback bounds for macOS native input method 3.

The helper's transport wall times remain diagnostics. All64 independent clock
handshakes constrain one offset interval; no favorable offset is selected.
"""
import math


def integer(value):
    if type(value) is not int or not 0 <= value < 2**63:
        raise ValueError('bounded integer dispatch observation required')
    return value


def bounds(samples, calibrations, kind):
    if kind not in ['search', 'browser-focus'] or len(samples) != 64 or len(calibrations) != 64:
        raise ValueError('complete native dispatch and clock cohorts required')
    lower, upper = [], []
    for n, clock in enumerate(calibrations):
        if integer(clock['sequence']) != n+1:
            raise ValueError('unique ordered clock handshakes required')
        before, native, after = [integer(clock[key]) for key in ['before_ns', 'native_ns', 'after_ns']]
        if not native <= before <= after or after-before > 2_000_000_000:
            raise ValueError('invalid native clock handshake')
        lower.append(before-native)
        upper.append(after-native)
    lo, hi = max(lower), min(upper)
    if lo > hi:
        raise ValueError('native and helper clocks have no common offset')
    rows, previous = [], 0
    for n, sample in enumerate(samples):
        ack = sample['ack']
        before = integer(sample['before_sequence'])
        sequence = integer(ack['sequence'])
        if (integer(sample['n']) != n or ack['kind'] != kind or sequence != before+1
                or before < previous or sequence > 4096
                or ack['query'] != ('x' if kind == 'search' and n % 2 == 0 else '')):
            raise ValueError('one uniquely identified actual callback per input required')
        start, posted, observed = [integer(sample[key]) for key in ['start_ns', 'posted_ns', 'observed_ns']]
        native = integer(ack['native_ns'])
        if not start <= posted <= observed or native+hi < start or native+lo > observed:
            raise ValueError('callback acknowledgement violates the physical input interval')
        # The observed state contains this acknowledgement, so it is also a
        # conservative upper bound. This does not choose a clock offset.
        rows.append({'sequence': sequence, 'native_ns': native,
                     'lower_ms': max(0, native+lo-start)/1e6,
                     'upper_ms': (min(native+hi, observed)-start)/1e6,
                     'wall_ms': (observed-start)/1e6})
        previous = sequence
    return {'offset_lower_ns': lo, 'offset_upper_ns': hi, 'samples': rows,
            'p95_lower_ms': sorted(row['lower_ms'] for row in rows)[60],
            'p95_upper_ms': sorted(row['upper_ms'] for row in rows)[60],
            'wall_p95_ms': sorted(row['wall_ms'] for row in rows)[60]}


def measure(measured):
    """Validate helper witnesses before replacing wall proxies with upper bounds."""
    for prefix, kind in [('input', 'search'), ('pointer', 'browser-focus')]:
        samples = measured[prefix+'_dispatch_samples']
        result = bounds(samples, measured['clock_calibrations'], kind)
        walls = measured[prefix+'_ms']
        states = measured[prefix+'_states']
        if (len(walls) != 64 or len(states) != 64
                or any(type(wall) not in (int, float) or not math.isfinite(wall) or wall < 0 for wall in walls)):
            raise ValueError('complete raw input walls and actual states required')
        if measured[prefix+'_p95_ms'] != sorted(walls)[60]:
            raise ValueError('retained helper wall percentile differs from raw observations')
        for raw, state, row, wall in zip(samples, states, result['samples'], walls):
            key = 'search' if kind == 'search' else 'browser_focus'
            if (state['dispatch'][key] != raw['ack'] or abs(wall-row['wall_ms']) > 1e-9
                    or (kind == 'search' and state['query'] != raw['ack']['query'])
                    or (kind == 'browser-focus' and state['focus_key'] != 31)):
                raise ValueError('actual state and retained dispatch witness differ')
        measured[prefix+'_observer_wall_ms'] = walls
        measured[prefix+'_observer_wall_p95_ms'] = result['wall_p95_ms']
        measured[prefix+'_dispatch_bounds'] = result
        measured[prefix+'_ms'] = [row['upper_ms'] for row in result['samples']]
        measured[prefix+'_p95_ms'] = result['p95_upper_ms']
    measured['input_method_version'] = 3
    return measured


def verify_events(measured, events):
    """Bind every measured callback to the complete post-join native event log."""
    if not 106 <= len(events) <= 4096:
        raise ValueError('bounded complete native callback log required')
    counters = {'search': 0, 'browser-focus': 0}
    witnesses, prior = {}, 0
    for event in events:
        kind = event['kind']
        if kind not in counters or integer(event['sequence']) != counters[kind]+1 or integer(event['native_ns']) < prior:
            raise ValueError('ordered unique native callback log required')
        counters[kind] += 1
        prior = event['native_ns']
        witnesses[(kind, event['sequence'])] = event
    if counters['search'] != 106:
        raise ValueError('64 actual edits and42 completion edits required')
    for prefix in ['input', 'pointer']:
        for sample in measured[prefix+'_dispatch_samples']:
            ack = sample['ack']
            if witnesses.get((ack['kind'], ack['sequence'])) != ack:
                raise ValueError('measured acknowledgement absent from joined native log')
