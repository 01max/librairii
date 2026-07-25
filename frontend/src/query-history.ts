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

export type CollectionScope = 'all' | 'shelf' | 'custom';

function hasMembershipCriteria(query: CollectionQuery): boolean {
    return query.name !== '' ||
        query.languages.length > 0 ||
        query.compatibilities.length > 0 ||
        query.booleanFilters.length > 0 ||
        query.choiceFilters.length > 0;
}

type CollectionHistoryState = {
    collectionQuery?: {
        hash: string;
        shelfId: number | null;
        scope?: CollectionScope;
    };
};

export class CollectionQueryHistory {
    readonly #target: CollectionWindow;
    readonly #fallback: CollectionQuery;

    constructor(target: CollectionWindow, fallback: CollectionQuery) {
        this.#target = target;
        this.#fallback = canonicalCollectionQuery(fallback);
    }

    current(): CollectionQuery {
        if (this.#target.location.hash === '') {
            return canonicalCollectionQuery(this.#fallback);
        }
        try {
            return decodeCollectionQuery(this.#target.location.hash);
        } catch (error) {
            if (!(error instanceof InvalidCollectionQuery)) {
                throw error;
            }
            return canonicalCollectionQuery(this.#fallback);
        }
    }

    currentShelfID(): number | null {
        const state = this.#target.history.state as CollectionHistoryState | null;
        const entry = state?.collectionQuery;
        if (
            !entry ||
            entry.hash !== this.#target.location.hash ||
            (
                entry.shelfId !== null &&
                (!Number.isSafeInteger(entry.shelfId) || entry.shelfId <= 0)
            )
        ) {
            return null;
        }
        return entry.shelfId;
    }

    currentScope(): CollectionScope {
        const shelfID = this.currentShelfID();
        if (shelfID !== null) {
            return 'shelf';
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
        ) => void,
    ): () => void {
        const onHistory = () => listener(
            this.current(),
            this.currentShelfID(),
            this.currentScope(),
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
        const resolvedShelfID = shelfId === undefined
            ? this.currentShelfID()
            : shelfId;
        const resolvedScope = resolvedShelfID !== null
            ? 'shelf'
            : scope ?? (
                hasMembershipCriteria(canonical) ? 'custom' : 'all'
            );
        const location = `${this.#target.location.pathname}${this.#target.location.search}` +
            hash;
        this.#target.history[method]({
            collectionQuery: {
                hash,
                shelfId: resolvedShelfID,
                scope: resolvedScope,
            },
        }, '', location);
        return canonical;
    }
}
