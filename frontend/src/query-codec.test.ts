import {expect, test} from 'vitest';
import fixtures from '../../internal/library/testdata/story_library_query_codec.json';
import {
    type CollectionQuery,
    decodeCollectionQuery,
    encodeCollectionQuery,
} from './query-codec';

test('matches the shared Go collection-query codec fixtures', () => {
    for (const fixture of fixtures) {
        const query = fixture.query as CollectionQuery;
        expect(encodeCollectionQuery(query), fixture.name).toBe(fixture.hash);
        expect(decodeCollectionQuery(fixture.hash), fixture.name).toEqual(query);
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
