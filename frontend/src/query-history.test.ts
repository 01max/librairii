import {waitFor} from '@testing-library/react';
import {beforeEach, expect, test, vi} from 'vitest';
import {CollectionQueryHistory} from './query-history';
import {type CollectionQuery, DEFAULT_COLLECTION_QUERY} from './query-codec';

const applicationDefault: CollectionQuery = {
    ...DEFAULT_COLLECTION_QUERY,
    pageSize: 12,
    sort: 'imported_desc',
};

beforeEach(() => {
    window.history.replaceState(null, '', '/');
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
    expect(reloaded.currentScope()).toBe('shelf');
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
        'shelf',
    ));
    window.history.forward();
    await waitFor(() => expect(listener).toHaveBeenLastCalledWith(
        filtered,
        8,
        'shelf',
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
    expect(history.clearAll()).toEqual(applicationDefault);
});
