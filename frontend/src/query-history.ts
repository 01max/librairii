import {
    canonicalCollectionQuery,
    type CollectionQuery,
    decodeCollectionQuery,
    encodeCollectionQuery,
    InvalidCollectionQuery,
} from './query-codec';

type CollectionWindow = Pick<Window, 'addEventListener' | 'removeEventListener'> & {
    history: Pick<History, 'pushState' | 'replaceState'>;
    location: Pick<Location, 'hash' | 'pathname' | 'search'>;
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

    push(query: CollectionQuery): CollectionQuery {
        return this.#write('pushState', query);
    }

    replace(query: CollectionQuery): CollectionQuery {
        return this.#write('replaceState', query);
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
            booleanFilters: [],
            choiceFilters: [],
            page: 1,
        });
    }

    subscribe(listener: (query: CollectionQuery) => void): () => void {
        const onHistory = () => listener(this.current());
        this.#target.addEventListener('popstate', onHistory);
        return () => this.#target.removeEventListener('popstate', onHistory);
    }

    #write(
        method: 'pushState' | 'replaceState',
        query: CollectionQuery,
    ): CollectionQuery {
        const canonical = canonicalCollectionQuery(query);
        const location = `${this.#target.location.pathname}${this.#target.location.search}` +
            encodeCollectionQuery(canonical);
        this.#target.history[method](null, '', location);
        return canonical;
    }
}
