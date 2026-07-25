import {
    type CSSProperties,
    useCallback,
    useEffect,
    useMemo,
    useRef,
    useState,
} from 'react';
import './App.css';
import {
    ActiveOperations,
    ApplicationStatus,
    CancelOperation,
    CreateShelf,
    DeleteShelf,
    DuplicateShelf,
    ListShelves,
    OfficialMetadataStatus,
    OpenShelf,
    OperationSnapshot,
    PreviewShelves,
    QueryStories,
    RenameShelf,
    ReorderShelves,
    ReplaceShelfQuery,
    RefreshOfficialMetadata,
    RemoveStory,
    SelectAndImportStories,
    StoryDetail as LoadStoryDetail,
    TagAssignmentWorkspace as LoadTagAssignmentWorkspace,
    TagCatalog as LoadTagCatalog,
} from '../wailsjs/go/main/App';
import {EventsOn} from '../wailsjs/runtime/runtime';
import {library, metadata, operations, shelves, tagging} from '../wailsjs/go/models';
import {
    describeImport,
    operationIsActive,
    operationIsTerminal,
} from './import-state';
import {CollectionQueryHistory} from './query-history';
import {
    type CompatibilityFilter,
    type CollectionQuery,
    DEFAULT_COLLECTION_QUERY,
} from './query-codec';
import TagManager from './TagManager';
import TagAssignmentEditor from './TagAssignmentEditor';
import {
    describeMetadataRefresh,
    describeMetadataStatus,
} from './metadata-state';
import {useModalFocus} from './modal-focus';

const coverPalettes = [
    ['#31559f', '#f7c85b', '#e06a53'],
    ['#ef765f', '#ffe380', '#7354b8'],
    ['#48a88d', '#f5bc41', '#226a63'],
    ['#6a66d9', '#fc8e73', '#343272'],
    ['#c06d9d', '#f8d164', '#743d7b'],
    ['#de9954', '#fff0ab', '#887b42'],
    ['#4aa9c9', '#ffe079', '#326779'],
    ['#d77c6f', '#ffd86b', '#8c4d4c'],
] as const;

const compatibilityOptions: Array<{
    value: CompatibilityFilter;
    label: string;
}> = [
    {value: 'compatible', label: 'Compatible'},
    {value: 'missing', label: 'Archive missing'},
    {value: 'invalid', label: 'Verification failed'},
];

function languageLabel(locale: string): string {
    const language = locale.split('-')[0]?.toLowerCase();
    switch (language) {
        case 'en':
            return 'English';
        case 'fr':
            return 'French';
        case 'de':
            return 'German';
        case 'es':
            return 'Spanish';
        case 'it':
            return 'Italian';
        default:
            return locale;
    }
}

function artworkURL(artworkID?: string): string | undefined {
    return artworkID ? `/artwork/${encodeURIComponent(artworkID)}` : undefined;
}

function formatDuration(durationSeconds?: number): string {
    if (durationSeconds === undefined) {
        return '—';
    }
    const minutes = Math.max(0, Math.round(durationSeconds / 60));
    return `${minutes} min`;
}

function metadataAttribution(story: library.StorySummary): string {
    const author = story.author || 'Local story';
    if (!story.official) {
        return `${author} · ${story.sources.title} metadata`;
    }
    const timestamp = story.official.activatedAt || story.official.fetchedAt;
    const freshness = timestamp
        ? ` · synced ${new Date(timestamp).toLocaleDateString()}`
        : '';
    return `${author} · Official metadata from Lunii catalog${freshness}`;
}

const initialCollectionQuery: CollectionQuery = {
    ...DEFAULT_COLLECTION_QUERY,
    pageSize: 12,
    sort: 'imported_desc',
};

function collectionQueryFromShelf(
    saved: shelves.SavedLibraryQuery,
    current: CollectionQuery,
): CollectionQuery {
    return {
        name: saved.name ?? '',
        languages: [...(saved.languages ?? [])],
        compatibilities: [...(saved.compatibilities ?? [])] as CompatibilityFilter[],
        booleanFilters: (saved.booleanFilters ?? []).map((filter) => ({
            definitionId: filter.definitionId,
            state: filter.state as CollectionQuery['booleanFilters'][number]['state'],
        })),
        choiceFilters: (saved.choiceFilters ?? []).map((filter) => ({
            definitionId: filter.definitionId,
            valueIds: [...filter.valueIds],
        })),
        page: 1,
        pageSize: current.pageSize,
        sort: current.sort,
    };
}

function shelfAttentionMessage(reason?: string): string {
    if (reason === 'unmigratable_query') {
        return 'This saved query uses an unsupported or damaged format and cannot be evaluated safely.';
    }
    return 'One or more saved tag criteria no longer exist. The original query is preserved until you explicitly replace it.';
}

function hasUnavailableCriteria(
    query: CollectionQuery,
    catalog: tagging.Catalog | null,
): boolean {
    if (!catalog) {
        return false;
    }
    const definitions = new Map(
        catalog.definitions.map((definition) => [definition.id, definition]),
    );
    return query.booleanFilters.some(
        (filter) => definitions.get(filter.definitionId)?.kind !== 'boolean',
    ) || query.choiceFilters.some((filter) => {
        const definition = definitions.get(filter.definitionId);
        if (definition?.kind !== 'choice') {
            return true;
        }
        const values = new Set(definition.values.map((value) => value.id));
        return filter.valueIds.some((valueID) => !values.has(valueID));
    });
}

function isAllStoriesQuery(query: CollectionQuery): boolean {
    return query.name === '' &&
        query.languages.length === 0 &&
        query.compatibilities.length === 0 &&
        query.booleanFilters.length === 0 &&
        query.choiceFilters.length === 0;
}

function withoutUnavailableCriteria(
    query: CollectionQuery,
    catalog: tagging.Catalog,
): CollectionQuery {
    const definitions = new Map(
        catalog.definitions.map((definition) => [definition.id, definition]),
    );
    return {
        ...query,
        booleanFilters: query.booleanFilters.filter(
            (filter) => definitions.get(filter.definitionId)?.kind === 'boolean',
        ),
        choiceFilters: query.choiceFilters.flatMap((filter) => {
            const definition = definitions.get(filter.definitionId);
            if (definition?.kind !== 'choice') {
                return [];
            }
            const values = new Set(definition.values.map((value) => value.id));
            const valueIds = filter.valueIds.filter((valueID) => values.has(valueID));
            return valueIds.length > 0 ? [{...filter, valueIds}] : [];
        }),
        page: 1,
    };
}

type CoverStyle = CSSProperties & {
    '--sky': string;
    '--sun': string;
    '--land': string;
};

type ShelfDialogMode = 'save' | 'rename' | 'duplicate';

type CollectionShelfRow = {
    key: string;
    name: string;
    storyCount: number;
    source: string;
    stories: library.StorySummary[];
    shelfID?: number;
};

function paletteFor(storyID: number): CoverStyle {
    const palette = coverPalettes[Math.abs(storyID) % coverPalettes.length];
    return {
        '--sky': palette[0],
        '--sun': palette[1],
        '--land': palette[2],
    };
}

function chunkStories(stories: library.StorySummary[]): library.StorySummary[][] {
    const rows: library.StorySummary[][] = [];
    for (let index = 0; index < stories.length; index += 6) {
        rows.push(stories.slice(index, index + 6));
    }
    return rows;
}

function formatBytes(byteSize: number): string {
    if (byteSize < 1024 * 1024) {
        return `${Math.max(1, Math.round(byteSize / 1024))} KB`;
    }
    return `${(byteSize / (1024 * 1024)).toFixed(1)} MB`;
}

function compatibilityLabel(value: string): string {
    switch (value) {
        case 'compatible':
            return 'Verified';
        case 'missing':
            return 'Missing';
        default:
            return 'Needs attention';
    }
}

function App() {
    const queryHistory = useMemo(
        () => new CollectionQueryHistory(window, initialCollectionQuery),
        [],
    );
    const initialHistoryState = useMemo(() => ({
        query: queryHistory.current(),
        shelfID: queryHistory.currentShelfID(),
    }), [queryHistory]);
    const [applicationState, setApplicationState] = useState('initializing');
    const [collectionQuery, setCollectionQuery] = useState(
        initialHistoryState.query,
    );
    const [page, setPage] = useState<library.Page | null>(null);
    const [selectedID, setSelectedID] = useState<number | null>(null);
    const [selectedIDs, setSelectedIDs] = useState<number[]>([]);
    const [detail, setDetail] = useState<library.StoryDetail | null>(null);
    const [detailRevision, setDetailRevision] = useState(0);
    const [operationSnapshots, setOperationSnapshots] = useState<operations.Snapshot[]>([]);
    const [metadataStatus, setMetadataStatus] =
        useState<metadata.CatalogStatus | null>(null);
    const [requestError, setRequestError] = useState<string | null>(null);
    const [detailsOpen, setDetailsOpen] = useState(false);
    const [removing, setRemoving] = useState(false);
    const [removalError, setRemovalError] = useState<string | null>(null);
    const [removalNotice, setRemovalNotice] = useState<string | null>(null);
    const [expandingCollection, setExpandingCollection] = useState(false);
    const [tagManagerOpen, setTagManagerOpen] = useState(false);
    const [tagAssignmentOpen, setTagAssignmentOpen] = useState(false);
    const [tagAssignmentStoryIDs, setTagAssignmentStoryIDs] = useState<number[]>([]);
    const [assignmentWorkspace, setAssignmentWorkspace] =
        useState<tagging.AssignmentWorkspace | null>(null);
    const [tagCatalog, setTagCatalog] = useState<tagging.Catalog | null>(null);
    const [savedShelves, setSavedShelves] = useState<shelves.Summary[]>([]);
    const [savedShelfStories, setSavedShelfStories] =
        useState<Record<number, library.StorySummary[]>>({});
    const [activeShelfID, setActiveShelfID] = useState<number | null>(
        initialHistoryState.shelfID,
    );
    const [selectedShelfIDs, setSelectedShelfIDs] = useState<number[]>([]);
    const [shelfSelectionPreview, setShelfSelectionPreview] =
        useState<shelves.SelectionPreview | null>(null);
    const [shelfSelectionBusy, setShelfSelectionBusy] = useState(false);
    const [shelfSelectionError, setShelfSelectionError] = useState<string | null>(null);
    const [shelfDialogMode, setShelfDialogMode] = useState<ShelfDialogMode | null>(null);
    const [shelfDialogName, setShelfDialogName] = useState('');
    const [shelfDialogError, setShelfDialogError] = useState<string | null>(null);
    const [shelfBusy, setShelfBusy] = useState(false);
    const [repairShelfID, setRepairShelfID] = useState<number | null>(null);
    const [deleteShelfID, setDeleteShelfID] = useState<number | null>(null);
    const activeShelfIDRef = useRef<number | null>(initialHistoryState.shelfID);
    const refreshedOperations = useRef(new Set<string>());
    const searchInput = useRef<HTMLInputElement>(null);
    const collectionRequestGeneration = useRef(0);
    const shelfPreviewGeneration = useRef(0);
    const shelfDialog = useRef<HTMLFormElement>(null);
    const shelfDialogInitialFocus = useRef<HTMLInputElement>(null);
    const repairShelfDialog = useRef<HTMLElement>(null);
    const repairShelfInitialFocus = useRef<HTMLButtonElement>(null);
    const deleteShelfDialog = useRef<HTMLElement>(null);
    const deleteShelfInitialFocus = useRef<HTMLButtonElement>(null);
    useModalFocus(shelfDialog, shelfDialogInitialFocus, shelfDialogMode !== null);
    useModalFocus(repairShelfDialog, repairShelfInitialFocus, repairShelfID !== null);
    useModalFocus(deleteShelfDialog, deleteShelfInitialFocus, deleteShelfID !== null);
    const activateShelf = useCallback((shelfID: number | null) => {
        activeShelfIDRef.current = shelfID;
        setActiveShelfID(shelfID);
    }, []);

    useEffect(() => {
        if (
            shelfDialogMode === null &&
            repairShelfID === null &&
            deleteShelfID === null
        ) {
            return;
        }
        const closeOnEscape = (event: KeyboardEvent) => {
            if (event.key !== 'Escape' || shelfBusy) {
                return;
            }
            if (deleteShelfID !== null) {
                setDeleteShelfID(null);
            } else if (repairShelfID !== null) {
                setRepairShelfID(null);
            } else {
                setShelfDialogMode(null);
            }
        };
        window.addEventListener('keydown', closeOnEscape);
        return () => window.removeEventListener('keydown', closeOnEscape);
    }, [deleteShelfID, repairShelfID, shelfBusy, shelfDialogMode]);

    const loadCollection = useCallback(async () => {
        const generation = ++collectionRequestGeneration.current;
        try {
            const result = await QueryStories(new library.StoryLibraryQuery(collectionQuery));
            if (generation !== collectionRequestGeneration.current) {
                return;
            }
            if (!result.page) {
                setRequestError(
                    result.error?.message ?? 'The story collection could not be loaded.',
                );
                return;
            }
            setRequestError(null);
            setPage(result.page);
            setDetail(null);
            setDetailRevision((current) => current + 1);
            setSelectedIDs((current) => {
                const visible = current.filter((storyID) => (
                    result.page?.stories.some((story) => story.id === storyID)
                ));
                if (visible.length > 0) {
                    return visible;
                }
                return result.page?.stories[0] ? [result.page.stories[0].id] : [];
            });
            setSelectedID((current) => {
                if (result.page?.stories.some((story) => story.id === current)) {
                    return current;
                }
                return result.page?.stories[0]?.id ?? null;
            });
        } catch {
            if (generation === collectionRequestGeneration.current) {
                setRequestError('The story collection could not be loaded.');
            }
        }
    }, [collectionQuery]);
    const loadCollectionRef = useRef(loadCollection);
    useEffect(() => {
        loadCollectionRef.current = loadCollection;
    }, [loadCollection]);

    useEffect(() => {
        collectionRequestGeneration.current++;
    }, [collectionQuery]);

    useEffect(() => {
        queryHistory.replace(queryHistory.current(), activeShelfIDRef.current);
        return queryHistory.subscribe((query, shelfID) => {
            collectionRequestGeneration.current++;
            activateShelf(shelfID);
            setCollectionQuery(query);
        });
    }, [activateShelf, queryHistory]);

    const refreshMetadataStatus = useCallback(async () => {
        try {
            const response = await OfficialMetadataStatus();
            if (response.error) {
                setRequestError(response.error.message);
                return;
            }
            setMetadataStatus(response.status);
        } catch {
            setRequestError('Official metadata status could not be refreshed.');
        }
    }, []);

    const loadSavedShelves = useCallback(async () => {
        try {
            const response = await ListShelves();
            if (response.error) {
                setRequestError(response.error.message);
                return;
            }
            const nextShelves = response.shelves ?? [];
            setSavedShelves(nextShelves);
            setSelectedShelfIDs((current) => current.filter((shelfID) => (
                nextShelves.some((shelf) => shelf.id === shelfID)
            )));
            const activeID = activeShelfIDRef.current;
            const active = nextShelves.find((shelf) => shelf.id === activeID);
            if (activeID !== null && (!active || active.validity === 'needs_attention')) {
                if (active?.validity === 'needs_attention') {
                    setRepairShelfID(active.id);
                    setPage(null);
                }
                activateShelf(null);
                queryHistory.replace(queryHistory.current(), null);
            }
        } catch {
            setRequestError('Saved shelves could not be loaded.');
        }
    }, [activateShelf, queryHistory]);

    const reconcileOperation = useCallback((snapshot: operations.Snapshot) => {
        setOperationSnapshots((current) => {
            const index = current.findIndex((candidate) => candidate.id === snapshot.id);
            if (index === -1) {
                return [snapshot, ...current];
            }
            if (operationIsTerminal(current[index])) {
                return current;
            }
            const next = [...current];
            next[index] = snapshot;
            return next;
        });
        if (
            operationIsTerminal(snapshot) &&
            !refreshedOperations.current.has(snapshot.id)
        ) {
            refreshedOperations.current.add(snapshot.id);
            void loadCollectionRef.current();
            void loadSavedShelves();
            if (snapshot.kind === 'metadata_sync') {
                void refreshMetadataStatus();
                void LoadTagCatalog().then((response) => {
                    if (response.catalog) {
                        setTagCatalog(response.catalog);
                    }
                });
            }
        }
    }, [loadSavedShelves, refreshMetadataStatus]);

    useEffect(() => {
        let active = true;
        void (async () => {
            try {
                const response = await ApplicationStatus();
                const state = response.error?.code ?? response.status.state;
                if (!active) {
                    return;
                }
                setApplicationState(state);
                if (state === 'ready') {
                    const catalogResponse = await LoadTagCatalog();
                    if (active) {
                        setTagCatalog(catalogResponse.catalog ?? null);
                    }
                    const shelvesResponse = await ListShelves();
                    if (!active) {
                        return;
                    }
                    if (shelvesResponse.error) {
                        setRequestError(shelvesResponse.error.message);
                    } else {
                        const nextShelves = shelvesResponse.shelves ?? [];
                        setSavedShelves(nextShelves);
                        const activeID = activeShelfIDRef.current;
                        const active = nextShelves.find(
                            (shelf) => shelf.id === activeID,
                        );
                        if (
                            activeID !== null &&
                            (!active || active.validity === 'needs_attention')
                        ) {
                            if (active?.validity === 'needs_attention') {
                                setRepairShelfID(active.id);
                                setPage(null);
                            }
                            activateShelf(null);
                            queryHistory.replace(queryHistory.current(), null);
                        }
                    }
                    const metadataResponse = await OfficialMetadataStatus();
                    if (!active) {
                        return;
                    }
                    if (metadataResponse.error) {
                        setRequestError(metadataResponse.error.message);
                    } else {
                        setMetadataStatus(metadataResponse.status);
                    }
                    const operationsResponse = await ActiveOperations();
                    if (!active) {
                        return;
                    }
                    if (operationsResponse.error) {
                        setRequestError(operationsResponse.error.message);
                    } else {
                        for (
                            const snapshot of [...operationsResponse.operations].reverse()
                        ) {
                            reconcileOperation(snapshot);
                        }
                    }
                }
            } catch {
                if (active) {
                    setApplicationState('internal');
                    setRequestError('The application could not be reached.');
                }
            }
        })();
        return () => {
            active = false;
        };
    }, [activateShelf, queryHistory, reconcileOperation]);

    useEffect(() => {
        if (applicationState === 'ready') {
            const timer = window.setTimeout(() => void loadCollection(), 0);
            return () => window.clearTimeout(timer);
        }
    }, [applicationState, loadCollection]);

    useEffect(() => {
        if (applicationState !== 'ready' || selectedShelfIDs.length === 0) {
            return;
        }
        let active = true;
        void (async () => {
            setShelfSelectionBusy(true);
            setShelfSelectionError(null);
            try {
                const response = await PreviewShelves(selectedShelfIDs);
                if (!active) {
                    return;
                }
                if (!response.preview) {
                    setShelfSelectionPreview(null);
                    setShelfSelectionError(
                        response.error?.message ??
                        'The shelf selection could not be previewed.',
                    );
                    return;
                }
                setShelfSelectionPreview(response.preview);
            } catch {
                if (!active) {
                    return;
                }
                setShelfSelectionPreview(null);
                setShelfSelectionError('The shelf selection could not be previewed.');
            } finally {
                if (active) {
                    setShelfSelectionBusy(false);
                }
            }
        })();
        return () => {
            active = false;
        };
    }, [applicationState, savedShelves, selectedShelfIDs]);

    useEffect(() => {
        const unsubscribe = EventsOn('operation:changed', (value: unknown) => {
            reconcileOperation(new operations.Snapshot(value));
        });
        return unsubscribe;
    }, [reconcileOperation]);

    const activeOperationKey = operationSnapshots
        .filter((snapshot) => operationIsActive(snapshot))
        .map((snapshot) => snapshot.id)
        .join('\n');
    useEffect(() => {
        if (!activeOperationKey) {
            return;
        }
        let active = true;
        const requestsInFlight = new Set<string>();
        const operationIDs = activeOperationKey.split('\n');
        const reconcile = async (operationID: string) => {
            if (requestsInFlight.has(operationID)) {
                return;
            }
            requestsInFlight.add(operationID);
            try {
                const response = await OperationSnapshot(operationID);
                if (!active) {
                    return;
                }
                if (response.error) {
                    setRequestError(response.error.message);
                } else if (response.operation) {
                    reconcileOperation(response.operation);
                }
            } catch {
                if (active) {
                    setRequestError('Operation progress could not be refreshed.');
                }
            } finally {
                requestsInFlight.delete(operationID);
            }
        };
        const reconcileAll = () => {
            for (const operationID of operationIDs) {
                void reconcile(operationID);
            }
        };
        reconcileAll();
        const timer = window.setInterval(reconcileAll, 1_000);
        return () => {
            active = false;
            window.clearInterval(timer);
        };
    }, [activeOperationKey, reconcileOperation]);

    async function startImport() {
        setRequestError(null);
        const response = await SelectAndImportStories();
        if (response.error) {
            setRequestError(response.error.message);
            return;
        }
        if (response.operation) {
            reconcileOperation(response.operation);
        }
    }

    async function startMetadataRefresh() {
        setRequestError(null);
        const response = await RefreshOfficialMetadata();
        if (response.error) {
            setRequestError(response.error.message);
            return;
        }
        if (response.operation) {
            reconcileOperation(response.operation);
        }
    }

    async function cancelOperation(operation: operations.Snapshot) {
        if (!operationIsActive(operation)) {
            return;
        }
        const response = await CancelOperation(operation.id);
        if (response.error) {
            setRequestError(response.error.message);
        } else if (response.operation) {
            reconcileOperation(response.operation);
        }
    }

    async function confirmRemoval() {
        if (!selected) {
            return;
        }
        setRemoving(true);
        setRemovalError(null);
        const title = selected.title;
        const response = await RemoveStory(selected.id);
        setRemoving(false);
        if (response.error) {
            setRemovalError(response.error.message);
            return;
        }
        setDetailsOpen(false);
        setDetail(null);
        setSelectedID(null);
        setRemovalNotice(`${title} was moved to application trash.`);
        await loadCollection();
        await loadSavedShelves();
    }

    async function loadAllStories() {
        const generation = ++collectionRequestGeneration.current;
        setExpandingCollection(true);
        setRequestError(null);
        try {
            const first = await QueryStories(new library.StoryLibraryQuery({
                ...collectionQuery,
                page: 1,
                pageSize: 100,
            }));
            if (generation !== collectionRequestGeneration.current) {
                return;
            }
            if (!first.page) {
                setRequestError(first.error?.message ?? 'The full collection could not be loaded.');
                return;
            }
            const byID = new Map(first.page.stories.map((story) => [story.id, story]));
            for (let pageNumber = 2; pageNumber <= first.page.totalPages; pageNumber += 1) {
                const next = await QueryStories(new library.StoryLibraryQuery({
                    ...collectionQuery,
                    page: pageNumber,
                    pageSize: 100,
                }));
                if (generation !== collectionRequestGeneration.current) {
                    return;
                }
                if (!next.page) {
                    setRequestError(next.error?.message ?? 'The full collection could not be loaded.');
                    return;
                }
                for (const story of next.page.stories) {
                    byID.set(story.id, story);
                }
            }
            setPage(new library.Page({
                ...first.page,
                stories: [...byID.values()],
                page: 1,
                pageSize: byID.size,
                totalPages: 1,
            }));
        } finally {
            setExpandingCollection(false);
        }
    }

    useEffect(() => {
        if (selectedID === null) {
            return;
        }
        let active = true;
        void LoadStoryDetail(selectedID).then((response) => {
            if (active) {
                setDetail(response.detail ?? null);
            }
        });
        return () => {
            active = false;
        };
    }, [detailRevision, selectedID]);

    const selectedStoryKey = selectedIDs.join(',');
    const tagAssignmentTargetKey = tagAssignmentStoryIDs.join(',');
    useEffect(() => {
        if (!selectedStoryKey) {
            return;
        }
        let active = true;
        void LoadTagAssignmentWorkspace(
            selectedStoryKey.split(',').map(Number),
        ).then((response) => {
            if (active) {
                setAssignmentWorkspace(response.workspace ?? null);
            }
        });
        return () => {
            active = false;
        };
    }, [selectedStoryKey]);

    const acceptEditorWorkspace = useCallback(
        (workspace: tagging.AssignmentWorkspace) => {
            if (tagAssignmentTargetKey === selectedStoryKey) {
                setAssignmentWorkspace(workspace);
            }
        },
        [selectedStoryKey, tagAssignmentTargetKey],
    );

    const stories = useMemo(() => page?.stories ?? [], [page]);
    const rows = useMemo(() => chunkStories(stories), [stories]);
    const selectedSummary = stories.find((story) => story.id === selectedID) ?? null;
    const selected = detail?.story ?? selectedSummary;
    const selectedPalette = selected ? paletteFor(selected.id) : undefined;
    const activeShelf = savedShelves.find((shelf) => shelf.id === activeShelfID) ?? null;
    const allStoriesActive = activeShelfID === null &&
        isAllStoriesQuery(collectionQuery);
    const previewableShelves = useMemo(() => savedShelves.filter(
        (shelf) => shelf.validity === 'valid',
    ), [savedShelves]);
    const showSavedShelfRows = allStoriesActive && previewableShelves.length > 0;
    const repairShelf = savedShelves.find((shelf) => shelf.id === repairShelfID) ?? null;
    const deleteShelf = savedShelves.find((shelf) => shelf.id === deleteShelfID) ?? null;
    const activeOperations = operationSnapshots.filter((snapshot) => (
        operationIsActive(snapshot)
    ));
    const visibleOperations = activeOperations.length > 0
        ? activeOperations
        : operationSnapshots.slice(0, 1);
    const importing = activeOperations.some((snapshot) => snapshot.kind === 'import');
    const metadataRefreshing = activeOperations.some(
        (snapshot) => snapshot.kind === 'metadata_sync',
    );
    const hasVisibleMetadataOperation = visibleOperations.some(
        (snapshot) => snapshot.kind === 'metadata_sync',
    );
    const derivedDefinitions = tagCatalog?.definitions.filter(
        (definition) => definition.source === 'derived',
    ) ?? [];
    const editableDefinitions = tagCatalog?.definitions.filter(
        (definition) => definition.source !== 'derived',
    ) ?? [];
    const languageOptions = [...new Set([
        ...(metadataStatus?.locale ? [metadataStatus.locale] : []),
        ...collectionQuery.languages,
    ])].sort();
    const collectionRows = useMemo<CollectionShelfRow[]>(() => (
        showSavedShelfRows
            ? previewableShelves.map((shelf) => ({
                key: `saved-shelf-${shelf.id}`,
                name: shelf.name,
                storyCount: shelf.count,
                source: 'Saved shelf',
                stories: savedShelfStories[shelf.id] ?? [],
                shelfID: shelf.id,
            }))
            : rows.map((row, rowIndex) => ({
                key: `collection-${rowIndex}`,
                name: rowIndex === 0 ? 'Recently added' : 'More stories',
                storyCount: row.length,
                source: 'Local archive',
                stories: row,
            }))
    ), [
        previewableShelves,
        rows,
        savedShelfStories,
        showSavedShelfRows,
    ]);
    const empty = page !== null && page.totalItems === 0 && !importing;
    const assignedTags = assignmentWorkspace?.catalog.definitions.flatMap((definition) => {
        const state = assignmentWorkspace.states.find(
            (candidate) => candidate.definitionId === definition.id,
        );
        if (!state) {
            return [];
        }
        if (definition.kind === 'boolean' && state.assignedStories > 0) {
            return [{
                key: `${definition.id}`,
                color: definition.color,
                label: state.assignedStories === assignmentWorkspace.requestedStories
                    ? definition.source === 'derived'
                        ? `System-derived · ${definition.label}`
                        : definition.label
                    : `${definition.source === 'derived'
                        ? 'System-derived · '
                        : ''}${definition.label} · Mixed`,
            }];
        }
        return definition.values.flatMap((value) => {
            const valueState = state.values.find(
                (candidate) => candidate.valueId === value.id,
            );
            if (!valueState || valueState.assignedStories === 0) {
                return [];
            }
            return [{
                key: `${definition.id}-${value.id}`,
                color: definition.color,
                label: valueState.assignedStories === assignmentWorkspace.requestedStories
                    ? `${definition.source === 'derived'
                        ? 'System-derived · '
                        : ''}${definition.label} · ${value.label}`
                    : `${definition.source === 'derived'
                        ? 'System-derived · '
                        : ''}${definition.label} · ${value.label} · Mixed`,
            }];
        });
    }) ?? [];

    useEffect(() => {
        const generation = ++shelfPreviewGeneration.current;
        if (applicationState !== 'ready' || !showSavedShelfRows) {
            return;
        }
        let active = true;
        void Promise.all(previewableShelves.map(async (shelf) => {
            try {
                const response = await OpenShelf(
                    shelf.id,
                    new library.ListRequest({
                        page: 1,
                        pageSize: 6,
                        sort: collectionQuery.sort,
                    }),
                );
                return [
                    shelf.id,
                    response.evaluation?.page.stories ?? [],
                ] as const;
            } catch {
                return [shelf.id, []] as const;
            }
        })).then((previews) => {
            if (!active || generation !== shelfPreviewGeneration.current) {
                return;
            }
            setSavedShelfStories(Object.fromEntries(previews));
        });
        return () => {
            active = false;
        };
    }, [
        applicationState,
        collectionQuery.sort,
        previewableShelves,
        showSavedShelfRows,
    ]);

    const updateQuery = useCallback((
        next: CollectionQuery,
        shelfID?: number | null,
    ) => {
        setCollectionQuery(queryHistory.push(next, shelfID));
    }, [queryHistory]);

    function toggleShelfSelection(shelfID: number) {
        setSelectedShelfIDs((current) => current.includes(shelfID)
            ? current.filter((candidate) => candidate !== shelfID)
            : [...current, shelfID]);
    }

    async function openSavedShelf(shelfID: number) {
        setShelfBusy(true);
        setRequestError(null);
        try {
            const response = await OpenShelf(
                shelfID,
                new library.ListRequest({
                    page: 1,
                    pageSize: collectionQuery.pageSize,
                    sort: collectionQuery.sort,
                }),
            );
            if (!response.evaluation) {
                setRequestError(
                    response.error?.message ?? 'The saved shelf could not be opened.',
                );
                return;
            }
            activateShelf(shelfID);
            setPage(response.evaluation.page);
            updateQuery(collectionQueryFromShelf(
                response.evaluation.query,
                collectionQuery,
            ), shelfID);
        } catch {
            setRequestError('The saved shelf could not be opened.');
        } finally {
            setShelfBusy(false);
        }
    }

    function openAllStories() {
        activateShelf(null);
        updateQuery({
            ...collectionQuery,
            name: '',
            languages: [],
            compatibilities: [],
            booleanFilters: [],
            choiceFilters: [],
            page: 1,
        }, null);
    }

    function showShelfDialog(mode: ShelfDialogMode) {
        setShelfDialogMode(mode);
        setShelfDialogError(null);
        setShelfDialogName(mode === 'rename' ? activeShelf?.name ?? '' : '');
    }

    async function submitShelfDialog() {
        if (!shelfDialogMode) {
            return;
        }
        setShelfBusy(true);
        setShelfDialogError(null);
        try {
            const response = shelfDialogMode === 'save'
                ? await CreateShelf(
                    shelfDialogName,
                    new library.StoryLibraryQuery(collectionQuery),
                )
                : shelfDialogMode === 'rename' && activeShelf
                    ? await RenameShelf(activeShelf.id, shelfDialogName)
                    : shelfDialogMode === 'duplicate' && activeShelf
                        ? await DuplicateShelf(activeShelf.id, shelfDialogName)
                        : null;
            if (!response?.shelf) {
                setShelfDialogError(
                    response?.error?.message ?? 'The saved shelf could not be changed.',
                );
                return;
            }
            const changedShelf = response.shelf;
            setShelfDialogMode(null);
            setShelfDialogName('');
            await loadSavedShelves();
            if (shelfDialogMode === 'save') {
                queryHistory.replace(collectionQuery, changedShelf.id);
                activateShelf(changedShelf.id);
            } else if (shelfDialogMode === 'duplicate') {
                await openSavedShelf(changedShelf.id);
            }
        } catch {
            setShelfDialogError('The saved shelf could not be changed.');
        } finally {
            setShelfBusy(false);
        }
    }

    async function updateActiveShelf() {
        if (!activeShelf) {
            return;
        }
        setShelfBusy(true);
        setRequestError(null);
        try {
            const response = await ReplaceShelfQuery(
                activeShelf.id,
                new library.StoryLibraryQuery(collectionQuery),
            );
            if (!response.shelf) {
                setRequestError(
                    response.error?.message ?? 'The saved shelf could not be updated.',
                );
                return;
            }
            await loadSavedShelves();
        } catch {
            setRequestError('The saved shelf could not be updated.');
        } finally {
            setShelfBusy(false);
        }
    }

    async function moveShelf(shelfID: number, direction: -1 | 1) {
        const index = savedShelves.findIndex((shelf) => shelf.id === shelfID);
        const target = index + direction;
        if (index < 0 || target < 0 || target >= savedShelves.length) {
            return;
        }
        const ordered = [...savedShelves];
        [ordered[index], ordered[target]] = [ordered[target], ordered[index]];
        setShelfBusy(true);
        try {
            const response = await ReorderShelves(ordered.map((shelf) => shelf.id));
            if (response.error) {
                setRequestError(response.error.message);
            } else {
                setSavedShelves(response.shelves ?? []);
            }
        } catch {
            setRequestError('Saved shelves could not be reordered.');
        } finally {
            setShelfBusy(false);
        }
    }

    async function confirmShelfDeletion() {
        if (!deleteShelf) {
            return;
        }
        setShelfBusy(true);
        setShelfDialogError(null);
        try {
            const response = await DeleteShelf(deleteShelf.id);
            if (!response.success) {
                setShelfDialogError(
                    response.error?.message ?? 'The saved shelf could not be deleted.',
                );
                return;
            }
            setDeleteShelfID(null);
            if (activeShelfID === deleteShelf.id) {
                queryHistory.replace(collectionQuery, null);
                activateShelf(null);
            }
            if (repairShelfID === deleteShelf.id) {
                setRepairShelfID(null);
            }
            setSelectedShelfIDs((current) => current.filter(
                (shelfID) => shelfID !== deleteShelf.id,
            ));
            await loadSavedShelves();
        } catch {
            setShelfDialogError('The saved shelf could not be deleted.');
        } finally {
            setShelfBusy(false);
        }
    }

    async function repairShelfWithCurrentQuery() {
        if (!repairShelf || unavailableCriteria) {
            return;
        }
        setShelfBusy(true);
        setShelfDialogError(null);
        try {
            const response = await ReplaceShelfQuery(
                repairShelf.id,
                new library.StoryLibraryQuery(collectionQuery),
            );
            if (!response.shelf) {
                setShelfDialogError(
                    response.error?.message ?? 'The saved shelf could not be repaired.',
                );
                return;
            }
            setRepairShelfID(null);
            queryHistory.replace(collectionQuery, repairShelf.id);
            activateShelf(repairShelf.id);
            await loadSavedShelves();
        } catch {
            setShelfDialogError('The saved shelf could not be repaired.');
        } finally {
            setShelfBusy(false);
        }
    }

    const reconcileTagCatalog = useCallback((catalog: tagging.Catalog) => {
        setTagCatalog(catalog);
        setAssignmentWorkspace((current) => current
            ? new tagging.AssignmentWorkspace({...current, catalog})
            : current);
        void loadSavedShelves();
    }, [loadSavedShelves]);

    function setBooleanFilter(definitionId: number, state: 'ignored' | 'true' | 'false') {
        updateQuery({
            ...collectionQuery,
            booleanFilters: state === 'ignored'
                ? collectionQuery.booleanFilters.filter(
                    (filter) => filter.definitionId !== definitionId,
                )
                : [
                    ...collectionQuery.booleanFilters.filter(
                        (filter) => filter.definitionId !== definitionId,
                    ),
                    {definitionId, state},
                ],
            page: 1,
        });
    }

    function toggleChoiceFilter(definitionId: number, valueId: number) {
        const current = collectionQuery.choiceFilters.find(
            (filter) => filter.definitionId === definitionId,
        )?.valueIds ?? [];
        const values = current.includes(valueId)
            ? current.filter((candidate) => candidate !== valueId)
            : [...current, valueId];
        updateQuery({
            ...collectionQuery,
            choiceFilters: [
                ...collectionQuery.choiceFilters.filter(
                    (filter) => filter.definitionId !== definitionId,
                ),
                ...(values.length > 0 ? [{definitionId, valueIds: values}] : []),
            ],
            page: 1,
        });
    }

    function removeChoiceFilter(definitionId: number) {
        updateQuery({
            ...collectionQuery,
            choiceFilters: collectionQuery.choiceFilters.filter(
                (filter) => filter.definitionId !== definitionId,
            ),
            page: 1,
        });
    }

    function removeUnavailableCriteria() {
        if (!tagCatalog) {
            return;
        }
        updateQuery(withoutUnavailableCriteria(collectionQuery, tagCatalog));
    }

    function toggleLanguage(locale: string) {
        const languages = collectionQuery.languages.includes(locale)
            ? collectionQuery.languages.filter((candidate) => candidate !== locale)
            : [...collectionQuery.languages, locale];
        updateQuery({...collectionQuery, languages, page: 1});
    }

    function toggleCompatibility(value: CompatibilityFilter) {
        const compatibilities = collectionQuery.compatibilities.includes(value)
            ? collectionQuery.compatibilities.filter((candidate) => candidate !== value)
            : [...collectionQuery.compatibilities, value];
        updateQuery({...collectionQuery, compatibilities, page: 1});
    }

    const unavailableCriteria = hasUnavailableCriteria(collectionQuery, tagCatalog);
    const activeFilters = [
        ...(collectionQuery.name
            ? [{
                key: 'name',
                label: `Name contains “${collectionQuery.name}”`,
                remove: () => updateQuery({...collectionQuery, name: '', page: 1}),
            }]
            : []),
        ...collectionQuery.languages.map((locale) => ({
            key: `language-${locale}`,
            label: `Language · ${languageLabel(locale)}`,
            remove: () => toggleLanguage(locale),
        })),
        ...collectionQuery.compatibilities.map((value) => ({
            key: `compatibility-${value}`,
            label: `Import status · ${
                compatibilityOptions.find((option) => option.value === value)?.label ?? value
            }`,
            remove: () => toggleCompatibility(value),
        })),
        ...collectionQuery.booleanFilters.flatMap((filter) => {
            const definition = tagCatalog?.definitions.find(
                (candidate) => candidate.id === filter.definitionId,
            );
            return definition?.kind === 'boolean'
                ? [{
                    key: `boolean-${filter.definitionId}`,
                    label: filter.state === 'true'
                        ? definition.label
                        : `Not ${definition.label}`,
                    remove: () => setBooleanFilter(filter.definitionId, 'ignored'),
                }]
                : [{
                    key: `unavailable-boolean-${filter.definitionId}`,
                    label: `Unavailable saved criterion · definition ${
                        filter.definitionId
                    }`,
                    remove: () => setBooleanFilter(filter.definitionId, 'ignored'),
                }];
        }),
        ...collectionQuery.choiceFilters.flatMap((filter) => {
            const definition = tagCatalog?.definitions.find(
                (candidate) => candidate.id === filter.definitionId,
            );
            if (definition?.kind !== 'choice') {
                return [{
                    key: `unavailable-choice-${filter.definitionId}`,
                    label: `Unavailable saved criterion · definition ${
                        filter.definitionId
                    }`,
                    remove: () => removeChoiceFilter(filter.definitionId),
                }];
            }
            return filter.valueIds.flatMap((valueID) => {
                const value = definition.values.find(
                    (candidate) => candidate.id === valueID,
                );
                return value
                    ? [{
                        key: `choice-${definition.id}-${value.id}`,
                        label: `${definition.label} · ${value.label}`,
                        remove: () => toggleChoiceFilter(definition.id, value.id),
                    }]
                    : [{
                        key: `unavailable-choice-${definition.id}-${valueID}`,
                        label: `Unavailable ${definition.label} value · ${valueID}`,
                        remove: () => toggleChoiceFilter(definition.id, valueID),
                    }];
            });
        }),
    ];

    const renderTagFacet = (definition: tagging.DefinitionWithValues) => (
        <div className="facet" key={definition.id}>
            <div className="facet-title">{definition.label} <b>−</b></div>
            {definition.kind === 'boolean' ? (
                <>
                    <label className="choice">
                        <input
                            type="checkbox"
                            checked={collectionQuery.booleanFilters.some(
                                (filter) => (
                                    filter.definitionId === definition.id &&
                                    filter.state === 'true'
                                ),
                            )}
                            onChange={(event) => setBooleanFilter(
                                definition.id,
                                event.currentTarget.checked ? 'true' : 'ignored',
                            )}
                        />
                        <i style={{'--c': definition.color} as CSSProperties}/>
                        {definition.label}
                    </label>
                    <label className="choice">
                        <input
                            type="checkbox"
                            checked={collectionQuery.booleanFilters.some(
                                (filter) => (
                                    filter.definitionId === definition.id &&
                                    filter.state === 'false'
                                ),
                            )}
                            onChange={(event) => setBooleanFilter(
                                definition.id,
                                event.currentTarget.checked ? 'false' : 'ignored',
                            )}
                        />
                        <i style={{'--c': definition.color} as CSSProperties}/>
                        Not {definition.label}
                    </label>
                </>
            ) : definition.values.map((value) => (
                <label className="choice" key={value.id}>
                    <input
                        type="checkbox"
                        checked={collectionQuery.choiceFilters.some(
                            (filter) => (
                                filter.definitionId === definition.id &&
                                filter.valueIds.includes(value.id)
                            ),
                        )}
                        onChange={() => toggleChoiceFilter(definition.id, value.id)}
                    />
                    <i style={{'--c': definition.color} as CSSProperties}/>
                    {value.label}
                </label>
            ))}
        </div>
    );

    return (
        <div className="app">
            <span className="sr-only" data-testid="application-state">
                Application state: {applicationState}
            </span>
            <aside className="rail" aria-label="Primary navigation">
                <div className="mark" aria-label="Librairii">
                    <i/><i/><i/>
                </div>
                <nav aria-label="Primary navigation">
                    <button className="active" type="button" aria-label="Story collection">▦</button>
                    <button type="button" aria-label="Saved shelves">◇</button>
                    <button type="button" aria-label="Imports">↓</button>
                    <button type="button" aria-label="Exports">↗</button>
                </nav>
                <div className="avatar" aria-label="Local profile">ML</div>
            </aside>

            <aside className="filters" aria-label="Saved shelves and refinements">
                <h1>Librairii</h1>
                <div className="path">Local story archive</div>
                <div className="caption">Saved shelves</div>
                <nav className="saved">
                    <button
                        className={`saved-picker${allStoriesActive ? ' active' : ''}`}
                        type="button"
                        aria-current={allStoriesActive ? 'page' : undefined}
                        disabled={shelfBusy}
                        onClick={openAllStories}
                    >
                        <i style={{'--c': '#405cf5'} as CSSProperties}/>
                        All stories
                        <span>{allStoriesActive ? page?.totalItems ?? 0 : 'All'}</span>
                    </button>
                    {savedShelves.map((shelf, index) => (
                        <div className="saved-entry" key={shelf.id}>
                            <div className="saved-entry-row">
                                <input
                                    className="shelf-selector"
                                    type="checkbox"
                                    aria-label={`Select ${shelf.name} for combined shelf preview`}
                                    checked={selectedShelfIDs.includes(shelf.id)}
                                    disabled={
                                        shelfBusy || shelf.validity === 'needs_attention'
                                    }
                                    onChange={() => toggleShelfSelection(shelf.id)}
                                />
                                <button
                                    className={`saved-picker${
                                        activeShelfID === shelf.id ? ' active' : ''
                                    }${
                                        shelf.validity === 'needs_attention'
                                            ? ' needs-attention'
                                            : ''
                                    }`}
                                    type="button"
                                    aria-current={
                                        activeShelfID === shelf.id ? 'page' : undefined
                                    }
                                    aria-label={shelf.validity === 'needs_attention'
                                        ? `${shelf.name}, needs attention`
                                        : `${shelf.name}, ${shelf.count} ${
                                            shelf.count === 1 ? 'story' : 'stories'
                                        }`}
                                    disabled={shelfBusy}
                                    onClick={() => shelf.validity === 'needs_attention'
                                        ? setRepairShelfID(shelf.id)
                                        : void openSavedShelf(shelf.id)}
                                >
                                    <i
                                        style={{
                                            '--c': coverPalettes[
                                                index % coverPalettes.length
                                            ][0],
                                        } as CSSProperties}
                                    />
                                    {shelf.name}
                                    <span>
                                        {shelf.validity === 'needs_attention'
                                            ? '!'
                                            : shelf.count}
                                    </span>
                                </button>
                            </div>
                            {activeShelfID === shelf.id && (
                                <div className="saved-tools" aria-label={`${shelf.name} actions`}>
                                    <button
                                        type="button"
                                        disabled={shelfBusy || index === 0}
                                        aria-label={`Move ${shelf.name} up`}
                                        onClick={() => void moveShelf(shelf.id, -1)}
                                    >
                                        ↑
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy || index === savedShelves.length - 1}
                                        aria-label={`Move ${shelf.name} down`}
                                        onClick={() => void moveShelf(shelf.id, 1)}
                                    >
                                        ↓
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy}
                                        aria-label={`Rename ${shelf.name}`}
                                        onClick={() => showShelfDialog('rename')}
                                    >
                                        ✎
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy}
                                        aria-label={`Duplicate ${shelf.name}`}
                                        onClick={() => showShelfDialog('duplicate')}
                                    >
                                        ⧉
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy}
                                        aria-label={`Delete ${shelf.name}`}
                                        onClick={() => {
                                            setShelfDialogError(null);
                                            setDeleteShelfID(shelf.id);
                                        }}
                                    >
                                        ×
                                    </button>
                                </div>
                            )}
                        </div>
                    ))}
                </nav>
                {selectedShelfIDs.length > 0 && (
                    <section className="combined-shelves" aria-label="Combined shelf preview">
                        <div className="combined-shelves-title">
                            <b>Combined preview</b>
                            <span>{selectedShelfIDs.length} selected</span>
                        </div>
                        {shelfSelectionBusy && <p>Resolving current membership…</p>}
                        {shelfSelectionError && (
                            <p className="dialog-error">{shelfSelectionError}</p>
                        )}
                        {shelfSelectionPreview && !shelfSelectionBusy && (
                            <>
                                <ul>
                                    {shelfSelectionPreview.shelves.map((shelf) => (
                                        <li key={shelf.id}>
                                            <span>{shelf.name}</span>
                                            <b>{shelf.count}</b>
                                        </li>
                                    ))}
                                </ul>
                                <strong>
                                    {shelfSelectionPreview.uniqueStoryCount}{' '}
                                    unique {shelfSelectionPreview.uniqueStoryCount === 1
                                        ? 'story'
                                        : 'stories'}
                                </strong>
                                <p>
                                    {shelfSelectionPreview.overlapCount === 0
                                        ? 'No overlapping memberships.'
                                        : `${shelfSelectionPreview.overlapCount} overlapping ${
                                            shelfSelectionPreview.overlapCount === 1
                                                ? 'membership'
                                                : 'memberships'
                                        } collapsed.`}
                                </p>
                                <small>
                                    Sources: {shelfSelectionPreview.sourceShelfNames.join(', ')}
                                </small>
                            </>
                        )}
                    </section>
                )}
                <button
                    className="manage shelf-save"
                    type="button"
                    disabled={shelfBusy}
                    onClick={() => showShelfDialog('save')}
                >
                    ＋ Save current query
                </button>
                {activeShelf && (
                    <button
                        className="manage shelf-update"
                        type="button"
                        disabled={shelfBusy}
                        onClick={() => void updateActiveShelf()}
                    >
                        ↻ Update “{activeShelf.name}”
                    </button>
                )}
                <div className="caption">Refine this shelf</div>
                {derivedDefinitions.map(renderTagFacet)}
                <div className="facet">
                    <div className="facet-title">Language <b>−</b></div>
                    {languageOptions.map((locale) => (
                        <label className="choice" key={locale}>
                            <input
                                type="checkbox"
                                checked={collectionQuery.languages.includes(locale)}
                                onChange={() => toggleLanguage(locale)}
                            />
                            <i style={{'--c': '#6779e8'} as CSSProperties}/>
                            {languageLabel(locale)}
                        </label>
                    ))}
                </div>
                <div className="facet">
                    <div className="facet-title">Import status <b>−</b></div>
                    {compatibilityOptions.map((option) => (
                        <label className="choice" key={option.value}>
                            <input
                                type="checkbox"
                                checked={collectionQuery.compatibilities.includes(option.value)}
                                onChange={() => toggleCompatibility(option.value)}
                            />
                            <i style={{'--c': '#f5bc41'} as CSSProperties}/>
                            {option.label}
                        </label>
                    ))}
                </div>
                {editableDefinitions.map(renderTagFacet)}
                <button
                    className="manage"
                    type="button"
                    onClick={() => setTagManagerOpen(true)}
                >
                    ＋ Manage tags
                </button>
            </aside>

            <main className="main">
                <div className="top">
                    <label className="search">
                        <span aria-hidden="true">⌕</span>
                        <input
                            ref={searchInput}
                            type="search"
                            aria-label="Search stories"
                            placeholder={`Search ${page?.totalItems ?? 0} stories, authors, publishers…`}
                            value={collectionQuery.name}
                            onChange={(event) => updateQuery({
                                ...collectionQuery,
                                name: event.currentTarget.value,
                                page: 1,
                            })}
                        />
                    </label>
                    <button
                        type="button"
                        disabled={metadataRefreshing}
                        onClick={() => void startMetadataRefresh()}
                    >
                        {metadataRefreshing ? 'Refreshing…' : '↻ Sync'}
                    </button>
                    <button
                        className="import"
                        type="button"
                        disabled={importing}
                        onClick={() => void startImport()}
                    >
                        {importing ? 'Importing…' : '＋ Import stories'}
                    </button>
                </div>

                <div className="heading">
                    <div>
                        <h2>{activeShelf?.name ?? 'My story shelves'}</h2>
                        <p>
                            {activeShelf ? 'Dynamic saved shelf' : 'Stories in your local archive'}
                            {' · '}
                            {page?.totalItems ?? 0} archives
                        </p>
                    </div>
                    <label className="sort">
                        <span className="sr-only">Sort stories</span>
                        <select
                            value={collectionQuery.sort}
                            onChange={(event) => updateQuery({
                                ...collectionQuery,
                                sort: event.currentTarget.value as CollectionQuery['sort'],
                                page: 1,
                            })}
                        >
                            <option value="imported_desc">Recently added</option>
                            <option value="name_asc">Name A–Z</option>
                        </select>
                    </label>
                </div>

                {activeFilters.length > 0 && (
                    <section className="active-query" aria-label="Active filters">
                        <span>{page?.totalItems ?? 0} matching stories</span>
                        {activeFilters.map((filter) => (
                            <button
                                className="query-chip"
                                type="button"
                                key={filter.key}
                                aria-label={`Remove filter ${filter.label}`}
                                onClick={() => {
                                    filter.remove();
                                    searchInput.current?.focus();
                                }}
                            >
                                {filter.label} ×
                            </button>
                        ))}
                        <button
                            type="button"
                            onClick={() => {
                                updateQuery({
                                    ...collectionQuery,
                                    name: '',
                                    languages: [],
                                    compatibilities: [],
                                    booleanFilters: [],
                                    choiceFilters: [],
                                    page: 1,
                                });
                                searchInput.current?.focus();
                            }}
                        >
                            Clear all
                        </button>
                    </section>
                )}

                {requestError && (
                    <section className="collection-state error" data-state="failed-import">
                        <div className="state-mark" aria-hidden="true">!</div>
                        <div>
                            <h3>The action could not be completed</h3>
                            <p>{requestError}</p>
                        </div>
                    </section>
                )}

                {removalNotice && (
                    <section className="collection-state success" data-state="removal-success">
                        <div className="state-mark" aria-hidden="true">✓</div>
                        <div>
                            <h3>Story removed from this collection</h3>
                            <p>{removalNotice}</p>
                        </div>
                    </section>
                )}

                {metadataStatus && !hasVisibleMetadataOperation && (() => {
                    const notice = describeMetadataStatus(metadataStatus);
                    return (
                        <section
                            className={`collection-state ${notice.tone}`}
                            data-state={notice.state}
                            aria-live="polite"
                        >
                            <div className="state-mark" aria-hidden="true">
                                {notice.tone === 'success' ? '✓' : '↻'}
                            </div>
                            <div className="state-copy">
                                <h3>{notice.title}</h3>
                                <p>{notice.message}</p>
                            </div>
                        </section>
                    );
                })()}

                {visibleOperations.map((operation) => {
                    const notice = operation.kind === 'metadata_sync'
                        ? describeMetadataRefresh(operation, metadataStatus)
                        : describeImport(operation);
                    const operationActive = operationIsActive(operation);
                    return (
                        <section
                            className={`collection-state ${notice.tone}`}
                            data-state={notice.state}
                            aria-live="polite"
                            key={operation.id}
                        >
                            <div className="state-mark" aria-hidden="true">
                                {notice.tone === 'success'
                                    ? '✓'
                                    : notice.tone === 'working'
                                        ? operation.kind === 'metadata_sync' ? '↻' : '↓'
                                        : '!'}
                            </div>
                            <div className="state-copy">
                                <h3>{notice.title}</h3>
                                <p>{notice.message}</p>
                                {operation.kind === 'import' &&
                                    operation.items.length > 0 &&
                                    !operationActive && (
                                    <ul className="operation-items">
                                        {operation.items.map((item) => (
                                            <li key={item.id}>
                                                <b>{item.sourceName}</b>
                                                <span>{item.outcomeMessage || item.status}</span>
                                            </li>
                                        ))}
                                    </ul>
                                )}
                            </div>
                            {operationActive && (
                                <button
                                    type="button"
                                    onClick={() => void cancelOperation(operation)}
                                >
                                    {operation.kind === 'metadata_sync'
                                        ? 'Cancel refresh'
                                        : 'Cancel import'}
                                </button>
                            )}
                        </section>
                    );
                })}

                {empty && (
                    <section className="empty-library" data-state="empty">
                        <div className="empty-mark" aria-hidden="true">
                            <i/><i/><i/>
                        </div>
                        <h3>
                            {activeShelf
                                ? `${activeShelf.name} is currently empty`
                                : 'Build your local story archive'}
                        </h3>
                        <p>
                            {activeShelf
                                ? 'This saved query remains valid and will fill automatically when stories start matching it.'
                                : 'Import story packs you already own. Librairii validates and preserves each archive in managed local storage.'}
                        </p>
                        {activeShelf && (
                            <button
                                type="button"
                                onClick={() => updateQuery({
                                    ...collectionQuery,
                                    name: '',
                                    languages: [],
                                    compatibilities: [],
                                    booleanFilters: [],
                                    choiceFilters: [],
                                    page: 1,
                                })}
                            >
                                Edit shelf query
                            </button>
                        )}
                        <button
                            className="import"
                            type="button"
                            onClick={() => void startImport()}
                        >
                            ＋ Import your first stories
                        </button>
                    </section>
                )}

                {collectionRows.map((row) => (
                    <section className="shelf" key={row.key}>
                        <div className="shelf-head">
                            <h3>{row.name}</h3>
                            <span>
                                {row.storyCount} {
                                    row.storyCount === 1 ? 'story' : 'stories'
                                } · {row.source}
                            </span>
                            <button
                                type="button"
                                disabled={row.shelfID === undefined
                                    ? expandingCollection ||
                                        stories.length >= (page?.totalItems ?? 0)
                                    : shelfBusy}
                                onClick={() => row.shelfID === undefined
                                    ? void loadAllStories()
                                    : void openSavedShelf(row.shelfID)}
                            >
                                {row.shelfID !== undefined
                                    ? 'View all →'
                                    : stories.length >= (page?.totalItems ?? 0)
                                    ? 'All shown'
                                    : expandingCollection
                                        ? 'Loading…'
                                        : 'View all →'}
                            </button>
                        </div>
                        <div className="story-row">
                            {row.stories.map((story) => (
                                <button
                                    className={`story${selectedIDs.includes(story.id) ? ' selected' : ''}`}
                                    style={paletteFor(story.id)}
                                    type="button"
                                    key={story.id}
                                    aria-pressed={selectedIDs.includes(story.id)}
                                    aria-keyshortcuts="Control+Enter Meta+Enter"
                                    onClick={(event) => {
                                        setDetail(null);
                                        if (event.ctrlKey || event.metaKey || event.shiftKey) {
                                            const next = selectedIDs.includes(story.id)
                                                ? selectedIDs.length === 1
                                                    ? selectedIDs
                                                    : selectedIDs.filter((storyID) => storyID !== story.id)
                                                : [...selectedIDs, story.id];
                                            setSelectedIDs(next);
                                            setSelectedID(next.includes(story.id) ? story.id : next[0]);
                                        } else {
                                            setSelectedIDs([story.id]);
                                            setSelectedID(story.id);
                                        }
                                    }}
                                >
                                    <div className={`cover${story.artworkId ? ' has-artwork' : ''}`}>
                                        {story.artworkId && (
                                            <img
                                                src={artworkURL(story.artworkId)}
                                                alt=""
                                                loading="lazy"
                                            />
                                        )}
                                        <b>{story.title}</b>
                                    </div>
                                    <small>
                                        {story.author || story.uuid}
                                    </small>
                                </button>
                            ))}
                        </div>
                    </section>
                ))}
                {!showSavedShelfRows && (page?.totalPages ?? 0) > 1 && (
                    <nav className="pagination" aria-label="Collection pages">
                        <button
                            type="button"
                            disabled={collectionQuery.page <= 1}
                            onClick={() => updateQuery({
                                ...collectionQuery,
                                page: collectionQuery.page - 1,
                            })}
                        >
                            ← Previous
                        </button>
                        <span>
                            Page {page?.page ?? collectionQuery.page} of {page?.totalPages}
                        </span>
                        <button
                            type="button"
                            disabled={collectionQuery.page >= (page?.totalPages ?? 1)}
                            onClick={() => updateQuery({
                                ...collectionQuery,
                                page: collectionQuery.page + 1,
                            })}
                        >
                            Next →
                        </button>
                    </nav>
                )}
            </main>

            {selected && (
                <aside className="drawer" aria-label={`${selected.title} details`}>
                    <div
                        className={`drawer-art${selected.artworkId ? ' has-artwork' : ''}`}
                        style={selectedPalette}
                    >
                        {selected.artworkId && (
                            <img
                                src={artworkURL(selected.artworkId)}
                                alt={`${selected.title} artwork`}
                            />
                        )}
                    </div>
                    <div className="drawer-info">
                        <h3>{selected.title}</h3>
                        <p>{metadataAttribution(selected)}</p>
                        <div className="facts">
                            <div className="fact">
                                <span>Duration</span>
                                <b>{formatDuration(selected.official?.durationSeconds)}</b>
                            </div>
                            <div className="fact">
                                <span>Language</span>
                                <b>
                                    {selected.official?.language
                                        ? languageLabel(selected.official.language)
                                        : '—'}
                                </b>
                            </div>
                            <div className="fact">
                                <span>Status</span>
                                <b>{compatibilityLabel(selected.compatibility)}</b>
                            </div>
                        </div>
                    </div>
                    <div className="drawer-tags">
                        <div className="tag-title">
                            {selectedIDs.length > 1
                                ? `${selectedIDs.length} stories selected`
                                : 'My tags'}
                            <button
                                type="button"
                                onClick={() => {
                                    setTagAssignmentStoryIDs([...selectedIDs]);
                                    setTagAssignmentOpen(true);
                                }}
                            >
                                Edit tags
                            </button>
                        </div>
                        <div className="tags">
                            {assignedTags.map((tag) => (
                                <span className="tag" key={tag.key}>
                                    <i
                                        style={{'--c': tag.color} as CSSProperties}
                                        aria-hidden="true"
                                    />
                                    {tag.label}
                                </span>
                            ))}
                        </div>
                        <div className="archive">
                            {detail?.archive.originalFilename ?? 'Loading archive details…'}
                            {' · '}
                            {formatBytes(detail?.archive.byteSize ?? selected.byteSize)}
                            {' · '}
                            {compatibilityLabel(
                                detail?.archive.verification ?? selected.compatibility,
                            )}
                        </div>
                    </div>
                    <div className="drawer-actions">
                        <button
                            type="button"
                            onClick={() => {
                                setRemovalError(null);
                                setDetailsOpen(true);
                            }}
                        >
                            Open details
                        </button>
                        <button className="export" type="button">Add to export →</button>
                    </div>
                </aside>
            )}

            {detailsOpen && selected && (
                <div className="dialog-backdrop">
                    <section
                        className="detail-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="story-detail-title"
                    >
                        <div className="dialog-kicker">Story details</div>
                        <h3 id="story-detail-title">{selected.title}</h3>
                        <p className="dialog-description">
                            {selected.description || 'No story description is available yet.'}
                        </p>
                        <dl className="dialog-facts">
                            <div>
                                <dt>Story UUID</dt>
                                <dd>{selected.uuid}</dd>
                            </div>
                            <div>
                                <dt>Archive</dt>
                                <dd>{detail?.archive.originalFilename ?? 'Loading…'}</dd>
                            </div>
                            <div>
                                <dt>Verification</dt>
                                <dd>{compatibilityLabel(selected.compatibility)}</dd>
                            </div>
                            <div>
                                <dt>Checksum</dt>
                                <dd>{detail?.archive.sha256 ?? 'Loading…'}</dd>
                            </div>
                        </dl>
                        <div className="removal-confirmation">
                            <b>Remove from this library?</b>
                            <p>
                                The managed archive will move to application trash before its
                                active record is deleted. Your original source file is unchanged.
                            </p>
                        </div>
                        {removalError && <p className="dialog-error">{removalError}</p>}
                        <div className="dialog-actions">
                            <button
                                type="button"
                                disabled={removing}
                                onClick={() => setDetailsOpen(false)}
                            >
                                Cancel
                            </button>
                            <button
                                className="danger"
                                type="button"
                                disabled={removing}
                                onClick={() => void confirmRemoval()}
                            >
                                {removing ? 'Moving to trash…' : 'Move to trash'}
                            </button>
                        </div>
                    </section>
                </div>
            )}
            {shelfDialogMode && (
                <div className="dialog-backdrop">
                    <form
                        ref={shelfDialog}
                        className="detail-dialog shelf-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="shelf-dialog-title"
                        onSubmit={(event) => {
                            event.preventDefault();
                            void submitShelfDialog();
                        }}
                    >
                        <div className="dialog-kicker">Saved shelves</div>
                        <h3 id="shelf-dialog-title">
                            {shelfDialogMode === 'save'
                                ? 'Save the current query'
                                : shelfDialogMode === 'rename'
                                    ? 'Rename this shelf'
                                    : 'Duplicate this shelf'}
                        </h3>
                        <p>
                            {shelfDialogMode === 'save'
                                ? 'The name search and active filters will stay dynamic as your local library changes.'
                                : 'Choose a unique name for this saved shelf.'}
                        </p>
                        <label className="dialog-field">
                            <span>Shelf name</span>
                            <input
                                ref={shelfDialogInitialFocus}
                                maxLength={80}
                                value={shelfDialogName}
                                onChange={(event) => setShelfDialogName(
                                    event.currentTarget.value,
                                )}
                            />
                        </label>
                        {shelfDialogError && (
                            <p className="dialog-error">{shelfDialogError}</p>
                        )}
                        <div className="dialog-actions">
                            <button
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => setShelfDialogMode(null)}
                            >
                                Cancel
                            </button>
                            <button
                                className="export"
                                type="submit"
                                disabled={shelfBusy || shelfDialogName.trim() === ''}
                            >
                                {shelfBusy
                                    ? 'Saving…'
                                    : shelfDialogMode === 'rename'
                                        ? 'Rename shelf'
                                        : shelfDialogMode === 'duplicate'
                                            ? 'Duplicate shelf'
                                            : 'Save shelf'}
                            </button>
                        </div>
                    </form>
                </div>
            )}
            {repairShelf && (
                <div className="dialog-backdrop">
                    <section
                        ref={repairShelfDialog}
                        className="detail-dialog shelf-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="repair-shelf-title"
                    >
                        <div className="dialog-kicker">Saved shelves</div>
                        <h3 id="repair-shelf-title">Repair “{repairShelf.name}”</h3>
                        <p>{shelfAttentionMessage(repairShelf.attentionReason)}</p>
                        <div className="removal-confirmation">
                            <b>Evaluation and export are blocked.</b>
                            <p>
                                Replacing the criteria is explicit and permanent. Stories and
                                managed archives are never changed.
                            </p>
                        </div>
                        {shelfDialogError && (
                            <p className="dialog-error">{shelfDialogError}</p>
                        )}
                        <div className="dialog-actions">
                            <button
                                ref={repairShelfInitialFocus}
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => setRepairShelfID(null)}
                            >
                                Cancel
                            </button>
                            <button
                                className="danger"
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => {
                                    setRepairShelfID(null);
                                    setDeleteShelfID(repairShelf.id);
                                }}
                            >
                                Delete shelf
                            </button>
                            {unavailableCriteria && (
                                <button
                                    type="button"
                                    disabled={shelfBusy || !tagCatalog}
                                    onClick={removeUnavailableCriteria}
                                >
                                    Remove unavailable criteria
                                </button>
                            )}
                            <button
                                className="export"
                                type="button"
                                disabled={shelfBusy || unavailableCriteria}
                                onClick={() => void repairShelfWithCurrentQuery()}
                            >
                                {shelfBusy ? 'Replacing…' : 'Replace with current query'}
                            </button>
                        </div>
                    </section>
                </div>
            )}
            {deleteShelf && (
                <div className="dialog-backdrop">
                    <section
                        ref={deleteShelfDialog}
                        className="detail-dialog shelf-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="delete-shelf-title"
                    >
                        <div className="dialog-kicker">Saved shelves</div>
                        <h3 id="delete-shelf-title">Delete “{deleteShelf.name}”?</h3>
                        <p>
                            Only this saved query will be removed. Matching stories and their
                            managed archives will stay in your library.
                        </p>
                        {shelfDialogError && (
                            <p className="dialog-error">{shelfDialogError}</p>
                        )}
                        <div className="dialog-actions">
                            <button
                                ref={deleteShelfInitialFocus}
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => setDeleteShelfID(null)}
                            >
                                Cancel
                            </button>
                            <button
                                className="danger"
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => void confirmShelfDeletion()}
                            >
                                {shelfBusy ? 'Deleting…' : 'Delete shelf'}
                            </button>
                        </div>
                    </section>
                </div>
            )}
            {tagManagerOpen && (
                <TagManager
                    onClose={() => setTagManagerOpen(false)}
                    onCatalogChange={reconcileTagCatalog}
                />
            )}
            {tagAssignmentOpen && tagAssignmentStoryIDs.length > 0 && (
                <TagAssignmentEditor
                    storyIDs={tagAssignmentStoryIDs}
                    onClose={() => {
                        setTagAssignmentOpen(false);
                        setTagAssignmentStoryIDs([]);
                    }}
                    onWorkspaceChange={acceptEditorWorkspace}
                    onAssignmentsChange={async () => {
                        await loadCollection();
                        await loadSavedShelves();
                    }}
                />
            )}
        </div>
    );
}

export default App;
