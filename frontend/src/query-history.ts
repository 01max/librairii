import {
    canonicalCollectionQuery,
    type CollectionQuery,
    decodeCollectionQuery,
    encodeCollectionQuery,
    InvalidCollectionQuery,
} from './query-codec';

type CollectionWindow = Pick<Window, 'addEventListener' | 'removeEventListener'> & {
    history: Pick<History, 'pushState' | 'replaceState' | 'state'>;
    location: Pick<Location, 'hash' | 'pathname' | 'search'>;
};

export type CollectionScope = 'all' | 'collection' | 'shelf' | 'custom';

function hasMembershipCriteria(query: CollectionQuery): boolean {
    return !!query.anyOf?.length || query.name !== '' ||
        query.languages.length > 0 ||
        query.compatibilities.length > 0 ||
        query.booleanFilters.length > 0 ||
        query.choiceFilters.length > 0;
}

function sameMembershipCriteria(
    left: CollectionQuery,
    right: CollectionQuery,
): boolean {
    return JSON.stringify([
        left.name,
        left.languages,
        left.compatibilities,
        left.booleanFilters,
        left.choiceFilters,
        left.anyOf,
    ]) === JSON.stringify([
        right.name,
        right.languages,
        right.compatibilities,
        right.booleanFilters,
        right.choiceFilters,
        right.anyOf,
    ]);
}

type CollectionHistoryState = {
    collectionQuery?: {
        hash: string;
        shelfId?: number | null;
        collectionId?: number | null;
        scope?: CollectionScope;
        group?: {kind: 'shelf'; id: number; manualOrder?: boolean};
    };
};

export class CollectionQueryHistory {
    readonly #target: CollectionWindow;
    readonly #fallback: CollectionQuery;
    #legacyBrokenTagID = 0;

    configureLegacyBrokenTagID(id: number) { this.#legacyBrokenTagID = id; }

    constructor(target: CollectionWindow, fallback: CollectionQuery) {
        this.#target = target;
        this.#fallback = canonicalCollectionQuery(fallback);
    }

    current(): CollectionQuery {
        if (this.#target.location.hash === '') {
            return canonicalCollectionQuery(this.#fallback);
        }
        try {
            return decodeCollectionQuery(this.#target.location.hash, this.#legacyBrokenTagID);
        } catch (error) {
            if (!(error instanceof InvalidCollectionQuery)) {
                throw error;
            }
            return canonicalCollectionQuery(this.#fallback);
        }
    }

    currentCollectionID(): number | null {
        const state = this.#target.history.state as CollectionHistoryState | null;
        const entry = state?.collectionQuery;
        if (!entry || entry.hash !== this.#target.location.hash) return null;
        if (entry.group?.kind === 'shelf') return null;
        const id = entry.collectionId ?? entry.shelfId ?? null;
        return id !== null && Number.isSafeInteger(id) && id > 0 ? id : null;
    }

    // Compatibility for callers being migrated from dynamic Shelf terminology.
    currentShelfID(): number | null { return this.currentCollectionID(); }

    currentCuratedShelfID(): number | null {
        const entry = (this.#target.history.state as CollectionHistoryState | null)?.collectionQuery;
        const id = entry?.hash === this.#target.location.hash && entry.group?.kind === 'shelf' ? entry.group.id : null;
        return id !== null && Number.isSafeInteger(id) && id > 0 ? id : null;
    }

    currentShelfManualOrder(): boolean {
        const entry = (this.#target.history.state as CollectionHistoryState | null)?.collectionQuery;
        return entry?.hash === this.#target.location.hash && entry.group?.kind === 'shelf' ? entry.group.manualOrder !== false : true;
    }

    pushShelf(id: number, query: CollectionQuery = this.#fallback, manualOrder?: boolean): CollectionQuery {
        const canonical = canonicalCollectionQuery(query);
        const hash = encodeCollectionQuery(canonical);
        this.#target.history.pushState({collectionQuery: {hash, group: {kind: 'shelf', id, manualOrder: manualOrder ?? (this.currentCuratedShelfID() === id ? this.currentShelfManualOrder() : true)}, scope: 'shelf'}}, '',
            `${this.#target.location.pathname}${this.#target.location.search}${hash}`);
        return canonical;
    }

    currentScope(): CollectionScope {
        if (this.currentCuratedShelfID() !== null) return 'shelf';
        const shelfID = this.currentShelfID();
        if (shelfID !== null) {
            return 'collection';
        }
        const state = this.#target.history.state as CollectionHistoryState | null;
        const entry = state?.collectionQuery;
        if (
            entry?.hash === this.#target.location.hash &&
            (entry.scope === 'all' || entry.scope === 'custom')
        ) {
            return entry.scope;
        }
        return hasMembershipCriteria(this.current()) ? 'custom' : 'all';
    }

    push(
        query: CollectionQuery,
        shelfId?: number | null,
        scope?: CollectionScope,
    ): CollectionQuery {
        return this.#write('pushState', query, shelfId, scope);
    }

    replace(
        query: CollectionQuery,
        shelfId?: number | null,
        scope?: CollectionScope,
    ): CollectionQuery {
        return this.#write('replaceState', query, shelfId, scope);
    }

    clearBoolean(definitionId: number): CollectionQuery {
        const current = this.current();
        return this.push({
            ...current,
            booleanFilters: current.booleanFilters.filter(
                (filter) => filter.definitionId !== definitionId,
            ),
            page: 1,
        });
    }

    clearChoice(definitionId: number): CollectionQuery {
        const current = this.current();
        return this.push({
            ...current,
            choiceFilters: current.choiceFilters.filter(
                (filter) => filter.definitionId !== definitionId,
            ),
            page: 1,
        });
    }

    clearAll(): CollectionQuery {
        const current = this.current();
        return this.push({
            ...current,
            anyOf: undefined,
            brokenState: undefined,
            name: '',
            languages: [],
            compatibilities: [],
            booleanFilters: [],
            choiceFilters: [],
            page: 1,
        });
    }

    subscribe(
        listener: (
            query: CollectionQuery,
            shelfId: number | null,
            scope: CollectionScope,
            curatedShelfId: number | null,
        ) => void,
    ): () => void {
        const onHistory = () => listener(
            this.current(),
            this.currentShelfID(),
            this.currentScope(),
            this.currentCuratedShelfID(),
        );
        this.#target.addEventListener('popstate', onHistory);
        return () => this.#target.removeEventListener('popstate', onHistory);
    }

    #write(
        method: 'pushState' | 'replaceState',
        query: CollectionQuery,
        shelfId?: number | null,
        scope?: CollectionScope,
    ): CollectionQuery {
        const canonical = canonicalCollectionQuery(query);
        const hash = encodeCollectionQuery(canonical);
        const current = this.current();
        const currentScope = this.currentScope();
        const resolvedShelfID = shelfId === undefined
            ? this.currentShelfID()
            : shelfId;
        const resolvedScope = resolvedShelfID !== null
            ? 'collection'
            : scope ?? (
                currentScope !== 'collection' &&
                    sameMembershipCriteria(current, canonical)
                    ? currentScope
                    : hasMembershipCriteria(canonical) ? 'custom' : 'all'
            );
        const location = `${this.#target.location.pathname}${this.#target.location.search}` +
            hash;
        this.#target.history[method]({
            collectionQuery: {
                hash,
                collectionId: resolvedShelfID,
                scope: resolvedScope,
            },
        }, '', location);
        return canonical;
    }
}
