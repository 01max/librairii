import {describe, expect, it} from 'vitest';
import {canonicalCollectionQuery, decodeCollectionQuery, DEFAULT_COLLECTION_QUERY, encodeCollectionQuery} from './query-codec';

describe('always-visible broken query contract', () => {
    it('normalizes old hide and only settings while retaining other criteria', () => {
        const query = decodeCollectionQuery('#/library?v=2&name=moon&brokenVisibility=only&brokenState=true');
        expect(query.name).toBe('moon');
        expect(query.brokenVisibility).toBeUndefined();
        expect(query.brokenState).toBeUndefined();
        expect(encodeCollectionQuery(query)).toContain('v=3');
        expect(encodeCollectionQuery(query)).not.toContain('broken');
    });

    it('retires only actual legacy built-in references', () => {
        expect(decodeCollectionQuery('#/library?v=1&bool=9:true', 9).booleanFilters).toEqual([]);
        expect(decodeCollectionQuery('#/library?v=3&bool=9:true', 9).booleanFilters).toEqual([{definitionId: 9, state: 'true'}]);
    });

    it('rejects new built-in restrictions and corrupt duplicates', () => {
        expect(() => decodeCollectionQuery('#/library?v=3&brokenVisibility=only')).toThrow();
        expect(() => decodeCollectionQuery('#/library?v=1&bool=9:true&bool=9:false', 9)).toThrow();
        expect(() => canonicalCollectionQuery({...DEFAULT_COLLECTION_QUERY, brokenState: 'true'})).toThrow();
    });
});
