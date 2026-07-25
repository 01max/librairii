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

type CollectionHistoryState = {
    collectionQuery?: {
        hash: string;
        shelfId: number | null;
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

    push(query: CollectionQuery, shelfId?: number | null): CollectionQuery {
        return this.#write('pushState', query, shelfId);
    }

    replace(query: CollectionQuery, shelfId?: number | null): CollectionQuery {
        return this.#write('replaceState', query, shelfId);
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
        listener: (query: CollectionQuery, shelfId: number | null) => void,
    ): () => void {
        const onHistory = () => listener(this.current(), this.currentShelfID());
        this.#target.addEventListener('popstate', onHistory);
        return () => this.#target.removeEventListener('popstate', onHistory);
    }

    #write(
        method: 'pushState' | 'replaceState',
        query: CollectionQuery,
        shelfId?: number | null,
    ): CollectionQuery {
        const canonical = canonicalCollectionQuery(query);
        const hash = encodeCollectionQuery(canonical);
        const location = `${this.#target.location.pathname}${this.#target.location.search}` +
            hash;
        this.#target.history[method]({
            collectionQuery: {
                hash,
                shelfId: shelfId === undefined ? this.currentShelfID() : shelfId,
            },
        }, '', location);
        return canonical;
    }
}
