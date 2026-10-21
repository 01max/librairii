"""Read-only frozen-source/derivative policy; no Go execution or parity claim."""
import ast
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import stat
import sys

HISTORICAL_INDEX_SHA = '10ea8fe193c594ce153ae7a1734c41ab80c9d406cc7ea314c8c7c471b7a92a99'
PREVIOUS_QUERY_INDEX_SHA = '13dfd314abfda9a9553d152a1d8a5586f8bd1205a08c177af9bbec57a51ae822'
INDEX_SHA = 'cd8fcdac9ccdd46f1f06294f27bc07e9f8ed40f79d16cdab4d39c7c0cd837d78'
SOURCE_COUNT = 63
QUERY_MAP = '.config/query-codec-source-map.json'
QUERY_MAP_SHA = '90142c7152cb85c7b78545559bd5f7691797c7fbfbc3e6390955572c3dd90792'
HISTORICAL_MANIFEST_SHA = '2f8bd6db37191346b6719d7345a46a0b4d3163be745d5fae6b7f8ebf429a0409'
BUNDLE = 'go-1.26.5-xtext-0.43.0-unicode15-cldr32'
APPROVED_BUNDLES = {BUNDLE, 'go-1.26.5-json-v1-studio'}
DERIVATIVES = 'crates/librairii-core/src/collection_codec/compatibility'
# Actual ORIGINAL8471 closure delivered onto current94b; source bytes only.
SELECTED_MANIFEST_SHA = 'b596bbf0f7b55698f9bbb4b0f78c084bd8b09a7c0b1293a6c1c55dd3c621603f'
SELECTED_KINDS = {
    'codec-reference.json': 'generated',
    'codec_oracle.go.in': 'authored',
    'extract_language.py': 'translated',
    'extract_normalization.py': 'translated',
    'language-cases.json': 'generated',
    'language-reference.json': 'generated',
    'language.rs': 'translated',
    'language_data.rs': 'generated',
    'language_oracle.go.in': 'authored',
    'mod.rs': 'authored',
    'normalization-reference.json': 'generated',
    'normalization.rs': 'translated',
    'normalization_data.rs': 'generated',
    'normalization_oracle.go.in': 'authored',
    'unicode_properties.rs': 'generated',
    'update_source_manifest.py': 'authored',
    'verify_codec.py': 'authored',
    'verify_language.py': 'authored',
    'verify_normalization.py': 'authored',
}
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
    'runtime-query-normalization': [],
    'runtime-query-page': [],
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
RUNTIME_SELECTIONS = {
    'oracle73a93303/internal/library/story_query.go': 'runtime-query-normalization',
    'oracle73a93303/internal/library/query.go': 'runtime-query-page',
}
RUNTIME_SCOPES = {
    'runtime-query-normalization': 'ONLY borrowed runtime StoryLibraryQuery fields and the single '
        'normalizeStoryLibraryQuery/normalizeStoryLibraryQueryNode pass, its existing criteria '
        'validation and canonicalLanguage boundary. Reuse the SAME frozen normalization pipeline '
        'with checked input-derived resource bounds; saved-query fixed budgets/passes remain '
        'unchanged. No saved-payload decoding/canonicalization, SQL, projection or live authority.',
    'runtime-query-page': 'ONLY ListRequest page/pageSize/sort fields, DefaultPageSize=24, '
        'MaxPageSize=100, SortNameAscending=name_asc, SortImportedNewest=imported_desc and '
        'normalizeListRequest defaults/validation (page default1). No SQL, list execution, output '
        'materialization, metadata, destination/reveal or other query.go behavior.',
}
RUNTIME_SCOPE = 'runtime-one-pass-query-normalization-and-page-sort-only'
RUNTIME_LEAVES = ('crates/librairii-core/src/query/mod.rs',
                  'crates/librairii-core/src/collection_codec/runtime.rs')
RUNTIME_EXCLUDED = ['SQL and database effects', 'live definition/value ownership or revision authority',
                    'saved wire/canonicalization budget or pass changes',
                    'query execution/output materialization and metadata/reveal']
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
    if root.is_symlink() or getattr(root, 'is_junction', lambda: False)():
        raise ValueError('aliased source-policy root')
    current = root
    for part in value.split('/'):
        current = current / part
        if current.is_symlink() or getattr(current, 'is_junction', lambda: False)():
            raise ValueError('aliased source-policy path')
    return current


def regular(root, value):
    path = safe(root, value)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise ValueError('missing selected source-policy file') from error
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise ValueError('nonregular or hard-linked selected source-policy file')
    return path



def pinned(root, record):
    path = regular(root, record['path'])
    data = path.read_bytes()
    if len(data) != record['bytes'] or digest(data) != record['sha256']:
        raise ValueError('changed source-policy file')
    return data


def selection(path):
    if path in RUNTIME_SELECTIONS:
        roles = ['product-boundary'] if path.endswith('/story_query.go') else []
        return roles + [RUNTIME_SELECTIONS[path]]
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
    if (set(registry) != {'version', 'product_license', 'bundles'}
            or type(registry['version']) is not int or registry['version'] != 1
            or registry['product_license'] is not None
            or not isinstance(registry['bundles'], list)
            or len(registry['bundles']) != len(APPROVED_BUNDLES)
            or any(not isinstance(bundle, dict) or not isinstance(bundle.get('id'), str)
                   for bundle in registry['bundles'])
            or {bundle['id'] for bundle in registry['bundles']} != APPROVED_BUNDLES):
        raise ValueError('source registry must not assign a product license or widen scope')
    bundle = next(bundle for bundle in registry['bundles'] if bundle['id'] == BUNDLE)
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
            or bundle.get('runtime_selection_scopes') != RUNTIME_SCOPES
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
    previous = {k: v for k, v in index.items()
                if k not in ('runtimeSourceFiles', 'runtimeSourceProvenance')}
    if digest((json.dumps(previous, indent=2) + '\n').encode()) != PREVIOUS_QUERY_INDEX_SHA:
        raise ValueError('changed previous62 source identities or provenance')
    historical = {k: v for k, v in previous.items()
                  if k not in ('querySourceFiles', 'querySourceProvenance')}
    if digest((json.dumps(historical, indent=2) + '\n').encode()) != HISTORICAL_INDEX_SHA:
        raise ValueError('changed historical49 source identities or provenance')
    query_sources = index.get('querySourceFiles', [])
    if (len(query_sources) != len(QUERY_SELECTIONS)
            or {r['path'] for r in query_sources} != set(QUERY_SELECTIONS)
            or index.get('querySourceProvenance', {}).get('historicalSourceIndexSha256') != HISTORICAL_INDEX_SHA):
        raise ValueError('missing extra or duplicate query origins')
    runtime_sources = index.get('runtimeSourceFiles', [])
    if (len(runtime_sources) != 1 or runtime_sources[0]['path'] !=
            'oracle73a93303/internal/library/query.go'
            or index.get('runtimeSourceProvenance', {}).get('previousQuerySourceIndexSha256') !=
            PREVIOUS_QUERY_INDEX_SHA):
        raise ValueError('missing extra or duplicate runtime origins')
    sources = [f for package in index['packages'] for f in package['files']]
    if len(sources) != 43:
        raise ValueError('changed package-source set')
    sources += index['supplementalSourceFiles']
    if len(sources) != 49:
        raise ValueError('changed historical49 source set')
    sources += query_sources
    sources += runtime_sources
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
            'historical_python_baseline_sha256', 'delivery_lineage', 'previous_query_index_sha256', 'runtime', 'identity_additions', 'url_codec', 'tagging'}
    if (set(value) != keys or value['version'] != 2 or value['bundle'] != BUNDLE
            or value['scope'] != 'single-materialized-core-query-codec'
            or value['lookup_index_sha256'] != INDEX_SHA
            or value['historical_index_sha256'] != HISTORICAL_INDEX_SHA
            or value['previous_query_index_sha256'] != PREVIOUS_QUERY_INDEX_SHA
            or value['historical_manifest_sha256'] != HISTORICAL_MANIFEST_SHA
            or value['historical_inline'] != HISTORICAL_INLINE):
        raise ValueError('invalid materialized core map or historical inline identity')
    tagging = value['tagging']
    if (set(tagging) != {'path', 'bytes', 'sha256', 'source', 'source_sha256', 'selection', 'notices', 'recipe'}
            or tagging['path'] != 'crates/librairii-core/src/tagging/unicode15.rs'
            or tagging['source'] != 'Go1.26.5/src/unicode/tables.go'
            or tagging['source_sha256'] != by_path[tagging['source']]['sha256']
            or tagging['notices'] != ['LICENSE-GO-BSD.txt', 'PATENTS-GO.txt', 'LICENSE-UNICODE-V3.txt']
            or tagging['selection'] != 'Unicode15 Letter and Nd ranges only; simple lowercase/NFKD reuse the original codec pipeline.'
            or tagging['recipe']['path'] != 'xtask/extract_tag_categories.py'):
        raise ValueError('invalid frozen tag category source')
    pinned(root, tagging)
    pinned(root, tagging['recipe'])
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
    check_runtime_map(root, value['runtime'], by_path)
    check_url_map(root, value['url_codec'])
    complete = {'mod.rs', 'runtime.rs', 'url.rs', 'compatibility/source-manifest.json'} | {
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
    delivered = check_delivery_lineage(root, value)
    for kind, path, expected_sha, count in [
            ('rust', '.config/rust-test-baseline.json', HISTORICAL_RUST_BASELINE_SHA, 12),
            ('python', '.config/rust-python-test-baseline.json', HISTORICAL_PYTHON_BASELINE_SHA, 3)]:
        if value['historical_' + kind + '_baseline_sha256'] != expected_sha:
            raise ValueError('changed historical discovery baseline identity')
        baseline = delivered[kind]
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


def check_delivery_lineage(root, value):
    """Retain current-base assertions while checking original codec history."""
    delivery = value['delivery_lineage']
    pins = {
        'base_commit': '94b2807e2e23275e574fb030e74fb31223a5497f',
        'codec_commit': '8471bc2b38aa9efce931c3e5fb520116418672a8',
        'base_rust_baseline_sha256': 'b0ed4de7f6d7348f44c96d9d3b03518bc18f30dcd82de74e013294b184227ab6',
        'base_python_baseline_sha256': '134d96e0ee9281a89bfc36d1159bdb58a8914d73da2a0ae72fb5389a9647e3af',
        'codec_rust_baseline_sha256': 'd6a9f409abcb1e0264ee75b0b89d452bdc062e6abb4c161deee84a21464504a0',
        'codec_python_baseline_sha256': '9b2f8529cfaf8d90108c9c11a0477c7766968328b80efe619421282c7ded7b9c',
    }
    if (set(delivery) != set(pins) | {'retained_rust_additions',
            'retained_studio_python_additions', 'codec_python_additions', 'qualification'}
            or any(delivery[key] != expected for key, expected in pins.items())):
        raise ValueError('changed current-base or original-codec discovery identity')

    def remove(baseline, additions):
        if (not isinstance(additions, list)
                or len({item['position'] for item in additions}) != len(additions)):
            raise ValueError('invalid retained discovery additions')
        for item in sorted(additions, key=lambda item: item['position'], reverse=True):
            if (set(item) != {'position', 'identity'} or type(item['position']) is not int
                    or item['position'] < 0 or item['position'] >= len(baseline['required'])
                    or baseline['required'][item['position']] != item['identity']):
                raise ValueError('changed retained discovery addition')
            del baseline['required'][item['position']]

    def encoded(baseline):
        return digest((json.dumps(baseline, indent=2) + '\n').encode())

    originals = {}
    for kind, path in [('rust', '.config/rust-test-baseline.json'),
                       ('python', '.config/rust-python-test-baseline.json')]:
        baseline = decode(safe(root, path).read_bytes())
        additions = value['identity_additions'][kind]
        if (not isinstance(additions, list)
                or len({x['position'] for x in additions}) != len(additions)):
            raise ValueError('invalid explicit discovery additions')
        for addition in sorted(additions, key=lambda x: x['position'], reverse=True):
            if (set(addition) != {'position', 'identity', 'reason'}
                    or type(addition['position']) is not int or addition['position'] < 0
                    or addition['position'] >= len(baseline['required'])
                    or baseline['required'][addition['position']] != addition['identity']
                    or not isinstance(addition['reason'], str) or not addition['reason'].strip()):
                raise ValueError('unobserved or misplaced discovery addition')
            del baseline['required'][addition['position']]
        base = copy.deepcopy(baseline)
        if kind == 'rust':
            for move in value['identity_migrations']['rust']:
                if base['required'][move['position']] != move['new']:
                    raise ValueError('changed current-base codec discovery move')
                base['required'][move['position']] = move['old']
            remove(baseline, delivery['retained_rust_additions'])
        else:
            remove(base, delivery['codec_python_additions'])
            remove(baseline, delivery['retained_studio_python_additions'])
        if encoded(base) != pins['base_' + kind + '_baseline_sha256']:
            raise ValueError('unrelated current-base discovery identities changed')
        if encoded(baseline) != pins['codec_' + kind + '_baseline_sha256']:
            raise ValueError('original codec discovery lineage changed')
        originals[kind] = baseline
    return originals


def check_selected(root):
    """Require the actual selected nineteen-file closure on the CLI path.

    check() also validates the materialized core map and permits the extra
    owned synthetic derivative used by existing provenance regressions. The
    CLI additionally requires the exact selected nineteen-record manifest;
    it binds every selected path, byte hash, kind and recipe.
    """
    directory = safe(root, DERIVATIVES)
    manifest_path = regular(directory, 'source-manifest.json')
    data = manifest_path.read_bytes()
    if digest(data) != SELECTED_MANIFEST_SHA:
        raise ValueError('changed source-policy file: selected derivative manifest')
    manifest = decode(data)
    records = manifest['files']
    if (len(records) != len(SELECTED_KINDS)
            or {record['path']: record['kind'] for record in records} != SELECTED_KINDS):
        raise ValueError('changed selected derivative paths or kinds')
    expected = set(SELECTED_KINDS) | {'source-manifest.json'}
    actual = set()
    for path in directory.iterdir():
        regular(directory, path.name)
        actual.add(path.name)
    if actual != expected:
        raise ValueError('extra or missing selected source-policy file')
    regular(root, CORE_PATH)
    return check(root)



def check_url_map(root, value):
    if (set(value) != {'oracle', 'origin', 'scope', 'leaf'}
            or value['oracle'] != '73a93303a9a44cf0a92eaf525331561ed8c04460'
            or value['origin'] != {'path': 'internal/library/query_codec.go', 'bytes': 12015,
                'sha256': 'ac644cb802938cfc2ce23bdb80d334f47f3d20e4376a4f2c6df8d2fe79bb4389'}
            or value['scope'] != 'bounded URL v1-v3 decoding, native request persistence and typed browser-history handover; no system URL protocol or live authority'
            or set(value['leaf']) != {'path', 'bytes', 'sha256'}
            or value['leaf']['path'] != 'crates/librairii-core/src/collection_codec/url.rs'):
        raise ValueError('unreviewed URL origin, scope or leaf')
    pinned(root, value['leaf'])


def check_runtime_map(root, value, by_path):
    if (set(value) != {'scope', 'origins', 'selection_scopes', 'leaves', 'notices',
                       'recipe', 'excluded'} or value['scope'] != RUNTIME_SCOPE
            or value['selection_scopes'] != RUNTIME_SCOPES
            or value['excluded'] != RUNTIME_EXCLUDED or value['notices'] != []
            or not isinstance(value['recipe'], str) or not value['recipe'].strip()):
        raise ValueError('invalid bounded runtime query scope or notices')
    expected = [{'path': path, 'sha256': by_path[path]['sha256'],
                 'header_sha256': by_path[path]['first12LinesSha256'], 'selection': role}
                for path, role in RUNTIME_SELECTIONS.items()]
    if value['origins'] != expected:
        raise ValueError('unapproved runtime query origin header or selection')
    leaves = value['leaves']
    if len(leaves) != len(RUNTIME_LEAVES) or [x['path'] for x in leaves] != list(RUNTIME_LEAVES):
        raise ValueError('missing duplicate or extra materialized runtime query leaf')
    for leaf in leaves:
        if (set(leaf) != {'path', 'bytes', 'sha256', 'kind', 'header_bytes', 'header_sha256'}
                or leaf['kind'] != 'translated' or not isinstance(leaf['header_bytes'], int)
                or leaf['header_bytes'] < 1 or leaf['header_bytes'] > leaf['bytes']):
            raise ValueError('invalid materialized runtime query leaf')
        content = pinned(root, leaf)
        if digest(content[:leaf['header_bytes']]) != leaf['header_sha256']:
            raise ValueError('changed materialized runtime query header identity')
    directory = safe(root, RUNTIME_LEAVES[0]).parent
    actual = set()
    for path in directory.rglob('*'):
        name = path.relative_to(directory).as_posix()
        safe(directory, name)
        if path.is_file():
            actual.add(name)
    if actual != {'mod.rs'}:
        raise ValueError('incomplete or extra materialized runtime query namespace')


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
    report = check_selected(root)
    if sys.argv[1:] == ['--inventory']:
        inventory(root, report)
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
