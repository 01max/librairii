#!/usr/bin/env python3
"""Source-bound retained workload measurements on each matching native host."""
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'target/quality/native-scale'
TARGETS = {'aarch64-apple-darwin': ('Darwin', 'arm64'),
           'x86_64-apple-darwin': ('Darwin', 'x86_64'),
           'x86_64-unknown-linux-gnu': ('Linux', 'x86_64')}
BUDGETS = {'collectionQuery': 100, 'substringSearch': 120, 'combinedFilters': 150,
           'collectionCounts': 250, 'deepPagination': 120, 'artworkLoad': 50}


def output(argv):
    return subprocess.check_output(argv, cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hardware():
    if platform.system() == 'Darwin':
        def sysctl(key):
            return output(['/usr/sbin/sysctl', '-n', key])
        return {'cpu': sysctl('machdep.cpu.brand_string'), 'model': sysctl('hw.model'),
                'memory_bytes': int(sysctl('hw.memsize')), 'logical_cpus': int(sysctl('hw.logicalcpu'))}
    cpu = next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines()
               if line.startswith('model name'))
    memory = next(line.split()[1] for line in Path('/proc/meminfo').read_text().splitlines()
                  if line.startswith('MemTotal:'))
    return {'cpu': cpu, 'memory_bytes': int(memory) * 1024, 'logical_cpus': os.cpu_count()}


def verify_queries(report):
    if (report['format'] != 'librairii.scale-backend' or report['version'] != 1
            or report['stories'] != 5000 or not report['fixture_retired'] or not report['database_closed']):
        raise ValueError('incomplete retained fixture or positive shutdown')
    queries = report['queries']
    if set(queries['metrics']) != set(BUDGETS):
        raise ValueError('all six retained performance gates are required')
    for name, budget in BUDGETS.items():
        metric = queries['metrics'][name]
        samples = metric['wall_ms']
        count = 100 if name == 'artworkLoad' else 20
        if len(samples) != count or any(type(n) not in (int, float) or not math.isfinite(n) or n < 0 for n in samples):
            raise ValueError('invalid complete warmed samples: ' + name)
        cpu = metric['process_cpu_ms']
        if len(cpu) != count or any(type(n) not in (int, float) or not math.isfinite(n) or n < 0 for n in cpu):
            raise ValueError('complete process CPU diagnostics required: ' + name)
        p95 = sorted(samples)[math.ceil(count * 0.95) - 1]
        if (metric['budget_ms'] != budget or metric['p95_ms'] != p95
                or metric['passed'] != (p95 <= budget) or not metric['passed']):
            raise ValueError('retained wall p95 gate failed: ' + name)
    rows = [json.loads(line) for line in (ROOT / 'preservation/corpus/performance/stories.jsonl').read_text().splitlines()]
    ordered = sorted(rows, key=lambda r: (r['display_name_normalized'], r['uuid'], r['id']))
    expected = {'collectionQuery': (ordered, 0), 'substringSearch': ([r for r in ordered if 'moon' in r['display_name_normalized']], 0),
                'combinedFilters': ([r for r in ordered if 'forest' in r['display_name_normalized'] and r['language'] == 'en-GB'
                                     and r['validation_state'] == 'valid' and r['id'] % 3 == 0], 0),
                'deepPagination': (ordered, 4776)}
    for name, (matching, offset) in expected.items():
        result = queries['metrics'][name]['result']
        if result != {'total': len(matching), 'ids': [row['id'] for row in matching[offset:offset + 24]]}:
            raise ValueError('retained query outcome differs: ' + name)
    if queries['metrics']['collectionCounts']['result'] != [624, 2500, 4000, 5000, 1666, 85]:
        raise ValueError('six frozen Collection counts differ')
    if queries['metrics']['artworkLoad']['result'] != {'files': 24, 'pixel_sum': 1173893696} or not queries['passed']:
        raise ValueError('complete local artwork workload missing')
    phases = queries['metrics']['artworkLoad']['phases_warmup_then_100']
    phase_keys = {p + '_' + clock + '_ms' for p in ['lookup', 'read', 'decode'] for clock in ['wall', 'cpu']}
    if len(phases) != 101 or any(set(p) != phase_keys or any(type(n) not in (int, float)
            or not math.isfinite(n) or n < 0 for n in p.values()) for p in phases):
        raise ValueError('complete warmup and measured artwork phase diagnostics required')
    plans = queries['query_plans']
    if set(plans) != set(BUDGETS):
        raise ValueError('all measured query/count/artwork plans required')
    for name in expected:
        plan = plans[name]
        page, count = plan['page'], plan['count']
        offset = 4776 if name == 'deepPagination' else 0
        if (plan['page_size'] != 24 or plan['page_number'] != offset // 24 + 1
                or not page['statement'].endswith('LIMIT ? OFFSET ?')
                or page['parameters'][-3:] != [{'type': 'text', 'value': 'en-GB'},
                    {'type': 'integer', 'value': 24}, {'type': 'integer', 'value': offset}]
                or not count['statement'].startswith('SELECT COUNT(*)')
                or count['parameters'] != page['parameters'][:-3]
                or not page['steps'] or not count['steps']):
            raise ValueError('plan/bound values do not describe measured page and count: ' + name)
    if len(plans['collectionCounts']) != 6 or any(not p['steps'] or not p['statement'].startswith('SELECT COUNT(*)') for p in plans['collectionCounts']):
        raise ValueError('six saved-Collection count plans required')
    artwork = plans['artworkLoad']
    if (artwork['statement'] != 'SELECT embedded_artwork_path FROM stories WHERE id=?'
            or artwork['parameter_type'] != 'integer' or not artwork['steps']
            or artwork['ids'] != queries['metrics']['collectionQuery']['result']['ids']):
        raise ValueError('actual per-asset record lookup evidence required')


def verify_shelves(report):
    if (report['format'] != 'librairii.scale-backend' or report['version'] != 1 or report['stories'] != 5000
            or not report['fixture_retired'] or not report['database_closed']):
        raise ValueError('complete owned Shelf fixture and positive shutdown required')
    result = report['shelves']
    oracle = json.loads((ROOT / 'preservation/corpus/performance/review5000.json').read_text())
    if (result['initial_additions'] != oracle['initialAdditions'] or result['approved'] != oracle['approved']
            or result['remaining_additions'] != oracle['remainingAdditions']
            or result['exclusion_uuids'] != [r['uuid'] for r in oracle['initialAdditions'][50:]]
            or result['large_member_uuids'] != [r['uuid'] for r in oracle['initialAdditions']]):
        raise ValueError('frozen Shelf identity/order/provenance/outcome differs')
    expected_exclusions = [r['uuid'] for r in oracle['initialAdditions'][50:]]
    if (result['rollback_exclusion_uuids'] != expected_exclusions
            or result['final_exclusion_uuids'] != expected_exclusions
            or result['small_member_uuids'] != [r['uuid'] for r in oracle['initialAdditions'][:50]]
            or result['rollback'] != {'outcome': 'proven_not_committed',
                'error': 'controlled scale rollback', 'intermediate_manual_members': 2525}):
        raise ValueError('exact post-rollback/success membership and exclusion proof required')
    warnings = result['warnings']
    if (len(warnings['warnings']) != 32 or len(set(warnings['warnings'])) != 32
            or not warnings['incomplete'] or warnings['removal_count'] != 0
            or warnings['healthy_additions'] != 2500 or not warnings['approved_forest_retained']
            or not warnings['explicit_healthy_add_committed']):
        raise ValueError('incomplete sources must retain members and allow explicit healthy additions')
    for name in ['review_ranges', 'creation_ranges']:
        if warnings[name] != {'total': 32, 'page_lengths': [25, 7], 'next_and_huge_empty': True}:
            raise ValueError('complete bounded warning ranges required')
    samples = result['preview_ms']
    if (len(samples) != 4 or any(type(n) not in (int, float) or not math.isfinite(n) or n < 0 or n > 5000 for n in samples)
            or result['ceiling_ms'] != 5000 or not math.isfinite(result['apply50_ms']) or not 0 <= result['apply50_ms'] <= 5000):
        raise ValueError('retained four-preview/50-add five-second gate failed')
    for name, count in [('review_ranges', 5000), ('creation_ranges', 5000), ('detachment_ranges', 5000), ('exclusion_ranges', 4950)]:
        ranges = result[name]
        if ranges['total'] != count or ranges['page_lengths'] != [25] * (count // 25):
            raise ValueError('complete bounded 25-row surface required: ' + name)
        if not ranges['next_and_huge_empty']:
            raise ValueError('out-of-range pages must be empty')
    if [r['total'] for r in result['deletion_ranges']] != [25, 2500]:
        raise ValueError('one affected Shelf at a time, with exact detachment impact')
    for ranges in result['deletion_ranges']:
        if ranges['page_lengths'] != [25] * (ranges['total'] // 25) or not ranges['next_and_huge_empty']:
            raise ValueError('unbounded or incomplete deletion rows')
    if (not result['passed'] or not result['one_shelf_at_a_time']
            or not result['failed_deletion_preserved_all_members']
            or not result['successful_deletion_preserved_members_order_exclusions']
            or result['detached_manual_members'] != 5000 or result['retained_large_members'] != 5000):
        raise ValueError('large atomic rollback/detachment/preservation proof missing')


def main():
    if output(['git', 'status', '--porcelain', '--untracked-files=normal']):
        raise ValueError('final scale evidence requires a clean committed source tree')
    compiler = output(['rustc', '-vV'])
    pin = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    if 'release: ' + pin + '\n' not in compiler + '\n':
        raise ValueError('scale evidence requires the workspace compiler pin')
    host = compiler.split('host: ')[1].splitlines()[0]
    target = os.environ.get('RUST_TARGET', host)
    if host != target or TARGETS.get(target) != (platform.system(), platform.machine()):
        raise ValueError('scale evidence requires the matching native host')
    machine = hardware()
    if target == 'x86_64-apple-darwin':
        translated = subprocess.run(['/usr/sbin/sysctl', '-n', 'sysctl.proc_translated'], capture_output=True, text=True)
        if 'Intel' not in machine['cpu'] or translated.stdout.strip() == '1':
            raise ValueError('Intel evidence requires actual Intel hardware')
    if DESTINATION.exists():
        marker = DESTINATION / '.scale-fixture-root'
        if DESTINATION.is_symlink() or not marker.is_file() or marker.read_bytes() != b'native-scale-16\n':
            raise ValueError('refuse to replace unowned scale evidence')
        shutil.rmtree(DESTINATION)
    DESTINATION.mkdir(parents=True)
    (DESTINATION / '.scale-fixture-root').write_text('native-scale-16\n')
    manifest = {'format': 'librairii.native-scale', 'version': 1, 'complete': False,
                'backend_method_version': 2,
                'performance_profile': 'production release; opt-level3; debug assertions and overflow checks disabled',
                'source_commit': output(['git', 'rev-parse', 'HEAD']), 'target': target,
                'rustc': compiler, 'hardware': machine, 'platform': platform.platform(),
                'cargo_lock_sha256': digest(ROOT / 'Cargo.lock'), 'scope': 'Task16.1-16.2 backend and Shelf gates; remaining Task16 gates separate'}
    fixture = ROOT / 'preservation/corpus/performance'
    manifest['fixtures'] = {str(path.relative_to(ROOT)): digest(path) for path in
                            [fixture / 'stories.jsonl', fixture / 'distribution.json', fixture / 'review5000.json',
                             *sorted((fixture / 'catalog/embedded').glob('*.png'))]}
    with (DESTINATION / 'compiler-artifacts.jsonl').open('w') as events, (DESTINATION / 'build.log').open('w') as log:
        subprocess.run(['cargo', 'build', '--locked', '--release', '--target', target,
                        '-p', 'librairii-desktop', '--example', 'scale-backend', '--message-format=json'],
                       cwd=ROOT, stdout=events, stderr=log, check=True)
    events = [json.loads(line) for line in (DESTINATION / 'compiler-artifacts.jsonl').read_text().splitlines()]
    artifacts = [e for e in events if e.get('reason') == 'compiler-artifact' and e.get('target', {}).get('name') == 'scale-backend'
                 and e['target']['kind'] == ['example'] and e.get('executable')]
    if len(artifacts) != 1:
        raise ValueError('Cargo must report the exact scale observer executable')
    artifact = artifacts[0]
    if artifact['profile']['opt_level'] != '3' or artifact['profile']['debug_assertions'] or artifact['profile']['overflow_checks']:
        raise ValueError('scale observer must use the exact production release profile')
    binary = DESTINATION / 'scale-backend'
    shutil.copy2(artifact['executable'], binary)
    manifest['executable'] = {'path': binary.name, 'sha256': digest(binary), 'bytes': binary.stat().st_size,
                              'cargo_path': artifact['executable'], 'profile': artifact['profile']}
    (DESTINATION / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    with (DESTINATION / 'backend.json').open('w') as result, (DESTINATION / 'backend.log').open('w') as log:
        subprocess.run([str(binary)], cwd=ROOT, stdout=result, stderr=log, check=True, timeout=180)
    verify_queries(json.loads((DESTINATION / 'backend.json').read_text()))
    with (DESTINATION / 'shelves.json').open('w') as result, (DESTINATION / 'shelves.log').open('w') as log:
        subprocess.run([str(binary), '--shelves'], cwd=ROOT, stdout=result, stderr=log, check=True, timeout=180)
    verify_shelves(json.loads((DESTINATION / 'shelves.json').read_text()))
    if output(['git', 'status', '--porcelain', '--untracked-files=normal']) or output(['git', 'rev-parse', 'HEAD']) != manifest['source_commit']:
        raise ValueError('source changed during measurement')
    if digest(binary) != manifest['executable']['sha256'] or binary.stat().st_size != manifest['executable']['bytes']:
        raise ValueError('retained executable changed during measurement')
    manifest['complete'] = True
    manifest['files'] = {p.name: digest(p) for p in DESTINATION.iterdir() if p.is_file() and p.name != 'manifest.json'}
    (DESTINATION / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps({'complete': True, 'source': manifest['source_commit'], 'target': target, 'backend_method_version': 2,
                      'hardware': machine, 'six_retained_backend_budgets': 'passed', 'retained_shelf_gates': 'passed'}))


if __name__ == '__main__':
    main()
