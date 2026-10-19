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
            sha256: 'd2fdad56f171829119aab462e40192b321ae7ca0f0b184aa6da28cbe4faade36',
            normativeSelector: '.app',
            excludedSelector: '.back',
        });
    });

    test('reproduces every visible sample value', () => {
        expect(CANONICAL_PARITY_FIXTURE.savedCollections.map(
            ({name, count}) => [name, count],
        )).toEqual([
            ['Bedtime', 12],
            ['Adventures', 16],
            ['Favorites', 9],
        ]);
        expect(CANONICAL_PARITY_FIXTURE.mainCollections.map(
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
