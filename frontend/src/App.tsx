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
    ListStories,
    OperationSnapshot,
    RemoveStory,
    SelectAndImportStories,
    StoryDetail as LoadStoryDetail,
} from '../wailsjs/go/main/App';
import {EventsOn} from '../wailsjs/runtime/runtime';
import {library, operations} from '../wailsjs/go/models';
import {
    describeImport,
    operationIsActive,
    operationIsTerminal,
} from './import-state';

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
    const [applicationState, setApplicationState] = useState('initializing');
    const [page, setPage] = useState<library.Page | null>(null);
    const [selectedID, setSelectedID] = useState<number | null>(null);
    const [detail, setDetail] = useState<library.StoryDetail | null>(null);
    const [detailRevision, setDetailRevision] = useState(0);
    const [operationSnapshots, setOperationSnapshots] = useState<operations.Snapshot[]>([]);
    const [requestError, setRequestError] = useState<string | null>(null);
    const [detailsOpen, setDetailsOpen] = useState(false);
    const [removing, setRemoving] = useState(false);
    const [removalError, setRemovalError] = useState<string | null>(null);
    const [removalNotice, setRemovalNotice] = useState<string | null>(null);
    const [expandingCollection, setExpandingCollection] = useState(false);
    const refreshedOperations = useRef(new Set<string>());

    const loadCollection = useCallback(async () => {
        const result = await ListStories({
            page: 1,
            pageSize: 12,
            sort: 'imported_desc',
        });
        if (!result.page) {
            setRequestError(result.error?.message ?? 'The story collection could not be loaded.');
            return;
        }
        setPage(result.page);
        setDetail(null);
        setDetailRevision((current) => current + 1);
        setSelectedID((current) => {
            if (result.page?.stories.some((story) => story.id === current)) {
                return current;
            }
            return result.page?.stories[0]?.id ?? null;
        });
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
            void loadCollection();
        }
    }, [loadCollection]);

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
                    await loadCollection();
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
    }, [loadCollection, reconcileOperation]);

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
                    setRequestError('Import progress could not be refreshed.');
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

    async function cancelImport(operation: operations.Snapshot) {
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
        setExpandingCollection(true);
        setRequestError(null);
        try {
            const first = await ListStories({
                page: 1,
                pageSize: 100,
                sort: 'imported_desc',
            });
            if (!first.page) {
                setRequestError(first.error?.message ?? 'The full collection could not be loaded.');
                return;
            }
            const byID = new Map(first.page.stories.map((story) => [story.id, story]));
            for (let pageNumber = 2; pageNumber <= first.page.totalPages; pageNumber += 1) {
                const next = await ListStories({
                    page: pageNumber,
                    pageSize: 100,
                    sort: 'imported_desc',
                });
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
    const importing = activeOperations.length > 0;
    const empty = page !== null && page.totalItems === 0 && !importing;

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
                <div className="facet">
                    <div className="facet-title">Age <b>−</b></div>
                    <label className="choice">
                        <input type="checkbox" disabled/>
                        <i style={{'--c': '#ff705c'} as CSSProperties}/>
                        3–5 years
                        <span>0</span>
                    </label>
                    <label className="choice">
                        <input type="checkbox" disabled/>
                        <i style={{'--c': '#ff705c'} as CSSProperties}/>
                        6–8 years
                        <span>0</span>
                    </label>
                </div>
                <div className="facet">
                    <div className="facet-title">Language <b>＋</b></div>
                </div>
                <div className="facet">
                    <div className="facet-title">Import status <b>＋</b></div>
                </div>
                <button className="manage" type="button">＋ Manage tags</button>
            </aside>

            <main className="main">
                <div className="top">
                    <label className="search">
                        <span aria-hidden="true">⌕</span>
                        <input
                            type="search"
                            aria-label="Search stories"
                            placeholder={`Search ${page?.totalItems ?? 0} stories, authors, publishers…`}
                        />
                    </label>
                    <button type="button">↻ Sync</button>
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
                    <div className="sort">Recently added ⌄</div>
                </div>

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

                {visibleOperations.map((operation) => {
                    const importNotice = describeImport(operation);
                    const operationActive = operationIsActive(operation);
                    return (
                        <section
                            className={`collection-state ${importNotice.tone}`}
                            data-state={importNotice.state}
                            aria-live="polite"
                            key={operation.id}
                        >
                            <div className="state-mark" aria-hidden="true">
                                {importNotice.tone === 'success'
                                    ? '✓'
                                    : importNotice.tone === 'working'
                                        ? '↓'
                                        : '!'}
                            </div>
                            <div className="state-copy">
                                <h3>{importNotice.title}</h3>
                                <p>{importNotice.message}</p>
                                {operation.items.length > 0 && !operationActive && (
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
                                    onClick={() => void cancelImport(operation)}
                                >
                                    Cancel import
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
                                    className={`story${story.id === selectedID ? ' selected' : ''}`}
                                    style={paletteFor(story.id)}
                                    type="button"
                                    key={story.id}
                                    aria-pressed={story.id === selectedID}
                                    onClick={() => {
                                        setDetail(null);
                                        setSelectedID(story.id);
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
                            My tags
                            <button type="button">Edit tags</button>
                        </div>
                        <div className="tags"/>
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
        </div>
    );
}

export default App;
