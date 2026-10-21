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
QUERY_MAP_SHA = 'aaa5fbab00cb69b8b3c178012d8a8ae63778670fc1825ee92d5cffff0f9ad8ed'
HISTORICAL_MANIFEST_SHA = '2f8bd6db37191346b6719d7345a46a0b4d3163be745d5fae6b7f8ebf429a0409'
BUNDLE = 'go-1.26.5-xtext-0.43.0-unicode15-cldr32'
DERIVATIVES = 'crates/librairii-storage/src/migrations/compatibility'
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
LOOKUP_BINDINGS = {'crates/librairii-storage/src/migrations/compatibility/extract_normalization.py': 'INDEX_HASH',
 'crates/librairii-storage/src/migrations/compatibility/extract_language.py': 'INDEX_HASH',
 'crates/librairii-storage/src/migrations/compatibility/update_source_manifest.py': 'INDEX_SHA',
 'crates/librairii-storage/src/migrations/compatibility/verify_normalization.py': 'INDEX_SHA'}
INLINE_PATH = 'crates/librairii-storage/src/migrations.rs'
INLINE_BEGIN = 'mod collection_codec {\n'
INLINE_END = '\n/// Storage-local transitional inspection result.'
INLINE_MATERIALIZATION = 'existing-storage-inline-reference-map; final translated core leaf and its retained headers are not materialized'

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
    if (bundle['id'] != BUNDLE or bundle['scope'] != 'private-storage-4.4-query-codec-origins'
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
                'scope': 'current-storage-inline-query-origins-only', 'future_core_leaf': None}):
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
            'historical_manifest_sha256', 'historical_derivatives', 'current_inline',
            'future_core_leaf', 'origins', 'notices', 'recipe', 'lookup_bindings', 'fixed_references'}
    if (set(value) != keys or value['version'] != 1 or value['bundle'] != BUNDLE
            or value['scope'] != 'current-storage-inline-query-origins-only'
            or value['lookup_index_sha256'] != INDEX_SHA
            or value['historical_index_sha256'] != HISTORICAL_INDEX_SHA
            or value['historical_manifest_sha256'] != HISTORICAL_MANIFEST_SHA):
        raise ValueError('invalid query map or lookup index binding')
    if value['future_core_leaf'] is not None:
        raise ValueError('future core leaf is unmaterialized')
    historical = value['historical_derivatives']
    if digest((json.dumps(historical, indent=2) + '\n').encode()) != HISTORICAL_MANIFEST_SHA:
        raise ValueError('changed historical19 derivative lineage')
    originals = historical['files']
    if len(originals) != 19:
        raise ValueError('changed historical19 derivative set')
    current = {r['path']: r for r in records}
    for original in originals:
        now = current.get(original['path'])
        if now is None:
            raise ValueError('missing historical derivative')
        ignored = {'bytes', 'sha256'} if DERIVATIVES + '/' + original['path'] in LOOKUP_BINDINGS else set()
        if ({k: v for k, v in now.items() if k not in ignored}
                != {k: v for k, v in original.items() if k not in ignored}):
            raise ValueError('changed historical derivative lineage or fixed bytes')
    expected = [{'path': p, 'sha256': by_path[p]['sha256'],
                 'header_sha256': by_path[p]['first12LinesSha256'], 'selection': role}
                for p, role in QUERY_SELECTIONS.items()]
    if value['origins'] != expected:
        raise ValueError('unapproved query origin header or selection')
    if value['notices'] != ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt']:
        raise ValueError('incomplete query notice attribution')
    if not isinstance(value['recipe'], str) or not value['recipe'].strip():
        raise ValueError('missing query recipe')
    inline = value['current_inline']
    if (set(inline) != {'path', 'bytes', 'sha256', 'begin', 'end', 'fragment_bytes',
                       'fragment_sha256', 'materialization'} or inline['path'] != INLINE_PATH
            or inline['begin'] != INLINE_BEGIN or inline['end'] != INLINE_END
            or inline['materialization'] != INLINE_MATERIALIZATION):
        raise ValueError('invalid materialized inline codec claim')
    content = pinned(root, inline)
    begin, end = INLINE_BEGIN.encode(), INLINE_END.encode()
    if content.count(begin) != 1 or content.count(end) != 1:
        raise ValueError('ambiguous inline codec boundary')
    fragment = content[content.index(begin):content.index(end, content.index(begin))]
    if len(fragment) != inline['fragment_bytes'] or digest(fragment) != inline['fragment_sha256']:
        raise ValueError('changed inline codec fragment')
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
