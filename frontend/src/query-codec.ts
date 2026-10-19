import casefold from './search-casefold.json';

export const STORY_LIBRARY_QUERY_VERSION = 1;
export const STORY_LIBRARY_HASH_PATH = '#/library';

export type BooleanFilterState = 'ignored' | 'true' | 'false';
export type CompatibilityFilter = 'compatible' | 'missing' | 'invalid';

export type CollectionBooleanFilter = {
    definitionId: number;
    state: BooleanFilterState;
};

export type CollectionChoiceFilter = {
    definitionId: number;
    valueIds: number[];
};

export type CollectionQuery = {
    anyOf?: CollectionQuery[];
    includeBroken?: boolean;
    name: string;
    languages: string[];
    compatibilities: CompatibilityFilter[];
    booleanFilters: CollectionBooleanFilter[];
    choiceFilters: CollectionChoiceFilter[];
    page: number;
    pageSize: number;
    sort: 'name_asc' | 'imported_desc';
};

export const DEFAULT_COLLECTION_QUERY: CollectionQuery = {
    includeBroken: false,
    name: '',
    languages: [],
    compatibilities: [],
    booleanFilters: [],
    choiceFilters: [],
    page: 1,
    pageSize: 24,
    sort: 'name_asc',
};

const maxNameRunes = 200;
const maxFilterGroups = 50;
const maxValuesPerFilter = 100;

export class InvalidCollectionQuery extends Error {}

export function encodeCollectionQuery(input: CollectionQuery): string {
    const query = canonicalCollectionQuery(input);
    const entries: Array<[string, string]> = [['v', String(STORY_LIBRARY_QUERY_VERSION)]];
    if (query.anyOf?.length) entries.push(['any', JSON.stringify(query.anyOf)]);
    if (query.includeBroken) {
        entries.push(['includeBroken', 'true']);
    }
    if (query.name) {
        entries.push(['name', query.name]);
    }
    if (query.page !== 1) {
        entries.push(['page', String(query.page)]);
    }
    if (query.pageSize !== DEFAULT_COLLECTION_QUERY.pageSize) {
        entries.push(['size', String(query.pageSize)]);
    }
    if (query.sort !== DEFAULT_COLLECTION_QUERY.sort) {
        entries.push(['sort', query.sort]);
    }
    for (const locale of query.languages) {
        entries.push(['language', locale]);
    }
    for (const compatibility of query.compatibilities) {
        entries.push(['compatibility', compatibility]);
    }
    for (const filter of query.booleanFilters) {
        if (filter.state !== 'ignored') {
            entries.push(['bool', `${filter.definitionId}:${filter.state}`]);
        }
    }
    for (const filter of query.choiceFilters) {
        entries.push(['choice', `${filter.definitionId}:${filter.valueIds.join(',')}`]);
    }
    entries.sort(([left], [right]) => left.localeCompare(right));
    return `${STORY_LIBRARY_HASH_PATH}?${new URLSearchParams(entries).toString()}`;
}

export function decodeCollectionQuery(hash: string): CollectionQuery {
    if (hash === '') {
        return canonicalCollectionQuery(DEFAULT_COLLECTION_QUERY);
    }
    const queryIndex = hash.indexOf('?');
    if (
        queryIndex === -1 ||
        hash.slice(0, queryIndex) !== STORY_LIBRARY_HASH_PATH
    ) {
        throw new InvalidCollectionQuery('Invalid collection hash path.');
    }
    const parameters = new URLSearchParams(hash.slice(queryIndex + 1));
    const allowed = new Set([
        'any',
        'v',
        'includeBroken',
        'name',
        'page',
        'size',
        'sort',
        'language',
        'compatibility',
        'bool',
        'choice',
    ]);
    for (const key of parameters.keys()) {
        if (!allowed.has(key)) {
            throw new InvalidCollectionQuery(`Unknown collection query field: ${key}`);
        }
    }
    if (oneValue(parameters, 'v', true) !== String(STORY_LIBRARY_QUERY_VERSION)) {
        throw new InvalidCollectionQuery('Unsupported collection query version.');
    }

    const page = optionalInteger(parameters, 'page') ?? 1;
    const pageSize = optionalInteger(parameters, 'size') ?? DEFAULT_COLLECTION_QUERY.pageSize;
    const sort = oneValue(parameters, 'sort', false) || DEFAULT_COLLECTION_QUERY.sort;
    const booleanFilters = parameters.getAll('bool').map((encoded) => {
        const [definition, state, ...extra] = encoded.split(':');
        if (extra.length > 0 || !isBooleanState(state)) {
            throw new InvalidCollectionQuery('Invalid boolean filter.');
        }
        return {
            definitionId: positiveInteger(definition),
            state,
        };
    });
    const choiceFilters = parameters.getAll('choice').map((encoded) => {
        const separator = encoded.indexOf(':');
        if (separator === -1 || separator === encoded.length - 1) {
            throw new InvalidCollectionQuery('Invalid choice filter.');
        }
        return {
            definitionId: positiveInteger(encoded.slice(0, separator)),
            valueIds: encoded
                .slice(separator + 1)
                .split(',')
                .map(positiveInteger),
        };
    });
    const includeBroken = oneValue(parameters, 'includeBroken', false);
    if (includeBroken && includeBroken !== 'true') {
        throw new InvalidCollectionQuery('Invalid broken-story visibility.');
    }
    let anyOf: CollectionQuery[] | undefined;
    const alternatives = oneValue(parameters, 'any', false);
    if (alternatives) {
        try {
            if (alternatives.length > 262144) throw new Error();
            anyOf = JSON.parse(alternatives);
            if (!Array.isArray(anyOf)) throw new Error();
        } catch { throw new InvalidCollectionQuery('Invalid combined query.'); }
    }
    return canonicalCollectionQuery({
        ...(anyOf?.length ? {anyOf} : {}),
        includeBroken: includeBroken === 'true',
        name: oneValue(parameters, 'name', false),
        languages: parameters.getAll('language'),
        compatibilities: parameters.getAll('compatibility') as CompatibilityFilter[],
        booleanFilters,
        choiceFilters,
        page,
        pageSize,
        sort: sort as CollectionQuery['sort'],
    });
}

export function canonicalCollectionQuery(input: CollectionQuery): CollectionQuery {
    return canonicalQueryNode(input, {remaining: 50});
}

function canonicalQueryNode(input: CollectionQuery, budget: {remaining: number}): CollectionQuery {
    if (--budget.remaining < 0 || !input || typeof input !== 'object' ||
        (input.anyOf !== undefined && !Array.isArray(input.anyOf)) ||
        typeof input.name !== 'string' || !Array.isArray(input.languages) ||
        !Array.isArray(input.compatibilities) || !Array.isArray(input.booleanFilters) || !Array.isArray(input.choiceFilters)) {
        throw new InvalidCollectionQuery('Invalid combined query.');
    }
    const allowed = new Set(['anyOf', 'includeBroken', 'name', 'languages', 'compatibilities', 'booleanFilters', 'choiceFilters', 'page', 'pageSize', 'sort']);
    const anyOf = input.anyOf?.map((branch) => {
        if (!branch || typeof branch !== 'object' || Array.isArray(branch) || Object.keys(branch).some((key) => !allowed.has(key))) {
            throw new InvalidCollectionQuery('Invalid combined query branch.');
        }
        return canonicalQueryNode({
            ...DEFAULT_COLLECTION_QUERY,
            ...branch,
            languages: branch.languages ?? [],
            compatibilities: branch.compatibilities ?? [],
            booleanFilters: branch.booleanFilters ?? [],
            choiceFilters: branch.choiceFilters ?? [],
        }, budget);
    });
    const name = normalizeSearchText(input.name);
    const languages = [...new Set(input.languages.map(canonicalLanguage))].sort();
    const compatibilities = [...new Set(input.compatibilities)].sort();
    const groupCount = input.booleanFilters.length +
        input.choiceFilters.length +
        Number(languages.length > 0) +
        Number(compatibilities.length > 0);
    if (
        [...name].length > maxNameRunes ||
        !Number.isInteger(input.page) ||
        input.page < 1 ||
        !Number.isInteger(input.pageSize) ||
        input.pageSize < 1 ||
        input.pageSize > 100 ||
        !['name_asc', 'imported_desc'].includes(input.sort) ||
        groupCount > maxFilterGroups ||
        languages.length > maxValuesPerFilter ||
        compatibilities.length > 3 ||
        compatibilities.some((value) => !isCompatibility(value))
    ) {
        throw new InvalidCollectionQuery('Collection query is outside its bounds.');
    }
    const definitions = new Set<number>();
    const booleanFilters = input.booleanFilters.map((filter) => {
        const definitionId = safePositiveInteger(filter.definitionId);
        if (!isBooleanState(filter.state) || definitions.has(definitionId)) {
            throw new InvalidCollectionQuery('Invalid boolean filter.');
        }
        definitions.add(definitionId);
        return {definitionId, state: filter.state};
    });
    const choiceFilters = input.choiceFilters.map((filter) => {
        const definitionId = safePositiveInteger(filter.definitionId);
        if (
            definitions.has(definitionId) ||
            filter.valueIds.length === 0 ||
            filter.valueIds.length > maxValuesPerFilter
        ) {
            throw new InvalidCollectionQuery('Invalid choice filter.');
        }
        definitions.add(definitionId);
        const valueIds = [...new Set(filter.valueIds.map(safePositiveInteger))]
            .sort((left, right) => left - right);
        return {definitionId, valueIds};
    });
    booleanFilters.sort((left, right) => left.definitionId - right.definitionId);
    choiceFilters.sort((left, right) => left.definitionId - right.definitionId);
    return {
        ...(anyOf?.length ? {anyOf} : {}),
        includeBroken: input.includeBroken ?? false,
        name,
        languages,
        compatibilities,
        booleanFilters,
        choiceFilters,
        page: input.page,
        pageSize: input.pageSize,
        sort: input.sort,
    };
}

function canonicalLanguage(value: string): string {
    const normalized = value.trim().replaceAll('_', '-');
    if (normalized === '' || normalized.length > 35) {
        throw new InvalidCollectionQuery('Invalid official language filter.');
    }
    try {
        return Intl.getCanonicalLocales(normalized)[0];
    } catch {
        throw new InvalidCollectionQuery('Invalid official language filter.');
    }
}

function normalizeSearchText(value: string): string {
    return [...value]
        .map((character) => (casefold as Record<string, string>)[character] ?? character)
        .join('')
        .normalize('NFKD')
        .replace(/\p{Mn}/gu, '')
        .replace(/\p{White_Space}+/gu, ' ')
        .replace(/^ | $/gu, '');
}

function oneValue(
    parameters: URLSearchParams,
    key: string,
    required: boolean,
): string {
    const values = parameters.getAll(key);
    if (values.length === 0 && !required) {
        return '';
    }
    if (values.length !== 1 || values[0] === '') {
        throw new InvalidCollectionQuery(`Invalid ${key} field.`);
    }
    return values[0];
}

function optionalInteger(parameters: URLSearchParams, key: string): number | undefined {
    const value = oneValue(parameters, key, false);
    return value ? positiveInteger(value) : undefined;
}

function positiveInteger(value: string): number {
    if (!/^[1-9]\d*$/u.test(value)) {
        throw new InvalidCollectionQuery('Expected a positive integer.');
    }
    return safePositiveInteger(Number(value));
}

function safePositiveInteger(value: number): number {
    if (!Number.isSafeInteger(value) || value <= 0) {
        throw new InvalidCollectionQuery('Expected a safe positive integer.');
    }
    return value;
}

function isBooleanState(value: string): value is BooleanFilterState {
    return ['ignored', 'true', 'false'].includes(value);
}

function isCompatibility(value: string): value is CompatibilityFilter {
    return ['compatible', 'missing', 'invalid'].includes(value);
}
