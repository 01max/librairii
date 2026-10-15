import {describe, expect, test} from 'vitest';
import {
    CANONICAL_PARITY_FIXTURE,
    CANONICAL_PROTOTYPE,
} from './parity-fixture';

describe('canonical parity fixture', () => {
    test('records the immutable prototype boundary', () => {
        expect(CANONICAL_PROTOTYPE).toEqual({
            path: 'openspec/ui-prototypes/05-archive-shelves.html',
            snapshotPath: 'testdata/ui-prototypes/05-archive-shelves.html',
            sha256: '6544e87395c92760931d703568a88dad0639b66856f94f1386db49cc94284cd9',
            normativeSelector: '.app',
            excludedSelector: '.back',
        });
    });

    test('reproduces every visible sample value', () => {
        expect(CANONICAL_PARITY_FIXTURE.savedShelves.map(
            ({name, count}) => [name, count],
        )).toEqual([
            ['Bedtime', 12],
            ['Adventures', 16],
            ['Favorites', 9],
        ]);
        expect(CANONICAL_PARITY_FIXTURE.mainShelves.map(
            ({name, count}) => [name, count],
        )).toEqual([
            ['Bedtime', 9],
            ['Weekend adventures', 12],
        ]);
        expect(CANONICAL_PARITY_FIXTURE.stories.map(
            ({title}) => title,
        )).toHaveLength(12);
        expect(CANONICAL_PARITY_FIXTURE.selectedDetail).toMatchObject({
            title: 'The Little Prince',
            facts: [
                {label: 'Stories', value: '12'},
                {label: 'Duration', value: '54 min'},
                {label: 'Language', value: 'English'},
            ],
            archive: {
                originalFilename: 'the-little-prince.v2.pk',
                verification: 'compatible',
            },
        });
        expect(CANONICAL_PARITY_FIXTURE.selectedDetail.tags.map(
            ({label}) => label,
        )).toEqual([
            'Age · 3–5',
            'Mood · Bedtime',
            'Favorite',
        ]);
    });
});
