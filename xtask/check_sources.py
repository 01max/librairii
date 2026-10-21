"""Read-only frozen-source/derivative policy; no Go execution or parity claim."""
import ast
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import sys

HISTORICAL_INDEX_SHA = '10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99'
INDEX_SHA = '13dfd314abfda9a9553d152a1d8a5586f8bd1205a08c177af9bbec57a51ae822'
SOURCE_COUNT = 62
QUERY_MAP = '.config/query-codec-source-map.json'
QUERY_MAP_SHA = 'b3872c2d4fe612faa75a1e1c673f307bb0d929813332dd21960e649e932faf90'
HISTORICAL_MANIFEST_SHA = '2f8bd6db37191346b6719d7345a46a0b4d3163be745d5fae6b7f8ebf429a0409'
BUNDLE = 'go-1.26.5-xtext-0.43.0-unicode15-cldr32'
DERIVATIVES = 'crates/librairii-core/src/collection_codec/compatibility'
EXCLUDED = ['regionTypes.ccTLD-bit1-value2', 'internal/language.Region.TLD', 'language.Region.TLD']
NOTICE_REQUIREMENTS = {
    'normalization': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt', 'LICENSE-UNICODE-V3.txt'],
    'language-default-without-dns': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt',
        'LICENSE-CLDR32-UNICODE.txt', 'LICENSE-CLDR32-APACHE.txt', 'LICENSE-CC0.txt'],
    'product-boundary': [],
    'query-product': [],
    'query-json-wire': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt'],
    'query-json-i64': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt'],
    'query-json-output': ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt'],
}

QUERY_SELECTIONS = {'Go1.26.5/src/encoding/json/scanner.go': 'query-json-wire',
 'Go1.26.5/src/encoding/json/decode.go': 'query-json-wire',
 'Go1.26.5/src/encoding/json/fold.go': 'query-json-wire',
 'Go1.26.5/src/encoding/json/stream.go': 'query-json-wire',
 'Go1.26.5/src/unicode/utf8/utf8.go': 'query-json-wire',
 'Go1.26.5/src/unicode/utf16/utf16.go': 'query-json-wire',
 'Go1.26.5/src/encoding/json/encode.go': 'query-json-output',
 'Go1.26.5/src/encoding/json/tables.go': 'query-json-output',
 'Go1.26.5/src/strconv/number.go': 'query-json-i64',
 'Go1.26.5/src/internal/strconv/atoi.go': 'query-json-i64',
 'oracle73a93303/internal/collections/query.go': 'query-product',
 'oracle73a93303/internal/collectionquery/references.go': 'query-product',
 'oracle73a93303/internal/collectionquery/unique_keys.go': 'query-product'}
QUERY_SCOPES = {'query-product': 'ONLY frozen v1-v5 saved-query payload/reference/unique-key validation and '
                  'canonical membership conversion; no CRUD, SQL, source/revision authority or new '
                  'query semantics.',
 'query-json-wire': 'ONLY scanner single-complete-document grammar, ordered '
                    'duplicate/key/null/type handling, unquote and per-byte UTF8/UTF16 replacement '
                    'plus fixed ASCII schema matching. stream.go is ordering reference, never '
                    'trailing-document admission; no generic reflection decoder.',
 'query-json-i64': 'ONLY public strconv/number.go ParseInt delegation/error projection and '
                   'internal/strconv/atoi.go ParseInt plus ParseUint dependency for signed base10 '
                   'bitSize64 JSON integer lexemes. No generic strconv, float coercion or wider '
                   'integer grammar.',
 'query-json-output': 'ONLY fixed Query serialization order/omission and string escaping '
                      'references including HTML <>& and U+2028/U+2029 from encode.go/tables.go; '
                      'no generic encoder/reflection/API.'}
LOOKUP_BINDINGS = {'crates/librairii-core/src/collection_codec/compatibility/extract_normalization.py': 'INDEX_HASH',
 'crates/librairii-core/src/collection_codec/compatibility/extract_language.py': 'INDEX_HASH',
 'crates/librairii-core/src/collection_codec/compatibility/update_source_manifest.py': 'INDEX_SHA',
 'crates/librairii-core/src/collection_codec/compatibility/verify_normalization.py': 'INDEX_SHA'}
CORE_PATH = 'crates/librairii-core/src/collection_codec/mod.rs'
OLD_DERIVATIVES = 'crates/librairii-storage/src/migrations/compatibility'
STORAGE_CALLER = 'crates/librairii-storage/src/migrations.rs'
HISTORICAL_INLINE = {'path': 'crates/librairii-storage/src/migrations.rs',
 'bytes': 48784,
 'sha256': 'c01e805177d99b33e1f0bd5f5c4b0e5463b2190f185b53906558e80dffe2ff59',
 'begin': 'mod collection_codec {\n',
 'end': '\n/// Storage-local transitional inspection result.',
 'fragment_bytes': 31508,
 'fragment_sha256': 'fc8ff89fa3425cbc0ed8baff6082b3c80c27a3447f7563d5c05b95d8106f3b4f',
 'materialization': 'existing-storage-inline-reference-map; final translated core leaf and its '
                    'retained headers are not materialized'}
HISTORICAL_RUST_BASELINE_SHA = '33e43537409dd3d5dc07fac28572fe23d3df0b038d33fc7883fab79d3ef28b68'
HISTORICAL_PYTHON_BASELINE_SHA = '3deb3eabe0e6b31170f26629c042214da2371b023235d1adf5cf2429664fa836'

NOTICE_ORIGINS = {
    'LICENSE-GO-BSD.txt': 'go-xtext-LICENSE.txt',
    'PATENTS-GO.txt': 'go-xtext-PATENTS.txt',
    'LICENSE-UNICODE-V3.txt': 'current/unicode-license.txt',
    'LICENSE-CLDR32-UNICODE.txt': 'cldr32-unicode-license.txt',
    'LICENSE-CLDR32-APACHE.txt': 'cldr32-apache-license.txt',
    'LICENSE-CC0.txt': 'cc0-1.0-legalcode.txt',
    'PROVENANCE.md': None,
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate source-policy JSON field')
        result[key] = value
    return result


def decode(data):
    return json.loads(data.decode('utf-8'), object_pairs_hook=unique_object)


def relative(value):
    if (not isinstance(value, str) or not value or '\\' in value or ':' in value
            or PurePosixPath(value).is_absolute()
            or any(p in ('', '.', '..') for p in value.split('/'))):
        raise ValueError('noncanonical source-policy path')
    return value


def safe(root, value):
    value = relative(value)
    current = root
    for part in value.split('/'):
        current = current / part
        if current.is_symlink() or getattr(current, 'is_junction', lambda: False)():
            raise ValueError('aliased source-policy path')
    return current


def pinned(root, record):
    path = safe(root, record['path'])
    if not path.is_file():
        raise ValueError('missing source-policy file')
    data = path.read_bytes()
    if len(data) != record['bytes'] or digest(data) != record['sha256']:
        raise ValueError('changed source-policy file')
    return data


def selection(path):
    if path in QUERY_SELECTIONS:
        return [QUERY_SELECTIONS[path]]
    if path.startswith(('golang.org/x/text@v0.43.0/internal/language/',
                        'golang.org/x/text@v0.43.0/internal/tag/',
                        'golang.org/x/text@v0.43.0/language/')):
        return ['language-default-without-dns']
    if path.endswith(('internal/searchtext/searchtext.go', 'internal/library/story_query.go')):
        return ['product-boundary']
    return ['normalization']


def check(root):
    registry = decode(safe(root, '.config/upstream-sources.json').read_bytes())
    if (set(registry) != {'version', 'product_license', 'bundles'} or registry['version'] != 1
            or registry['product_license'] is not None or len(registry['bundles']) != 1):
        raise ValueError('source registry must not assign a product license or widen scope')
    bundle = registry['bundles'][0]
    if (bundle['id'] != BUNDLE or bundle['scope'] != 'single-core-query-codec'
            or bundle['source_index'] != '.config/upstream-sources/' + BUNDLE + '/source-index.json'
            or bundle['notices_root'] != '.config/upstream-notices/' + BUNDLE
            or bundle['source_index_sha256'] != INDEX_SHA
            or bundle['source_identities'] != SOURCE_COUNT or bundle['package_files'] != 43
            or bundle['derivative_root'] != DERIVATIVES
            or bundle['derivative_manifest'] != 'source-manifest.json'
            or bundle['excluded'] != EXCLUDED
            or bundle['selection_notices'] != NOTICE_REQUIREMENTS
            or bundle.get('query_selection_scopes') != QUERY_SCOPES
            or bundle.get('query_codec') != {'map_path': QUERY_MAP, 'map_sha256': QUERY_MAP_SHA,
                'scope': 'single-materialized-core-query-codec', 'core_leaf': CORE_PATH}):
        raise ValueError('unreviewed source selection, scope or DNS policy')
    index_path = safe(root, bundle['source_index'])
    data = index_path.read_bytes()
    if digest(data) != INDEX_SHA:
        raise ValueError('changed authenticated source index')
    if {p.relative_to(index_path.parent).as_posix() for p in index_path.parent.rglob('*')} != {'source-index.json'}:
        raise ValueError('extra files beside authenticated source index')
    index = decode(data)
    historical = {k: v for k, v in index.items()
                  if k not in ('querySourceFiles', 'querySourceProvenance')}
    if digest((json.dumps(historical, indent=2) + '\n').encode()) != HISTORICAL_INDEX_SHA:
        raise ValueError('changed historical49 source identities or provenance')
    query_sources = index.get('querySourceFiles', [])
    if (len(query_sources) != len(QUERY_SELECTIONS)
            or {r['path'] for r in query_sources} != set(QUERY_SELECTIONS)
            or index.get('querySourceProvenance', {}).get('historicalSourceIndexSha256') != HISTORICAL_INDEX_SHA):
        raise ValueError('missing extra or duplicate query origins')
    sources = [f for package in index['packages'] for f in package['files']]
    if len(sources) != 43:
        raise ValueError('changed package-source set')
    sources += index['supplementalSourceFiles']
    if len(sources) != 49:
        raise ValueError('changed historical49 source set')
    sources += query_sources
    by_path = {f['path']: f for f in sources}
    if len(sources) != SOURCE_COUNT or len(by_path) != SOURCE_COUNT:
        raise ValueError('changed or duplicate source identities')
    if bundle['source_selections'] != {p: selection(p) for p in by_path}:
        raise ValueError('unreviewed source subset')
    for source in sources:
        relative(source['path'])
        header = bytes.fromhex(source['headerFirst12LinesHex'])
        if digest(header) != source['first12LinesSha256']:
            raise ValueError('changed original source-header identity')
    notice_root = safe(root, bundle['notices_root'])
    notice_records = bundle['notices']
    names = {r['path'] for r in notice_records}
    expected_names = set().union(*NOTICE_REQUIREMENTS.values()) | {'PROVENANCE.md'}
    if names != expected_names or len(names) != len(notice_records):
        raise ValueError('missing or duplicate notice inventory')
    actual_names = set()
    for path in notice_root.rglob('*'):
        name = path.relative_to(notice_root).as_posix()
        safe(notice_root, name)
        if path.is_file():
            actual_names.add(name)
    if names != actual_names:
        raise ValueError('extra or missing notice files')
    for record in notice_records:
        if (set(record) != {'path', 'bytes', 'sha256', 'upstream_path'}
                or record['upstream_path'] != NOTICE_ORIGINS[record['path']]):
            raise ValueError('unreviewed upstream notice origin')
        pinned(notice_root, record)
        if record['upstream_path'] is not None:
            original = next(r for r in index['noticePayloads'] if r['path'] == record['upstream_path'])
            if (record['sha256'], record['bytes']) != (original['sha256'], original['bytes']):
                raise ValueError('notice differs from authenticated handoff')
    derivative_root = safe(root, DERIVATIVES)
    records = []
    if derivative_root.exists():
        manifest_path = safe(derivative_root, 'source-manifest.json')
        manifest = decode(manifest_path.read_bytes())
        if set(manifest) != {'version', 'bundle', 'files'} or manifest['version'] != 1 or manifest['bundle'] != BUNDLE:
            raise ValueError('invalid private derivative manifest')
        records = manifest['files']
        actual = set()
        for path in derivative_root.rglob('*'):
            name = path.relative_to(derivative_root).as_posix()
            safe(derivative_root, name)
            if path.is_file() and name != 'source-manifest.json':
                actual.add(name)
        if not records or len({r['path'] for r in records}) != len(records) or {r['path'] for r in records} != actual:
            raise ValueError('unrecorded or missing private derivative')
        for record in records:
            if set(record) != {'path', 'bytes', 'sha256', 'kind', 'recipe', 'sources', 'notices'}:
                raise ValueError('invalid derivative record')
            content = pinned(derivative_root, record)
            if not isinstance(record['recipe'], str) or not record['recipe'].strip():
                raise ValueError('missing original-to-derived recipe')
            kind = record['kind']
            if kind == 'authored':
                if record['sources'] or record['notices']:
                    raise ValueError('authored glue cannot claim upstream provenance')
            elif kind in ('translated', 'generated') and record['sources']:
                required = set()
                seen = set()
                for use in record['sources']:
                    if set(use) != {'path', 'sha256', 'header_sha256', 'selection'} or use['path'] in seen:
                        raise ValueError('duplicate or invalid derivative source')
                    seen.add(use['path'])
                    original = by_path.get(use['path'])
                    if (original is None or use['sha256'] != original['sha256']
                            or use['header_sha256'] != original['first12LinesSha256']
                            or use['selection'] not in bundle['source_selections'][use['path']]):
                        raise ValueError('unapproved original identity or source subset')
                    required.update(NOTICE_REQUIREMENTS[use['selection']])
                    header = bytes.fromhex(original['headerFirst12LinesHex'])
                    for line in header.splitlines():
                        if line.startswith(b'// Copyright') and line not in content:
                            raise ValueError('dropped original source copyright header')
                if set(record['notices']) != required or len(set(record['notices'])) != len(record['notices']):
                    raise ValueError('incomplete derivative notice attribution')
            else:
                raise ValueError('invalid derivative kind or missing original sources')
    query_map = check_query_map(root, bundle, by_path, records)
    return {
        'version': 1, 'bundle': BUNDLE, 'product_license': None,
        'source_index_sha256': INDEX_SHA, 'source_identities': sources,
        'notices': notice_records, 'private_derivatives': records,
        'query_codec': query_map,
        'state': 'private-provenance-recorded' if records else 'awaiting-private-implementation',
        'excluded': EXCLUDED,
        'qualification': 'source inventory only; no algorithm, parity, effect or platform qualification',
    }


def check_query_map(root, bundle, by_path, records):
    data = safe(root, QUERY_MAP).read_bytes()
    if digest(data) != QUERY_MAP_SHA:
        raise ValueError('changed query source map')
    value = decode(data)
    keys = {'version', 'bundle', 'scope', 'lookup_index_sha256', 'historical_index_sha256',
            'historical_manifest_sha256', 'historical_derivatives', 'historical_inline',
            'core_leaf', 'origins', 'notices', 'recipe', 'lookup_bindings', 'fixed_references',
            'current_derivative_changes', 'identity_migrations', 'historical_rust_baseline_sha256',
            'historical_python_baseline_sha256'}
    if (set(value) != keys or value['version'] != 2 or value['bundle'] != BUNDLE
            or value['scope'] != 'single-materialized-core-query-codec'
            or value['lookup_index_sha256'] != INDEX_SHA
            or value['historical_index_sha256'] != HISTORICAL_INDEX_SHA
            or value['historical_manifest_sha256'] != HISTORICAL_MANIFEST_SHA
            or value['historical_inline'] != HISTORICAL_INLINE):
        raise ValueError('invalid materialized core map or historical inline identity')
    historical = value['historical_derivatives']
    if digest((json.dumps(historical, indent=2) + '\n').encode()) != HISTORICAL_MANIFEST_SHA:
        raise ValueError('changed historical19 derivative lineage')
    originals = historical['files']
    if len(originals) != 19:
        raise ValueError('changed historical19 derivative set')
    current = {r['path']: r for r in records}
    adaptations = {'extract_normalization.py', 'extract_language.py', 'update_source_manifest.py',
                   'verify_normalization.py', 'mod.rs', 'language.rs', 'normalization.rs'}
    differences = []
    for original in originals:
        now = current.get(original['path'])
        if now is None:
            raise ValueError('missing historical derivative')
        ignored = {'bytes', 'sha256'} if original['path'] in adaptations else set()
        if ({k: v for k, v in now.items() if k not in ignored}
                != {k: v for k, v in original.items() if k not in ignored}):
            raise ValueError('changed historical derivative lineage or fixed bytes')
        if now != original:
            differences.append({k: now[k] for k in ('path', 'bytes', 'sha256')})
    changes = value['current_derivative_changes']
    if (len(changes) != len(differences)
            or [{k: x[k] for k in ('path', 'bytes', 'sha256')} for x in changes] != differences
            or any(not isinstance(x.get('reason'), str) or not x['reason'].strip() for x in changes)):
        raise ValueError('unrecorded current derivative adaptation')
    expected = [{'path': p, 'sha256': by_path[p]['sha256'],
                 'header_sha256': by_path[p]['first12LinesSha256'], 'selection': role}
                for p, role in QUERY_SELECTIONS.items()]
    if value['origins'] != expected:
        raise ValueError('unapproved query origin header or selection')
    if value['notices'] != ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt']:
        raise ValueError('incomplete query notice attribution')
    if not isinstance(value['recipe'], str) or not value['recipe'].strip():
        raise ValueError('missing query recipe')
    leaf = value['core_leaf']
    if (set(leaf) != {'path', 'bytes', 'sha256', 'kind', 'header_bytes', 'header_sha256', 'materialization'}
            or leaf['path'] != CORE_PATH or leaf['kind'] != 'translated'
            or leaf['materialization'] != 'single actual core codec; no SQL, live-reference or effect authority'):
        raise ValueError('invalid materialized core codec claim')
    content = pinned(root, leaf)
    header = content[:leaf['header_bytes']]
    if digest(header) != leaf['header_sha256']:
        raise ValueError('changed materialized core header identity')
    for origin in expected:
        original_header = bytes.fromhex(by_path[origin['path']]['headerFirst12LinesHex'])
        for line in original_header.splitlines():
            if line.startswith(b'// Copyright') and line not in header:
                raise ValueError('dropped original core copyright header')
    complete = {'mod.rs', 'compatibility/source-manifest.json'} | {
        'compatibility/' + x['path'] for x in records}
    directory = safe(root, CORE_PATH).parent
    actual = set()
    for path in directory.rglob('*'):
        name = path.relative_to(directory).as_posix()
        safe(directory, name)
        if path.is_file():
            actual.add(name)
    if actual != complete:
        raise ValueError('incomplete or extra materialized core closure')
    if safe(root, OLD_DERIVATIVES).exists():
        raise ValueError('old storage compatibility duplicate remains')
    caller = safe(root, STORAGE_CALLER).read_bytes()
    if b'mod collection_codec {' in caller or b'mod compatibility;' in caller:
        raise ValueError('old storage inline codec duplicate remains')
    if b'use librairii_core::collection_codec;' not in caller:
        raise ValueError('storage caller does not delegate to core codec')
    expected_bindings = [{'path': path, 'constant': name, 'lookup_index_sha256': INDEX_SHA,
                          'historical_index_sha256': HISTORICAL_INDEX_SHA}
                         for path, name in LOOKUP_BINDINGS.items()]
    if value['lookup_bindings'] != expected_bindings:
        raise ValueError('untruthful query lookup helper binding')
    for path, name in LOOKUP_BINDINGS.items():
        tree = ast.parse(safe(root, path).read_bytes())
        constants = {target.id: node.value.value
                     for node in tree.body if isinstance(node, ast.Assign)
                     and isinstance(node.value, ast.Constant)
                     for target in node.targets if isinstance(target, ast.Name)}
        if constants.get(name) != INDEX_SHA or constants.get('HISTORICAL_INDEX_SHA') != HISTORICAL_INDEX_SHA:
            raise ValueError('changed lookup versus historical helper index')
    expected_refs = [r for r in originals if r['path'] in
                     ('codec-reference.json', 'normalization-reference.json', 'language-reference.json')]
    if value['fixed_references'] != expected_refs:
        raise ValueError('changed historical reference provenance')
    for record in expected_refs:
        pinned(safe(root, DERIVATIVES), record)
    for kind, path, expected_sha, count in [
            ('rust', '.config/rust-test-baseline.json', HISTORICAL_RUST_BASELINE_SHA, 12),
            ('python', '.config/rust-python-test-baseline.json', HISTORICAL_PYTHON_BASELINE_SHA, 3)]:
        if value['historical_' + kind + '_baseline_sha256'] != expected_sha:
            raise ValueError('changed historical discovery baseline identity')
        baseline = decode(safe(root, path).read_bytes())
        moves = value['identity_migrations'][kind]
        if len(moves) != count or len({x['position'] for x in moves}) != count:
            raise ValueError('incomplete explicit discovery migration')
        for move in moves:
            position = move['position']
            if baseline['required'][position] != move['new'] or not move['reason'].strip():
                raise ValueError('unobserved or misplaced discovery migration')
            baseline['required'][position] = move['old']
        if digest((json.dumps(baseline, indent=2) + '\n').encode()) != expected_sha:
            raise ValueError('unrelated discovery identities changed')
    return value


def inventory(root, report):
    source = safe(root, '.config/upstream-notices/' + BUNDLE)
    destination = safe(root, 'target/quality/notices/' + BUNDLE)
    destination.mkdir(parents=True, exist_ok=True)
    for record in report['notices']:
        output = safe(destination, record['path'])
        shutil.copyfile(safe(source, record['path']), output)
        pinned(destination, record)
    summary = safe(root, 'target/quality/notices/NOTICES-frozen-sources.txt')
    summary.write_text('Private frozen-source notices. Product license remains undecided.\n'
        + BUNDLE + '\nState: ' + report['state'] + '\n'
        + report['qualification'] + '\nFiles: ' + BUNDLE + '/\n', encoding='utf-8')


def main():
    if sys.argv[1:] not in ([], ['--inventory']):
        raise ValueError('usage: check_sources.py [--inventory]')
    root = Path(__file__).resolve().parent.parent
    report = check(root)
    if sys.argv[1:] == ['--inventory']:
        inventory(root, report)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
