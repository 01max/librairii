import {waitFor} from '@testing-library/react';
import {beforeEach, expect, test, vi} from 'vitest';
import {CollectionQueryHistory} from './query-history';
import {type CollectionQuery, DEFAULT_COLLECTION_QUERY, canonicalCollectionQuery} from './query-codec';

const applicationDefault: CollectionQuery = {
    ...DEFAULT_COLLECTION_QUERY,
    pageSize: 12,
    sort: 'imported_desc',
};

beforeEach(() => {
    window.history.replaceState(null, '', '/');
});

test('exports bounded semantic back and forward history without raw browser state', async () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.replace(applicationDefault, null, 'all');
    history.push({...applicationDefault, name: 'Bonjour é'}, 7);
    history.pushShelf(7, {...applicationDefault, name: 'Revue'}, false);
    const unsubscribe = history.subscribe(() => {});
    expect(history.handover()).toMatchObject({position: 2, frames: [
        {kind: 'library'}, {kind: 'collection', id: 7}, {kind: 'shelf', id: 7, manualOrder: false},
    ]});
    window.history.back();
    await waitFor(() => expect(history.handover().position).toBe(1));
    expect(history.handover().frames).toHaveLength(3);
    expect(JSON.stringify(history.handover())).not.toContain('handoverKey');
    window.history.forward();
    await waitFor(() => expect(history.handover().position).toBe(2));
    for (let page = 1; page <= 110; page++) history.push({...applicationDefault, page}, null, 'all');
    expect(history.handover().frames).toHaveLength(100);
    expect(history.handover().position).toBe(99);
    unsubscribe();
});

test('restores the same query when history is reconstructed like a reload', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    const pushed = history.push({
        ...applicationDefault,
        name: 'Dragon',
        page: 2,
    }, 7);

    const reloaded = new CollectionQueryHistory(window, applicationDefault);
    expect(reloaded.current()).toEqual(pushed);
    expect(reloaded.currentShelfID()).toBe(7);
    expect(reloaded.currentScope()).toBe('collection');
});

test('older browser entries cannot collide with keys allocated after reload', async () => {
    const first = new CollectionQueryHistory(window, applicationDefault);
    first.replace({...applicationDefault, name: 'A'}, null);
    first.push({...applicationDefault, name: 'B'}, null);
    const reloaded = new CollectionQueryHistory(window, applicationDefault);
    reloaded.handover();
    reloaded.push({...applicationDefault, name: 'C'}, null);
    window.history.back();
    await waitFor(() => expect(reloaded.current().name).toBe('b'));
    reloaded.handover();
    window.history.back();
    await waitFor(() => expect(reloaded.current().name).toBe('a'));
    expect(reloaded.handover()).toEqual({frames: [{hash: window.location.hash, kind: 'custom'}], position: 0});
});

test('restores back and forward query states', async () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.replace(applicationDefault, null);
    const dragon = history.push({...applicationDefault, name: 'dragon'}, 7);
    const filtered = history.push({
        ...dragon,
        booleanFilters: [{definitionId: 7, state: 'true'}],
    }, 8);
    const listener = vi.fn();
    const unsubscribe = history.subscribe(listener);

    window.history.back();
    await waitFor(() => expect(listener).toHaveBeenLastCalledWith(
        dragon,
        7,
        'collection',
        null,
    ));
    window.history.forward();
    await waitFor(() => expect(listener).toHaveBeenLastCalledWith(
        filtered,
        8,
        'collection',
        null,
    ));
    unsubscribe();
});

test('restores custom shelf-less queries without activating All stories', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.push({...applicationDefault, name: 'dragon'}, null);

    const reloaded = new CollectionQueryHistory(window, applicationDefault);
    expect(reloaded.currentScope()).toBe('custom');

    reloaded.replace(applicationDefault, null);
    expect(reloaded.currentScope()).toBe('all');
});

test('keeps sort and pagination changes in the All stories scope', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.replace({
        ...applicationDefault,
        page: 2,
        sort: 'name_asc',
    }, null);

    expect(history.currentScope()).toBe('all');

    history.push({
        ...applicationDefault,
        name: 'dragon',
        sort: 'name_asc',
    }, null);
    expect(history.currentScope()).toBe('custom');

    const cleared = history.clearAll();
    expect(cleared.sort).toBe('name_asc');
    expect(history.currentScope()).toBe('all');
});

test('preserves an explicit All scope when only sorting a filtered fixture', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    const filtered = {
        ...applicationDefault,
        choiceFilters: [{definitionId: 10, valueIds: [101]}],
    };
    history.replace(filtered, null, 'all');

    history.push({...filtered, sort: 'name_asc'});
    expect(history.currentScope()).toBe('all');

    history.push({...filtered, sort: 'name_asc', name: 'forest'});
    expect(history.currentScope()).toBe('custom');
});

test('ignores shelf identity that does not belong to the current query route', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.push({...applicationDefault, name: 'dragon'}, 7);
    window.history.replaceState(
        window.history.state,
        '',
        '/#/library?name=moon&size=12&sort=imported_desc&v=1',
    );

    expect(history.currentShelfID()).toBeNull();
});

test('clears one criterion or all membership filters while retaining sort', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.replace({
        ...applicationDefault,
        name: 'dragon',
        page: 3,
        booleanFilters: [{definitionId: 7, state: 'false'}],
        choiceFilters: [{definitionId: 9, valueIds: [11, 12]}],
    });

    expect(history.clearBoolean(7)).toMatchObject({
        name: 'dragon',
        page: 1,
        booleanFilters: [],
        choiceFilters: [{definitionId: 9, valueIds: [11, 12]}],
        sort: 'imported_desc',
    });
    expect(history.clearChoice(9)).toMatchObject({
        name: 'dragon',
        choiceFilters: [],
    });
    expect(history.clearAll()).toEqual(canonicalCollectionQuery(applicationDefault));
});


test('keeps curated scope identities separate from legacy dynamic shelf ids on reload', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.pushShelf(7, {...applicationDefault, name: 'night'});
    const reloaded = new CollectionQueryHistory(window, applicationDefault);
    expect(reloaded.currentCuratedShelfID()).toBe(7);
    expect(reloaded.currentCollectionID()).toBeNull();
    expect(reloaded.currentScope()).toBe('shelf');
    expect(reloaded.current().name).toBe('night');
    history.push(applicationDefault, 7);
    expect(history.currentCollectionID()).toBe(7);
    expect(history.currentCuratedShelfID()).toBeNull();
});

test('preserves manual versus view sorting within a curated scope across reloads and refinements', () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.pushShelf(7, applicationDefault, false);
    const reloaded = new CollectionQueryHistory(window, applicationDefault);
    expect(reloaded.currentShelfManualOrder()).toBe(false);
    reloaded.pushShelf(7, {...applicationDefault, name: 'moon'});
    expect(reloaded.currentShelfManualOrder()).toBe(false);
    reloaded.pushShelf(7, applicationDefault, true);
    expect(reloaded.currentShelfManualOrder()).toBe(true);
});
