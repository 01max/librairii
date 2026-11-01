"""Read-only parity observations. Executed cases do not certify complete parity."""
import hashlib
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET

from .evidence import (MAX_JSON, archive, bound, candidate, file_record, hexadecimal,
                       installed, member_digest, member_json, read_json,
                       relative_name, require)
from .files import digest, regular, write_json
from .identity import TARGETS
from packaged_acceptance.run import qualified


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = 'docs/rewrite/section-19-parity-audit.json'


def source_bytes(source, path):
    hexadecimal(source, 40)
    relative_name(path)
    return subprocess.check_output(['git', 'show', source + ':' + path], cwd=ROOT)


def contract_sources(contract, source):
    """Bind reviewed implementation and assertion files to the observed source."""
    records = {}
    for key in ('planning_register', 'implementation_index'):
        record = contract[key]
        data = (ROOT / record['path']).read_bytes()
        require(hashlib.sha256(data).hexdigest() == record['sha256'], key + ' changed')
        records[key] = read_json(data)
    ids = ['P' + str(n).zfill(2) for n in range(1, 61)]
    for rows in (contract['rows'], records['planning_register']['rows'],
                 records['implementation_index']['rows']):
        require([r['id'] for r in rows] == ids, 'all sixty distinct ordered parity IDs required')
    pinned = {}
    for row, implementation in zip(contract['rows'], records['implementation_index']['rows'], strict=True):
        require(row['accountable_owner'] and row['workstream'] == implementation['workstream']
                and row['assertions_reviewed'] and row['automated_cases'], 'incomplete parity obligation')
        require(row['release_holds'] and all(k in contract['release_holds'] for k in row['release_holds']),
                'unknown or missing release hold')
        require(all(k in contract['scope_decisions'] for k in row['scope_decisions']), 'unknown scope decision')
        for path, sha in implementation['source_sha256'].items():
            require(path not in pinned or pinned[path] == sha, 'conflicting source pin')
            pinned[path] = sha
        identities = [(c['suite'], c['name']) for c in row['automated_cases']]
        require(len(identities) == len(set(identities)), 'duplicate mapped case')
        for case in row['automated_cases']:
            path, sha = case['source'], case['source_sha256']
            require(path not in pinned or pinned[path] == sha, 'conflicting assertion pin')
            pinned[path] = sha
    for path, sha in pinned.items():
        require(hashlib.sha256(source_bytes(source, path)).hexdigest() == sha,
                'reviewed implementation/assertion differs at observed source: ' + path)
    return records, pinned


def sealed_files(z, prefix, files):
    require(isinstance(files, dict) and files, 'sealed evidence files required')
    for name, expected in files.items():
        relative_name(name)
        actual = member_digest(z, prefix + name)
        if isinstance(expected, str):
            require(actual['sha256'] == expected, 'native file hash mismatch: ' + prefix + name)
        else:
            require(actual == {k: expected[k] for k in ('bytes', 'sha256')},
                    'installed file size/hash mismatch: ' + prefix + name)


def passing_cases(z, source, target, required):
    require(z.read('quality/source.txt').decode().strip() == source, 'quality source mismatch')
    discovery = member_json(z, 'quality/test-discovery.json')
    meta = discovery['rust-build-meta']
    require(meta['platforms']['host']['platform']['triple'] == target
            and [r['platform']['triple'] for r in meta['platforms']['targets']] == [target],
            'discovery target mismatch')
    discovered = set()
    for suite, value in discovery['rust-suites'].items():
        require(value['status'] == 'listed' and value['binary-id'] == suite
                and value['build-platform'] == 'target', 'incomplete test discovery')
        for name, case in value['testcases'].items():
            require(case['ignored'] is False and case['filter-match']['status'] == 'matches',
                    'ignored or filtered native test')
            discovered.add((suite, name))
    require(z.getinfo('nextest/ci/junit.xml').file_size <= MAX_JSON, 'oversized JUnit')
    junit = ET.fromstring(z.read('nextest/ci/junit.xml'))
    cases = list(junit.iter('testcase'))
    identities = [(c.attrib['classname'], c.attrib['name']) for c in cases]
    require(len(cases) == len(set(identities)) == len(discovered) == discovery['test-count']
            == int(junit.attrib['tests']) and set(identities) == discovered, 'JUnit/discovery case mismatch')
    require(all(int(junit.attrib[k]) == 0 for k in ('skipped', 'failures', 'errors'))
            and all(not list(c) for c in cases), 'failed, skipped or incomplete native test')
    for case in required:
        if not case.get('targets') or target in case['targets']:
            require((case['suite'], case['name']) in discovered, 'cumulative native test missing')
    return discovered


def quality_observations(z, source, target):
    manifest = member_json(z, 'quality/native-ui/manifest.json')
    bound(manifest, source, target)
    sealed_files(z, 'quality/native-ui/', manifest['files'])
    reports = {}
    for scale in ('1', '2'):
        name = 'quality/native-ui/scale-' + scale + '/report.json'
        require(name.removeprefix('quality/native-ui/') in manifest['files'], 'unsealed native report')
        reports[name] = member_json(z, name)
    return reports


def installed_observations(z, package, source, target):
    installed(z, package, source, target)
    qualification = member_json(z, 'gui/qualification.json')
    expected = ({'x11-fresh', 'x11-upgraded', 'wayland-fresh', 'wayland-upgraded'}
                if 'linux' in target else {'fresh', 'upgraded'})
    lanes = qualification['lanes']
    require(len(lanes) == len(expected) and {r['name'] for r in lanes} == expected,
            'missing or duplicate installed lane')
    reports = {}
    for lane in lanes:
        name = 'gui/' + lane['name'] + ('/case/report.json' if 'linux' in target else '/report.json')
        require(lane['passed'] is True and member_digest(z, name)['sha256'] == lane['sha256'],
                'failed or changed installed lane')
        report = member_json(z, name)
        bound(report, source, target)
        case = 'fresh' if lane['name'].endswith('fresh') else 'upgraded'
        session = lane['name'].split('-', 1)[0] if 'linux' in target else 'macOS'
        backend = 'winit-skia' if case == 'fresh' else 'winit-skia-software'
        require(qualified(report) and report['artifact'] == package['artifact']
                and report['case'] == case and report['session'] == session
                and report['backend'] == backend and report['host'] == qualification['host']
                and report['method'] == qualification['method']
                and report['processes'] == lane['processes']
                and report['signing'] == package['signing']
                and report['public_release_qualified'] is False
                and set(report['checks']) == set(lane['checks']),
                'incomplete or foreign installed observations')
        sealed_files(z, name.removesuffix('report.json'), report['files'])
        reports[name] = report
    return reports


def row_observations(row, cases, quality, installed_reports):
    selected = [(c['suite'], c['name']) for c in row['automated_cases']]
    require(all(c in cases for c in selected), row['id'] + ': mapped test did not pass')
    native = []
    for name, report in quality.items():
        require(all(report.get(k) == 'passed' for k in row['native_callbacks']),
                row['id'] + ': missing/failed native callback')
        native.extend({'member': name, 'field': k, 'observed': report[k]} for k in row['native_callbacks'])
    installed_rows = []
    for name, report in installed_reports.items():
        require(all(k in report['checks'] for k in row['installed_checks']),
                row['id'] + ': missing installed workflow check')
        installed_rows.extend({'member': name, 'check': k} for k in row['installed_checks'])
    return {'passing_cases': [{'suite': s, 'name': n} for s, n in selected],
            'native_callbacks': native, 'installed_checks': installed_rows,
            'release_acceptance': 'pending independent final-candidate review'}


def audit(input_path, output_path):
    input_path, output_path = Path(input_path), Path(output_path)
    root = input_path.resolve().parent
    require(output_path.parent.resolve() == root, 'write parity observations beside the input plan')
    if output_path.exists() or output_path.is_symlink():
        raise FileExistsError('parity output already exists')
    require(regular(input_path).st_size <= MAX_JSON, 'oversized input plan')
    input_data = input_path.read_bytes()
    input_sha = hashlib.sha256(input_data).hexdigest()
    plan = read_json(input_data)
    require(plan['format'] == 'librairii.release-evidence-input' and plan['schema'] == 1,
            'unsupported evidence input plan')
    source, tree = hexadecimal(plan['source_commit'], 40), hexadecimal(plan['source_tree'], 40)
    observed_tree = subprocess.check_output(['git', 'rev-parse', source + '^{tree}'], cwd=ROOT, text=True).strip()
    require(tree == observed_tree, 'observed source tree differs from plan')
    contract_data = (ROOT / CONTRACT).read_bytes()
    contract = read_json(contract_data)
    require(contract['format'] == 'librairii.parity-audit-contract' and contract['schema'] == 1,
            'unsupported parity contract')
    records, pins = contract_sources(contract, source)
    required = read_json(source_bytes(source, '.config/rust-test-baseline.json'))['required']
    rows = plan['targets']
    require(len(rows) == len(TARGETS) and {r['target'] for r in rows} == set(TARGETS), 'three distinct targets required')
    observations, archives = {}, []
    for selection in rows:
        target = selection['target']
        require(set(selection['reports']) == {'quality', 'scale', 'installed'}, 'three report families required')
        require('signed_candidate' not in selection, 'this audit covers unsigned internal candidates only')
        package, _ = candidate(root, selection['candidate'], source, tree, target)
        archives.extend([selection['candidate'], *selection['reports'].values()])
        with archive(root, selection['reports']['quality']) as z:
            cases = passing_cases(z, source, target, required)
            quality = quality_observations(z, source, target)
        with archive(root, selection['reports']['installed']) as z:
            gui = installed_observations(z, package, source, target)
        observations[target] = {'archive_selections': selection, 'distribution': package['artifact'],
                                'native_test_count': len(cases),
                                'rows': {r['id']: row_observations(r, cases, quality, gui) for r in contract['rows']}}
        # Retain raw scale identities; numerical acceptance is independently reviewed.
        with archive(root, selection['reports']['scale']) as z:
            for name in ('manifest.json', 'desktop/manifest.json', 'resources/manifest.json'):
                record = member_json(z, name)
                bound(record, source, target)
                require(record['complete'] is True and record['cargo_lock_sha256'] == package['cargo_lock_sha256'],
                        'incomplete or foreign scale evidence')
    require(digest(input_path) == input_sha and (ROOT / CONTRACT).read_bytes() == contract_data,
            'input or parity contract changed during audit')
    for selected in archives:
        file_record(root, selected, provider=True)
    result = {'format': 'librairii.parity-observations', 'schema': 1, 'source_commit': source,
              'source_tree': tree, 'input_plan_sha256': input_sha,
              'auditor_sha256': digest(__file__), 'contract_sha256': hashlib.sha256(contract_data).hexdigest(),
              'reviewed_source_sha256': pins, 'targets': observations,
              'rows': [dict(r, retained_contract=p['retained_behavior'])
                       for r, p in zip(contract['rows'], records['planning_register']['rows'], strict=True)],
              'scope_decisions': contract['scope_decisions'], 'release_holds': contract['release_holds'],
              'provider_authenticated': False, 'semantic_coverage_verified': False,
              'numerical_acceptance_verified': False, 'public_release_qualified': False,
              'limitations': ['Named cases and selected fields are observed, not a proof that every contract subcase is covered.',
                              'Review raw assertions, images, spoken audio, recovery and numerical traces independently.',
                              'Unsigned observations cannot close final signed-byte or promotion gates.']}
    write_json(output_path, result)
    return result
