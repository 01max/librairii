import {t} from './i18n';
import {useLanguage} from './use-language';
import ModalBackdrop from './ModalBackdrop';
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
    RecoverStorage,
    RevealExportDestination,
    RenameShelf,
    ReorderShelves,
    ReplaceShelfQuery,
    RefreshOfficialMetadata,
    RemoveStory,
    SelectAndImportStories,
    SelectAndPreflightExport,
    StartPreparedExport,
    StoryDetail as LoadStoryDetail,
    TagAssignmentWorkspace as LoadTagAssignmentWorkspace,
    TagCatalog as LoadTagCatalog,
} from '../wailsjs/go/main/App';
import {EventsOn} from '../wailsjs/runtime/runtime';
import {
    exporter,
    library,
    metadata,
    operations,
    shelves,
    tagging,
} from '../wailsjs/go/models';
import {
    describeImport,
    operationIsActive,
    operationIsTerminal,
} from './import-state';
import {
    type CollectionScope,
    CollectionQueryHistory,
} from './query-history';
import {
    type CompatibilityFilter,
    type CollectionQuery,
    DEFAULT_COLLECTION_QUERY,
} from './query-codec';
import TagManager from './TagManager';
import StoryDetailsDialog from './StoryDetailsDialog';
import LibraryNavigation from './LibraryNavigation';
import LibrarySidebar from './LibrarySidebar';
import LibraryViewPicker from './LibraryViewPicker';
import {useLibraryView} from './library-view';
import {useNoticePreferences} from './notice-preferences';
import OperationHistoryDialog from './OperationHistoryDialog';
import ExportReport from './ExportReport';
import LuniiSync from './LuniiSync';
import StoryCover from './StoryCover';
import TagAssignmentEditor from './TagAssignmentEditor';
import {describeMetadataRefresh} from './metadata-state';
import {useModalFocus} from './modal-focus';
import {describeExport} from './export-state';
import {runPackagedAcceptance} from './packaged-acceptance';

const coverPalettes = [
    ['#31559f', '#f7c85b', '#e06a53'],
    ['#ef765f', '#ffe380', '#7354b8'],
    ['#48a88d', '#f5bc41', '#226a63'],
    ['#6a66d9', '#fc8e73', '#343272'],
    ['#c06d9d', '#f8d164', '#743d7b'],
    ['#de9954', '#fff0ab', '#887b42'],
    ['#4aa9c9', '#ffe079', '#326779'],
    ['#d77c6f', '#ffd86b', '#8c4d4c'],
    ['#4b799a', '#ffbd69', '#294e6c'],
    ['#8071b4', '#f2b962', '#493873'],
    ['#bf8e4d', '#ffe791', '#7d5f33'],
    ['#579c85', '#f5c75a', '#315f53'],
] as const;

const savedShelfColors = ['#ff705c', '#55b79a', '#f5bc41'] as const;

const collectionExpansionBatchSize = 50;

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
            return t("English");
        case 'fr':
            return t("French");
        case 'de':
            return t("German");
        case 'es':
            return t("Spanish");
        case 'it':
            return t("Italian");
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
    return t("{0} min", minutes);
}

function metadataAttribution(story: library.StorySummary): string {
    const author = story.author || t("Local story");
    if (!story.official) {
        return t("{0} · {1} metadata", author, story.sources.title);
    }
    const timestamp = story.official.activatedAt || story.official.fetchedAt;
    if (!timestamp) {
        return author;
    }
    const syncedAt = new Date(timestamp);
    const today = new Date();
    const sameDay = syncedAt.getFullYear() === today.getFullYear() &&
        syncedAt.getMonth() === today.getMonth() &&
        syncedAt.getDate() === today.getDate();
    return t("{0} · metadata synced {1}", author, sameDay ? 'today' : syncedAt.toLocaleDateString());
}

function savedShelfColor(index: number): string {
    return savedShelfColors[index % savedShelfColors.length];
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
        includeBroken: saved.includeBroken ?? false,
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

function savedShelfSource(
    query: shelves.SavedLibraryQuery,
    catalogs: Array<tagging.Catalog | null | undefined>,
): string {
    const definitions = new Map(catalogs.flatMap(
        (catalog) => catalog?.definitions.map(
            (definition) => [definition.id, definition.label] as const,
        ) ?? [],
    ));
    const definitionIDs = [
        ...(query.booleanFilters ?? []).map((filter) => filter.definitionId),
        ...(query.choiceFilters ?? []).map((filter) => filter.definitionId),
    ];
    const labels = [...new Set(definitionIDs.flatMap(
        (definitionID) => definitions.get(definitionID) ?? [],
    ))];
    if (labels.length === 1) {
        return t("{0} tag", labels[0]);
    }
    if (definitionIDs.length > 0) {
        return t("Tag filters");
    }
    return t("Saved shelf");
}

function shelfAttentionMessage(reason?: string): string {
    if (reason === 'unmigratable_query') {
        return t("This saved query uses an unsupported or damaged format and cannot be evaluated safely.");
    }
    return t("One or more saved tag criteria no longer exist. The original query is preserved until you explicitly replace it.");
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
    error?: string;
};

type SavedShelfPreviewDetails = {
    name: string;
    count: number;
    source: string;
};

function paletteFor(storyID: number): CoverStyle {
    const palette = coverPalettes[
        Math.abs(storyID - 1) % coverPalettes.length
    ];
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

function yieldForRendering(): Promise<void> {
    return new Promise((resolve) => {
        window.requestAnimationFrame(() => resolve());
    });
}

function formatBytes(byteSize: number): string {
    if (byteSize <= 0) {
        return '0 KB';
    }
    if (byteSize < 1024 * 1024) {
        return `${Math.max(1, Math.round(byteSize / 1024))} KB`;
    }
    return `${(byteSize / (1024 * 1024)).toFixed(1)} MB`;
}

function compatibilityLabel(value: string): string {
    switch (value) {
        case 'compatible':
            return t("Verified");
        case 'missing':
            return t("Missing");
        default:
            return t("Needs attention");
    }
}

function App() {
    useLanguage();
    const [libraryView, setLibraryView] = useLibraryView();
    const [sidebarOpen, setSidebarOpen] = useState(false);
    const closeSidebar = useCallback(() => setSidebarOpen(false), []);
    const queryHistory = useMemo(
        () => new CollectionQueryHistory(window, initialCollectionQuery),
        [],
    );
    const initialHistoryState = useMemo(() => {
        const query = queryHistory.current();
        const shelfID = queryHistory.currentShelfID();
        return {
            query,
            shelfID,
            scope: queryHistory.currentScope(),
        };
    }, [queryHistory]);
    const [applicationState, setApplicationState] = useState('initializing');
    const [applicationError, setApplicationError] = useState('');
    const [storageRecoveryAvailable, setStorageRecoveryAvailable] = useState(false);
    const [storageRecovering, setStorageRecovering] = useState(false);
    const [collectionQuery, setCollectionQuery] = useState(
        initialHistoryState.query,
    );
    const includeBrokenRef = useRef(!!collectionQuery.includeBroken);
    useEffect(() => {
        includeBrokenRef.current = !!collectionQuery.includeBroken;
    }, [collectionQuery.includeBroken]);
    const [page, setPage] = useState<library.Page | null>(null);
    const [selectedID, setSelectedID] = useState<number | null>(null);
    const [selectedIDs, setSelectedIDs] = useState<number[]>([]);
    const [detail, setDetail] = useState<library.StoryDetail | null>(null);
    const [detailRevision, setDetailRevision] = useState(0);
    const [operationSnapshots, setOperationSnapshots] = useState<operations.Snapshot[]>([]);
    const {dismissedOperations, luniiSyncDismissed, dismissOperation, setLuniiSyncDismissed} =
        useNoticePreferences();
    const [historyOpen, setHistoryOpen] = useState(false);
    const luniiSyncToggle = useRef<HTMLButtonElement>(null);
    const previousSyncDismissal = useRef(luniiSyncDismissed);
    useEffect(() => {
        if (luniiSyncDismissed && !previousSyncDismissal.current) {
            luniiSyncToggle.current?.focus();
        }
        previousSyncDismissal.current = luniiSyncDismissed;
    }, [luniiSyncDismissed]);
    const [metadataStarting, setMetadataStarting] = useState(false);
    const [metadataStatus, setMetadataStatus] =
        useState<metadata.CatalogStatus | null>(null);
    const [requestError, setRequestError] = useState<string | null>(null);
    const [languageFacetOpen, setLanguageFacetOpen] = useState(false);
    const [importFacetOpen, setImportFacetOpen] = useState(false);
    const [collapsedTagFacets, setCollapsedTagFacets] = useState<Set<number>>(new Set());
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
    const [facetCounts, setFacetCounts] = useState<Record<number, number>>({});
    const [collectionRevision, setCollectionRevision] = useState(0);
    const [savedShelves, setSavedShelves] = useState<shelves.Summary[]>([]);
    const [savedShelfStories, setSavedShelfStories] =
        useState<Record<number, library.StorySummary[]>>({});
    const [savedShelfPreviewDetails, setSavedShelfPreviewDetails] =
        useState<Record<number, SavedShelfPreviewDetails>>({});
    const [savedShelfPreviewErrors, setSavedShelfPreviewErrors] =
        useState<Record<number, string>>({});
    const [activeShelfID, setActiveShelfID] = useState<number | null>(
        initialHistoryState.shelfID,
    );
    const [collectionScope, setCollectionScope] = useState<CollectionScope>(
        initialHistoryState.scope,
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
    const [exportPreflight, setExportPreflight] =
        useState<exporter.PreflightReport | null>(null);
    const [exportPreflightRequest, setExportPreflightRequest] =
        useState<exporter.PreflightRequest | null>(null);
    const [exportPreparing, setExportPreparing] = useState(false);
    const [exportStarting, setExportStarting] = useState(false);
    const [exportStartError, setExportStartError] = useState<string | null>(null);
    const activeShelfIDRef = useRef<number | null>(initialHistoryState.shelfID);
    const refreshedOperations = useRef(new Set<string>());
    const searchInput = useRef<HTMLInputElement>(null);
    const openDetailsButton = useRef<HTMLButtonElement>(null);
    const collectionRequestGeneration = useRef(0);
    const shelfPreviewGeneration = useRef(0);
    const shelfListRequestGeneration = useRef(0);
    const shelfDialog = useRef<HTMLFormElement>(null);
    const shelfDialogInitialFocus = useRef<HTMLInputElement>(null);
    const repairShelfDialog = useRef<HTMLElement>(null);
    const repairShelfInitialFocus = useRef<HTMLButtonElement>(null);
    const deleteShelfDialog = useRef<HTMLElement>(null);
    const deleteShelfInitialFocus = useRef<HTMLButtonElement>(null);
    const exportPreflightDialog = useRef<HTMLElement>(null);
    const exportPreflightInitialFocus = useRef<HTMLButtonElement>(null);
    useModalFocus(shelfDialog, shelfDialogInitialFocus, shelfDialogMode !== null);
    useModalFocus(repairShelfDialog, repairShelfInitialFocus, repairShelfID !== null);
    useModalFocus(deleteShelfDialog, deleteShelfInitialFocus, deleteShelfID !== null);
    useModalFocus(
        exportPreflightDialog,
        exportPreflightInitialFocus,
        exportPreflight !== null,
    );
    const activateShelf = useCallback((
        shelfID: number | null,
        scope: CollectionScope,
    ) => {
        activeShelfIDRef.current = shelfID;
        setActiveShelfID(shelfID);
        setCollectionScope(scope);
    }, []);

    useEffect(() => {
        if (
            shelfDialogMode === null &&
            repairShelfID === null &&
            deleteShelfID === null &&
            exportPreflight === null
        ) {
            return;
        }
        const closeOnEscape = (event: KeyboardEvent) => {
            if (
                event.key !== 'Escape' ||
                shelfBusy ||
                exportPreparing ||
                exportStarting
            ) {
                return;
            }
            if (exportPreflight !== null) {
                setExportPreflight(null);
            } else if (deleteShelfID !== null) {
                setDeleteShelfID(null);
            } else if (repairShelfID !== null) {
                setRepairShelfID(null);
            } else {
                setShelfDialogMode(null);
            }
        };
        window.addEventListener('keydown', closeOnEscape);
        return () => window.removeEventListener('keydown', closeOnEscape);
    }, [
        deleteShelfID,
        exportPreflight,
        exportPreparing,
        exportStarting,
        repairShelfID,
        shelfBusy,
        shelfDialogMode,
    ]);

    const loadCollection = useCallback(async () => {
        const generation = ++collectionRequestGeneration.current;
        try {
            const result = await QueryStories(new library.StoryLibraryQuery(collectionQuery));
            if (generation !== collectionRequestGeneration.current) {
                return;
            }
            if (!result.page) {
                setRequestError(
                    result.error?.message ?? t("The story collection could not be loaded."),
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
                setRequestError(t("The story collection could not be loaded."));
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
        queryHistory.replace(
            queryHistory.current(),
            activeShelfIDRef.current,
            initialHistoryState.scope,
        );
        return queryHistory.subscribe((query, shelfID, scope) => {
            collectionRequestGeneration.current++;
            activateShelf(shelfID, scope);
            setCollectionQuery(query);
        });
    }, [activateShelf, initialHistoryState.scope, queryHistory]);

    const refreshMetadataStatus = useCallback(async () => {
        try {
            const response = await OfficialMetadataStatus();
            if (response.error) {
                setRequestError(response.error.message);
                return;
            }
            setMetadataStatus(response.status);
        } catch {
            setRequestError(t("Official metadata status could not be refreshed."));
        }
    }, []);

    const loadSavedShelves = useCallback(async () => {
        try {
            const generation = ++shelfListRequestGeneration.current;
            const response = await ListShelves(includeBrokenRef.current);
            if (generation !== shelfListRequestGeneration.current) return;
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
                activateShelf(null, 'custom');
                queryHistory.replace(queryHistory.current(), null);
            }
        } catch {
            setRequestError(t("Saved shelves could not be loaded."));
        }
    }, [activateShelf, queryHistory]);

    const previousVisibility = useRef(!!collectionQuery.includeBroken);
    useEffect(() => {
        const visibility = !!collectionQuery.includeBroken;
        if (visibility !== previousVisibility.current) {
            previousVisibility.current = visibility;
            if (applicationState === 'ready') void loadSavedShelves();
        }
    }, [collectionQuery.includeBroken, applicationState, loadSavedShelves]);

    const invalidateCollection = useCallback(async () => {
        setCollectionRevision((current) => current + 1);
        await Promise.all([loadCollectionRef.current(), loadSavedShelves()]);
    }, [loadSavedShelves]);

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
            void invalidateCollection();
            if (snapshot.kind === 'metadata_sync') {
                void refreshMetadataStatus();
                void LoadTagCatalog().then((response) => {
                    if (response.catalog) {
                        setTagCatalog(response.catalog);
                    }
                });
            }
        }
    }, [invalidateCollection, refreshMetadataStatus]);

    useEffect(() => {
        let active = true;
        void (async () => {
            try {
                const response = await ApplicationStatus();
                const state = response.status.state;
                if (!active) {
                    return;
                }
                setApplicationState(state);
                setApplicationError(response.error?.message ?? '');
                setStorageRecoveryAvailable(response.status.recoveryAvailable);
                if (state === 'ready') {
                    const catalogResponse = await LoadTagCatalog();
                    if (active) {
                        setTagCatalog(catalogResponse.catalog ?? null);
                    }
                    const shelvesResponse = await ListShelves(includeBrokenRef.current);
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
                            activateShelf(null, 'custom');
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
                    setRequestError(t("The application could not be reached."));
                }
            }
        })();
        return () => {
            active = false;
        };
    }, [activateShelf, queryHistory, reconcileOperation]);

    const recoverStorage = async () => {
        setStorageRecovering(true);
        setApplicationError('');
        try {
            const response = await RecoverStorage();
            if (response.error) {
                setApplicationError(response.error.message);
                return;
            }
            window.location.reload();
        } catch {
            setApplicationError(t("Storage recovery could not be started."));
        } finally {
            setStorageRecovering(false);
        }
    };

    useEffect(() => {
        if (applicationState === 'ready') {
            const timer = window.setTimeout(() => void loadCollection(), 0);
            return () => window.clearTimeout(timer);
        }
    }, [applicationState, loadCollection]);

    useEffect(() => {
        if (applicationState === 'ready') {
            void runPackagedAcceptance();
        }
    }, [applicationState]);

    useEffect(() => {
        if (applicationState !== 'ready' || selectedShelfIDs.length === 0) {
            return;
        }
        let active = true;
        void (async () => {
            setShelfSelectionBusy(true);
            setShelfSelectionError(null);
            try {
                const response = await PreviewShelves(selectedShelfIDs, !!collectionQuery.includeBroken);
                if (!active) {
                    return;
                }
                if (!response.preview) {
                    setShelfSelectionPreview(null);
                    setShelfSelectionError(
                        response.error?.message ??
                        t("The shelf selection could not be previewed."),
                    );
                    return;
                }
                setShelfSelectionPreview(response.preview);
            } catch {
                if (!active) {
                    return;
                }
                setShelfSelectionPreview(null);
                setShelfSelectionError(t("The shelf selection could not be previewed."));
            } finally {
                if (active) {
                    setShelfSelectionBusy(false);
                }
            }
        })();
        return () => {
            active = false;
        };
    }, [applicationState, savedShelves, selectedShelfIDs, collectionQuery.includeBroken]);

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
                    setRequestError(t("Operation progress could not be refreshed."));
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
        if (metadataStarting || metadataRefreshing) {
            return;
        }
        setMetadataStarting(true);
        setRequestError(null);
        try {
            const response = await RefreshOfficialMetadata();
            if (response.error) {
                setRequestError(response.error.message);
            } else if (response.operation) {
                reconcileOperation(response.operation);
            }
        } catch {
            setRequestError(t("Could not start the Lunii sync. Please try again."));
        } finally {
            setMetadataStarting(false);
        }
    }

    async function prepareExport(
        sourceType: 'selection' | 'current_query' | 'shelf' | 'shelves',
    ) {
        const shelfIDs = sourceType === 'shelf'
            ? activeShelfID === null
                ? selectedShelfIDs.slice(0, 1)
                : [activeShelfID]
            : sourceType === 'shelves'
                ? selectedShelfIDs
                : [];
        await preflightExport(new exporter.PreflightRequest({
            sourceType,
            includeBroken: !!collectionQuery.includeBroken,
            storyIds: sourceType === 'selection' ? [...selectedIDs] : [],
            query: sourceType === 'current_query'
                ? new library.StoryLibraryQuery(collectionQuery)
                : new library.StoryLibraryQuery({}),
            shelfIds: [...shelfIDs],
        }));
    }

    async function preflightExport(request: exporter.PreflightRequest) {
        const showError = exportPreflight ? setExportStartError : setRequestError;
        setExportPreparing(true);
        setExportStartError(null);
        setRequestError(null);
        try {
            const response = await SelectAndPreflightExport(request);
            if (response.error) {
                showError(response.error.message);
            } else if (response.preflight) {
                setExportPreflightRequest(request);
                setExportPreflight(response.preflight);
            }
        } catch {
            showError(t("The export could not be prepared."));
        } finally {
            setExportPreparing(false);
        }
    }

    async function startPreparedExport() {
        if (!exportPreflight?.preparationId || !exportPreflight.canExport) {
            return;
        }
        setExportStarting(true);
        setExportStartError(null);
        try {
            const response = await StartPreparedExport(exportPreflight.preparationId);
            if (response.error) {
                setExportStartError(response.error.message);
            } else if (response.operation) {
                reconcileOperation(response.operation);
                setExportPreflight(null);
            }
        } catch {
            setExportStartError(t("The prepared export could not be started."));
        } finally {
            setExportStarting(false);
        }
    }

    async function revealExportDestination(operationID: string) {
        setRequestError(null);
        try {
            const response = await RevealExportDestination(operationID);
            const message = response.error?.message ?? null;
            setRequestError(message);
            return message;
        } catch {
            const message = t("The export destination could not be revealed.");
            setRequestError(message);
            return message;
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
        if (!selected || removing) {
            return;
        }
        setRemoving(true);
        setRemovalError(null);
        const title = selected.title;
        try {
            const response = await RemoveStory(selected.id);
            if (response.error) {
                setRemovalError(response.error.message);
                return;
            }
            setDetailsOpen(false);
            setDetail(null);
            setSelectedID(null);
            setRemovalNotice(t("{0} was moved to application trash.", title));
            await invalidateCollection();
        } catch {
            setRemovalError(t("The story could not be moved to trash. Please try again."));
        } finally {
            setRemoving(false);
        }
    }

    async function loadAllStories() {
        const generation = ++collectionRequestGeneration.current;
        setExpandingCollection(true);
        setRequestError(null);
        try {
            const first = await QueryStories(new library.StoryLibraryQuery({
                ...collectionQuery,
                page: 1,
                pageSize: collectionExpansionBatchSize,
            }));
            if (generation !== collectionRequestGeneration.current) {
                return;
            }
            if (!first.page) {
                setRequestError(first.error?.message ?? t("The full collection could not be loaded."));
                return;
            }
            const byID = new Map(first.page.stories.map((story) => [story.id, story]));
            const publishBatch = () => {
                setPage(new library.Page({
                    ...first.page,
                    stories: [...byID.values()],
                    page: 1,
                    pageSize: byID.size,
                    totalPages: 1,
                }));
            };
            publishBatch();
            for (let pageNumber = 2; pageNumber <= first.page.totalPages; pageNumber += 1) {
                const next = await QueryStories(new library.StoryLibraryQuery({
                    ...collectionQuery,
                    page: pageNumber,
                    pageSize: collectionExpansionBatchSize,
                }));
                if (generation !== collectionRequestGeneration.current) {
                    return;
                }
                if (!next.page) {
                    setRequestError(next.error?.message ?? t("The full collection could not be loaded."));
                    return;
                }
                for (const story of next.page.stories) {
                    byID.set(story.id, story);
                }
                publishBatch();
                await yieldForRendering();
            }
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
    const rows = useMemo(() => libraryView === 'large'
        ? chunkStories(stories)
        : stories.length ? [stories] : [], [libraryView, stories]);
    const activeShelf = savedShelves.find((shelf) => shelf.id === activeShelfID) ?? null;
    const allStoriesActive = activeShelfID === null &&
        collectionScope === 'all';
    const previewableShelves = useMemo(() => savedShelves.filter(
        (shelf) => shelf.validity === 'valid',
    ), [savedShelves]);
    const showSavedShelfRows = allStoriesActive && previewableShelves.length > 0;
    const repairShelf = savedShelves.find((shelf) => shelf.id === repairShelfID) ?? null;
    const deleteShelf = savedShelves.find((shelf) => shelf.id === deleteShelfID) ?? null;
    const activeOperations = operationSnapshots.filter((snapshot) => (
        operationIsActive(snapshot)
    ));
    const latestTerminalExport = operationSnapshots.find((snapshot) => (
        snapshot.kind === 'export' && operationIsTerminal(snapshot)
    ));
    const primaryVisibleOperations = activeOperations.length > 0
        ? activeOperations
        : operationSnapshots.slice(0, 1);
    const visibleOperations = (latestTerminalExport &&
        !primaryVisibleOperations.some(
            (snapshot) => snapshot.id === latestTerminalExport.id,
        )
        ? [...primaryVisibleOperations, latestTerminalExport]
        : primaryVisibleOperations).filter((snapshot) => !dismissedOperations.has(snapshot.id));
    const importing = activeOperations.some((snapshot) => snapshot.kind === 'import');
    const metadataRefreshing = activeOperations.some(
        (snapshot) => snapshot.kind === 'metadata_sync',
    );
    const derivedDefinitions = tagCatalog?.definitions.filter(
        (definition) => definition.source === 'derived',
    ) ?? [];
    const editableDefinitions = tagCatalog?.definitions.filter(
        (definition) => definition.source !== 'derived' && definition.key !== 'broken',
    ) ?? [];
    useEffect(() => {
        const values = tagCatalog?.definitions.flatMap((definition) => (
            definition.source === 'derived' && definition.kind === 'choice'
                ? definition.values.map((value) => ({
                    definitionID: definition.id,
                    valueID: value.id,
                }))
                : []
        )) ?? [];
        if (applicationState !== 'ready' || values.length === 0) {
            return;
        }
        let active = true;
        void Promise.all(values.map(async ({definitionID, valueID}) => {
            const response = await QueryStories(new library.StoryLibraryQuery({
                ...initialCollectionQuery,
                choiceFilters: [{
                    definitionId: definitionID,
                    valueIds: [valueID],
                }],
                page: 1,
                pageSize: 1,
            }));
            return [valueID, response.page?.totalItems] as const;
        })).then((counts) => {
            if (!active) {
                return;
            }
            setFacetCounts(Object.fromEntries(counts.flatMap(
                ([valueID, count]) => count === undefined
                    ? []
                    : [[valueID, count]],
            )));
        }).catch(() => {
            if (active) {
                setFacetCounts({});
            }
        });
        return () => {
            active = false;
        };
    }, [applicationState, collectionRevision, tagCatalog]);
    const languageOptions = [...new Set([
        ...(metadataStatus?.locale ? [metadataStatus.locale] : []),
        ...collectionQuery.languages,
    ])].sort();
    const collectionRows = useMemo<CollectionShelfRow[]>(() => (
        [
            ...rows.map((row, rowIndex) => ({
                key: `collection-${rowIndex}`,
                name: rowIndex === 0 ? t("Recently added") : t("More stories"),
                storyCount: row.length,
                source: t("Local archive"),
                stories: row,
            })),
            ...(showSavedShelfRows ? previewableShelves.map((shelf) => ({
                key: `saved-shelf-${shelf.id}`,
                name: savedShelfPreviewDetails[shelf.id]?.name ?? shelf.name,
                storyCount: savedShelfPreviewDetails[shelf.id]?.count ?? shelf.count,
                source: savedShelfPreviewDetails[shelf.id]?.source ?? t("Saved shelf"),
                stories: savedShelfStories[shelf.id] ?? [],
                shelfID: shelf.id,
                error: savedShelfPreviewErrors[shelf.id],
            })) : []),
        ]
    ), [
        previewableShelves,
        rows,
        savedShelfPreviewDetails,
        savedShelfPreviewErrors,
        savedShelfStories,
        showSavedShelfRows,
    ]);
    const renderedStories = useMemo(
        () => collectionRows.flatMap((row) => row.stories),
        [collectionRows],
    );
    const selectedSummary = renderedStories.find(
        (story) => story.id === selectedID,
    ) ?? null;
    const selected = selectedSummary
        ? detail?.story.id === selectedID ? detail.story : selectedSummary
        : null;
    const selectedPalette = selected ? paletteFor(selected.id) : undefined;
    const selectedShelfCount = selected
        ? savedShelves.find((shelf) => (
            savedShelfStories[shelf.id]?.some((story) => story.id === selected.id)
        ))?.count
        : undefined;
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
                derived: definition.source === 'derived',
                label: state.assignedStories === assignmentWorkspace.requestedStories
                    ? definition.label
                    : `${definition.label} · Mixed`,
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
                derived: definition.source === 'derived',
                label: valueState.assignedStories === assignmentWorkspace.requestedStories
                    ? `${definition.label} · ${value.label}`
                    : `${definition.label} · ${value.label} · Mixed`,
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
                        includeBroken: !!collectionQuery.includeBroken,
                        page: 1,
                        pageSize: 6,
                        sort: collectionQuery.sort,
                    }),
                );
                if (!response.evaluation) {
                    return {
                        shelfID: shelf.id,
                        stories: [] as library.StorySummary[],
                        details: undefined,
                        error: response.error?.message ??
                            t("{0} could not be previewed.", shelf.name),
                    };
                }
                return {
                    shelfID: shelf.id,
                    stories: response.evaluation.page.stories,
                    details: {
                        name: response.evaluation.shelf.name,
                        count: response.evaluation.page.totalItems,
                        source: savedShelfSource(
                            response.evaluation.query,
                            [tagCatalog, assignmentWorkspace?.catalog],
                        ),
                    },
                    error: '',
                };
            } catch {
                return {
                    shelfID: shelf.id,
                    stories: [] as library.StorySummary[],
                    details: undefined,
                    error: t("{0} could not be previewed.", shelf.name),
                };
            }
        })).then((previews) => {
            if (!active || generation !== shelfPreviewGeneration.current) {
                return;
            }
            setSavedShelfStories(Object.fromEntries(previews.map((preview) => (
                [preview.shelfID, preview.stories]
            ))));
            setSavedShelfPreviewDetails(Object.fromEntries(previews.flatMap(
                (preview) => preview.details
                    ? [[preview.shelfID, preview.details]]
                    : [],
            )));
            setSavedShelfPreviewErrors(Object.fromEntries(previews.flatMap(
                (preview) => preview.error
                    ? [[preview.shelfID, preview.error]]
                    : [],
            )));
        });
        return () => {
            active = false;
        };
    }, [
        applicationState,
        assignmentWorkspace?.catalog,
        collectionQuery.sort,
        collectionQuery.includeBroken,
        previewableShelves,
        showSavedShelfRows,
        tagCatalog,
    ]);

    const updateQuery = useCallback((
        next: CollectionQuery,
        shelfID?: number | null,
    ) => {
        setCollectionQuery(queryHistory.push(next, shelfID));
        setCollectionScope(queryHistory.currentScope());
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
                    includeBroken: !!collectionQuery.includeBroken,
                    page: 1,
                    pageSize: collectionQuery.pageSize,
                    sort: collectionQuery.sort,
                }),
            );
            if (!response.evaluation) {
                setRequestError(
                    response.error?.message ?? t("The saved shelf could not be opened."),
                );
                return;
            }
            activateShelf(shelfID, 'shelf');
            setPage(response.evaluation.page);
            updateQuery(collectionQueryFromShelf(
                response.evaluation.query,
                collectionQuery,
            ), shelfID);
        } catch {
            setRequestError(t("The saved shelf could not be opened."));
        } finally {
            setShelfBusy(false);
        }
    }

    function openAllStories() {
        activateShelf(null, 'all');
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
                    response?.error?.message ?? t("The saved shelf could not be changed."),
                );
                return;
            }
            const changedShelf = response.shelf;
            setShelfDialogMode(null);
            setShelfDialogName('');
            await loadSavedShelves();
            if (shelfDialogMode === 'save') {
                queryHistory.replace(collectionQuery, changedShelf.id);
                activateShelf(changedShelf.id, 'shelf');
            } else if (shelfDialogMode === 'duplicate') {
                await openSavedShelf(changedShelf.id);
            }
        } catch {
            setShelfDialogError(t("The saved shelf could not be changed."));
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
                    response.error?.message ?? t("The saved shelf could not be updated."),
                );
                return;
            }
            await loadSavedShelves();
        } catch {
            setRequestError(t("The saved shelf could not be updated."));
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
                // Reordering does not change membership; preserve current visibility counts.
                const counts = new Map(savedShelves.map((shelf) => [shelf.id, shelf.count]));
                setSavedShelves((response.shelves ?? []).map((shelf) => new shelves.Summary({
                    ...shelf, count: counts.get(shelf.id) ?? shelf.count,
                })));
            }
        } catch {
            setRequestError(t("Saved shelves could not be reordered."));
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
                    response.error?.message ?? t("The saved shelf could not be deleted."),
                );
                return;
            }
            setDeleteShelfID(null);
            if (activeShelfID === deleteShelf.id) {
                queryHistory.replace(collectionQuery, null);
                activateShelf(null, 'custom');
            }
            if (repairShelfID === deleteShelf.id) {
                setRepairShelfID(null);
            }
            setSelectedShelfIDs((current) => current.filter(
                (shelfID) => shelfID !== deleteShelf.id,
            ));
            await loadSavedShelves();
        } catch {
            setShelfDialogError(t("The saved shelf could not be deleted."));
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
                    response.error?.message ?? t("The saved shelf could not be repaired."),
                );
                return;
            }
            setRepairShelfID(null);
            queryHistory.replace(collectionQuery, repairShelf.id);
            activateShelf(repairShelf.id, 'shelf');
            await loadSavedShelves();
        } catch {
            setShelfDialogError(t("The saved shelf could not be repaired."));
        } finally {
            setShelfBusy(false);
        }
    }

    const reconcileTagCatalog = useCallback((catalog: tagging.Catalog) => {
        setTagCatalog(catalog);
        setAssignmentWorkspace((current) => current
            ? new tagging.AssignmentWorkspace({...current, catalog})
            : current);
        void invalidateCollection();
    }, [invalidateCollection]);

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
                label: t("Name contains “{0}”", collectionQuery.name),
                remove: () => updateQuery({...collectionQuery, name: '', page: 1}),
            }]
            : []),
        ...collectionQuery.languages.map((locale) => ({
            key: `language-${locale}`,
            label: t("Language · {0}", languageLabel(locale)),
            remove: () => toggleLanguage(locale),
        })),
        ...collectionQuery.compatibilities.map((value) => ({
            key: `compatibility-${value}`,
            label: t("Import status · {0}", compatibilityOptions.find((option) => option.value === value)?.label ?? value),
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
                        : t("Not {0}", definition.label),
                    remove: () => setBooleanFilter(filter.definitionId, 'ignored'),
                }]
                : [{
                    key: `unavailable-boolean-${filter.definitionId}`,
                    label: t("Unavailable saved criterion · definition {0}", filter.definitionId),
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
                    label: t("Unavailable saved criterion · definition {0}", filter.definitionId),
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
                        label: t("Unavailable {0} value · {1}", definition.label, valueID),
                        remove: () => toggleChoiceFilter(definition.id, valueID),
                    }];
            });
        }),
    ];

    const renderTagFacet = (definition: tagging.DefinitionWithValues) => (
        <div className="facet" key={definition.id}>
            <button
                className="facet-title"
                type="button"
                aria-expanded={!collapsedTagFacets.has(definition.id)}
                aria-controls={`tag-facet-${definition.id}`}
                onClick={() => setCollapsedTagFacets((current) => {
                    const next = new Set(current);
                    if (next.has(definition.id)) next.delete(definition.id);
                    else next.add(definition.id);
                    return next;
                })}
            >
                {definition.label} <b aria-hidden="true">{collapsedTagFacets.has(definition.id) ? '+' : '−'}</b>
            </button>
            <div id={`tag-facet-${definition.id}`} hidden={collapsedTagFacets.has(definition.id)}>
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
                            {t("Not")}{definition.label}
                        </label>
                    </>
                ) : definition.values.map((value) => (
                    <label className="choice" key={value.id}>
                        {(() => {
                            const checkedByQuery = collectionQuery.choiceFilters.some(
                                (filter) => (
                                    filter.definitionId === definition.id &&
                                    filter.valueIds.includes(value.id)
                                ),
                            );
                            return (
                                <input
                                    type="checkbox"
                                    checked={checkedByQuery}
                                    onChange={() => toggleChoiceFilter(
                                        definition.id,
                                        value.id,
                                    )}
                                />
                            );
                        })()}
                        <i style={{'--c': definition.color} as CSSProperties}/>
                        {value.label}
                        {facetCounts[value.id] !== undefined && (
                            <span aria-hidden="true">{facetCounts[value.id]}</span>
                        )}
                    </label>
                ))}
            </div>
        </div>
    );

    return (
        <div className={`app view-${libraryView}`}>
            <span className="sr-only" data-testid="application-state">
                {t("Application state:")}{applicationState}
            </span>
            <aside className="rail" aria-label={t("Primary navigation")} inert={sidebarOpen || historyOpen}>
                <div className="mark" role="img" aria-label="Librairii">
                    <i/><i/><i/>
                </div>
                <LibraryNavigation
                    libraryActive={allStoriesActive}
                    disabled={applicationState !== 'ready' || shelfBusy}
                    importing={importing}
                    exporting={exportPreparing}
                    onLibrary={openAllStories}
                    onTags={() => setTagManagerOpen(true)}
                    onImport={() => void startImport()}
                    onExport={() => void prepareExport('current_query')}
                />
            </aside>

            <LibrarySidebar open={sidebarOpen} onClose={closeSidebar} inert={historyOpen}
                suspended={tagManagerOpen || tagAssignmentOpen || shelfDialogMode !== null ||
                    repairShelfID !== null || deleteShelfID !== null || exportPreflight !== null || historyOpen}>
                <h1>Librairii</h1>
                <div className="path">~/Stories/Librairii</div>
                <div className="caption">{t("Saved shelves")}</div>
                <nav className="saved">
                    <button
                        className={`saved-picker${allStoriesActive ? ' active' : ''}`}
                        type="button"
                        aria-current={allStoriesActive ? 'page' : undefined}
                        disabled={shelfBusy}
                        onClick={openAllStories}
                    >
                        <i style={{'--c': '#405cf5'} as CSSProperties}/>
                        {t("All stories")}<span>{allStoriesActive ? page?.totalItems ?? 0 : t("All")}</span>
                    </button>
                    {savedShelves.map((shelf, index) => (
                        <div className="saved-entry" key={shelf.id}>
                            <div className="saved-entry-row">
                                <input
                                    className="shelf-selector"
                                    type="checkbox"
                                    aria-label={t("Select {0} for combined shelf preview", shelf.name)}
                                    checked={selectedShelfIDs.includes(shelf.id)}
                                    disabled={
                                        shelfBusy ||
                                        (
                                            shelf.validity === 'needs_attention' &&
                                            !selectedShelfIDs.includes(shelf.id)
                                        )
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
                                        ? t("{0}, needs attention", shelf.name)
                                        : `${shelf.name}, ${shelf.count} ${
                                            shelf.count === 1 ? t("story") : t("stories")
                                        }`}
                                    disabled={shelfBusy}
                                    onClick={() => shelf.validity === 'needs_attention'
                                        ? setRepairShelfID(shelf.id)
                                        : void openSavedShelf(shelf.id)}
                                >
                                    <i
                                        style={{
                                            '--c': savedShelfColor(index),
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
                                <div className="saved-tools" aria-label={t("{0} actions", shelf.name)}>
                                    <button
                                        type="button"
                                        disabled={shelfBusy || index === 0}
                                        aria-label={t("Move {0} up", shelf.name)}
                                        onClick={() => void moveShelf(shelf.id, -1)}
                                    >
                                        ↑
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy || index === savedShelves.length - 1}
                                        aria-label={t("Move {0} down", shelf.name)}
                                        onClick={() => void moveShelf(shelf.id, 1)}
                                    >
                                        ↓
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy}
                                        aria-label={t("Rename {0}", shelf.name)}
                                        onClick={() => showShelfDialog('rename')}
                                    >
                                        ✎
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy}
                                        aria-label={t("Duplicate {0}", shelf.name)}
                                        onClick={() => showShelfDialog('duplicate')}
                                    >
                                        ⧉
                                    </button>
                                    <button
                                        type="button"
                                        disabled={shelfBusy}
                                        aria-label={t("Delete {0}", shelf.name)}
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
                <label className="include-broken">
                    <input
                        type="checkbox"
                        checked={!!collectionQuery.includeBroken}
                        onChange={(event) => {
                            const next = {...collectionQuery, includeBroken: event.currentTarget.checked, page: 1};
                            if (allStoriesActive) {
                                setCollectionQuery(queryHistory.push(next, null, 'all'));
                            } else {
                                updateQuery(next);
                            }
                        }}
                    />
                    {t("Include broken stories")}</label>
                {selectedShelfIDs.length > 0 && (
                    <section className="combined-shelves" aria-label={t("Combined shelf preview")}>
                        <div className="combined-shelves-title">
                            <b>{t("Combined preview")}</b>
                            <span>{selectedShelfIDs.length} {' ' + t("selected")}</span>
                        </div>
                        {shelfSelectionBusy && <p>{t("Resolving current membership…")}</p>}
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
                                    {t("unique")}{shelfSelectionPreview.uniqueStoryCount === 1
                                        ? t("story")
                                        : t("stories")}
                                </strong>
                                <p>
                                    {shelfSelectionPreview.overlapCount === 0
                                        ? t("No overlapping memberships.")
                                        : t("{0} overlapping {1} collapsed.", shelfSelectionPreview.overlapCount, shelfSelectionPreview.overlapCount === 1
                                                ? 'membership'
                                                : 'memberships')}
                                </p>
                                <small>
                                    {t("Sources:")}{shelfSelectionPreview.sourceShelfNames.join(', ')}
                                </small>
                                <button
                                    className="combined-export"
                                    type="button"
                                    disabled={
                                        exportPreparing ||
                                        shelfSelectionPreview.uniqueStoryCount === 0
                                    }
                                    onClick={() => void prepareExport(
                                        selectedShelfIDs.length === 1
                                            ? 'shelf'
                                            : 'shelves',
                                    )}
                                >
                                    {exportPreparing
                                        ? t("Preparing…")
                                        : selectedShelfIDs.length === 1
                                            ? t("Export selected shelf")
                                            : t("Export selected shelves")}
                                </button>
                            </>
                        )}
                    </section>
                )}
                {activeFilters.length > 0 && !showSavedShelfRows && (
                    <button
                        className="manage shelf-save"
                        type="button"
                        disabled={shelfBusy}
                        onClick={() => showShelfDialog('save')}
                    >
                        {t("＋ Save current query")}</button>
                )}
                {activeShelf && (
                    <button
                        className="manage shelf-update"
                        type="button"
                        disabled={shelfBusy}
                        onClick={() => void updateActiveShelf()}
                    >
                        {t("↻ Update “")}{activeShelf.name}”
                    </button>
                )}
                <div className="caption">{t("Refine this shelf")}</div>
                {derivedDefinitions.map(renderTagFacet)}
                <div className="facet">
                    <button
                        className="facet-title"
                        type="button"
                        aria-expanded={languageFacetOpen}
                        onClick={() => setLanguageFacetOpen((open) => !open)}
                    >
                        {t("Language")}<b>{languageFacetOpen ? '−' : '＋'}</b>
                    </button>
                    {languageFacetOpen && languageOptions.map((locale) => (
                            <label className="choice" key={locale}>
                                <input
                                    type="checkbox"
                                    checked={collectionQuery.languages.includes(locale)}
                                    onChange={() => toggleLanguage(locale)}
                                />
                                <i style={{'--c': '#6779e8'} as CSSProperties}/>
                                {t(languageLabel(locale))}
                            </label>
                        ))}
                </div>
                <div className="facet">
                    <button
                        className="facet-title"
                        type="button"
                        aria-expanded={importFacetOpen}
                        onClick={() => setImportFacetOpen((open) => !open)}
                    >
                        {t("Import status")}<b>{importFacetOpen ? '−' : '＋'}</b>
                    </button>
                    {importFacetOpen && compatibilityOptions.map((option) => (
                            <label className="choice" key={option.value}>
                                <input
                                    type="checkbox"
                                    checked={collectionQuery.compatibilities.includes(option.value)}
                                    onChange={() => toggleCompatibility(option.value)}
                                />
                                <i style={{'--c': '#f5bc41'} as CSSProperties}/>
                                {t(option.label)}
                            </label>
                        ))}
                </div>
                {editableDefinitions.map(renderTagFacet)}
                <button
                    className="manage"
                    type="button"
                    onClick={() => setTagManagerOpen(true)}
                >
                    {t("＋ Manage tags")}</button>
            </LibrarySidebar>

            <main className="main" inert={sidebarOpen || historyOpen}>
                <div className="top">
                    <label className="search">
                        <span aria-hidden="true">⌕</span>
                        <input
                            ref={searchInput}
                            type="search"
                            aria-label={t("Search stories")}
                            placeholder={t("Search {0} stories, authors, publishers…", page?.totalItems ?? 0)}
                            value={collectionQuery.name}
                            onChange={(event) => updateQuery({
                                ...collectionQuery,
                                name: event.currentTarget.value,
                                page: 1,
                            })}
                        />
                    </label>
                </div>

                <div className="heading">
                    <div>
                        <h2>{activeShelf?.name ?? t("My story shelves")}</h2>
                        <p>
                            {activeShelf
                                ? t("Dynamic saved shelf")
                                : t("Stories grouped by your tags")}
                            {' · '}
                            {page?.totalItems ?? 0} {t("archives")}</p>
                    </div>
                    <div className="heading-actions">
                        {activeFilters.length > 0 && !showSavedShelfRows && (
                            <button
                                className="scope-export"
                                type="button"
                                disabled={exportPreparing}
                                onClick={() => void prepareExport('current_query')}
                            >
                                {t("Export current result")}</button>
                        )}
                        {activeShelf && (
                            <button
                                className="scope-export"
                                type="button"
                                disabled={exportPreparing}
                                onClick={() => void prepareExport('shelf')}
                            >
                                {t("Export shelf")}</button>
                        )}
                        <label className="sort">
                            <span className="sr-only">{t("Sort stories")}</span>
                            <select
                                value={collectionQuery.sort}
                                onChange={(event) => updateQuery({
                                    ...collectionQuery,
                                    sort: event.currentTarget.value as CollectionQuery['sort'],
                                    page: 1,
                                })}
                            >
                                <option value="imported_desc">{t("Recently added")}</option>
                                <option value="name_asc">{t("Name A–Z")}</option>
                            </select>
                        </label>
                    </div>
                </div>

                <div className="collection-toolbar">
                    <button className="sidebar-toggle" type="button"
                        aria-expanded={sidebarOpen} aria-controls="library-sidebar"
                        onClick={() => setSidebarOpen(true)}>
                        {t("Shelves &amp; filters")}</button>
                    <LibraryViewPicker view={libraryView} onChange={setLibraryView}/>
                    <button type="button" className="history-disclosure"
                        disabled={applicationState !== 'ready'} onClick={() => setHistoryOpen(true)}>
                        {t("History")}</button>
                    {luniiSyncDismissed && (
                        <button ref={luniiSyncToggle} type="button" className="sync-disclosure"
                            onClick={() => {
                                setLuniiSyncDismissed(false);
                                searchInput.current?.focus();
                            }}>
                            {t("Show Lunii sync")}</button>
                    )}
                </div>

                {activeFilters.length > 0 && !showSavedShelfRows && (
                    <section className="active-query" aria-label={t("Active filters")}>
                        <span>{page?.totalItems ?? 0} {' ' + t("matching stories")}</span>
                        {activeFilters.map((filter) => (
                            <button
                                className="query-chip"
                                type="button"
                                key={filter.key}
                                aria-label={t("Remove filter {0}", filter.label)}
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
                            {t("Clear all")}</button>
                    </section>
                )}

                {applicationState === 'ready' && !luniiSyncDismissed && (
                    <LuniiSync
                        status={metadataStatus}
                        busy={metadataStarting || metadataRefreshing}
                        onSync={() => void startMetadataRefresh()}
                        onDismiss={() => setLuniiSyncDismissed(true)}
                    />
                )}

                {applicationState === 'initializing' && (
                    <section
                        className="collection-state working"
                        data-state="loading-library"
                        aria-live="polite"
                    >
                        <div className="state-mark" aria-hidden="true">↻</div>
                        <div className="state-copy">
                            <h3>{t("Opening your local library")}</h3>
                            <p>
                                {t("Checking managed storage, SQLite, and interrupted operations…")}</p>
                        </div>
                    </section>
                )}

                {applicationState === 'recovery' && (
                    <section
                        className="collection-state error"
                        data-state="storage-recovery"
                        role="alert"
                    >
                        <div className="state-mark" aria-hidden="true">!</div>
                        <div className="state-copy">
                            <h3>{t("Local library needs recovery")}</h3>
                            <p>
                                {applicationError ||
                                    'Librairii cannot safely open its application data.'}
                            </p>
                            {storageRecoveryAvailable && (
                                <p>
                                    {t("Continue only to preserve the incompatible database in a unique recovery folder and create a fresh Librairii database. Managed archives are never deleted.")}</p>
                            )}
                        </div>
                        {storageRecoveryAvailable && (
                            <button
                                type="button"
                                disabled={storageRecovering}
                                onClick={() => void recoverStorage()}
                            >
                                {storageRecovering
                                    ? t("Preserving database…")
                                    : t("Preserve database and create new library")}
                            </button>
                        )}
                    </section>
                )}

                {requestError && (
                    <section className="collection-state error" data-state="failed-action">
                        <div className="state-mark" aria-hidden="true">!</div>
                        <div>
                            <h3>{t("The action could not be completed")}</h3>
                            <p>{requestError}</p>
                        </div>
                        <button type="button" className="notice-dismiss" aria-label={t("Close action error")}
                            onClick={() => {
                                setRequestError(null);
                                searchInput.current?.focus();
                            }}>
                            <span aria-hidden="true">×</span>
                        </button>
                    </section>
                )}

                {removalNotice && (
                    <section className="collection-state success" data-state="removal-success">
                        <div className="state-mark" aria-hidden="true">✓</div>
                        <div>
                            <h3>{t("Story removed from this collection")}</h3>
                            <p>{removalNotice}</p>
                        </div>
                        <button type="button" className="notice-dismiss" aria-label={t("Close removal notice")}
                            onClick={() => {
                                setRemovalNotice(null);
                                searchInput.current?.focus();
                            }}>
                            <span aria-hidden="true">×</span>
                        </button>
                    </section>
                )}

                {visibleOperations.map((operation) => {
                    const notice = operation.kind === 'metadata_sync'
                        ? describeMetadataRefresh(operation, metadataStatus)
                        : operation.kind === 'export'
                            ? describeExport(operation)
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
                                        ? operation.kind === 'metadata_sync'
                                            ? '↻'
                                            : operation.kind === 'export' ? '↗' : '↓'
                                        : '!'}
                            </div>
                            <div className="state-copy">
                                <h3>{notice.title}</h3>
                                <p>{notice.message}</p>
                                {operation.kind === 'export' &&
                                    !operationActive && (
                                    <ExportReport
                                        operation={operation}
                                        onReveal={(operationID) => {
                                            void revealExportDestination(operationID);
                                        }}
                                    />
                                )}
                                {operation.kind === 'export' &&
                                    operationActive &&
                                    operation.items.length > 0 && (
                                    <ul
                                        className="operation-items"
                                        aria-label={t("Export story progress")}
                                    >
                                        {operation.items.map((item) => (
                                            <li key={item.id}>
                                                <b>
                                                    {item.storyTitle ||
                                                        item.outputName ||
                                                        item.sourceName}
                                                </b>
                                                <span>
                                                    {formatBytes(item.completedBytes)}
                                                    {' of '}
                                                    {formatBytes(item.totalBytes)}
                                                    {' · '}
                                                    {item.status}
                                                </span>
                                            </li>
                                        ))}
                                    </ul>
                                )}
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
                            {!operationActive && (
                                <button type="button" className="notice-dismiss"
                                    aria-label={t("Close {0}", notice.title)}
                                    onClick={() => {
                                        dismissOperation(operation.id);
                                        searchInput.current?.focus();
                                    }}>
                                    <span aria-hidden="true">×</span>
                                </button>
                            )}
                            {operationActive && (
                                <button
                                    type="button"
                                    onClick={() => void cancelOperation(operation)}
                                >
                                    {operation.kind === 'metadata_sync'
                                        ? t("Cancel refresh")
                                        : operation.kind === 'export'
                                            ? t("Cancel export")
                                            : t("Cancel import")}
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
                                ? t("{0} is currently empty", activeShelf.name)
                                : t("Build your local story archive")}
                        </h3>
                        <p>
                            {activeShelf
                                ? t("This saved query remains valid and will fill automatically when stories start matching it.")
                                : t("Import story packs you already own. Librairii validates and preserves each archive in managed local storage.")}
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
                                {t("Edit shelf query")}</button>
                        )}
                        <button
                            className="import"
                            type="button"
                            onClick={() => void startImport()}
                        >
                            {t("＋ Import your first stories")}</button>
                    </section>
                )}

                {collectionRows.map((row) => (
                    <section className="shelf" key={row.key}>
                        <div className="shelf-head">
                            <h3>{row.name}</h3>
                            <span>
                                {row.storyCount} {
                                    row.storyCount === 1 ? t("story") : t("stories")
                                } · {row.source}
                            </span>
                            <button
                                type="button"
                                aria-label={row.shelfID === undefined
                                    ? undefined
                                    : t("View all stories in {0}", row.name)}
                                disabled={row.shelfID === undefined
                                    ? expandingCollection ||
                                        stories.length >= (page?.totalItems ?? 0)
                                    : shelfBusy}
                                onClick={() => row.shelfID === undefined
                                    ? void loadAllStories()
                                    : void openSavedShelf(row.shelfID)}
                            >
                                {row.shelfID !== undefined
                                    ? t("View all →")
                                    : stories.length >= (page?.totalItems ?? 0)
                                    ? t("All shown")
                                    : expandingCollection
                                        ? t("Loading…")
                                        : t("View all →")}
                            </button>
                        </div>
                        {row.error && (
                            <p className="dialog-error" role="alert">{row.error}</p>
                        )}
                        <div className={`story-row${row.shelfID === undefined ? '' : ' preview'}`}>
                            {row.stories.map((story) => (
                                <div className={`story-card${story.broken ? ' is-broken' : ''}`} key={story.id}>
                                    <button
                                        className={`story${selectedIDs.includes(story.id) ? ' selected' : ''}`}
                                        style={paletteFor(story.id)}
                                        type="button"
                                        aria-label={story.title}
                                        aria-describedby={story.broken ? `broken-${row.key}-${story.id}` : undefined}
                                        aria-pressed={selectedIDs.includes(story.id)}
                                        aria-keyshortcuts="Control+Enter Meta+Enter"
                                        onClick={(event) => {
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
                                        {libraryView === 'list' ? (
                                            <span className="story-list-info">
                                                <span className="story-list-title">{story.title}</span>
                                                <span className="story-list-meta">
                                                    {story.author || t("Local story")}
                                                </span>
                                            </span>
                                        ) : <StoryCover story={story}/>}
                                        {story.broken && (
                                            <span
                                                className="broken-badge"
                                                id={`broken-${row.key}-${story.id}`}
                                                style={{'--broken-color': tagCatalog?.definitions.find(
                                                    (definition) => definition.key === 'broken',
                                                )?.color ?? '#FF705C'} as CSSProperties}
                                            >
                                                <i aria-hidden="true"/>{t("Broken")}</span>
                                        )}
                                    </button>
                                    {libraryView !== 'list' && <p className="story-title">{story.title}</p>}
                                </div>
                            ))}
                        </div>
                    </section>
                ))}
                {(page?.totalPages ?? 0) > 1 && (
                    <nav className="pagination" aria-label={t("Collection pages")}>
                        <button
                            type="button"
                            disabled={collectionQuery.page <= 1}
                            onClick={() => updateQuery({
                                ...collectionQuery,
                                page: collectionQuery.page - 1,
                            })}
                        >
                            {t("← Previous")}</button>
                        <span>
                            {t("Page")}{page?.page ?? collectionQuery.page} {' ' + t("of") + ' '}{page?.totalPages}
                        </span>
                        <button
                            type="button"
                            disabled={collectionQuery.page >= (page?.totalPages ?? 1)}
                            onClick={() => updateQuery({
                                ...collectionQuery,
                                page: collectionQuery.page + 1,
                            })}
                        >
                            {t("Next →")}</button>
                    </nav>
                )}
            </main>

            {selected && (
                <aside className="drawer" aria-label={t("{0} details", selected.title)} inert={sidebarOpen || historyOpen}>
                    <div
                        className={`drawer-art${selected.artworkId ? ' has-artwork' : ''}`}
                        style={selectedPalette}
                    >
                        {selected.artworkId && (
                            <img
                                src={artworkURL(selected.artworkId)}
                                alt={t("{0} artwork", selected.title)}
                                decoding="async"
                            />
                        )}
                    </div>
                    <div className="drawer-info">
                        <h3>{selected.title}</h3>
                        <p>{metadataAttribution(selected)}</p>
                        <div className="facts">
                            <div className="fact">
                                <span>{t("Stories")}</span>
                                <b>{selectedShelfCount ?? '—'}</b>
                            </div>
                            <div className="fact">
                                <span>{t("Duration")}</span>
                                <b>{formatDuration(selected.official?.durationSeconds)}</b>
                            </div>
                            <div className="fact">
                                <span>{t("Language")}</span>
                                <b>
                                    {selected.official?.language
                                        ? languageLabel(selected.official.language)
                                        : '—'}
                                </b>
                            </div>
                        </div>
                    </div>
                    <div className="drawer-tags">
                        <div className="tag-title">
                            {selectedIDs.length > 1
                                ? t("{0} stories selected", selectedIDs.length)
                                : t("My tags")}
                        </div>
                        <div className="tags">
                            {assignedTags.map((tag) => (
                                <span
                                    className="tag"
                                    key={tag.key}
                                    aria-label={tag.derived
                                        ? t("System-derived tag: {0}", tag.label)
                                        : undefined}
                                >
                                    <i
                                        style={{'--c': tag.color} as CSSProperties}
                                        aria-hidden="true"
                                    />
                                    {tag.label}
                                </span>
                            ))}
                        </div>
                    </div>
                    <div className="drawer-actions">
                        <button
                            ref={openDetailsButton}
                            type="button"
                            onClick={() => {
                                setRemovalError(null);
                                setDetailsOpen(true);
                            }}
                        >
                            {t("Open details")}</button>
                        {selectedIDs.length > 1 && (
                            <button
                                className="export"
                                type="button"
                                disabled={exportPreparing}
                                onClick={() => void prepareExport('selection')}
                            >
                                {exportPreparing
                                    ? t("Preparing…")
                                    : t("Export {0} selected stories →", selectedIDs.length)}
                            </button>
                        )}
                    </div>
                </aside>
            )}

            {exportPreflight && (
                <ModalBackdrop onDismiss={() => setExportPreflight(null)} disabled={exportStarting || exportPreparing}>
                    <section
                        ref={exportPreflightDialog}
                        className="detail-dialog export-preflight"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="export-preflight-title"
                        data-state={exportPreflight.blocked
                            ? 'export-preflight-blocked'
                            : exportPreflight.partial
                                ? 'export-preflight-partial'
                                : 'export-preflight-ready'}
                    >
                        <div className="dialog-kicker">{t("Export preflight")}</div>
                        <h3 id="export-preflight-title">
                            {exportPreflight.blocked
                                ? t("Export is blocked")
                                : exportPreflight.partial
                                    ? t("Review partial export")
                                    : t("Ready to export")}
                        </h3>
                        <p className="dialog-description">
                            {exportPreflight.blocked
                                ? t("Resolve the items below before copying can begin.")
                                : t("{0} of {1} resolved {2} ready for byte-preserving export.", exportPreflight.readyCount, exportPreflight.resolvedCount, exportPreflight.resolvedCount === 1
                                        ? 'story is'
                                        : 'stories are')}
                        </p>
                        <dl className="dialog-facts export-preflight-facts">
                            <div>
                                <dt>{t("Destination")}</dt>
                                <dd>{exportPreflight.destination || t("Not available")}</dd>
                            </div>
                            <div>
                                <dt>{t("Total size")}</dt>
                                <dd>{formatBytes(exportPreflight.totalBytes)}</dd>
                            </div>
                            <div>
                                <dt>{t("Formats")}</dt>
                                <dd>
                                    {(exportPreflight.detectedFormats ?? []).join(', ') || '—'}
                                </dd>
                            </div>
                            <div>
                                <dt>{t("Overlap collapsed")}</dt>
                                <dd>{exportPreflight.collapsedOverlap}</dd>
                            </div>
                        </dl>
                        {(exportPreflight.issues ?? []).length > 0 && (
                            <ul className="preflight-issues" aria-label={t("Export blockers")}>
                                {exportPreflight.issues.map((issue) => (
                                    <li key={issue.code}>
                                        <b>{issue.blocks ? t("Blocked") : t("Notice")}</b>
                                        <span>{issue.message}</span>
                                    </li>
                                ))}
                            </ul>
                        )}
                        {(exportPreflight.items ?? []).length > 0 && (
                            <ul className="preflight-items" aria-label={t("Export story checks")}>
                                {exportPreflight.items.map((item) => (
                                    <li
                                        className={item.disposition}
                                        key={item.storyId}
                                    >
                                        <span>
                                            <b>{item.storyTitle}</b>
                                            <small>{item.outputName}</small>
                                        </span>
                                        <strong>{item.disposition}</strong>
                                        {item.issue && <p>{item.issue.message}</p>}
                                    </li>
                                ))}
                            </ul>
                        )}
                        {exportStartError && (
                            <p className="dialog-error" role="alert">
                                {exportStartError}
                            </p>
                        )}
                        <div className="dialog-actions">
                            <button
                                ref={exportPreflightInitialFocus}
                                type="button"
                                disabled={exportStarting || exportPreparing}
                                onClick={() => {
                                    setExportPreflight(null);
                                    setExportStartError(null);
                                }}
                            >
                                {t("Close")}</button>
                            {exportPreflightRequest && (
                                <button
                                    type="button"
                                    disabled={exportStarting || exportPreparing}
                                    onClick={() => void preflightExport(exportPreflightRequest)}
                                >
                                    {exportPreparing ? t("Preparing…") : t("Choose another folder")}
                                </button>
                            )}
                            <button
                                className="export"
                                type="button"
                                disabled={
                                    exportStarting ||
                                    exportPreparing ||
                                    !exportPreflight.canExport ||
                                    !exportPreflight.preparationId
                                }
                                onClick={() => void startPreparedExport()}
                            >
                                {exportStarting
                                    ? t("Starting…")
                                    : exportPreflight.canExport
                                        ? t("Start export")
                                        : t("Export unavailable")}
                            </button>
                        </div>
                    </section>
                </ModalBackdrop>
            )}

            {detailsOpen && selected && !tagAssignmentOpen && (
                <StoryDetailsDialog
                    key={selected.id}
                    story={selected}
                    archive={detail?.story.id === selected.id ? detail.archive : undefined}
                    artworkURL={artworkURL(selected.artworkId)}
                    coverStyle={selectedPalette}
                    verification={compatibilityLabel(selected.compatibility)}
                    removing={removing}
                    error={removalError}
                    tagSelectionCount={selectedIDs.length}
                    onEditTags={() => {
                        setTagAssignmentStoryIDs([...selectedIDs]);
                        setTagAssignmentOpen(true);
                    }}
                    onClose={() => {
                        setDetailsOpen(false);
                        openDetailsButton.current?.focus({preventScroll: true});
                    }}
                    onRemove={() => void confirmRemoval()}
                />
            )}
            {historyOpen && (
                <OperationHistoryDialog liveOperations={operationSnapshots}
                    onClose={() => setHistoryOpen(false)}
                    onReveal={revealExportDestination}/>
            )}

            {shelfDialogMode && (
                <ModalBackdrop onDismiss={() => setShelfDialogMode(null)} disabled={shelfBusy}>
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
                        <div className="dialog-kicker">{t("Saved shelves")}</div>
                        <h3 id="shelf-dialog-title">
                            {shelfDialogMode === 'save'
                                ? t("Save the current query")
                                : shelfDialogMode === 'rename'
                                    ? t("Rename this shelf")
                                    : t("Duplicate this shelf")}
                        </h3>
                        <p>
                            {shelfDialogMode === 'save'
                                ? t("The name search and active filters will stay dynamic as your local library changes.")
                                : t("Choose a unique name for this saved shelf.")}
                        </p>
                        <label className="dialog-field">
                            <span>{t("Shelf name")}</span>
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
                                {t("Cancel")}</button>
                            <button
                                className="export"
                                type="submit"
                                disabled={shelfBusy || shelfDialogName.trim() === ''}
                            >
                                {shelfBusy
                                    ? t("Saving…")
                                    : shelfDialogMode === 'rename'
                                        ? t("Rename shelf")
                                        : shelfDialogMode === 'duplicate'
                                            ? t("Duplicate shelf")
                                            : t("Save shelf")}
                            </button>
                        </div>
                    </form>
                </ModalBackdrop>
            )}
            {repairShelf && (
                <ModalBackdrop onDismiss={() => setRepairShelfID(null)} disabled={shelfBusy}>
                    <section
                        ref={repairShelfDialog}
                        className="detail-dialog shelf-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="repair-shelf-title"
                        data-state="shelf-repair"
                    >
                        <div className="dialog-kicker">{t("Saved shelves")}</div>
                        <h3 id="repair-shelf-title">{t("Repair “")}{repairShelf.name}”</h3>
                        <p>{shelfAttentionMessage(repairShelf.attentionReason)}</p>
                        <div className="removal-confirmation">
                            <b>{t("Evaluation and export are blocked.")}</b>
                            <p>
                                {t("Replacing the criteria is explicit and permanent. Stories and managed archives are never changed.")}</p>
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
                                {t("Cancel")}</button>
                            <button
                                className="danger"
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => {
                                    setRepairShelfID(null);
                                    setDeleteShelfID(repairShelf.id);
                                }}
                            >
                                {t("Delete shelf")}</button>
                            {unavailableCriteria && (
                                <button
                                    type="button"
                                    disabled={shelfBusy || !tagCatalog}
                                    onClick={removeUnavailableCriteria}
                                >
                                    {t("Remove unavailable criteria")}</button>
                            )}
                            <button
                                className="export"
                                type="button"
                                disabled={shelfBusy || unavailableCriteria}
                                onClick={() => void repairShelfWithCurrentQuery()}
                            >
                                {shelfBusy ? t("Replacing…") : t("Replace with current query")}
                            </button>
                        </div>
                    </section>
                </ModalBackdrop>
            )}
            {deleteShelf && (
                <ModalBackdrop onDismiss={() => setDeleteShelfID(null)} disabled={shelfBusy}>
                    <section
                        ref={deleteShelfDialog}
                        className="detail-dialog shelf-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="delete-shelf-title"
                        data-state="shelf-delete-confirmation"
                    >
                        <div className="dialog-kicker">{t("Saved shelves")}</div>
                        <h3 id="delete-shelf-title">{t("Delete “")}{deleteShelf.name}”?</h3>
                        <p>
                            {t("Only this saved query will be removed. Matching stories and their managed archives will stay in your library.")}</p>
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
                                {t("Cancel")}</button>
                            <button
                                className="danger"
                                type="button"
                                disabled={shelfBusy}
                                onClick={() => void confirmShelfDeletion()}
                            >
                                {shelfBusy ? t("Deleting…") : t("Delete shelf")}
                            </button>
                        </div>
                    </section>
                </ModalBackdrop>
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
                        if (selectedID === null || !tagAssignmentStoryIDs.includes(selectedID)) {
                            setDetailsOpen(false);
                            openDetailsButton.current?.focus({preventScroll: true});
                        }
                        setTagAssignmentOpen(false);
                        setTagAssignmentStoryIDs([]);
                    }}
                    onWorkspaceChange={acceptEditorWorkspace}
                    onAssignmentsChange={async () => {
                        await invalidateCollection();
                    }}
                />
            )}
        </div>
    );
}

export default App;
