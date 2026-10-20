import {expect, test} from 'vitest';
import {exportSourceContext} from './export-source';

test('historical shelf source values retain their dynamic Collection meaning', () => {
    expect(exportSourceContext({exportSourceType: 'shelves', sourceShelfNames: ['Old saved search']}))
        .toEqual({label: 'Source collections ·', names: ['Old saved search']});
});

test('new sources distinguish query Collections from approved Shelves', () => {
    expect(exportSourceContext({exportSourceType: 'collection', sourceCollectionNames: ['Adventures']}))
        .toEqual({label: 'Source collections ·', names: ['Adventures']});
    expect(exportSourceContext({exportSourceType: 'curated_shelf', sourceShelfNames: ['Léopold']}))
        .toEqual({label: 'Source shelves ·', names: ['Léopold']});
});
