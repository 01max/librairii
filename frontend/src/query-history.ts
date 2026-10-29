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
export type HandoverFrame = {hash: string; kind: 'library' | 'custom' | 'collection' | 'shelf'; id?: number; manualOrder?: boolean};

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
        handoverKey?: number;
    };
};

export class CollectionQueryHistory {
    readonly #target: CollectionWindow;
    readonly #fallback: CollectionQuery;
    #legacyBrokenTagID = 0;
    #handover: {key: number; frame: HandoverFrame}[] = [];
    #handoverPosition = 0;
    #nextHandoverKey = 1;

    handover(): {frames: HandoverFrame[]; position: number} {
        this.#observeHandover();
        return {frames: this.#handover.map(({frame}) => ({...frame})), position: this.#handoverPosition};
    }

    #frame(): HandoverFrame {
        const scope = this.currentScope();
        const hash = this.#target.location.hash || encodeCollectionQuery(this.#fallback);
        if (scope === 'shelf') return {hash, kind: 'shelf', id: this.currentCuratedShelfID()!, manualOrder: this.currentShelfManualOrder()};
        if (scope === 'collection') return {hash, kind: 'collection', id: this.currentCollectionID()!};
        return {hash, kind: scope === 'all' ? 'library' : 'custom'};
    }

    #observeHandover() {
        const frame = this.#frame();
        const entry = (this.#target.history.state as CollectionHistoryState | null)?.collectionQuery;
        let position = entry?.hash === this.#target.location.hash
            ? this.#handover.findIndex(({key}) => key === entry?.handoverKey) : -1;
        if (position < 0) position = this.#handover.findIndex((item) => JSON.stringify(item.frame) === JSON.stringify(frame));
        if (position >= 0) { this.#handoverPosition = position; return; }
        // Browser history before this session is not enumerable. Keep the
        // current semantic location without reading browser storage internals.
        this.#handover = [{key: 0, frame}]; this.#handoverPosition = 0;
    }

    #recordHandover(method: 'pushState' | 'replaceState') {
        const key = this.#nextHandoverKey++;
        const item = {key, frame: this.#frame()};
        if (method === 'pushState') {
            this.#handover.splice(this.#handoverPosition + 1);
            this.#handover.push(item);
            if (this.#handover.length > 100) this.#handover.shift();
            this.#handoverPosition = this.#handover.length - 1;
        } else this.#handover[this.#handoverPosition] = item;
        const state = this.#target.history.state as CollectionHistoryState;
        this.#target.history.replaceState({collectionQuery: {...state.collectionQuery, handoverKey: key}}, '',
            `${this.#target.location.pathname}${this.#target.location.search}${this.#target.location.hash}`);
    }

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
        this.#observeHandover();
        const canonical = canonicalCollectionQuery(query);
        const hash = encodeCollectionQuery(canonical);
        this.#target.history.pushState({collectionQuery: {hash, group: {kind: 'shelf', id, manualOrder: manualOrder ?? (this.currentCuratedShelfID() === id ? this.currentShelfManualOrder() : true)}, scope: 'shelf'}}, '',
            `${this.#target.location.pathname}${this.#target.location.search}${hash}`);
        this.#recordHandover('pushState');
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
        const onHistory = () => { this.#observeHandover(); listener(
            this.current(),
            this.currentShelfID(),
            this.currentScope(),
            this.currentCuratedShelfID(),
        ); };
        this.#target.addEventListener('popstate', onHistory);
        return () => this.#target.removeEventListener('popstate', onHistory);
    }

    #write(
        method: 'pushState' | 'replaceState',
        query: CollectionQuery,
        shelfId?: number | null,
        scope?: CollectionScope,
    ): CollectionQuery {
        this.#observeHandover();
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
        this.#recordHandover(method);
        return canonical;
    }
}
