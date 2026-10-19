import {useStoryFlags} from './use-story-flags';
import {CollectionDeletion} from './ShelfSources';
import ShelfWorkspace from './ShelfWorkspace';
import ShelfCreation from './ShelfCreation';
import SettingsPanel from './SettingsPanel';
import OutcomeText from './OutcomeText';
import {isStoryTag, systemLabel, systemValue} from './system-label';
import {t, currentLanguage} from './i18n';
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
    LibraryCompatibility,
    CancelOperation,
    CreateCollection,
    DuplicateCollection,
    ListCollections,
    ListShelves,
    ReorderShelves,
    AddStoriesToShelf,
    OfficialMetadataStatus,
    OpenCollection,
    OperationSnapshot,
    PreviewCollections,
    QueryStories,
    RecoverStorage,
    RevealExportDestination,
    RenameCollection,
    ReorderCollections,
    ReplaceCollectionQuery,
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
    collections,
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
import StoryFlagButton from './StoryFlagButton';
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

const savedCollectionColors = ['#ff705c', '#55b79a', '#f5bc41'] as const;

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
    return t("{0} · metadata synced {1}", author, sameDay ? t('today') : syncedAt.toLocaleDateString(currentLanguage()));
}

function savedCollectionColor(index: number): string {
    return savedCollectionColors[index % savedCollectionColors.length];
}

const initialCollectionQuery: CollectionQuery = {
    ...DEFAULT_COLLECTION_QUERY,
    pageSize: 12,
    sort: 'imported_desc',
};

function collectionQueryFromCollection(
    saved: collections.SavedLibraryQuery,
    current: CollectionQuery,
): CollectionQuery {
    return {
        ...(saved.anyOf?.length ? {anyOf: saved.anyOf.map((branch) => collectionQueryFromCollection(branch, current))} : {}),
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

function savedCollectionSource(
    query: collections.SavedLibraryQuery,
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
    return t("Saved collection");
}

function collectionAttentionMessage(reason?: string): string {
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
    return query.anyOf?.some((branch) => hasUnavailableCriteria(branch, catalog)) || query.booleanFilters.some(
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
        ...(query.anyOf?.length ? {anyOf: query.anyOf.map((branch) => withoutUnavailableCriteria(branch, catalog))} : {}),
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

type CollectionDialogMode = 'save' | 'rename' | 'duplicate';
type LibraryGroupRow = {
    key: string;
    name: string;
    storyCount: number;
    source: string;
    stories: library.StorySummary[];
    collectionID?: number;
    error?: string;
};

type SavedCollectionPreviewDetails = {
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
    const [settingsRevision,setSettingsRevision] = useState(0);
 const [libraryRoot,setLibraryRoot]=useState('');
 const acceptanceStarted=useRef(false);
    const [libraryView, setLibraryView] = useLibraryView();
    const [sidebarOpen, setSidebarOpen] = useState(false);
    const closeSidebar = useCallback(() => setSidebarOpen(false), []);
    const queryHistory = useMemo(
        () => new CollectionQueryHistory(window, initialCollectionQuery),
        [],
    );
    const initialHistoryState = useMemo(() => {
        const query = queryHistory.current();
        const collectionID = queryHistory.currentCollectionID();
        return {
            query,
            collectionID,
            scope: queryHistory.currentScope(),
        };
    }, [queryHistory]);
    const [curatedShelfID, setCuratedShelfID] = useState<number | null>(() => queryHistory.currentCuratedShelfID());
    const [curatedShelves, setCuratedShelves] = useState<shelves.Shelf[]>([]);
    const [shelfRevision, setShelfRevision] = useState(0);
    const [createShelfUUIDs, setCreateShelfUUIDs] = useState<string[] | null>(null);
    const [selectedCuratedIDs, setSelectedCuratedIDs] = useState<number[]>([]);
    const [addShelfID, setAddShelfID] = useState('');
    const openCuratedShelf = (id: number) => {
        setCollectionQuery(queryHistory.pushShelf(id));
        setCuratedShelfID(id);
        setSidebarOpen(false);
    };
    const [applicationState, setApplicationState] = useState('initializing');
    const [applicationError, setApplicationError] = useState('');
    const [storageRecoveryAvailable, setStorageRecoveryAvailable] = useState(false);
    const [storageRecovering, setStorageRecovering] = useState(false);
    const [collectionQuery, setCollectionQuery] = useState(
        initialHistoryState.query,
    );
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
    const inspectedStoryID = useRef<number | null>(null);
    const [inspectedStory, setInspectedStory] = useState<library.StorySummary | null>(null);
    const [inspectedArchive, setInspectedArchive] = useState<library.ArchiveDetails>();
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
    const [savedCollections, setSavedCollections] = useState<collections.Summary[]>([]);
    const [savedCollectionStories, setSavedCollectionStories] =
        useState<Record<number, library.StorySummary[]>>({});
    const [savedCollectionPreviewDetails, setSavedCollectionPreviewDetails] =
        useState<Record<number, SavedCollectionPreviewDetails>>({});
    const [savedCollectionPreviewErrors, setSavedCollectionPreviewErrors] =
        useState<Record<number, string>>({});
    const [activeCollectionID, setActiveCollectionID] = useState<number | null>(
        initialHistoryState.collectionID,
    );
    const [collectionScope, setCollectionScope] = useState<CollectionScope>(
        initialHistoryState.scope,
    );
    const [selectedCollectionIDs, setSelectedCollectionIDs] = useState<number[]>([]);
    const [collectionSelectionPreview, setCollectionSelectionPreview] =
        useState<collections.SelectionPreview | null>(null);
    const [collectionSelectionBusy, setCollectionSelectionBusy] = useState(false);
    const [collectionSelectionError, setCollectionSelectionError] = useState<string | null>(null);
    const [collectionDialogMode, setCollectionDialogMode] = useState<CollectionDialogMode | null>(null);
    const [collectionDialogName, setCollectionDialogName] = useState('');
    const [collectionDialogError, setCollectionDialogError] = useState<string | null>(null);
    const [collectionBusy, setCollectionBusy] = useState(false);
    const [repairCollectionID, setRepairCollectionID] = useState<number | null>(null);
    const [deleteCollectionID, setDeleteCollectionID] = useState<number | null>(null);
    const [exportPreflight, setExportPreflight] =
        useState<exporter.PreflightReport | null>(null);
    const [exportPreflightRequest, setExportPreflightRequest] =
        useState<exporter.PreflightRequest | null>(null);
    const [exportPreparing, setExportPreparing] = useState(false);
    const [exportStarting, setExportStarting] = useState(false);
    const [exportStartError, setExportStartError] = useState<string | null>(null);
    const activeCollectionIDRef = useRef<number | null>(initialHistoryState.collectionID);
    const refreshedOperations = useRef(new Set<string>());
    const searchInput = useRef<HTMLInputElement>(null);
    const openDetailsButton = useRef<HTMLButtonElement>(null);
    const collectionRequestGeneration = useRef(0);
    const collectionPreviewGeneration = useRef(0);
    const collectionListRequestGeneration = useRef(0);
    const collectionDialog = useRef<HTMLFormElement>(null);
    const collectionDialogInitialFocus = useRef<HTMLInputElement>(null);
    const repairCollectionDialog = useRef<HTMLElement>(null);
    const repairCollectionInitialFocus = useRef<HTMLButtonElement>(null);
    const exportPreflightDialog = useRef<HTMLElement>(null);
    const exportPreflightInitialFocus = useRef<HTMLButtonElement>(null);
    useModalFocus(collectionDialog, collectionDialogInitialFocus, collectionDialogMode !== null);
    useModalFocus(repairCollectionDialog, repairCollectionInitialFocus, repairCollectionID !== null);
    useModalFocus(
        exportPreflightDialog,
        exportPreflightInitialFocus,
        exportPreflight !== null,
    );
    const activateCollection = useCallback((
        collectionID: number | null,
        scope: CollectionScope,
    ) => {
        setCuratedShelfID(null);
        activeCollectionIDRef.current = collectionID;
        setActiveCollectionID(collectionID);
        setCollectionScope(scope);
    }, []);

    useEffect(() => {
        if (
            collectionDialogMode === null &&
            repairCollectionID === null &&
            deleteCollectionID === null &&
            exportPreflight === null
        ) {
            return;
        }
        const closeOnEscape = (event: KeyboardEvent) => {
            if (
                event.key !== 'Escape' ||
                collectionBusy ||
                exportPreparing ||
                exportStarting
            ) {
                return;
            }
            if (exportPreflight !== null) {
                setExportPreflight(null);
            } else if (deleteCollectionID !== null) {
                setDeleteCollectionID(null);
            } else if (repairCollectionID !== null) {
                setRepairCollectionID(null);
            } else {
                setCollectionDialogMode(null);
            }
        };
        window.addEventListener('keydown', closeOnEscape);
        return () => window.removeEventListener('keydown', closeOnEscape);
    }, [
        deleteCollectionID,
        exportPreflight,
        exportPreparing,
        exportStarting,
        repairCollectionID,
        collectionBusy,
        collectionDialogMode,
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
            const lastPage = Math.max(1, result.page.totalPages);
            if (collectionQuery.page > lastPage) {
                setCollectionQuery(queryHistory.replace({...collectionQuery, page: lastPage}, activeCollectionIDRef.current));
                return;
            }
            setRequestError(null);
            setPage(result.page);
            setDetail(null);
            setDetailRevision((current) => current + 1);
        } catch {
            if (generation === collectionRequestGeneration.current) {
                setRequestError(t("The story collection could not be loaded."));
            }
        }
    }, [collectionQuery, queryHistory]);
    const loadCollectionRef = useRef(loadCollection);
    useEffect(() => {
        loadCollectionRef.current = loadCollection;
    }, [loadCollection]);

    useEffect(() => {
        collectionRequestGeneration.current++;
    }, [collectionQuery]);

    useEffect(() => {
        return queryHistory.subscribe((query, collectionID, scope, shelfID) => {
            collectionRequestGeneration.current++;
            activateCollection(collectionID, scope);
            setCuratedShelfID(shelfID);
            setCollectionQuery(query);
        });
    }, [activateCollection, initialHistoryState.scope, queryHistory]);

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

    useEffect(() => {
        if (applicationState !== 'ready') return;
        let active = true;
        void ListShelves().then((response) => {
            if (!active) return;
            if (response.error) setRequestError(response.error.message);
            else setCuratedShelves(response.shelves ?? []);
        }).catch(() => { if (active) setRequestError(t('Shelves could not be loaded.')); });
        return () => { active = false; };
    }, [applicationState, shelfRevision, settingsRevision]);

    const loadSavedCollections = useCallback(async () => {
        try {
            const generation = ++collectionListRequestGeneration.current;
            const response = await ListCollections();
            if (generation !== collectionListRequestGeneration.current) return;
            if (response.error) {
                setRequestError(response.error.message);
                return;
            }
            const nextCollections = response.collections ?? [];
            setSavedCollections(nextCollections);
            setSelectedCollectionIDs((current) => current.filter((collectionID) => (
                nextCollections.some((collection) => collection.id === collectionID)
            )));
            const activeID = activeCollectionIDRef.current;
            const active = nextCollections.find((collection) => collection.id === activeID);
            if (activeID !== null && (!active || active.validity === 'needs_attention')) {
                if (active?.validity === 'needs_attention') {
                    setRepairCollectionID(active.id);
                    setPage(null);
                }
                activateCollection(null, 'custom');
                queryHistory.replace(queryHistory.current(), null);
            }
        } catch {
            setRequestError(t("Saved collections could not be loaded."));
        }
    }, [activateCollection, queryHistory]);

    const invalidateCollection = useCallback(async () => {
        setCollectionRevision((current) => current + 1);
        await Promise.all([loadCollectionRef.current(), loadSavedCollections()]);
    }, [loadSavedCollections]);

    const onFlagsChanged = useCallback(async (result: library.FlagResult) => {
        const changes = new Map(result.changes.map((change) => [change.storyId, change.current]));
        const refreshStories = (stories: library.StorySummary[]) => stories.map((story) => changes.has(story.id)
            ? new library.StorySummary({...story, broken: changes.get(story.id)}) : story);
        setSavedCollectionStories((current) => Object.fromEntries(Object.entries(current).map(([id, stories]) => [id, refreshStories(stories)])));
        setInspectedStory((current) => {
            const change = result.changes.find((item) => item.storyId === current?.id);
            return current && change ? new library.StorySummary({...current, broken: change.current}) : current;
        });
        await invalidateCollection();
    }, [invalidateCollection]);
    const flags = useStoryFlags(onFlagsChanged);

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
                if (state === 'ready') {
                    const compatibility = await LibraryCompatibility();
                    if (!active) return;
                    if (compatibility.error) throw new Error(compatibility.error.message);
                    queryHistory.configureLegacyBrokenTagID(compatibility.compatibility.legacyBrokenTagId);
                    const migratedQuery = queryHistory.current();
                    if (queryHistory.currentCuratedShelfID() === null) queryHistory.replace(migratedQuery, activeCollectionIDRef.current, initialHistoryState.scope);
                    setCollectionQuery(migratedQuery);
                }
                setApplicationState(state);
                setApplicationError(response.error?.message ?? '');
                setStorageRecoveryAvailable(response.status.recoveryAvailable);
                if (state === 'ready') {
                    const catalogResponse = await LoadTagCatalog();
                    if (active) {
                        setTagCatalog(catalogResponse.catalog ?? null);
                    }
                    const collectionsResponse = await ListCollections();
                    if (!active) {
                        return;
                    }
                    if (collectionsResponse.error) {
                        setRequestError(collectionsResponse.error.message);
                    } else {
                        const nextCollections = collectionsResponse.collections ?? [];
                        setSavedCollections(nextCollections);
                        const activeID = activeCollectionIDRef.current;
                        const active = nextCollections.find(
                            (collection) => collection.id === activeID,
                        );
                        if (
                            activeID !== null &&
                            (!active || active.validity === 'needs_attention')
                        ) {
                            if (active?.validity === 'needs_attention') {
                                setRepairCollectionID(active.id);
                                setPage(null);
                            }
                            activateCollection(null, 'custom');
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
    }, [activateCollection, initialHistoryState.scope, queryHistory, reconcileOperation, settingsRevision]);

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
        if ((applicationState === 'ready'||applicationState==='setup_required')&&!acceptanceStarted.current) {
 acceptanceStarted.current=true;
            void runPackagedAcceptance();
        }
    }, [applicationState]);

    useEffect(() => {
        if (applicationState !== 'ready' || selectedCollectionIDs.length === 0) {
            return;
        }
        let active = true;
        void (async () => {
            setCollectionSelectionBusy(true);
            setCollectionSelectionError(null);
            try {
                const response = await PreviewCollections(selectedCollectionIDs);
                if (!active) {
                    return;
                }
                if (!response.preview) {
                    setCollectionSelectionPreview(null);
                    setCollectionSelectionError(
                        response.error?.message ??
                        t("The collection selection could not be previewed."),
                    );
                    return;
                }
                setCollectionSelectionPreview(response.preview);
            } catch {
                if (!active) {
                    return;
                }
                setCollectionSelectionPreview(null);
                setCollectionSelectionError(t("The collection selection could not be previewed."));
            } finally {
                if (active) {
                    setCollectionSelectionBusy(false);
                }
            }
        })();
        return () => {
            active = false;
        };
    }, [applicationState, savedCollections, selectedCollectionIDs]);

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
        sourceType: 'selection' | 'current_query' | 'collection' | 'collections',
    ) {
        const collectionIDs = sourceType === 'collection'
            ? activeCollectionID === null
                ? selectedCollectionIDs.slice(0, 1)
                : [activeCollectionID]
            : sourceType === 'collections'
                ? selectedCollectionIDs
                : [];
        await preflightExport(new exporter.PreflightRequest({
            sourceType,
            storyIds: sourceType === 'selection' ? [...selectedIDs] : [],
            query: sourceType === 'current_query'
                ? new library.StoryLibraryQuery(collectionQuery)
                : new library.StoryLibraryQuery({}),
            collectionIds: [...collectionIDs],
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
        const target = inspectedStory ?? selected;
        if (!target || removing) {
            return;
        }
        setRemoving(true);
        setRemovalError(null);
        const title = target.title;
        try {
            const response = await RemoveStory(target.id);
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

    useEffect(() => { inspectedStoryID.current = inspectedStory?.id ?? null; }, [inspectedStory?.id]);

    useEffect(() => {
        if (selectedID === null) {
            return;
        }
        let active = true;
        void LoadStoryDetail(selectedID).then((response) => {
            if (active) {
                setDetail(response.detail ?? null);
                if (response.detail && inspectedStoryID.current === selectedID) {
                    setInspectedStory(response.detail.story);
                    setInspectedArchive(response.detail.archive);
                }
            }
        });
        return () => {
            active = false;
        };
    }, [detailRevision, selectedID]);

    useEffect(() => {
        if (!detailsOpen || !inspectedStory?.id || inspectedStory.id === selectedID) return;
        let active = true;
        void LoadStoryDetail(inspectedStory.id).then((response) => {
            if (active && response.detail) {
                setInspectedStory(response.detail.story);
                setInspectedArchive(response.detail.archive);
            }
        });
        return () => { active = false; };
    }, [detailsOpen, inspectedStory?.id, selectedID, detailRevision]);

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
    const activeCollection = savedCollections.find((collection) => collection.id === activeCollectionID) ?? null;
    const allStoriesActive = curatedShelfID === null && activeCollectionID === null &&
        collectionScope === 'all';
    const previewableCollections = useMemo(() => savedCollections.filter(
        (collection) => collection.validity === 'valid',
    ), [savedCollections]);
    const showSavedCollectionRows = allStoriesActive && previewableCollections.length > 0;
    const repairCollection = savedCollections.find((collection) => collection.id === repairCollectionID) ?? null;
    const deleteCollection = savedCollections.find((collection) => collection.id === deleteCollectionID) ?? null;
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
        (definition) => definition.source !== 'derived' && isStoryTag(definition),
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
    const collectionRows = useMemo<LibraryGroupRow[]>(() => (
        [
            ...rows.map((row, rowIndex) => ({
                key: `collection-${rowIndex}`,
                name: rowIndex === 0 ? "Recently added" : "More stories",
                storyCount: row.length,
                source: "Local archive",
                stories: row,
            })),
            ...(showSavedCollectionRows ? previewableCollections.map((collection) => ({
                key: `saved-collection-${collection.id}`,
                name: savedCollectionPreviewDetails[collection.id]?.name ?? collection.name,
                storyCount: savedCollectionPreviewDetails[collection.id]?.count ?? collection.count,
                source: savedCollectionPreviewDetails[collection.id]?.source ?? t("Saved collection"),
                stories: savedCollectionStories[collection.id] ?? [],
                collectionID: collection.id,
                error: savedCollectionPreviewErrors[collection.id],
            })) : []),
        ]
    ), [
        previewableCollections,
        rows,
        savedCollectionPreviewDetails,
        savedCollectionPreviewErrors,
        savedCollectionStories,
        showSavedCollectionRows,
    ]);
    const renderedStories = useMemo(
        () => collectionRows.flatMap((row) => row.stories),
        [collectionRows],
    );
    const [previousRenderedStories, setPreviousRenderedStories] = useState(renderedStories);
    if (renderedStories !== previousRenderedStories) {
        setPreviousRenderedStories(renderedStories);
        const visibleIDs = new Set(renderedStories.map((story) => story.id));
        const visibleSelection = selectedIDs.filter((id) => visibleIDs.has(id));
        const nextSelection = visibleSelection.length > 0
            ? visibleSelection : renderedStories.slice(0, 1).map((story) => story.id);
        setSelectedIDs(nextSelection);
        setSelectedID(selectedID !== null && nextSelection.includes(selectedID)
            ? selectedID : nextSelection[0] ?? null);
    }
    const selectedSummary = renderedStories.find(
        (story) => story.id === selectedID,
    ) ?? null;
    const selected = selectedSummary
        ? detail?.story.id === selectedID ? detail.story : selectedSummary
        : null;
    const inspected = inspectedStory ?? selected;
    const selectedFlaggedCount = selectedIDs.filter((id) => renderedStories.some((story) => story.id === id && story.broken)).length;
    const selectedPalette = selected ? paletteFor(selected.id) : undefined;
    const selectedCollectionCount = selected
        ? savedCollections.find((collection) => (
            savedCollectionStories[collection.id]?.some((story) => story.id === selected.id)
        ))?.count
        : undefined;
    const empty = page !== null && page.totalItems === 0 && !importing;
    const assignedTags = assignmentWorkspace?.catalog.definitions.filter(isStoryTag).flatMap((definition) => {
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
                    ? systemLabel(definition)
                    : `${systemLabel(definition)} · ${t("Mixed")}`,
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
                    ? `${systemLabel(definition)} · ${systemValue(definition,value)}`
                    : `${systemLabel(definition)} · ${systemValue(definition,value)} · ${t("Mixed")}`,
            }];
        });
    }) ?? [];

    useEffect(() => {
        const generation = ++collectionPreviewGeneration.current;
        if (applicationState !== 'ready' || !showSavedCollectionRows) {
            return;
        }
        let active = true;
        void Promise.all(previewableCollections.map(async (collection) => {
            try {
                const response = await OpenCollection(
                    collection.id,
                    new library.ListRequest({
                        page: 1,
                        pageSize: 6,
                        sort: collectionQuery.sort,
                    }),
                );
                if (!response.evaluation) {
                    return {
                        collectionID: collection.id,
                        stories: [] as library.StorySummary[],
                        details: undefined,
                        error: response.error?.message ??
                            t("{0} could not be previewed.", collection.name),
                    };
                }
                return {
                    collectionID: collection.id,
                    stories: response.evaluation.page.stories,
                    details: {
                        name: response.evaluation.collection.name,
                        count: response.evaluation.page.totalItems,
                        source: savedCollectionSource(
                            response.evaluation.query,
                            [tagCatalog, assignmentWorkspace?.catalog],
                        ),
                    },
                    error: '',
                };
            } catch {
                return {
                    collectionID: collection.id,
                    stories: [] as library.StorySummary[],
                    details: undefined,
                    error: t("{0} could not be previewed.", collection.name),
                };
            }
        })).then((previews) => {
            if (!active || generation !== collectionPreviewGeneration.current) {
                return;
            }
            setSavedCollectionStories(Object.fromEntries(previews.map((preview) => (
                [preview.collectionID, preview.stories]
            ))));
            setSavedCollectionPreviewDetails(Object.fromEntries(previews.flatMap(
                (preview) => preview.details
                    ? [[preview.collectionID, preview.details]]
                    : [],
            )));
            setSavedCollectionPreviewErrors(Object.fromEntries(previews.flatMap(
                (preview) => preview.error
                    ? [[preview.collectionID, preview.error]]
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
        
        collectionRevision,
        previewableCollections,
        showSavedCollectionRows,
        tagCatalog,
    ]);

    const updateQuery = useCallback((
        next: CollectionQuery,
        collectionID?: number | null,
    ) => {
        setCollectionQuery(queryHistory.push(next, collectionID));
        setCollectionScope(queryHistory.currentScope());
    }, [queryHistory]);

    function toggleCollectionSelection(collectionID: number) {
        setSelectedCollectionIDs((current) => current.includes(collectionID)
            ? current.filter((candidate) => candidate !== collectionID)
            : [...current, collectionID]);
    }

    async function openSavedCollection(collectionID: number) {
        setCollectionBusy(true);
        setRequestError(null);
        try {
            const response = await OpenCollection(
                collectionID,
                new library.ListRequest({
                    page: 1,
                    pageSize: collectionQuery.pageSize,
                    sort: collectionQuery.sort,
                }),
            );
            if (!response.evaluation) {
                setRequestError(
                    response.error?.message ?? t("The saved collection could not be opened."),
                );
                return;
            }
            activateCollection(collectionID, 'collection');
            setPage(response.evaluation.page);
            updateQuery(collectionQueryFromCollection(
                response.evaluation.query,
                collectionQuery,
            ), collectionID);
        } catch {
            setRequestError(t("The saved collection could not be opened."));
        } finally {
            setCollectionBusy(false);
        }
    }

    function openAllStories() {
        activateCollection(null, 'all');
        updateQuery({
            ...collectionQuery,
            anyOf: undefined,
            brokenState: undefined,
            name: '',
            languages: [],
            compatibilities: [],
            booleanFilters: [],
            choiceFilters: [],
            page: 1,
        }, null);
    }

    function showCollectionDialog(mode: CollectionDialogMode) {
        setCollectionDialogMode(mode);
        setCollectionDialogError(null);
        setCollectionDialogName(mode === 'rename' ? activeCollection?.name ?? '' : '');
    }

    async function submitCollectionDialog() {
        if (!collectionDialogMode) {
            return;
        }
        setCollectionBusy(true);
        setCollectionDialogError(null);
        try {
            let queryToSave = collectionQuery;
            if (collectionDialogMode === 'save' && selectedCollectionIDs.length > 0) {
                const selected = await Promise.all(selectedCollectionIDs.map((id) => OpenCollection(
                    id,
                    new library.ListRequest({
                        page: 1,
                        pageSize: 1,
                        sort: collectionQuery.sort,
                    }),
                )));
                if (selected.some((response) => !response.evaluation)) {
                    setCollectionDialogError(selected.find((response) => !response.evaluation)?.error?.message ?? t('The saved collection could not be opened.'));
                    return;
                }
                queryToSave = {
                    ...initialCollectionQuery,
                    sort: collectionQuery.sort,
                    pageSize: collectionQuery.pageSize,
                    anyOf: selected.map((response) => collectionQueryFromCollection(response.evaluation!.query, collectionQuery)),
                };
            }
            const response = collectionDialogMode === 'save'
                ? await CreateCollection(
                    collectionDialogName,
                    new library.StoryLibraryQuery(queryToSave),
                )
                : collectionDialogMode === 'rename' && activeCollection
                    ? await RenameCollection(activeCollection.id, collectionDialogName)
                    : collectionDialogMode === 'duplicate' && activeCollection
                        ? await DuplicateCollection(activeCollection.id, collectionDialogName)
                        : null;
            if (!response?.collection) {
                setCollectionDialogError(
                    response?.error?.message ?? t("The saved collection could not be changed."),
                );
                return;
            }
            const changedCollection = response.collection;
            setCollectionDialogMode(null);
            setCollectionDialogName('');
            await loadSavedCollections();
            if (collectionDialogMode === 'save') {
                setCollectionQuery(queryHistory.replace(queryToSave, changedCollection.id));
                setCollectionScope(queryHistory.currentScope());
                activateCollection(changedCollection.id, 'collection');
            } else if (collectionDialogMode === 'duplicate') {
                await openSavedCollection(changedCollection.id);
            }
        } catch {
            setCollectionDialogError(t("The saved collection could not be changed."));
        } finally {
            setCollectionBusy(false);
        }
    }

    async function updateActiveCollection() {
        if (!activeCollection) {
            return;
        }
        setCollectionBusy(true);
        setRequestError(null);
        try {
            const response = await ReplaceCollectionQuery(
                activeCollection.id,
                new library.StoryLibraryQuery(collectionQuery),
            );
            if (!response.collection) {
                setRequestError(
                    response.error?.message ?? t("The saved collection could not be updated."),
                );
                return;
            }
            await loadSavedCollections();
        } catch {
            setRequestError(t("The saved collection could not be updated."));
        } finally {
            setCollectionBusy(false);
        }
    }

    async function moveCollection(collectionID: number, direction: -1 | 1) {
        const index = savedCollections.findIndex((collection) => collection.id === collectionID);
        const target = index + direction;
        if (index < 0 || target < 0 || target >= savedCollections.length) {
            return;
        }
        const ordered = [...savedCollections];
        [ordered[index], ordered[target]] = [ordered[target], ordered[index]];
        setCollectionBusy(true);
        try {
            const response = await ReorderCollections(ordered.map((collection) => collection.id));
            if (response.error) {
                setRequestError(response.error.message);
            } else {
                // Reordering does not change membership; preserve current visibility counts.
                const counts = new Map(savedCollections.map((collection) => [collection.id, collection.count]));
                setSavedCollections((response.collections ?? []).map((collection) => new collections.Summary({
                    ...collection, count: counts.get(collection.id) ?? collection.count,
                })));
            }
        } catch {
            setRequestError(t("Saved collections could not be reordered."));
        } finally {
            setCollectionBusy(false);
        }
    }


    async function repairCollectionWithCurrentQuery() {
        if (!repairCollection || unavailableCriteria) {
            return;
        }
        setCollectionBusy(true);
        setCollectionDialogError(null);
        try {
            const response = await ReplaceCollectionQuery(
                repairCollection.id,
                new library.StoryLibraryQuery(collectionQuery),
            );
            if (!response.collection) {
                setCollectionDialogError(
                    response.error?.message ?? t("The saved collection could not be repaired."),
                );
                return;
            }
            setRepairCollectionID(null);
            queryHistory.replace(collectionQuery, repairCollection.id);
            activateCollection(repairCollection.id, 'collection');
            await loadSavedCollections();
        } catch {
            setCollectionDialogError(t("The saved collection could not be repaired."));
        } finally {
            setCollectionBusy(false);
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
                        ? systemLabel(definition)
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
                        label: `${systemLabel(definition)} · ${systemValue(definition,value)}`,
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
                {systemLabel(definition)} <b aria-hidden="true">{collapsedTagFacets.has(definition.id) ? '+' : '−'}</b>
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
                            {systemLabel(definition)}
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
                            {t("Not")}{' '}{systemLabel(definition)}
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
                        {systemValue(definition,value)}
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
 <SettingsPanel onLocation={setLibraryRoot} onReady={() => setSettingsRevision((value) => value + 1)}/>
            <span className="sr-only" data-testid="application-state">
                {t("Application state:")}{' '}{applicationState}
            </span>
            <aside className="rail" aria-label={t("Primary navigation")} inert={sidebarOpen || historyOpen}>
                <div className="mark" role="img" aria-label="Librairii">
                    <i/><i/><i/>
                </div>
                <LibraryNavigation
                    libraryActive={allStoriesActive}
                    disabled={applicationState !== 'ready' || collectionBusy}
                    importing={importing}
                    exporting={exportPreparing}
                    onLibrary={openAllStories}
                    onTags={() => setTagManagerOpen(true)}
                    onImport={() => void startImport()}
                    onExport={() => curatedShelfID === null ? void prepareExport('current_query') : void preflightExport(new exporter.PreflightRequest({sourceType: 'current_query', queryShelfId: curatedShelfID, query: collectionQuery}))}
                />
            </aside>

            <LibrarySidebar open={sidebarOpen} onClose={closeSidebar} inert={historyOpen}
                suspended={tagManagerOpen || tagAssignmentOpen || collectionDialogMode !== null ||
                    repairCollectionID !== null || deleteCollectionID !== null || exportPreflight !== null || historyOpen}>
                <h1>Librairii</h1>
                <div className="path">{libraryRoot}</div>
                <div className="caption">{t("Library")}</div>
                <nav className="saved">
                    <button
                        className={`saved-picker${allStoriesActive ? ' active' : ''}`}
                        type="button"
                        aria-current={allStoriesActive ? 'page' : undefined}
                        disabled={collectionBusy}
                        onClick={openAllStories}
                    >
                        <i style={{'--c': '#405cf5'} as CSSProperties}/>
                        {t("All stories")}<span>{allStoriesActive ? page?.totalItems ?? 0 : t("All")}</span>
                    </button>
                    <div className="caption">{t("Collections")}</div>
                    {savedCollections.map((collection, index) => (
                        <div className="saved-entry" key={collection.id}>
                            <div className="saved-entry-row">
                                <input
                                    className="shelf-selector"
                                    type="checkbox"
                                    aria-label={t("Select {0} for combined collection preview", collection.name)}
                                    checked={selectedCollectionIDs.includes(collection.id)}
                                    disabled={
                                        collectionBusy ||
                                        (
                                            collection.validity === 'needs_attention' &&
                                            !selectedCollectionIDs.includes(collection.id)
                                        )
                                    }
                                    onChange={() => toggleCollectionSelection(collection.id)}
                                />
                                <button
                                    className={`saved-picker${
                                        activeCollectionID === collection.id ? ' active' : ''
                                    }${
                                        collection.validity === 'needs_attention'
                                            ? ' needs-attention'
                                            : ''
                                    }`}
                                    type="button"
                                    aria-current={
                                        activeCollectionID === collection.id ? 'page' : undefined
                                    }
                                    aria-label={collection.validity === 'needs_attention'
                                        ? t("{0}, needs attention", collection.name)
                                        : `${collection.name}, ${collection.count} ${
                                            collection.count === 1 ? t("story") : t("stories")
                                        }`}
                                    disabled={collectionBusy}
                                    onClick={() => collection.validity === 'needs_attention'
                                        ? setRepairCollectionID(collection.id)
                                        : void openSavedCollection(collection.id)}
                                >
                                    <i
                                        style={{
                                            '--c': savedCollectionColor(index),
                                        } as CSSProperties}
                                    />
                                    {collection.name}
                                    <span>
                                        {collection.validity === 'needs_attention'
                                            ? '!'
                                            : collection.count}
                                    </span>
                                </button>
                            </div>
                            {activeCollectionID === collection.id && (
                                <div className="saved-tools" aria-label={t("{0} actions", collection.name)}>
                                    <button
                                        type="button"
                                        disabled={collectionBusy || index === 0}
                                        aria-label={t("Move {0} up", collection.name)}
                                        onClick={() => void moveCollection(collection.id, -1)}
                                    >
                                        ↑
                                    </button>
                                    <button
                                        type="button"
                                        disabled={collectionBusy || index === savedCollections.length - 1}
                                        aria-label={t("Move {0} down", collection.name)}
                                        onClick={() => void moveCollection(collection.id, 1)}
                                    >
                                        ↓
                                    </button>
                                    <button
                                        type="button"
                                        disabled={collectionBusy}
                                        aria-label={t("Rename {0}", collection.name)}
                                        onClick={() => showCollectionDialog('rename')}
                                    >
                                        ✎
                                    </button>
                                    <button
                                        type="button"
                                        disabled={collectionBusy}
                                        aria-label={t("Duplicate {0}", collection.name)}
                                        onClick={() => showCollectionDialog('duplicate')}
                                    >
                                        ⧉
                                    </button>
                                    <button
                                        type="button"
                                        disabled={collectionBusy}
                                        aria-label={t("Delete {0}", collection.name)}
                                        onClick={() => {
                                            setCollectionDialogError(null);
                                            setDeleteCollectionID(collection.id);
                                        }}
                                    >
                                        ×
                                    </button>
                                </div>
                            )}
                        </div>
                    ))}
                </nav>
                <section aria-label={t('Shelves')}>
                    <div className="caption">{t('Shelves')}</div><button type="button" disabled={applicationState !== 'ready'} onClick={() => setCreateShelfUUIDs([])}>{t('Create Shelf')}</button>
                    {curatedShelves.map((shelf, index) => <div key={shelf.id}><label><input type="checkbox" checked={selectedCuratedIDs.includes(shelf.id)} aria-label={t('Select Shelf {0} for export', shelf.name)} onChange={() => setSelectedCuratedIDs(selectedCuratedIDs.includes(shelf.id) ? selectedCuratedIDs.filter((id) => id !== shelf.id) : [...selectedCuratedIDs, shelf.id])}/><span className="sr-only">{shelf.name}</span></label><button className="saved-picker" type="button" key={shelf.id}
                        aria-current={curatedShelfID === shelf.id ? 'page' : undefined}
                        onClick={() => openCuratedShelf(shelf.id)}>{shelf.name}<span>{shelf.count}</span></button>
                        <button type="button" disabled={collectionBusy || index === 0} aria-label={t('Move Shelf {0} up', shelf.name)} onClick={() => {
                            const ids = curatedShelves.map((entry) => entry.id); [ids[index], ids[index - 1]] = [ids[index - 1], ids[index]];
                            setCollectionBusy(true); void ReorderShelves(ids).then((response) => {if (response.error) setRequestError(response.error.message); else setShelfRevision((value) => value + 1);}).catch(() => setRequestError(t('Shelves could not be reordered.'))).finally(() => setCollectionBusy(false));
                        }}>↑</button></div>)}
                    {selectedCuratedIDs.length > 0 && <button type="button" disabled={exportPreparing} onClick={() => void preflightExport(new exporter.PreflightRequest({sourceType: selectedCuratedIDs.length === 1 ? 'curated_shelf' : 'curated_shelves', shelfIds: selectedCuratedIDs, query: {}}))}>{t('Export selected Shelves')}</button>}
                </section>
                {selectedCollectionIDs.length > 0 && (
                    <section className="combined-shelves" aria-label={t("Combined collection preview")}>
                        <div className="combined-shelves-title">
                            <b>{t("Combined preview")}</b>
                            <span>{selectedCollectionIDs.length} {' ' + t("selected")}</span>
                        </div>
                        {collectionSelectionBusy && <p>{t("Resolving current membership…")}</p>}
                        {collectionSelectionError && (
                            <p className="dialog-error">{<OutcomeText text={collectionSelectionError}/> }</p>
                        )}
                        {collectionSelectionPreview && !collectionSelectionBusy && (
                            <>
                                <ul>
                                    {collectionSelectionPreview.collections.map((collection) => (
                                        <li key={collection.id}>
                                            <span>{collection.name}</span>
                                            <b>{collection.count}</b>
                                        </li>
                                    ))}
                                </ul>
                                <strong>
                                    {collectionSelectionPreview.uniqueStoryCount}{' '}
                                    {t("unique")} {collectionSelectionPreview.uniqueStoryCount === 1
                                        ? t("story")
                                        : t("stories")}
                                </strong>
                                <p>
                                    {collectionSelectionPreview.overlapCount === 0
                                        ? t("No overlapping memberships.")
                                        : t("{0} overlapping {1} collapsed.", collectionSelectionPreview.overlapCount, collectionSelectionPreview.overlapCount === 1
                                                ? 'membership'
                                                : 'memberships')}
                                </p>
                                <small>
                                    {t("Sources:")} {collectionSelectionPreview.sourceCollectionNames.join(', ')}
                                </small>
                                <button
                                    className="combined-export"
                                    type="button"
                                    disabled={
                                        exportPreparing ||
                                        collectionSelectionPreview.uniqueStoryCount === 0
                                    }
                                    onClick={() => void prepareExport(
                                        selectedCollectionIDs.length === 1
                                            ? 'collection'
                                            : 'collections',
                                    )}
                                >
                                    {exportPreparing
                                        ? t("Preparing…")
                                        : selectedCollectionIDs.length === 1
                                            ? t("Export selected collection")
                                            : t("Export selected collections")}
                                </button>
                            </>
                        )}
                    </section>
                )}

                {activeFilters.length > 0 && !showSavedCollectionRows && (
                    <button
                        className="manage collection-save"
                        type="button"
                        disabled={collectionBusy}
                        onClick={() => showCollectionDialog('save')}
                    >
                        {t("＋ Save current query")}</button>
                )}
                {activeCollection && (
                    <button
                        className="manage collection-update"
                        type="button"
                        disabled={collectionBusy}
                        onClick={() => void updateActiveCollection()}
                    >
                        {t("↻ Update “")}{activeCollection.name}”
                    </button>
                )}
                <div className="caption">{t("Refine this view")}</div>
                {derivedDefinitions.map(renderTagFacet)}
                <div className="facet">
                    <button
                        className="facet-title"
                        type="button"
                        aria-expanded={languageFacetOpen}
                        onClick={() => setLanguageFacetOpen((open) => !open)}
                    >
                        {t("Language")} <b>{languageFacetOpen ? '−' : '＋'}</b>
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
                        {t("Import status")} <b>{importFacetOpen ? '−' : '＋'}</b>
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

            <main tabIndex={-1} className="main" inert={sidebarOpen || historyOpen}>
                {curatedShelfID !== null && <><button className="sidebar-toggle" type="button" onClick={() => setSidebarOpen(true)}>{t('Library & filters')}</button>
                    <ShelfWorkspace key={curatedShelfID} id={curatedShelfID} revision={shelfRevision + collectionRevision + detailRevision}
                        collections={savedCollections} query={collectionQuery} onQuery={(query) => setCollectionQuery(queryHistory.pushShelf(curatedShelfID, query))}
                        onChanged={() => setShelfRevision((value) => value + 1)} onDeleted={openAllStories}
                        onExport={(request) => void preflightExport(request)} onInspect={(story) => {setInspectedStory(story); setDetailsOpen(true);}}/>
                </>}
                <div hidden={curatedShelfID !== null}>
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
                        <h2>{activeCollection?.name ?? t("My story shelves")}</h2>
                        <p>
                            {activeCollection
                                ? t("Dynamic saved collection")
                                : t("Stories grouped by your tags")}
                            {' · '}
                            {page?.totalItems ?? 0} {' '}{t("archives")}</p>
                    </div>
                    <div className="heading-actions">

                {activeFilters.length > 0 && !showSavedCollectionRows && (
                            <button
                                className="scope-export"
                                type="button"
                                disabled={exportPreparing}
                                onClick={() => void prepareExport('current_query')}
                            >
                                {t("Export current result")}</button>
                        )}
                        {activeCollection && (
                            <button
                                className="scope-export"
                                type="button"
                                disabled={exportPreparing}
                                onClick={() => void prepareExport('collection')}
                            >
                                {t("Export collection")}</button>
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
                        {t("Library &amp; filters")}</button>
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

                {flags.error && !detailsOpen && <p className="flag-feedback dialog-error" role="alert"><OutcomeText text={flags.error}/></p>}
                {flags.notice && !detailsOpen && <div className="flag-feedback" role="status">
                    <span>{flags.notice}</span>
                    {flags.canUndo && <button type="button" disabled={flags.pending} onClick={() => void flags.undo()}>{t('Undo')}</button>}
                </div>}

                {activeFilters.length > 0 && !showSavedCollectionRows && (
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
                                    anyOf: undefined,
                                    brokenState: undefined,
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
                                <OutcomeText text={applicationError || 'Librairii cannot safely open its application data.'}/>
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
                            <p>{<OutcomeText text={requestError}/> }</p>
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
                                                <span>{<OutcomeText text={item.outcomeMessage || item.status} status={item.status}/>}</span>
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
                            {activeCollection
                                ? t("{0} is currently empty", activeCollection.name)
                                : t("Build your local story archive")}
                        </h3>
                        <p>
                            {activeCollection
                                ? t("This saved query remains valid and will fill automatically when stories start matching it.")
                                : t("Import story packs you already own. Librairii validates and preserves each archive in managed local storage.")}
                        </p>
                        {activeCollection && (
                            <button
                                type="button"
                                onClick={() => updateQuery({
                                    ...collectionQuery,
                                    name: '',
                                    anyOf: undefined,
                                    brokenState: undefined,
                                    languages: [],
                                    compatibilities: [],
                                    booleanFilters: [],
                                    choiceFilters: [],
                                    page: 1,
                                })}
                            >
                                {t("Edit collection query")}</button>
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
                            <h3>{row.collectionID === undefined ? t(row.name) : row.name}</h3>
                            <span>
                                {row.storyCount} {
                                    row.storyCount === 1 ? t("story") : t("stories")
                                } · {row.collectionID === undefined ? t(row.source) : row.source}
                            </span>
                            <button
                                type="button"
                                aria-label={row.collectionID === undefined
                                    ? undefined
                                    : t("View all stories in {0}", row.name)}
                                disabled={row.collectionID === undefined
                                    ? expandingCollection ||
                                        stories.length >= (page?.totalItems ?? 0)
                                    : collectionBusy}
                                onClick={() => row.collectionID === undefined
                                    ? void loadAllStories()
                                    : void openSavedCollection(row.collectionID)}
                            >
                                {row.collectionID !== undefined
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
                        <div className={`story-row${row.collectionID === undefined ? '' : ' preview'}`}>
                            {row.stories.map((story) => (
                                <div className={`story-card${story.broken ? ' is-broken' : ''}`} key={story.id}>
                                    <button
                                        className={`story${selectedIDs.includes(story.id) ? ' selected' : ''}`}
                                        style={paletteFor(story.id)}
                                        type="button"
                                        data-story-id={story.id}
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

                                            >
                                                <span aria-hidden="true">⚠</span>{t("Broken")}</span>
                                        )}
                                    </button>
                                    <StoryFlagButton title={story.title} broken={!!story.broken}
                                        disabled={flags.pending || applicationState !== 'ready'}
                                        onChange={(broken) => void flags.set([story.id], broken)}/>
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
                            {t("Page")}{' '}{page?.page ?? collectionQuery.page} {' ' + t("of") + ' '}{page?.totalPages}
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
                </div>
            </main>

            {selected && curatedShelfID === null && (
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
                                <b>{selectedCollectionCount ?? '—'}</b>
                            </div>
                            <div className="fact">
                                <span>{t("Duration")}</span>
                                <b>{formatDuration(selected.official?.durationSeconds)}</b>
                            </div>
                            <div className="fact">
                                <span>{t("Language")}{' '}</span>
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
                        <label>{t('Add selection to Shelf')}<select value={addShelfID} onChange={(event) => setAddShelfID(event.target.value)}><option value="">{t('Choose Shelf')}</option>
                            {curatedShelves.map((shelf) => <option key={shelf.id} value={shelf.id}>{shelf.name}</option>)}</select></label>
                        <button type="button" disabled={!addShelfID || collectionBusy} onClick={() => {
                            setCollectionBusy(true);
                            void AddStoriesToShelf(Number(addShelfID), renderedStories.filter((story) => selectedIDs.includes(story.id)).map((story) => story.uuid)).then((response) => {
                                if (response.error) setRequestError(response.error.message); else setShelfRevision((value) => value + 1);
                            }).catch(() => setRequestError(t('Shelf could not be changed.'))).finally(() => setCollectionBusy(false));
                        }}>{t('Add selected stories')}</button>
                        <button type="button" onClick={() => setCreateShelfUUIDs(renderedStories.filter((story) => selectedIDs.includes(story.id)).map((story) => story.uuid))}>{t('Create Shelf from selection')}</button>
                        {selectedIDs.length > 1 ? <>
                            <span className="drawer-flag-count">{t('{0} selected · {1} flagged', selectedIDs.length, selectedFlaggedCount)}</span>
                            <button type="button" disabled={flags.pending} onClick={() => void flags.set(selectedIDs, true)}>{t('Mark selected as broken')}</button>
                            <button type="button" disabled={flags.pending} onClick={() => void flags.set(selectedIDs, false)}>{t('Clear broken flags')}</button>
                        </> : <>
                            {selected.broken && <span className="story-warning-label">⚠ {t('Broken')}</span>}
                            <button type="button" disabled={flags.pending} onClick={() => void flags.set([selected.id], !selected.broken)}>
                                {selected.broken ? t('Clear broken flag') : t('Mark as broken')}
                            </button>
                        </>}
                        <button
                            ref={openDetailsButton}
                            type="button"
                            onClick={() => {
                                setRemovalError(null);
                                setInspectedStory(selected);
                                setInspectedArchive(detail?.story.id === selected.id ? detail.archive : undefined);
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
                                        <span>{<OutcomeText text={issue.message}/> }</span>
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
                                        {item.issue && <p>{<OutcomeText text={item.issue.message}/> }</p>}
                                    </li>
                                ))}
                            </ul>
                        )}
                        {exportStartError && (
                            <p className="dialog-error" role="alert">
                                {<OutcomeText text={exportStartError}/> }
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

            {detailsOpen && inspected && !tagAssignmentOpen && (
                <StoryDetailsDialog
                    key={inspected.id}
                    story={inspected}
                    archive={inspectedArchive ?? (detail?.story.id === inspected.id ? detail.archive : undefined)}
                    artworkURL={artworkURL(inspected.artworkId)}
                    coverStyle={paletteFor(inspected.id)}
                    verification={compatibilityLabel(inspected.compatibility)}
                    removing={removing}
                    error={removalError}
                    flagging={flags.pending}
                    flagError={flags.error}
                    flagNotice={flags.notice}
                    canUndoFlag={flags.canUndo}
                    onFlagChange={(broken) => void flags.set([inspected.id], broken)}
                    onUndoFlag={() => void flags.undo()}
                    tagSelectionCount={selectedIDs.includes(inspected.id) ? selectedIDs.length : 1}
                    onEditTags={() => {
                        setTagAssignmentStoryIDs(selectedIDs.includes(inspected.id) ? [...selectedIDs] : [inspected.id]);
                        setTagAssignmentOpen(true);
                    }}
                    onClose={() => {
                        setDetailsOpen(false);
                        setInspectedStory(null);
                        setInspectedArchive(undefined);
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

            {createShelfUUIDs !== null && <ShelfCreation collections={savedCollections} manualUUIDs={createShelfUUIDs}
                onClose={() => setCreateShelfUUIDs(null)} onCreated={(id) => {setCreateShelfUUIDs(null); setShelfRevision((value) => value + 1); openCuratedShelf(id);}}/>}
            {collectionDialogMode && (
                <ModalBackdrop onDismiss={() => setCollectionDialogMode(null)} disabled={collectionBusy}>
                    <form
                        ref={collectionDialog}
                        className="detail-dialog collection-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="shelf-dialog-title"
                        onSubmit={(event) => {
                            event.preventDefault();
                            void submitCollectionDialog();
                        }}
                    >
                        <div className="dialog-kicker">{t("Saved collections")}</div>
                        <h3 id="shelf-dialog-title">
                            {collectionDialogMode === 'save'
                                ? t("Save the current query")
                                : collectionDialogMode === 'rename'
                                    ? t("Rename this collection")
                                    : t("Duplicate this collection")}
                        </h3>
                        <p>
                            {collectionDialogMode === 'save'
                                ? t("The name search and active filters will stay dynamic as your local library changes.")
                                : t("Choose a unique name for this saved collection.")}
                        </p>
                        <label className="dialog-field">
                            <span>{t("Collection name")}</span>
                            <input
                                ref={collectionDialogInitialFocus}
                                maxLength={80}
                                value={collectionDialogName}
                                onChange={(event) => setCollectionDialogName(
                                    event.currentTarget.value,
                                )}
                            />
                        </label>
                        {collectionDialogError && (
                            <p className="dialog-error">{<OutcomeText text={collectionDialogError}/> }</p>
                        )}
                        <div className="dialog-actions">
                            <button
                                type="button"
                                disabled={collectionBusy}
                                onClick={() => setCollectionDialogMode(null)}
                            >
                                {t("Cancel")}</button>
                            <button
                                className="export"
                                type="submit"
                                disabled={collectionBusy || collectionDialogName.trim() === ''}
                            >
                                {collectionBusy
                                    ? t("Saving…")
                                    : collectionDialogMode === 'rename'
                                        ? t("Rename collection")
                                        : collectionDialogMode === 'duplicate'
                                            ? t("Duplicate collection")
                                            : t("Save collection")}
                            </button>
                        </div>
                    </form>
                </ModalBackdrop>
            )}
            {repairCollection && (
                <ModalBackdrop onDismiss={() => setRepairCollectionID(null)} disabled={collectionBusy}>
                    <section
                        ref={repairCollectionDialog}
                        className="detail-dialog collection-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="repair-shelf-title"
                        data-state="shelf-repair"
                    >
                        <div className="dialog-kicker">{t("Saved collections")}</div>
                        <h3 id="repair-shelf-title">{t("Repair “")}{repairCollection.name}”</h3>
                        <p>{collectionAttentionMessage(repairCollection.attentionReason)}</p>
                        <div className="removal-confirmation">
                            <b>{t("Evaluation and export are blocked.")}</b>
                            <p>
                                {t("Replacing the criteria is explicit and permanent. Stories and managed archives are never changed.")}</p>
                        </div>
                        {collectionDialogError && (
                            <p className="dialog-error">{<OutcomeText text={collectionDialogError}/> }</p>
                        )}
                        <div className="dialog-actions">
                            <button
                                ref={repairCollectionInitialFocus}
                                type="button"
                                disabled={collectionBusy}
                                onClick={() => setRepairCollectionID(null)}
                            >
                                {t("Cancel")}</button>
                            <button
                                className="danger"
                                type="button"
                                disabled={collectionBusy}
                                onClick={() => {
                                    setRepairCollectionID(null);
                                    setDeleteCollectionID(repairCollection.id);
                                }}
                            >
                                {t("Delete collection")}</button>
                            {unavailableCriteria && (
                                <button
                                    type="button"
                                    disabled={collectionBusy || !tagCatalog}
                                    onClick={removeUnavailableCriteria}
                                >
                                    {t("Remove unavailable criteria")}</button>
                            )}
                            <button
                                className="export"
                                type="button"
                                disabled={collectionBusy || unavailableCriteria}
                                onClick={() => void repairCollectionWithCurrentQuery()}
                            >
                                {collectionBusy ? t("Replacing…") : t("Replace with current query")}
                            </button>
                        </div>
                    </section>
                </ModalBackdrop>
            )}
            {deleteCollection && <CollectionDeletion key={deleteCollection.id} id={deleteCollection.id} name={deleteCollection.name} onClose={() => setDeleteCollectionID(null)}
                onDeleted={() => {setDeleteCollectionID(null); if (activeCollectionID === deleteCollection.id) openAllStories(); setShelfRevision((value) => value + 1); void loadSavedCollections();}}/>}
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
