import {expect, test} from 'vitest';
import fixtures from '../../internal/library/testdata/story_library_query_codec.json';
import searchFixtures from '../../internal/searchtext/testdata/normalization.json';
import {
    type CollectionQuery,
    canonicalCollectionQuery,
    decodeCollectionQuery,
    encodeCollectionQuery,
} from './query-codec';

test('matches the shared Go collection-query codec fixtures', () => {
    for (const fixture of fixtures) {
        const query = fixture.query as CollectionQuery;
        expect(encodeCollectionQuery(query), fixture.name).toBe(fixture.hash);
        expect(decodeCollectionQuery(fixture.hash), fixture.name).toEqual({...query, includeBroken: query.includeBroken ?? false});
    }
});

test('matches shared Unicode search normalization fixtures', () => {
    for (const fixture of searchFixtures) {
        expect(canonicalCollectionQuery({
            name: fixture.input,
            languages: [],
            compatibilities: [],
            booleanFilters: [],
            choiceFilters: [],
            page: 1,
            pageSize: 24,
            sort: 'name_asc',
        }).name, fixture.input).toBe(fixture.normalized);
    }
});

test('canonicalizes filter and value ordering', () => {
    expect(encodeCollectionQuery({
        name: '',
        languages: ['fr-FR', 'en_gb', 'en-GB'],
        compatibilities: ['invalid', 'compatible', 'invalid'],
        booleanFilters: [
            {definitionId: 8, state: 'false'},
            {definitionId: 2, state: 'true'},
            {definitionId: 12, state: 'ignored'},
        ],
        choiceFilters: [{
            definitionId: 4,
            valueIds: [6, 5, 6],
        }],
        page: 1,
        pageSize: 24,
        sort: 'name_asc',
    })).toBe(
        '#/library?bool=2%3Atrue&bool=8%3Afalse&choice=4%3A5%2C6' +
        '&compatibility=compatible&compatibility=invalid' +
        '&language=en-GB&language=fr-FR&v=1',
    );
});

test('retains combined shelf membership across query navigation and reload', () => {
    const query: CollectionQuery = {...fixtures[0].query as CollectionQuery,
        anyOf: [
            {...fixtures[0].query as CollectionQuery, name: 'moon'},
            {...fixtures[0].query as CollectionQuery, name: 'forest'},
        ],
    };
    expect(decodeCollectionQuery(encodeCollectionQuery(query)))
        .toEqual(canonicalCollectionQuery(query));
});

test.each(['[null]', '["all"]', '[{"selection":[1]}]', '[{"languages":1}]'])(
    'rejects malformed combined query %s instead of broadening its membership', (any) => {
        expect(() => decodeCollectionQuery(`#/library?v=1&any=${encodeURIComponent(any)}`)).toThrow();
    },
);
