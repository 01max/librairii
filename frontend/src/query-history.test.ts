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
    });

    const reloaded = new CollectionQueryHistory(window, applicationDefault);
    expect(reloaded.current()).toEqual(pushed);
});

test('restores back and forward query states', async () => {
    const history = new CollectionQueryHistory(window, applicationDefault);
    history.replace(applicationDefault);
    const dragon = history.push({...applicationDefault, name: 'dragon'});
    const filtered = history.push({
        ...dragon,
        booleanFilters: [{definitionId: 7, state: 'true'}],
    });
    const listener = vi.fn();
    const unsubscribe = history.subscribe(listener);

    window.history.back();
    await waitFor(() => expect(listener).toHaveBeenLastCalledWith(dragon));
    window.history.forward();
    await waitFor(() => expect(listener).toHaveBeenLastCalledWith(filtered));
    unsubscribe();
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
