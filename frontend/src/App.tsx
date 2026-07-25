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
    OfficialMetadataStatus,
    OperationSnapshot,
    QueryStories,
    RefreshOfficialMetadata,
    RemoveStory,
    SelectAndImportStories,
    StoryDetail as LoadStoryDetail,
    TagAssignmentWorkspace as LoadTagAssignmentWorkspace,
    TagCatalog as LoadTagCatalog,
} from '../wailsjs/go/main/App';
import {EventsOn} from '../wailsjs/runtime/runtime';
import {library, metadata, operations, tagging} from '../wailsjs/go/models';
import {
    describeImport,
    operationIsActive,
    operationIsTerminal,
} from './import-state';
import {CollectionQueryHistory} from './query-history';
import {
    type CollectionQuery,
    DEFAULT_COLLECTION_QUERY,
} from './query-codec';
import TagManager from './TagManager';
import TagAssignmentEditor from './TagAssignmentEditor';
import {
    describeMetadataRefresh,
    describeMetadataStatus,
} from './metadata-state';

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

const initialCollectionQuery: CollectionQuery = {
    ...DEFAULT_COLLECTION_QUERY,
    pageSize: 12,
    sort: 'imported_desc',
};

type CoverStyle = CSSProperties & {
    '--sky': string;
    '--sun': string;
    '--land': string;
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
    const [applicationState, setApplicationState] = useState('initializing');
    const [collectionQuery, setCollectionQuery] = useState(
        () => queryHistory.current(),
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
    const refreshedOperations = useRef(new Set<string>());
    const searchInput = useRef<HTMLInputElement>(null);
    const collectionRequestGeneration = useRef(0);

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
        queryHistory.replace(collectionQuery);
        return queryHistory.subscribe((query) => {
            collectionRequestGeneration.current++;
            setCollectionQuery(query);
        });
    }, [collectionQuery, queryHistory]);

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
            if (snapshot.kind === 'metadata_sync') {
                void refreshMetadataStatus();
                void LoadTagCatalog().then((response) => {
                    if (response.catalog) {
                        setTagCatalog(response.catalog);
                    }
                });
            }
        }
    }, [refreshMetadataStatus]);

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
    }, [reconcileOperation]);

    useEffect(() => {
        if (applicationState === 'ready') {
            const timer = window.setTimeout(() => void loadCollection(), 0);
            return () => window.clearTimeout(timer);
        }
    }, [applicationState, loadCollection]);

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
    const hasMetadataOperation = operationSnapshots.some(
        (snapshot) => snapshot.kind === 'metadata_sync',
    );
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

    const updateQuery = useCallback((next: CollectionQuery) => {
        setCollectionQuery(queryHistory.push(next));
    }, [queryHistory]);

    const reconcileTagCatalog = useCallback((catalog: tagging.Catalog) => {
        setTagCatalog(catalog);
        setAssignmentWorkspace((current) => current
            ? new tagging.AssignmentWorkspace({...current, catalog})
            : current);
        const definitions = new Map(
            catalog.definitions.map((definition) => [definition.id, definition]),
        );
        const booleanFilters = collectionQuery.booleanFilters.filter(
            (filter) => definitions.get(filter.definitionId)?.kind === 'boolean',
        );
        const choiceFilters = collectionQuery.choiceFilters.flatMap((filter) => {
            const definition = definitions.get(filter.definitionId);
            if (definition?.kind !== 'choice') {
                return [];
            }
            const validValues = new Set(definition.values.map((value) => value.id));
            const valueIds = filter.valueIds.filter((valueID) => validValues.has(valueID));
            return valueIds.length > 0 ? [{...filter, valueIds}] : [];
        });
        if (
            booleanFilters.length !== collectionQuery.booleanFilters.length ||
            choiceFilters.length !== collectionQuery.choiceFilters.length ||
            choiceFilters.some((filter, index) => (
                filter.valueIds.length !== collectionQuery.choiceFilters[index]?.valueIds.length
            ))
        ) {
            setCollectionQuery(queryHistory.replace({
                ...collectionQuery,
                booleanFilters,
                choiceFilters,
                page: 1,
            }));
        }
    }, [collectionQuery, queryHistory]);

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

    const activeFilters = [
        ...(collectionQuery.name
            ? [{
                key: 'name',
                label: `Name contains “${collectionQuery.name}”`,
                remove: () => updateQuery({...collectionQuery, name: '', page: 1}),
            }]
            : []),
        ...collectionQuery.booleanFilters.flatMap((filter) => {
            const definition = tagCatalog?.definitions.find(
                (candidate) => candidate.id === filter.definitionId,
            );
            return definition
                ? [{
                    key: `boolean-${filter.definitionId}`,
                    label: filter.state === 'true'
                        ? definition.label
                        : `Not ${definition.label}`,
                    remove: () => setBooleanFilter(filter.definitionId, 'ignored'),
                }]
                : [];
        }),
        ...collectionQuery.choiceFilters.flatMap((filter) => {
            const definition = tagCatalog?.definitions.find(
                (candidate) => candidate.id === filter.definitionId,
            );
            return filter.valueIds.flatMap((valueID) => {
                const value = definition?.values.find((candidate) => candidate.id === valueID);
                return definition && value
                    ? [{
                        key: `choice-${definition.id}-${value.id}`,
                        label: `${definition.label} · ${value.label}`,
                        remove: () => toggleChoiceFilter(definition.id, value.id),
                    }]
                    : [];
            });
        }),
    ];

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
                    <button className="active" type="button">
                        <i style={{'--c': '#405cf5'} as CSSProperties}/>
                        All stories
                        <span>{page?.totalItems ?? 0}</span>
                    </button>
                </nav>
                <div className="caption">Refine this shelf</div>
                {tagCatalog?.definitions.map((definition) => (
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
                ))}
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
                        <h2>My story shelves</h2>
                        <p>Stories in your local archive · {page?.totalItems ?? 0} archives</p>
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

                {metadataStatus && !hasMetadataOperation && (() => {
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
                        <h3>Build your local story archive</h3>
                        <p>
                            Import story packs you already own. Librairii validates and preserves
                            each archive in managed local storage.
                        </p>
                        <button
                            className="import"
                            type="button"
                            onClick={() => void startImport()}
                        >
                            ＋ Import your first stories
                        </button>
                    </section>
                )}

                {rows.map((row, rowIndex) => (
                    <section className="shelf" key={rowIndex}>
                        <div className="shelf-head">
                            <h3>{rowIndex === 0 ? 'Recently added' : 'More stories'}</h3>
                            <span>{row.length} {row.length === 1 ? 'story' : 'stories'} · Local archive</span>
                            <button
                                type="button"
                                disabled={expandingCollection || stories.length >= (page?.totalItems ?? 0)}
                                onClick={() => void loadAllStories()}
                            >
                                {stories.length >= (page?.totalItems ?? 0)
                                    ? 'All shown'
                                    : expandingCollection
                                        ? 'Loading…'
                                        : 'View all →'}
                            </button>
                        </div>
                        <div className="story-row">
                            {row.map((story) => (
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
                                    <div className="cover"><b>{story.title}</b></div>
                                    <small>
                                        {story.author || story.uuid}
                                    </small>
                                </button>
                            ))}
                        </div>
                    </section>
                ))}
                {(page?.totalPages ?? 0) > 1 && (
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
                    <div className="drawer-art" style={selectedPalette}/>
                    <div className="drawer-info">
                        <h3>{selected.title}</h3>
                        <p>
                            {selected.author || 'Local story'} · {selected.sources.title} metadata
                        </p>
                        <div className="facts">
                            <div className="fact">
                                <span>Format</span>
                                <b>{selected.detectedFormat}</b>
                            </div>
                            <div className="fact">
                                <span>Size</span>
                                <b>{formatBytes(selected.byteSize)}</b>
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
                    onAssignmentsChange={loadCollection}
                />
            )}
        </div>
    );
}

export default App;
