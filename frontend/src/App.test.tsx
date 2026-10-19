import {setLanguage} from './i18n';
import {act, fireEvent, render, screen, waitFor, within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {
    ActiveOperations,
    ApplicationStatus,
    LibraryCompatibility,
    SetStoriesBroken,
    RestoreStoryFlags,
    CancelOperation,
    CreateCollection,
    DeleteCollection,
    ApplyCollectionDeletion,
    DuplicateCollection,
    ListCollections,
    ListShelves,
    OpenShelf,
    AddStoriesToShelf,
    OfficialMetadataStatus,
    OpenCollection,
    OperationHistory,
    OperationSnapshot,
    PackagedAcceptanceMode,
    PreviewCollections,
    QueryStories,
    RecordPackagedAcceptance,
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
    SetBooleanTag,
    StoryDetail as LoadStoryDetail,
    TagAssignmentWorkspace as LoadTagAssignmentWorkspace,
    TagCatalog as LoadTagCatalog,
} from '../wailsjs/go/main/App';
import {app, exporter, library, metadata, collections} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import App from './App';
import {
    DEFAULT_COLLECTION_QUERY,
    encodeCollectionQuery,
} from './query-codec';

vi.mock('../wailsjs/go/main/App', () => ({
    ActiveOperations: vi.fn(),
    ApplicationStatus: vi.fn(),
    SetStoriesBroken: vi.fn(),
    RestoreStoryFlags: vi.fn(),
    LibraryCompatibility: vi.fn(async () => ({compatibility: {legacyBrokenTagId: 0}})),
    CancelOperation: vi.fn(),
    CreateCollection: vi.fn(),
    DeleteCollection: vi.fn(),
    DuplicateCollection: vi.fn(),
    PreviewCollectionDeletion: vi.fn(async (id) => ({review: {token: String(id), shelves: []}})),
    ApplyCollectionDeletion: vi.fn(),
    ListCollections: vi.fn(),
    ListShelves: vi.fn(async () => ({shelves: []})),
    OpenShelf: vi.fn(),
    AddStoriesToShelf: vi.fn(),
    OfficialMetadataStatus: vi.fn(),
    OpenCollection: vi.fn(),
    OperationHistory: vi.fn(),
    OperationSnapshot: vi.fn(),
    PackagedAcceptanceMode: vi.fn(),
    PreviewCollections: vi.fn(),
    QueryStories: vi.fn(),
    RecordPackagedAcceptance: vi.fn(),
    RecoverStorage: vi.fn(),
    RevealExportDestination: vi.fn(),
    RenameCollection: vi.fn(),
    ReorderCollections: vi.fn(),
    ReplaceCollectionQuery: vi.fn(),
    RefreshOfficialMetadata: vi.fn(),
    RemoveStory: vi.fn(),
    SelectAndImportStories: vi.fn(),
    SelectAndPreflightExport: vi.fn(),
    StartPreparedExport: vi.fn(),
    StoryDetail: vi.fn(),
    TagAssignmentWorkspace: vi.fn(),
    TagCatalog: vi.fn(),
    SetBooleanTag: vi.fn(),
    SetChoiceTagValue: vi.fn(),
}));

vi.mock('../wailsjs/runtime/runtime', () => ({
    EventsOn: vi.fn(),
}));

const activeOperations = vi.mocked(ActiveOperations);
const applicationStatus = vi.mocked(ApplicationStatus);
const cancelOperation = vi.mocked(CancelOperation);
const createCollection = vi.mocked(CreateCollection);
const deleteCollection = vi.mocked(DeleteCollection);
const duplicateCollection = vi.mocked(DuplicateCollection);
const listCollections = vi.mocked(ListCollections);
const officialMetadataStatus = vi.mocked(OfficialMetadataStatus);
const openCollection = vi.mocked(OpenCollection);
const operationHistory = vi.mocked(OperationHistory);
const operationSnapshot = vi.mocked(OperationSnapshot);
const packagedAcceptanceMode = vi.mocked(PackagedAcceptanceMode);
const previewCollections = vi.mocked(PreviewCollections);
const queryStories = vi.mocked(QueryStories);
const recordPackagedAcceptance = vi.mocked(RecordPackagedAcceptance);
const recoverStorage = vi.mocked(RecoverStorage);
const revealExportDestination = vi.mocked(RevealExportDestination);
const renameCollection = vi.mocked(RenameCollection);
const reorderCollections = vi.mocked(ReorderCollections);
const replaceCollectionQuery = vi.mocked(ReplaceCollectionQuery);
const refreshOfficialMetadata = vi.mocked(RefreshOfficialMetadata);
const removeStory = vi.mocked(RemoveStory);
const selectAndImportStories = vi.mocked(SelectAndImportStories);
const selectAndPreflightExport = vi.mocked(SelectAndPreflightExport);
const startPreparedExport = vi.mocked(StartPreparedExport);
const setBooleanTag = vi.mocked(SetBooleanTag);
const loadStoryDetail = vi.mocked(LoadStoryDetail);
const loadTagAssignmentWorkspace = vi.mocked(LoadTagAssignmentWorkspace);
const loadTagCatalog = vi.mocked(LoadTagCatalog);
const eventsOn = vi.mocked(EventsOn);
let operationChanged: ((value: unknown) => void) | undefined;

const stories = [
    new library.StorySummary({
        id: 1,
        uuid: '00112233-4455-4677-8899-aabbccddeeff',
        title: 'Clockwork Forest',
        author: 'Lunii',
        sources: {
            title: 'official',
            description: 'fallback',
            author: 'official',
            artwork: 'fallback',
        },
        detectedFormat: 'zip',
        compatibility: 'compatible',
        byteSize: 1048576,
        importedAt: '2026-07-25T09:00:00Z',
    }),
    new library.StorySummary({
        id: 2,
        uuid: '11112222-3333-4444-8555-666677778888',
        title: 'Moonlit Workshop',
        sources: {
            title: 'embedded',
            description: 'fallback',
            author: 'fallback',
            artwork: 'fallback',
        },
        detectedFormat: '7z',
        compatibility: 'compatible',
        byteSize: 2097152,
        importedAt: '2026-07-25T08:00:00Z',
    }),
];

beforeEach(() => {
    setLanguage('en');
    window.localStorage.clear();
    vi.mocked(SetStoriesBroken).mockReset();
    vi.mocked(RestoreStoryFlags).mockReset();
    vi.mocked(LibraryCompatibility).mockResolvedValue(new app.LibraryCompatibilityResponse({compatibility: {legacyBrokenTagId: 0}}));
    window.history.replaceState(null, '', '/');
    activeOperations.mockReset();
    applicationStatus.mockReset();
    cancelOperation.mockReset();
    createCollection.mockReset();
    vi.mocked(ListShelves).mockResolvedValue(new app.ShelfListResponse({shelves: []}));
    deleteCollection.mockReset();
    vi.mocked(ApplyCollectionDeletion).mockImplementation(async (token) => DeleteCollection(Number(token)));
    duplicateCollection.mockReset();
    listCollections.mockReset();
    officialMetadataStatus.mockReset();
    openCollection.mockReset();
    operationHistory.mockReset();
    operationSnapshot.mockReset();
    packagedAcceptanceMode.mockReset();
    previewCollections.mockReset();
    queryStories.mockReset();
    recordPackagedAcceptance.mockReset();
    recoverStorage.mockReset();
    revealExportDestination.mockReset();
    renameCollection.mockReset();
    reorderCollections.mockReset();
    replaceCollectionQuery.mockReset();
    refreshOfficialMetadata.mockReset();
    removeStory.mockReset();
    selectAndImportStories.mockReset();
    selectAndPreflightExport.mockReset();
    startPreparedExport.mockReset();
    setBooleanTag.mockReset();
    loadStoryDetail.mockReset();
    loadTagAssignmentWorkspace.mockReset();
    loadTagCatalog.mockReset();
    eventsOn.mockReset();
    operationChanged = undefined;

    packagedAcceptanceMode.mockResolvedValue(false);
    recordPackagedAcceptance.mockResolvedValue(new app.MutationResponse({
        success: true,
    }));
    applicationStatus.mockResolvedValue(new app.StatusResponse({
        status: {
            state: 'ready',
            mutationsAllowed: true,
        },
    }));
    activeOperations.mockResolvedValue(new app.OperationListResponse({
        operations: [],
    }));
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [],
    }));
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {
            state: 'never_synced',
            locale: 'en-GB',
            matchedStoryCount: 0,
        },
    }));
    operationHistory.mockResolvedValue(new app.OperationHistoryResponse({page: {operations: []}}));
    operationSnapshot.mockResolvedValue(new app.OperationResponse({}));
    revealExportDestination.mockResolvedValue(new app.MutationResponse({
        success: true,
    }));
    previewCollections.mockResolvedValue(new app.CollectionSelectionPreviewResponse({}));
    queryStories.mockResolvedValue(new app.LibraryPageResponse({
        page: {
            stories,
            page: 1,
            pageSize: 12,
            totalItems: 2,
            totalPages: 1,
            sort: 'imported_desc',
        },
    }));
    loadStoryDetail.mockImplementation(async (storyID) => {
        const story = stories.find((candidate) => candidate.id === storyID) ?? stories[0];
        return new app.StoryDetailResponse({
            detail: {
                story,
                archive: {
                    originalFilename: `${story.title.toLowerCase().replaceAll(' ', '-')}.zip`,
                    detectedFormat: story.detectedFormat,
                    sha256: 'a'.repeat(64),
                    byteSize: story.byteSize,
                    verification: 'compatible',
                },
            },
        });
    });
    loadTagAssignmentWorkspace.mockImplementation(async (storyIDs) => (
        new app.TagAssignmentWorkspaceResponse({
            workspace: {
                catalog: {definitions: []},
                requestedStories: storyIDs.length,
                states: [],
            },
        })
    ));
    loadTagCatalog.mockResolvedValue(new app.TagCatalogResponse({
        catalog: {
            definitions: [{
                id: 1,
                key: 'favorite',
                normalizedKey: 'favorite',
                label: 'Favorite',
                color: '#ff705c',
                kind: 'boolean',
                source: 'user',
                presentation: 'warning',
                position: 0,
                protected: true,
                values: [],
            }, {
                id: 2,
                key: 'mood',
                normalizedKey: 'mood',
                label: 'Mood',
                color: '#405cf5',
                kind: 'choice',
                source: 'user',
                presentation: 'default',
                position: 0,
                protected: false,
                values: [{
                    id: 20,
                    definitionId: 2,
                    key: 'calm',
                    normalizedKey: 'calm',
                    label: 'Calm',
                    position: 0,
                }, {
                    id: 21,
                    definitionId: 2,
                    key: 'bold',
                    normalizedKey: 'bold',
                    label: 'Bold',
                    position: 1,
                }],
            }],
        },
    }));
    eventsOn.mockImplementation((_name, callback) => {
        operationChanged = callback;
        return vi.fn();
    });
    selectAndImportStories.mockResolvedValue(new app.OperationResponse({
        cancelled: true,
    }));
    selectAndPreflightExport.mockResolvedValue(new app.ExportPreflightResponse({
        cancelled: true,
    }));
    startPreparedExport.mockResolvedValue(new app.OperationResponse({}));
    refreshOfficialMetadata.mockResolvedValue(new app.OperationResponse({
        cancelled: true,
    }));
    cancelOperation.mockResolvedValue(new app.OperationResponse({
        cancelled: true,
    }));
    setBooleanTag.mockResolvedValue(new app.TagAssignmentResponse({
        result: {
            requestedStories: 1,
            changedStories: 1,
            assignmentsAdded: 1,
            assignmentsRemoved: 0,
        },
    }));
    removeStory.mockResolvedValue(new app.RemovalResponse({
        result: {
            storyId: 1,
            uuid: stories[0].uuid,
        },
    }));
});

test('switches story views without losing the query or selection and remembers the choice', async () => {
    const user = userEvent.setup();
    const {unmount} = render(<App/>);
    const story = await screen.findByRole('button', {name: 'Clockwork Forest'});
    await user.click(story);
    const query = window.location.hash;
    const queries = queryStories.mock.calls.length;
    const views = screen.getByRole('group', {name: 'Story view'});
    expect(within(views).getByRole('button', {name: 'Large covers'})).toHaveAttribute('aria-pressed', 'true');
    await user.click(within(views).getByRole('button', {name: 'Small covers'}));
    expect(story.closest('.app')).toHaveClass('view-small');
    await user.click(within(views).getByRole('button', {name: 'List'}));
    expect(story).toHaveAttribute('aria-pressed', 'true');
    expect(story.querySelector('.cover')).toBeNull();
    expect(within(story).getByText('Lunii')).toBeInTheDocument();
    expect(window.location.hash).toBe(query);
    expect(queryStories.mock.calls.length).toBe(queries);
    unmount();
    render(<App/>);
    await screen.findByRole('button', {name: 'Clockwork Forest'});
    expect(screen.getByRole('button', {name: 'List'})).toHaveAttribute('aria-pressed', 'true');
    expect(document.querySelector('.app')).toHaveClass('view-list');
});

test('ignores unknown stored story views and works without preference storage', async () => {
    window.localStorage.setItem('librairii.library-view', 'unknown');
    const {unmount} = render(<App/>);
    await screen.findByRole('button', {name: 'Clockwork Forest'});
    expect(screen.getByRole('button', {name: 'Large covers'})).toHaveAttribute('aria-pressed', 'true');
    unmount();
    const get = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('unavailable'); });
    const set = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('unavailable'); });
    try {
        render(<App/>);
        await screen.findByRole('button', {name: 'Clockwork Forest'});
        await userEvent.click(screen.getByRole('button', {name: 'List'}));
        expect(document.querySelector('.app')).toHaveClass('view-list');
    } finally {
        get.mockRestore();
        set.mockRestore();
    }
});

test('renders the canonical collection shell from typed library data', async () => {
    render(<App/>);

    expect(await screen.findByRole('heading', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(screen.getByRole('heading', {name: 'My story shelves'})).toBeInTheDocument();
    expect(screen.getByRole('navigation', {name: 'Primary navigation'})).toBeInTheDocument();
    expect(screen.getByText('Stories grouped by your tags · 2 archives'))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Clockwork Forest'}))
        .toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', {name: 'Open details'})).toBeEnabled();
});

test('uses the canonical state grammar while the local library opens', async () => {
    let resolveStatus!: (response: app.StatusResponse) => void;
    applicationStatus.mockImplementation(() => new Promise((resolve) => {
        resolveStatus = resolve;
    }));
    render(<App/>);

    const loading = screen.getByText('Opening your local library')
        .closest('[data-state="loading-library"]');
    expect(loading).not.toBeNull();
    expect(loading).toHaveClass('collection-state', 'working');

    resolveStatus(new app.StatusResponse({
        status: {
            state: 'ready',
            mutationsAllowed: true,
        },
    }));
    expect(await screen.findByRole('heading', {name: 'Clockwork Forest'}))
        .toBeInTheDocument();
    expect(screen.queryByText('Opening your local library'))
        .not.toBeInTheDocument();
});

test('offers explicit preservation when an incompatible database blocks startup', async () => {
    const user = userEvent.setup();
    applicationStatus.mockResolvedValue(new app.StatusResponse({
        status: {
            state: 'recovery',
            mutationsAllowed: false,
            recoveryAvailable: true,
        },
        error: {
            code: 'storage_unavailable',
            message: 'An incompatible database must be preserved.',
        },
    }));
    recoverStorage.mockResolvedValue(new app.MutationResponse({
        error: {
            code: 'storage_unavailable',
            message: 'The incompatible database could not be preserved.',
        },
    }));

    render(<App/>);

    const recovery = await screen.findByRole('alert');
    expect(within(recovery).getByRole('heading', {
        name: 'Local library needs recovery',
    })).toBeInTheDocument();
    expect(within(recovery).getByText(
        'An incompatible database must be preserved.',
    )).toBeInTheDocument();
    await user.click(within(recovery).getByRole('button', {
        name: 'Preserve database and create new library',
    }));
    expect(recoverStorage).toHaveBeenCalledOnce();
    expect(await within(recovery).findByText(
        'The incompatible database could not be preserved.',
    )).toBeInTheDocument();
});

test('offers a prominent first Lunii sync with an explanation and no prior sync time', async () => {
    render(<App/>);
    const sync = await screen.findByRole('region', {name: 'Official Lunii sync'});
    expect(within(sync).getByRole('button', {name: 'Sync with Lunii'})).toBeEnabled();
    expect(sync).toHaveTextContent('official Lunii API');
    expect(sync).toHaveTextContent('descriptions and cover images');
    expect(sync).toHaveTextContent('Last successful sync: Never');
    expect(sync).toHaveAttribute('data-state', 'first-lunii-sync');
});

test('keeps the last successful Lunii sync visible during later operations', async () => {
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {state: 'fresh', activatedAt: '2026-07-25T16:00:00Z', matchedStoryCount: 2},
    }));
    render(<App/>);
    const sync = await screen.findByRole('region', {name: 'Official Lunii sync'});
    expect(within(sync).getByText('Last successful sync:', {exact: false})).toBeInTheDocument();
    expect(sync.querySelector('time')).toHaveAttribute('dateTime', '2026-07-25T16:00:00Z');
    operationChanged?.({
        id: 'import-activity', kind: 'import', status: 'running',
        completedItems: 0, totalItems: 1, items: [],
    });
    expect(sync.querySelector('time')).toHaveAttribute('dateTime', '2026-07-25T16:00:00Z');
});

test('prevents duplicate Lunii sync requests while starting and offers retry after failure', async () => {
    const user = userEvent.setup();
    let rejectSync: ((reason?: unknown) => void) | undefined;
    refreshOfficialMetadata.mockImplementationOnce(() => new Promise((_, reject) => {
        rejectSync = reject;
    }));
    render(<App/>);
    const sync = await screen.findByRole('region', {name: 'Official Lunii sync'});
    await user.click(within(sync).getByRole('button', {name: 'Sync with Lunii'}));
    const starting = within(sync).getByRole('button', {name: 'Syncing with Lunii…'});
    expect(starting).toBeDisabled();
    await user.click(starting);
    expect(refreshOfficialMetadata).toHaveBeenCalledTimes(1);
    rejectSync?.(new Error('Disconnected'));
    expect(await screen.findByText('Could not start the Lunii sync. Please try again.'))
        .toBeInTheDocument();
    expect(within(sync).getByRole('button', {name: 'Sync with Lunii'})).toBeEnabled();
    expect(sync).toHaveTextContent('Last successful sync: Never');
});

test('keeps artwork links free of overlaid titles and author captions', async () => {
    queryStories.mockResolvedValue(new app.LibraryPageResponse({
        page: {stories: [{...stories[0], artworkId: 'forest-cover'}],
            page: 1, pageSize: 12, totalItems: 1, totalPages: 1, sort: 'imported_desc'},
    }));
    render(<App/>);
    const cover = await screen.findByRole('button', {name: 'Clockwork Forest'});
    expect(cover.querySelector('img')).toHaveAttribute('src', '/artwork/forest-cover');
    expect(cover).not.toHaveTextContent('Clockwork Forest');
    expect(cover).not.toHaveTextContent('Lunii');
    expect(cover.parentElement?.querySelector('.story-title')).toHaveTextContent('Clockwork Forest');
    expect(cover.parentElement?.querySelector('small')).toBeNull();
    fireEvent.error(cover.querySelector('img')!);
    expect(cover.querySelector('img')).toBeNull();
    expect(cover).toBeEnabled();
    expect(cover.parentElement?.querySelector('.story-title')).toHaveTextContent('Clockwork Forest');
});

test('makes the rail shortcuts visible and functional', async () => {
    const user = userEvent.setup();
    render(<App/>);
    await screen.findByRole('heading', {name: 'Clockwork Forest'});
    const rail = screen.getByRole('navigation', {name: 'Primary navigation'});
    expect(within(rail).getAllByRole('button').map((button) => button.textContent))
        .toEqual(['Library', 'Tags', 'Import', 'Export']);
    await user.type(screen.getByRole('searchbox'), 'moon');
    await user.click(within(rail).getByRole('button', {name: 'Library'}));
    expect(screen.getByRole('searchbox')).toHaveValue('');
    await user.click(within(rail).getByRole('button', {name: 'Tags'}));
    expect(await screen.findByRole('dialog', {name: 'Manage tags'})).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await user.click(within(rail).getByRole('button', {name: 'Import'}));
    expect(selectAndImportStories).toHaveBeenCalledTimes(1);
    await user.click(within(rail).getByRole('button', {name: 'Export current result'}));
    expect(selectAndPreflightExport).toHaveBeenCalledWith(expect.objectContaining({sourceType: 'current_query'}));
});

test('keeps interactive controls in their canonical regions', async () => {
    const user = userEvent.setup();
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {
            state: 'fresh',
            locale: 'en-GB',
            matchedStoryCount: 2,
            activatedAt: '2026-07-25T16:00:00Z',
        },
    }));
    render(<App/>);

    const rail = screen.getByRole('navigation', {name: 'Primary navigation'});
    const sidebar = screen.getByRole('complementary', {
        name: "Saved collections and refinements",
    });
    const main = screen.getByRole('main');
    const drawer = await screen.findByRole('complementary', {
        name: 'Clockwork Forest details',
    });

    expect(within(rail).getByRole('button', {name: 'Export current result'}))
        .toBeInTheDocument();
    expect(within(sidebar).getByRole('button', {name: '＋ Manage tags'}))
        .toBeInTheDocument();
    expect(within(main).getByRole('searchbox', {name: 'Search stories'}))
        .toBeInTheDocument();
    expect(within(main).getByRole('button', {name: 'Sync with Lunii'}))
        .toBeInTheDocument();
    expect(within(main).queryByRole('button', {name: '＋ Import stories'}))
        .not.toBeInTheDocument();
    expect(within(drawer).queryByRole('button', {name: 'Edit tags'}))
        .not.toBeInTheDocument();
    expect(within(drawer).getByRole('button', {name: 'Open details'}))
        .toBeInTheDocument();
    expect(within(drawer).queryByRole('button', {name: /export/i}))
        .not.toBeInTheDocument();

    const language = within(sidebar).getByRole('button', {name: 'Language ＋'});
    expect(within(sidebar).queryByRole('checkbox', {name: 'English'}))
        .not.toBeInTheDocument();
    await user.click(language);
    expect(language).toHaveAttribute('aria-expanded', 'true');
    expect(within(sidebar).getByRole('checkbox', {name: 'English'}))
        .toBeInTheDocument();
});

test('preflights explicit selections and complete current results', async () => {
    const user = userEvent.setup();
    selectAndPreflightExport.mockImplementation(async (request) => (
        new app.ExportPreflightResponse({
            preflight: new exporter.PreflightReport({
                preparationId: request.sourceType === 'selection'
                    ? 'selection-preflight'
                    : 'current-preflight',
                source: {type: request.sourceType},
                destination: 'Lunii export',
                resolvedCount: request.sourceType === 'selection' ? 2 : 12,
                readyCount: request.sourceType === 'selection' ? 1 : 12,
                totalBytes: 1048576,
                detectedFormats: ['seven_zip', 'zip'],
                collapsedOverlap: 0,
                items: request.sourceType === 'selection'
                    ? [{
                        storyId: 1,
                        storyUuid: stories[0].uuid,
                        storyTitle: 'Clockwork Forest',
                        outputName: 'clockwork-forest.zip',
                        detectedFormat: 'zip',
                        byteSize: 1048576,
                        disposition: 'ready',
                    }, {
                        storyId: 2,
                        storyUuid: stories[1].uuid,
                        storyTitle: 'Moonlit Workshop',
                        outputName: 'moonlit-workshop.7z',
                        detectedFormat: 'seven_zip',
                        byteSize: 2097152,
                        disposition: 'skipped',
                        issue: {
                            code: 'archive_missing',
                            message: 'Managed archive bytes are missing.',
                            blocks: false,
                        },
                    }]
                    : [],
                issues: [],
                partial: request.sourceType === 'selection',
                blocked: false,
                canExport: true,
            }),
        })
    ));
    startPreparedExport.mockResolvedValue(new app.OperationResponse({
        operation: {
            id: 'export-operation',
            kind: 'export',
            status: 'queued',
            exportSourceType: 'selection',
            completedItems: 0,
            totalItems: 2,
            totalBytes: 3145728,
            cancelRequested: false,
            createdAt: '2026-07-25T20:00:00Z',
            items: [{
                id: 1,
                storyId: 1,
                sourceName: 'clockwork-forest.zip',
                outputName: 'clockwork-forest.zip',
                status: 'pending',
                completedBytes: 0,
                totalBytes: 1048576,
            }, {
                id: 2,
                storyId: 2,
                sourceName: 'moonlit-workshop.7z',
                outputName: 'moonlit-workshop.7z',
                status: 'pending',
                completedBytes: 0,
                totalBytes: 2097152,
            }],
        },
    }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {
        name: 'Export current result',
    }));
    expect(selectAndPreflightExport).toHaveBeenLastCalledWith(
        expect.objectContaining({
            sourceType: 'current_query',
            query: expect.objectContaining({
                page: 1,
                pageSize: 12,
                sort: 'imported_desc',
            }),
        }),
    );
    expect(screen.getByRole('dialog', {name: 'Ready to export'}))
        .toBeInTheDocument();
    expect(screen.getByText('12 of 12 resolved stories are ready for byte-preserving export.'))
        .toBeInTheDocument();
    expect(screen.getByText('Lunii export')).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Close'}));

    fireEvent.click(screen.getByRole('button', {name: /^Moonlit Workshop$/}), {
        ctrlKey: true,
    });
    await user.click(screen.getByRole('button', {name: 'Export 2 selected stories →'}));
    expect(selectAndPreflightExport).toHaveBeenLastCalledWith(
        expect.objectContaining({
            sourceType: 'selection',
            storyIds: [1, 2],
        }),
    );
    expect(screen.getByRole('dialog', {name: 'Review partial export'}))
        .toBeInTheDocument();
    expect(screen.getByRole('list', {name: 'Export story checks'}))
        .toHaveTextContent('Managed archive bytes are missing.');
    await user.click(screen.getByRole('button', {name: 'Start export'}));
    expect(startPreparedExport).toHaveBeenCalledWith('selection-preflight');
    expect(await screen.findByRole('heading', {name: 'Export queued'}))
        .toBeInTheDocument();
});

test('renders empty and blocked export preflight guidance', async () => {
    const user = userEvent.setup();
    selectAndPreflightExport.mockResolvedValue(new app.ExportPreflightResponse({
        preflight: {
            source: {type: 'current_query'},
            destination: '',
            resolvedCount: 0,
            readyCount: 0,
            totalBytes: 0,
            detectedFormats: [],
            collapsedOverlap: 0,
            items: [],
            issues: [{
                code: 'empty_scope',
                message: "No stories match this export scope. Select stories, adjust the search or filters, or choose a collection with matching stories, then try again.",
                blocks: true,
            }],
            partial: false,
            blocked: true,
            canExport: false,
        },
    }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {
        name: 'Export current result',
    }));
    expect(screen.getByRole('dialog', {name: 'Export is blocked'}))
        .toBeInTheDocument();
    expect(screen.getByRole('list', {name: 'Export blockers'}))
        .toHaveTextContent(
            'Select stories, adjust the search or filters, or choose a collection with matching stories, then try again.',
        );
    expect(screen.getByRole('button', {name: 'Export unavailable'})).toBeDisabled();
});

test.each(['current_query', 'selection'] as const)(
    'retries a blocked %s export with the same scope and a new folder',
    async (sourceType) => {
        const user = userEvent.setup();
        const blocked = new exporter.PreflightReport({
            source: {type: sourceType},
            destination: 'Existing archives',
            resolvedCount: 2,
            readyCount: 0,
            totalBytes: 0,
            detectedFormats: ['zip'],
            collapsedOverlap: 0,
            items: [],
            issues: [{
                code: 'no_exportable_stories',
                message: 'Every story in this scope has a filename conflict. Choose another destination folder or resolve the duplicate filenames, then try again.',
                blocks: true,
            }],
            partial: true,
            blocked: true,
            canExport: false,
        });
        selectAndPreflightExport
            .mockResolvedValueOnce(new app.ExportPreflightResponse({preflight: blocked}))
            .mockResolvedValueOnce(new app.ExportPreflightResponse({cancelled: true}))
            .mockRejectedValueOnce(new Error('picker unavailable'))
            .mockResolvedValueOnce(new app.ExportPreflightResponse({
                error: {code: 'internal', message: 'The export destination picker could not be opened.'},
            }))
            .mockResolvedValueOnce(new app.ExportPreflightResponse({
                preflight: new exporter.PreflightReport({
                    ...blocked,
                    preparationId: 'new-folder-preflight',
                    destination: 'Empty export folder',
                    readyCount: 2,
                    partial: false,
                    blocked: false,
                    canExport: true,
                    issues: [],
                }),
            }));
        startPreparedExport.mockResolvedValue(new app.OperationResponse({}));
        render(<App/>);

        await screen.findByRole('complementary', {name: 'Clockwork Forest details'});
        if (sourceType === 'selection') {
            fireEvent.click(screen.getByRole('button', {name: /^Moonlit Workshop$/}), {
                ctrlKey: true,
            });
            await user.click(screen.getByRole('button', {name: 'Export 2 selected stories →'}));
        } else {
            await user.click(screen.getByRole('button', {name: 'Export current result'}));
        }
        const originalRequest = selectAndPreflightExport.mock.calls[0][0];
        expect(originalRequest.sourceType).toBe(sourceType);
        expect(screen.getByRole('button', {name: 'Export unavailable'})).toBeDisabled();
        expect(startPreparedExport).not.toHaveBeenCalled();

        await user.click(screen.getByRole('button', {name: 'Choose another folder'}));
        expect(screen.getByRole('dialog', {name: 'Export is blocked'})).toBeInTheDocument();
        expect(screen.getByText('Existing archives')).toBeInTheDocument();
        await user.click(screen.getByRole('button', {name: 'Choose another folder'}));
        expect(within(screen.getByRole('dialog')).getByRole('alert'))
            .toHaveTextContent('The export could not be prepared.');
        await user.click(screen.getByRole('button', {name: 'Choose another folder'}));
        expect(within(screen.getByRole('dialog')).getByRole('alert'))
            .toHaveTextContent('The export destination picker could not be opened.');
        await user.click(screen.getByRole('button', {name: 'Choose another folder'}));
        expect(selectAndPreflightExport).toHaveBeenCalledTimes(5);
        for (const [request] of selectAndPreflightExport.mock.calls) {
            expect(request).toEqual(originalRequest);
        }
        expect(screen.getByRole('dialog', {name: 'Ready to export'})).toBeInTheDocument();
        expect(screen.getByText('Empty export folder')).toBeInTheDocument();
        await user.click(screen.getByRole('button', {name: 'Start export'}));
        expect(startPreparedExport).toHaveBeenCalledWith('new-folder-preflight');
    },
);

test('renders saved collection preview rows and opens them from view all', async () => {
    const user = userEvent.setup();
    const summaries = [
        new collections.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        }),
        new collections.Summary({
            id: 8,
            name: 'Adventures',
            position: 1,
            validity: 'valid',
            count: 1,
        }),
    ];
    listCollections.mockResolvedValue(new app.CollectionListResponse({collections: summaries}));
    openCollection.mockImplementation(async (collectionID) => {
        const bedtime = collectionID === 7;
        return new app.CollectionEvaluationResponse({
            evaluation: {
                collection: {
                    id: collectionID,
                    name: bedtime ? 'Bedtime' : 'Adventures',
                    normalizedName: bedtime ? 'bedtime' : 'adventures',
                    position: bedtime ? 0 : 1,
                    queryVersion: 2,
                    queryPayload: bedtime
                        ? '{"name":"moon"}'
                        : '{"name":"forest"}',
                    validity: 'valid',
                },
                query: {name: bedtime ? 'moon' : 'forest'},
                page: {
                    stories: [bedtime ? stories[1] : stories[0]],
                    page: 1,
                    pageSize: 6,
                    totalItems: 1,
                    totalPages: 1,
                    sort: 'imported_desc',
                },
            },
        });
    });

    render(<App/>);

    const bedtimeRow = (await screen.findByRole('heading', {name: 'Bedtime'}))
        .closest('section');
    const adventuresRow = screen.getByRole('heading', {name: 'Adventures'})
        .closest('section');
    expect(bedtimeRow).not.toBeNull();
    expect(adventuresRow).not.toBeNull();
    expect(within(bedtimeRow!).getByText('1 story · Saved collection'))
        .toBeInTheDocument();
    expect(within(bedtimeRow!).getByRole('button', {
        name: 'View all stories in Bedtime',
    })).toBeInTheDocument();
    expect(within(adventuresRow!).getByRole('button', {
        name: 'View all stories in Adventures',
    })).toBeInTheDocument();
    await waitFor(() => expect(within(adventuresRow!).getByText('Clockwork Forest'))
        .toBeInTheDocument());
    await waitFor(() => expect(openCollection).toHaveBeenCalledWith(
        7,
        expect.objectContaining({page: 1, pageSize: 6, sort: 'imported_desc'}),
    ));

    await user.click(within(bedtimeRow!).getByRole('button', {
        name: 'View all stories in Bedtime',
    }));
    await waitFor(() => expect(openCollection).toHaveBeenCalledWith(
        7,
        expect.objectContaining({page: 1, pageSize: 12, sort: 'imported_desc'}),
    ));
    expect(screen.getByRole('button', {name: 'Bedtime, 1 story'}))
        .toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('moon');
});

test('keeps stories outside saved collection searches available in All stories', async () => {
    const user = userEvent.setup();
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [new collections.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        })],
    }));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        evaluation: {
            collection: {
                id: 7,
                name: 'Bedtime',
                normalizedName: 'bedtime',
                position: 0,
                queryVersion: 2,
                queryPayload: '{"name":"moon"}',
                validity: 'valid',
            },
            query: {name: 'moon'},
            page: {
                stories: [stories[1]],
                page: 1,
                pageSize: 6,
                totalItems: 1,
                totalPages: 1,
                sort: 'imported_desc',
            },
        },
    }));

    const view = render(<App/>);

    const bedtimeRow = (await screen.findByRole('heading', {name: 'Bedtime'}))
        .closest('section');
    expect(bedtimeRow).not.toBeNull();
    const visibleStory = await within(bedtimeRow!).findByRole('button', {
        name: /^Moonlit Workshop$/,
    });
    expect(visibleStory).toBeInTheDocument();
    const allStoriesRow = screen.getByRole('heading', {name: 'Recently added'}).closest('section')!;
    expect(within(allStoriesRow).getByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(within(allStoriesRow).getByRole('button', {name: 'Moonlit Workshop'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Bedtime, 1 story'}));
    expect(screen.getByRole('searchbox', {name: 'Search stories'})).toHaveValue('moon');
    await user.click(screen.getByRole('button', {name: /All stories/}));
    expect(screen.getByRole('searchbox', {name: 'Search stories'})).toHaveValue('');
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(expect.objectContaining({
        name: '', languages: [], compatibilities: [], booleanFilters: [], choiceFilters: [],
    })));
    view.unmount();
    render(<App/>);
    expect(await screen.findByRole('searchbox', {name: 'Search stories'})).toHaveValue('');
    const reloaded = (await screen.findByRole('heading', {name: 'Recently added'})).closest('section')!;
    expect(await within(reloaded).findByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
});

test('switches saved previews to filtered results and restores the custom URL', async () => {
    const user = userEvent.setup();
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [new collections.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        })],
    }));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        evaluation: {
            collection: {
                id: 7,
                name: 'Bedtime',
                normalizedName: 'bedtime',
                position: 0,
                queryVersion: 2,
                queryPayload: '{}',
                validity: 'valid',
            },
            query: {},
            page: {
                stories: [stories[1]],
                page: 1,
                pageSize: 6,
                totalItems: 1,
                totalPages: 1,
                sort: 'imported_desc',
            },
        },
    }));
    const allStoriesQuery = {
        ...DEFAULT_COLLECTION_QUERY,
        pageSize: 12,
        sort: 'imported_desc' as const,
        choiceFilters: [{definitionId: 2, valueIds: [20]}],
    };
    const allStoriesHash = encodeCollectionQuery(allStoriesQuery);
    window.history.replaceState({
        collectionQuery: {
            hash: allStoriesHash,
            collectionId: null,
            scope: 'all',
        },
    }, '', `/${allStoriesHash}`);

    const first = render(<App/>);
    expect(await screen.findByRole('heading', {name: 'Bedtime'}))
        .toBeInTheDocument();

    await user.selectOptions(
        screen.getByRole('combobox', {name: 'Sort stories'}),
        'name_asc',
    );
    await waitFor(() => expect(openCollection).toHaveBeenLastCalledWith(
        7,
        expect.objectContaining({sort: 'name_asc'}),
    ));
    expect(screen.getByRole('heading', {name: 'Bedtime'}))
        .toBeInTheDocument();

    await user.type(
        screen.getByRole('searchbox', {name: 'Search stories'}),
        'forest',
    );
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({name: 'forest'}),
    ));
    expect(screen.queryByRole('heading', {name: 'Bedtime'}))
        .not.toBeInTheDocument();
    expect(screen.getByRole('heading', {name: 'Recently added'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: /All stories/}))
        .not.toHaveAttribute('aria-current');

    await user.click(screen.getByRole('button', {name: 'Clear all'}));
    expect(await screen.findByRole('heading', {name: 'Bedtime'}))
        .toBeInTheDocument();
    expect(screen.getByRole('combobox', {name: 'Sort stories'}))
        .toHaveValue('name_asc');

    await user.type(
        screen.getByRole('searchbox', {name: 'Search stories'}),
        'forest',
    );
    first.unmount();
    render(<App/>);
    expect(await screen.findByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('forest');
    expect(screen.queryByRole('heading', {name: 'Bedtime'}))
        .not.toBeInTheDocument();
    expect(await screen.findByRole('heading', {name: 'Recently added'}))
        .toBeInTheDocument();
});

test('surfaces saved collection API and transport preview failures', async () => {
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [new collections.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 2,
        })],
    }));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        error: {
            code: 'internal',
            message: 'Bedtime preview is temporarily unavailable.',
        },
    }));

    const view = render(<App/>);

    const bedtimeRow = (await screen.findByRole('heading', {name: 'Bedtime'}))
        .closest('section');
    expect(bedtimeRow).not.toBeNull();
    expect(within(bedtimeRow!).getByText('2 stories · Saved collection'))
        .toBeInTheDocument();
    expect(await within(bedtimeRow!).findByRole('alert'))
        .toHaveTextContent('Bedtime preview is temporarily unavailable.');

    view.unmount();
    openCollection.mockRejectedValue(new Error('bridge unavailable'));
    render(<App/>);
    const rejectedRow = (await screen.findByRole('heading', {name: 'Bedtime'}))
        .closest('section');
    expect(rejectedRow).not.toBeNull();
    expect(await within(rejectedRow!).findByRole('alert'))
        .toHaveTextContent('Bedtime could not be previewed.');
});

test('opens a dynamic saved collection and updates it from the current query', async () => {
    const user = userEvent.setup();
    const summary = new collections.Summary({
        id: 7,
        name: 'Bedtime',
        position: 0,
        validity: 'valid',
        count: 1,
    });
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [summary],
    }));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        evaluation: {
            collection: {
                id: 7,
                name: 'Bedtime',
                normalizedName: 'bedtime',
                position: 0,
                queryVersion: 2,
                queryPayload: '{"name":"moon"}',
                validity: 'valid',
            },
            query: {name: 'moon'},
            page: {
                stories: [stories[1]],
                page: 1,
                pageSize: 12,
                totalItems: 1,
                totalPages: 1,
                sort: 'imported_desc',
            },
        },
    }));
    replaceCollectionQuery.mockResolvedValue(new app.CollectionResponse({
        collection: {
            id: 7,
            name: 'Bedtime',
            normalizedName: 'bedtime',
            position: 0,
            queryVersion: 2,
            queryPayload: '{"name":"moon"}',
            validity: 'valid',
        },
    }));

    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'Bedtime, 1 story'}));
    await waitFor(() => expect(openCollection).toHaveBeenCalledWith(
        7,
        expect.objectContaining({page: 1, pageSize: 12, sort: 'imported_desc'}),
    ));
    expect(screen.getByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('moon');
    expect(await screen.findByRole('heading', {name: 'Bedtime'})).toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Bedtime, 1 story'}))
        .toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', {name: /All stories/}))
        .not.toHaveAttribute('aria-current');

    await user.click(screen.getByRole('button', {name: '↻ Update “Bedtime”'}));
    await waitFor(() => expect(replaceCollectionQuery).toHaveBeenCalledWith(
        7,
        expect.objectContaining({name: 'moon', page: 1}),
    ));
});

test('restores active collection identity across back, forward, and reload', async () => {
    const user = userEvent.setup();
    const summaries = [
        new collections.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        }),
        new collections.Summary({
            id: 8,
            name: 'Adventures',
            position: 1,
            validity: 'valid',
            count: 1,
        }),
    ];
    listCollections.mockResolvedValue(new app.CollectionListResponse({collections: summaries}));
    openCollection.mockImplementation(async (collectionID) => {
        const bedtime = collectionID === 7;
        const name = bedtime ? 'moon' : 'forest';
        const collectionName = bedtime ? 'Bedtime' : 'Adventures';
        return new app.CollectionEvaluationResponse({
            evaluation: {
                collection: {
                    id: collectionID,
                    name: collectionName,
                    normalizedName: collectionName.toLowerCase(),
                    position: bedtime ? 0 : 1,
                    queryVersion: 2,
                    queryPayload: `{"name":"${name}"}`,
                    validity: 'valid',
                },
                query: {name},
                page: {
                    stories: [bedtime ? stories[1] : stories[0]],
                    page: 1,
                    pageSize: 12,
                    totalItems: 1,
                    totalPages: 1,
                    sort: 'imported_desc',
                },
            },
        });
    });

    const view = render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Bedtime, 1 story'}));
    expect(await screen.findByRole('heading', {name: 'Bedtime'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Adventures, 1 story'}));
    expect(await screen.findByRole('heading', {name: 'Adventures'})).toBeInTheDocument();

    window.history.back();
    await waitFor(() => {
        expect(screen.getByRole('heading', {name: 'Bedtime'})).toBeInTheDocument();
        expect(screen.getByRole('searchbox', {name: 'Search stories'}))
            .toHaveValue('moon');
    });
    window.history.forward();
    await waitFor(() => {
        expect(screen.getByRole('heading', {name: 'Adventures'})).toBeInTheDocument();
        expect(screen.getByRole('searchbox', {name: 'Search stories'}))
            .toHaveValue('forest');
    });

    view.unmount();
    render(<App/>);
    expect(await screen.findByRole('heading', {name: 'Adventures'}))
        .toBeInTheDocument();
    expect(screen.getByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('forest');
});

test('saves the current query as a named active collection with its live count', async () => {
    const user = userEvent.setup();
    const saved = new collections.Summary({
        id: 8,
        name: 'Moon collection',
        position: 0,
        validity: 'valid',
        count: 2,
    });
    listCollections
        .mockResolvedValueOnce(new app.CollectionListResponse({collections: []}))
        .mockResolvedValue(new app.CollectionListResponse({collections: [saved]}));
    createCollection.mockResolvedValue(new app.CollectionResponse({
        collection: {
            id: 8,
            name: 'Moon collection',
            normalizedName: 'moon collection',
            position: 0,
            queryVersion: 2,
            queryPayload: '{"name":"moon"}',
            validity: 'valid',
        },
    }));
    render(<App/>);

    const search = await screen.findByRole('searchbox', {name: 'Search stories'});
    await user.type(search, 'moon');
    const saveOpener = screen.getByRole('button', {name: '＋ Save current query'});
    await user.click(saveOpener);
    expect(screen.getByRole('heading', {name: 'Save the current query'}))
        .toBeInTheDocument();
    const collectionName = screen.getByRole('textbox', {name: "Collection name"});
    await waitFor(() => expect(collectionName).toHaveFocus());
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('heading', {name: 'Save the current query'}))
        .not.toBeInTheDocument();
    expect(saveOpener).toHaveFocus();

    await user.click(saveOpener);
    await user.type(screen.getByRole('textbox', {name: "Collection name"}), "Moon collection");
    await user.click(screen.getByRole('button', {name: "Save collection"}));

    await waitFor(() => expect(createCollection).toHaveBeenCalledWith(
        'Moon collection',
        expect.objectContaining({name: 'moon', page: 1}),
    ));
    expect(await screen.findByRole('heading', {name: "Moon collection"})).toBeInTheDocument();
    expect(screen.getByRole('button', {name: "Moon collection, 2 stories"}))
        .toHaveClass('active');
});

test('renames, reorders, duplicates, and deletes saved collections', async () => {
    const user = userEvent.setup();
    let summaries = [
        new collections.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        }),
        new collections.Summary({
            id: 8,
            name: 'Adventures',
            position: 1,
            validity: 'valid',
            count: 2,
        }),
    ];
    listCollections.mockImplementation(async () => new app.CollectionListResponse({
        collections: summaries,
    }));
    openCollection.mockImplementation(async (collectionID) => {
        const collection = summaries.find((candidate) => candidate.id === collectionID);
        return new app.CollectionEvaluationResponse({
            evaluation: {
                collection: {
                    id: collectionID,
                    name: collection?.name ?? "Collection",
                    normalizedName: collection?.name.toLowerCase() ?? 'collection',
                    position: collection?.position ?? 0,
                    queryVersion: 2,
                    queryPayload: '{}',
                    validity: 'valid',
                },
                query: {},
                page: {
                    stories,
                    page: 1,
                    pageSize: 12,
                    totalItems: stories.length,
                    totalPages: 1,
                    sort: 'imported_desc',
                },
            },
        });
    });
    renameCollection.mockImplementation(async (collectionID, name) => {
        summaries = summaries.map((collection) => collection.id === collectionID
            ? new collections.Summary({...collection, name})
            : collection);
        return new app.CollectionResponse({
            collection: {
                id: collectionID,
                name,
                normalizedName: name.toLowerCase(),
                position: 0,
                queryVersion: 2,
                queryPayload: '{}',
                validity: 'valid',
            },
        });
    });
    reorderCollections.mockImplementation(async (orderedIDs) => {
        summaries = orderedIDs.map((id, position) => new collections.Summary({
            ...summaries.find((collection) => collection.id === id),
            position,
        }));
        return new app.CollectionListResponse({collections: summaries});
    });
    duplicateCollection.mockImplementation(async (_collectionID, name) => {
        const duplicate = new collections.Summary({
            id: 9,
            name,
            position: summaries.length,
            validity: 'valid',
            count: 1,
        });
        summaries = [...summaries, duplicate];
        return new app.CollectionResponse({
            collection: {
                id: duplicate.id,
                name,
                normalizedName: name.toLowerCase(),
                position: duplicate.position,
                queryVersion: 2,
                queryPayload: '{}',
                validity: 'valid',
            },
        });
    });
    deleteCollection.mockImplementation(async (collectionID) => {
        summaries = summaries.filter((collection) => collection.id !== collectionID);
        return new app.MutationResponse({success: true});
    });

    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Bedtime, 1 story'}));

    await user.click(screen.getByRole('button', {name: 'Rename Bedtime'}));
    const name = screen.getByRole('textbox', {name: "Collection name"});
    await user.clear(name);
    await user.type(name, 'Evening');
    await user.click(screen.getByRole('button', {name: "Rename collection"}));
    await waitFor(() => expect(renameCollection).toHaveBeenCalledWith(7, 'Evening'));

    await user.click(await screen.findByRole('button', {name: 'Move Evening down'}));
    await waitFor(() => expect(reorderCollections).toHaveBeenCalledWith([8, 7]));

    await user.click(screen.getByRole('button', {name: 'Duplicate Evening'}));
    await user.type(screen.getByRole('textbox', {name: "Collection name"}), 'Evening copy');
    await user.click(screen.getByRole('button', {name: "Duplicate collection"}));
    await waitFor(() => expect(duplicateCollection).toHaveBeenCalledWith(7, 'Evening copy'));
    expect(await screen.findByRole('heading', {name: 'Evening copy'})).toBeInTheDocument();

    const deleteOpener = screen.getByRole('button', {name: 'Delete Evening copy'});
    await user.click(deleteOpener);
    expect(screen.getByRole('heading', {name: 'Delete “Evening copy”?'}))
        .toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', {name: 'Cancel'}))
        .toHaveFocus());
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('heading', {name: 'Delete “Evening copy”?'}))
        .not.toBeInTheDocument();
    expect(deleteOpener).toHaveFocus();

    await user.click(deleteOpener);
    await user.click(screen.getByRole('button', {name: "Keep all Shelf stories and delete Collection"}));
    await waitFor(() => expect(deleteCollection).toHaveBeenCalledWith(9));
    expect(screen.queryByRole('button', {name: 'Evening copy, 1 story'}))
        .not.toBeInTheDocument();
});

test('keeps a deleted active collection query as custom instead of labeling all stories', async () => {
    const user = userEvent.setup();
    let summaries = [new collections.Summary({
        id: 7,
        name: 'Bedtime',
        position: 0,
        validity: 'valid',
        count: 1,
    })];
    listCollections.mockImplementation(async () => new app.CollectionListResponse({
        collections: summaries,
    }));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        evaluation: {
            collection: {
                id: 7,
                name: 'Bedtime',
                normalizedName: 'bedtime',
                position: 0,
                queryVersion: 2,
                queryPayload: '{"name":"moon"}',
                validity: 'valid',
            },
            query: {name: 'moon'},
            page: {
                stories: [stories[1]],
                page: 1,
                pageSize: 12,
                totalItems: 1,
                totalPages: 1,
                sort: 'imported_desc',
            },
        },
    }));
    deleteCollection.mockImplementation(async () => {
        summaries = [];
        return new app.MutationResponse({success: true});
    });

    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Bedtime, 1 story'}));
    await user.click(screen.getByRole('button', {name: 'Delete Bedtime'}));
    await user.click(screen.getByRole('button', {name: "Keep all Shelf stories and delete Collection"}));

    await waitFor(() => expect(deleteCollection).toHaveBeenCalledWith(7));
    expect(screen.getByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('moon');
    expect(screen.getByRole('button', {name: /All stories/}))
        .not.toHaveClass('active');
    expect(screen.queryByRole('button', {name: '↻ Update “Bedtime”'}))
        .not.toBeInTheDocument();
});

test('blocks unsafe collection evaluation until its query is explicitly replaced', async () => {
    const user = userEvent.setup();
    const invalid = new collections.Summary({
        id: 12,
        name: 'Old moods',
        position: 0,
        validity: 'needs_attention',
        attentionReason: 'missing_criteria',
        count: 0,
    });
    const repaired = new collections.Summary({
        id: 12,
        name: 'Old moods',
        position: 0,
        validity: 'valid',
        count: 2,
    });
    listCollections
        .mockResolvedValueOnce(new app.CollectionListResponse({collections: [invalid]}))
        .mockResolvedValue(new app.CollectionListResponse({collections: [repaired]}));
    replaceCollectionQuery.mockResolvedValue(new app.CollectionResponse({
        collection: {
            id: 12,
            name: 'Old moods',
            normalizedName: 'old moods',
            position: 0,
            queryVersion: 2,
            queryPayload: '{}',
            validity: 'valid',
        },
    }));

    render(<App/>);
    const repairOpener = await screen.findByRole('button', {
        name: 'Old moods, needs attention',
    });
    await user.click(repairOpener);

    expect(openCollection).not.toHaveBeenCalled();
    expect(screen.getByRole('heading', {name: 'Repair “Old moods”'}))
        .toBeInTheDocument();
    expect(screen.getByText('Evaluation and export are blocked.')).toBeInTheDocument();
    expect(screen.getByText(/original query is preserved/)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', {name: 'Cancel'}))
        .toHaveFocus());
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('heading', {name: 'Repair “Old moods”'}))
        .not.toBeInTheDocument();
    expect(repairOpener).toHaveFocus();

    await user.click(repairOpener);
    await user.click(screen.getByRole('button', {name: 'Replace with current query'}));
    await waitFor(() => expect(replaceCollectionQuery).toHaveBeenCalledWith(
        12,
        expect.objectContaining({
            name: '',
            booleanFilters: [],
            choiceFilters: [],
            page: 1,
        }),
    ));
    expect(await screen.findByRole('button', {name: 'Old moods, 2 stories'}))
        .toHaveClass('active');
});

test('requires explicit removal of unavailable criteria before collection repair', async () => {
    const user = userEvent.setup();
    window.history.replaceState(
        null,
        '',
        '/#/library?bool=999%3Atrue&size=12&sort=imported_desc&v=1',
    );
    const invalid = new collections.Summary({
        id: 15,
        name: 'Deleted tag collection',
        position: 0,
        validity: 'needs_attention',
        attentionReason: 'missing_criteria',
        count: 0,
    });
    const repaired = new collections.Summary({
        id: 15,
        name: 'Deleted tag collection',
        position: 0,
        validity: 'valid',
        count: 2,
    });
    listCollections
        .mockResolvedValueOnce(new app.CollectionListResponse({collections: [invalid]}))
        .mockResolvedValue(new app.CollectionListResponse({collections: [repaired]}));
    queryStories.mockImplementation(async (query) => (
        query.booleanFilters.length > 0
            ? new app.LibraryPageResponse({
                error: {
                    code: 'invalid_input',
                    message: 'The collection query contains an unavailable criterion.',
                },
            })
            : new app.LibraryPageResponse({
                page: {
                    stories,
                    page: 1,
                    pageSize: 12,
                    totalItems: 2,
                    totalPages: 1,
                    sort: 'imported_desc',
                },
            })
    ));
    replaceCollectionQuery.mockResolvedValue(new app.CollectionResponse({
        collection: {
            id: 15,
            name: 'Deleted tag collection',
            normalizedName: 'deleted tag collection',
            position: 0,
            queryVersion: 2,
            queryPayload: '{}',
            validity: 'valid',
        },
    }));

    render(<App/>);
    expect(await screen.findByRole('button', {
        name: 'Remove filter Unavailable saved criterion · definition 999',
    })).toBeInTheDocument();
    await user.click(screen.getByRole('button', {
        name: "Deleted tag collection, needs attention",
    }));

    const replace = screen.getByRole('button', {name: 'Replace with current query'});
    expect(replace).toBeDisabled();
    await user.click(screen.getByRole('button', {
        name: 'Remove unavailable criteria',
    }));
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({booleanFilters: []}),
    ));
    expect(replace).toBeEnabled();
    await user.click(replace);

    await waitFor(() => expect(replaceCollectionQuery).toHaveBeenCalledWith(
        15,
        expect.objectContaining({booleanFilters: []}),
    ));
});

test('moves an active collection into repair when refreshed validity becomes unsafe', async () => {
    const user = userEvent.setup();
    const valid = new collections.Summary({
        id: 16,
        name: 'Mood',
        position: 0,
        validity: 'valid',
        count: 1,
    });
    const invalid = new collections.Summary({
        id: 16,
        name: 'Mood',
        position: 0,
        validity: 'needs_attention',
        attentionReason: 'missing_criteria',
        count: 0,
    });
    listCollections
        .mockResolvedValueOnce(new app.CollectionListResponse({collections: [valid]}))
        .mockResolvedValue(new app.CollectionListResponse({collections: [invalid]}));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        evaluation: {
            collection: {
                id: 16,
                name: 'Mood',
                normalizedName: 'mood',
                position: 0,
                queryVersion: 2,
                queryPayload: '{"name":"moon"}',
                validity: 'valid',
            },
            query: {name: 'moon'},
            page: {
                stories: [stories[1]],
                page: 1,
                pageSize: 12,
                totalItems: 1,
                totalPages: 1,
                sort: 'imported_desc',
            },
        },
    }));

    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Mood, 1 story'}));
    expect(await screen.findByRole('heading', {name: 'Mood'})).toBeInTheDocument();
    operationChanged?.({
        id: '88882222-3333-4444-8555-666677778888',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T19:00:00Z',
        finishedAt: '2026-07-25T19:01:00Z',
        items: [],
    });

    expect(await screen.findByRole('heading', {name: 'Repair “Mood”'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Mood, needs attention'}))
        .not.toHaveClass('active');
});

test('previews a multi-collection union with per-collection and overlap counts', async () => {
    const user = userEvent.setup();
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [
            new collections.Summary({
                id: 7,
                name: 'Moon',
                position: 0,
                validity: 'valid',
                count: 2,
            }),
            new collections.Summary({
                id: 8,
                name: 'Forest',
                position: 1,
                validity: 'valid',
                count: 2,
            }),
        ],
    }));
    previewCollections.mockImplementation(async (collectionIDs) => (
        new app.CollectionSelectionPreviewResponse({
            preview: collectionIDs.length === 1
                ? {
                    collections: [{id: 7, name: 'Moon', count: 2}],
                    sourceCollectionNames: ['Moon'],
                    uniqueStoryCount: 2,
                    overlapCount: 0,
                }
                : {
                    collections: [
                        {id: 7, name: 'Moon', count: 2},
                        {id: 8, name: 'Forest', count: 2},
                    ],
                    sourceCollectionNames: ['Moon', 'Forest'],
                    uniqueStoryCount: 3,
                    overlapCount: 1,
                },
        })
    ));
    selectAndPreflightExport.mockImplementation(async (request) => (
        new app.ExportPreflightResponse({
            preflight: {
                source: {
                    type: request.sourceType,
                    collectionIds: request.collectionIds,
                    collectionNames: request.collectionIds?.length === 1
                        ? ['Forest']
                        : ['Moon', 'Forest'],
                },
                destination: 'Lunii export',
                resolvedCount: request.collectionIds?.length === 1 ? 2 : 3,
                readyCount: request.collectionIds?.length === 1 ? 2 : 3,
                totalBytes: 3145728,
                detectedFormats: ['zip'],
                collapsedOverlap: request.sourceType === 'collections' ? 1 : 0,
                items: [],
                issues: [],
                partial: false,
                blocked: false,
                canExport: true,
            },
        })
    ));

    render(<App/>);
    await user.click(await screen.findByRole('checkbox', {
        name: "Select Moon for combined collection preview",
    }));
    await waitFor(() => expect(previewCollections).toHaveBeenLastCalledWith([7]));
    await user.click(screen.getByRole('checkbox', {
        name: "Select Forest for combined collection preview",
    }));
    await waitFor(() => expect(previewCollections).toHaveBeenLastCalledWith([7, 8]));

    const preview = await screen.findByRole('region', {name: "Combined collection preview"});
    expect(preview).toHaveTextContent('Moon2');
    expect(preview).toHaveTextContent('Forest2');
    expect(preview).toHaveTextContent('3 unique stories');
    expect(preview).toHaveTextContent('1 overlapping membership collapsed.');
    expect(preview).toHaveTextContent('Sources: Moon, Forest');
    await user.click(within(preview).getByRole('button', {
        name: "Export selected collections",
    }));
    expect(selectAndPreflightExport).toHaveBeenLastCalledWith(
        expect.objectContaining({
            sourceType: 'collections',
            collectionIds: [7, 8],
        }),
    );
    expect(screen.getByText('1', {selector: '.export-preflight-facts dd'}))
        .toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Close'}));

    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [
            new collections.Summary({
                id: 7,
                name: 'Moon',
                position: 0,
                validity: 'needs_attention',
                attentionReason: 'missing_criteria',
                count: 0,
            }),
            new collections.Summary({
                id: 8,
                name: 'Forest',
                position: 1,
                validity: 'valid',
                count: 2,
            }),
        ],
    }));
    previewCollections.mockResolvedValue(new app.CollectionSelectionPreviewResponse({
        error: {
            code: 'conflict',
            message: "Repair the selected collection before continuing.",
        },
    }));
    operationChanged?.({
        id: '99992222-3333-4444-8555-666677778888',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T18:00:00Z',
        finishedAt: '2026-07-25T18:01:00Z',
        items: [],
    });

    const invalidSelection = await screen.findByRole('checkbox', {
        name: "Select Moon for combined collection preview",
    });
    await waitFor(() => expect(invalidSelection).toBeEnabled());
    expect(invalidSelection).toBeChecked();
    await waitFor(() => expect(previewCollections).toHaveBeenLastCalledWith([7, 8]));
    expect(await screen.findByText("Repair the selected collection before continuing."))
        .toBeInTheDocument();

    previewCollections.mockResolvedValue(new app.CollectionSelectionPreviewResponse({
        preview: {
            collections: [{id: 8, name: 'Forest', count: 2}],
            sourceCollectionNames: ['Forest'],
            uniqueStoryCount: 2,
            overlapCount: 0,
        },
    }));
    await user.click(invalidSelection);
    await waitFor(() => expect(previewCollections).toHaveBeenLastCalledWith([8]));
    expect(invalidSelection).not.toBeChecked();
    expect(screen.queryByText('Repair the selected collection before continuing.'))
        .not.toBeInTheDocument();
    expect(screen.getByText('2 unique stories')).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: "Export selected collection"}));
    expect(selectAndPreflightExport).toHaveBeenLastCalledWith(
        expect.objectContaining({
            sourceType: 'collection',
            collectionIds: [8],
        }),
    );
});

test('keeps a valid empty saved collection active with edit and import recovery actions', async () => {
    const user = userEvent.setup();
    listCollections.mockResolvedValue(new app.CollectionListResponse({
        collections: [new collections.Summary({
            id: 14,
            name: 'Quiet',
            position: 0,
            validity: 'valid',
            count: 0,
        })],
    }));
    openCollection.mockResolvedValue(new app.CollectionEvaluationResponse({
        evaluation: {
            collection: {
                id: 14,
                name: 'Quiet',
                normalizedName: 'quiet',
                position: 0,
                queryVersion: 2,
                queryPayload: '{"name":"quiet"}',
                validity: 'valid',
            },
            query: {name: 'quiet'},
            page: {
                stories: [],
                page: 1,
                pageSize: 12,
                totalItems: 0,
                totalPages: 0,
                sort: 'imported_desc',
            },
        },
    }));
    queryStories.mockResolvedValue(new app.LibraryPageResponse({
        page: {
            stories: [],
            page: 1,
            pageSize: 12,
            totalItems: 0,
            totalPages: 0,
            sort: 'imported_desc',
        },
    }));

    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Quiet, 0 stories'}));

    expect(await screen.findByRole('heading', {name: 'Quiet is currently empty'}))
        .toBeInTheDocument();
    expect(screen.getByText(/saved query remains valid/)).toBeInTheDocument();
    expect(screen.getByRole('button', {name: "Edit collection query"})).toBeInTheDocument();
    expect(screen.getByRole('button', {name: '＋ Import your first stories'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: '↻ Update “Quiet”'}))
        .toBeInTheDocument();
});

test('reloads collection results from hash back and forward navigation', async () => {
    window.history.replaceState(
        null,
        '',
        '/#/library?name=forest&size=12&sort=imported_desc&v=1',
    );
    render(<App/>);

    await waitFor(() => expect(queryStories).toHaveBeenCalledWith(
        expect.objectContaining({name: 'forest'}),
    ));
    window.history.pushState(
        null,
        '',
        '/#/library?name=moon&size=12&sort=imported_desc&v=1',
    );
    window.dispatchEvent(new PopStateEvent('popstate'));
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({name: 'moon'}),
    ));

    window.history.back();
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({name: 'forest'}),
    ));
    window.history.forward();
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({name: 'moon'}),
    ));
});

test('composes canonical search, tri-state and choice refinements with clear-all focus', async () => {
    const user = userEvent.setup();
    render(<App/>);
    const search = await screen.findByRole('searchbox', {name: 'Search stories'});

    await user.type(search, 'Forêt');
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({name: 'foret', page: 1}),
    ));
    await user.click(screen.getByRole('checkbox', {name: 'Calm'}));
    await user.click(screen.getByRole('checkbox', {name: 'Bold'}));

    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({
            booleanFilters: [],
            choiceFilters: [{definitionId: 2, valueIds: [20, 21]}],
        }),
    ));
    expect(screen.getByRole('button', {name: 'Remove filter Mood · Calm'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Remove filter Mood · Bold'}))
        .toBeInTheDocument();

    await user.click(screen.getByRole('button', {
        name: 'Remove filter Mood · Calm',
    }));
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({
            choiceFilters: [{definitionId: 2, valueIds: [21]}],
        }),
    ));
    expect(search).toHaveFocus();

    await user.click(screen.getByRole('button', {name: 'Clear all'}));
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({
            name: '',
            languages: [],
            compatibilities: [],
            booleanFilters: [],
            choiceFilters: [],
        }),
    ));
    expect(search).toHaveFocus();
});

test('composes official language, import status, and derived age refinements', async () => {
    const user = userEvent.setup();
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {
            state: 'fresh',
            locale: 'en-GB',
            matchedStoryCount: 2,
            activatedAt: '2026-07-25T16:00:00Z',
        },
    }));
    loadTagCatalog.mockResolvedValue(new app.TagCatalogResponse({
        catalog: {
            definitions: [{
                id: 3,
                key: 'age',
                normalizedKey: 'age',
                label: 'Age',
                color: '#267d5d',
                kind: 'choice',
                source: 'derived',
                presentation: 'system',
                position: 0,
                protected: true,
                values: [{
                    id: 30,
                    definitionId: 3,
                    key: '3-5',
                    normalizedKey: '3-5',
                    label: '3–5 years',
                    position: 0,
                }],
            }],
        },
    }));
    render(<App/>);

    await user.click(await screen.findByRole('checkbox', {name: '3–5 years'}));
    await user.click(screen.getByRole('button', {name: 'Language ＋'}));
    await user.click(screen.getByRole('checkbox', {name: 'English'}));
    await user.click(screen.getByRole('button', {name: 'Import status ＋'}));
    await user.click(screen.getByRole('checkbox', {name: 'Archive missing'}));

    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({
            languages: ['en-GB'],
            compatibilities: ['missing'],
            choiceFilters: [{definitionId: 3, valueIds: [30]}],
            page: 1,
        }),
    ));
    expect(window.location.hash).toContain('language=en-GB');
    expect(window.location.hash).toContain('compatibility=missing');
    expect(screen.getByRole('button', {name: 'Remove filter Language · English'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {
        name: 'Remove filter Import status · Archive missing',
    })).toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Remove filter Age · 3–5 years'}))
        .toBeInTheDocument();
});

test('refreshes derived facet counts when an import changes the collection', async () => {
    let ageCount = 1;
    loadTagCatalog.mockResolvedValue(new app.TagCatalogResponse({
        catalog: {
            definitions: [{
                id: 3,
                key: 'age',
                normalizedKey: 'age',
                label: 'Age',
                color: '#267d5d',
                kind: 'choice',
                source: 'derived',
                presentation: 'system',
                position: 0,
                protected: true,
                values: [{
                    id: 30,
                    definitionId: 3,
                    key: '3-5',
                    normalizedKey: '3-5',
                    label: '3–5 years',
                    position: 0,
                }],
            }],
        },
    }));
    queryStories.mockImplementation(async (request) => new app.LibraryPageResponse({
        page: {
            stories,
            page: 1,
            pageSize: request.pageSize,
            totalItems: request.choiceFilters?.length ? ageCount : stories.length,
            totalPages: 1,
            sort: 'imported_desc',
        },
    }));
    render(<App/>);
    const age = await screen.findByRole('checkbox', {name: '3–5 years'});
    await waitFor(() => expect(age.closest('label')).toHaveTextContent('1'));

    ageCount = 2;
    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [],
    });
    await waitFor(() => expect(age.closest('label')).toHaveTextContent('2'));
});

test('keeps matched metadata, artwork, provenance, and combined refinements usable from stale cache', async () => {
    const user = userEvent.setup();
    const artworkID = 'a'.repeat(64);
    const matched = new library.StorySummary({
        id: 10,
        uuid: '123e4567-e89b-42d3-a456-426614174000',
        title: 'The Clockwork Mountain',
        description: 'Official description',
        author: 'A. Example',
        artworkId: artworkID,
        sources: {
            title: 'official',
            description: 'official',
            author: 'official',
            artwork: 'official',
        },
        detectedFormat: 'zip',
        compatibility: 'compatible',
        byteSize: 1048576,
        importedAt: '2026-07-25T09:00:00Z',
        official: {
            locale: 'en-GB',
            publisher: 'Fixture Press',
            language: 'en-GB',
            durationSeconds: 3240,
            minimumAge: 3,
            maximumAge: 5,
            provenance: 'lunii_catalog',
            fetchedAt: '2026-07-24T16:00:00Z',
            activatedAt: '2026-07-24T16:01:00Z',
        },
    });
    const unmatched = new library.StorySummary({
        id: 11,
        uuid: '00112233-4455-4677-8899-aabbccddeeff',
        title: 'Unmatched local story',
        description: 'Embedded description',
        sources: {
            title: 'embedded',
            description: 'embedded',
            author: 'fallback',
            artwork: 'fallback',
        },
        detectedFormat: 'zip',
        compatibility: 'compatible',
        byteSize: 2097152,
        importedAt: '2026-07-25T08:00:00Z',
    });
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {
            state: 'stale_cache',
            locale: 'en-GB',
            matchedStoryCount: 1,
            activatedAt: '2026-07-24T16:01:00Z',
            errorMessage: 'Official metadata could not be downloaded.',
        },
    }));
    loadTagCatalog.mockResolvedValue(new app.TagCatalogResponse({
        catalog: {
            definitions: [{
                id: 3,
                key: 'age',
                label: 'Age',
                color: '#267d5d',
                kind: 'choice',
                source: 'derived',
                protected: true,
                values: [{
                    id: 30,
                    definitionId: 3,
                    key: '3-5',
                    label: '3–5 years',
                    position: 0,
                }],
            }, {
                id: 4,
                key: 'mood',
                label: 'Mood',
                color: '#405cf5',
                kind: 'choice',
                source: 'user',
                protected: false,
                values: [{
                    id: 40,
                    definitionId: 4,
                    key: 'calm',
                    label: 'Calm',
                    position: 0,
                }],
            }],
        },
    }));
    queryStories.mockImplementation(async (request) => {
        const filtered = request.name !== '' ||
            request.languages.length > 0 ||
            request.compatibilities.length > 0 ||
            request.choiceFilters.length > 0;
        const visible = filtered ? [matched] : [matched, unmatched];
        return new app.LibraryPageResponse({
            page: {
                stories: visible,
                page: 1,
                pageSize: 12,
                totalItems: visible.length,
                totalPages: 1,
                sort: 'imported_desc',
            },
        });
    });
    loadStoryDetail.mockImplementation(async (storyID) => {
        const story = storyID === matched.id ? matched : unmatched;
        return new app.StoryDetailResponse({
            detail: {
                story,
                archive: {
                    originalFilename: `${story.id}.zip`,
                    detectedFormat: story.detectedFormat,
                    sha256: 'c'.repeat(64),
                    byteSize: story.byteSize,
                    verification: story.compatibility,
                },
            },
        });
    });
    loadTagAssignmentWorkspace.mockImplementation(async (storyIDs) => (
        new app.TagAssignmentWorkspaceResponse({
            workspace: {
                catalog: (await loadTagCatalog()).catalog,
                requestedStories: storyIDs.length,
                states: [{
                    definitionId: 3,
                    assignedStories: storyIDs.includes(matched.id) ? 1 : 0,
                    values: [{
                        valueId: 30,
                        assignedStories: storyIDs.includes(matched.id) ? 1 : 0,
                    }],
                }, {
                    definitionId: 4,
                    assignedStories: storyIDs.includes(matched.id) ? 1 : 0,
                    values: [{
                        valueId: 40,
                        assignedStories: storyIDs.includes(matched.id) ? 1 : 0,
                    }],
                }],
            },
        })
    ));
    render(<App/>);

    expect(await screen.findByRole('heading', {name: 'Using saved Lunii story details'}))
        .toBeInTheDocument();
    expect(await screen.findByRole('heading', {name: 'The Clockwork Mountain'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {
        name: `Unmatched local story`,
    }))
        .toBeInTheDocument();
    expect(await screen.findByRole('img', {name: 'The Clockwork Mountain artwork'}))
        .toHaveAttribute('src', `/artwork/${artworkID}`);
    expect(screen.getByText(/A\. Example · metadata synced/))
        .toBeInTheDocument();
    expect(screen.getByText('54 min')).toBeInTheDocument();
    expect(screen.getByLabelText('System-derived tag: Age · 3–5 years'))
        .toHaveTextContent('Age · 3–5 years');

    await user.click(screen.getByRole('button', {
        name: `Unmatched local story`,
    }));
    expect(await screen.findByText('Local story · embedded metadata'))
        .toBeInTheDocument();
    await user.click(screen.getByRole('button', {
        name: 'The Clockwork Mountain',
    }));

    await user.type(screen.getByRole('searchbox', {name: 'Search stories'}), 'Clockwork');
    await user.click(screen.getByRole('button', {name: 'Language ＋'}));
    await user.click(screen.getByRole('checkbox', {name: 'English'}));
    await user.click(screen.getByRole('button', {name: 'Import status ＋'}));
    await user.click(screen.getByRole('checkbox', {name: 'Compatible'}));
    await user.click(screen.getByRole('checkbox', {name: '3–5 years'}));
    await user.click(screen.getByRole('checkbox', {name: 'Calm'}));
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({
            name: 'clockwork',
            languages: ['en-GB'],
            compatibilities: ['compatible'],
            choiceFilters: [
                {definitionId: 3, valueIds: [30]},
                {definitionId: 4, valueIds: [40]},
            ],
        }),
    ));
    expect(screen.getByRole('heading', {name: 'The Clockwork Mountain'}))
        .toBeInTheDocument();
});

test('changes sort and pages from their canonical collection controls', async () => {
    const user = userEvent.setup();
    queryStories.mockResolvedValue(new app.LibraryPageResponse({
        page: {
            stories,
            page: 1,
            pageSize: 12,
            totalItems: 48,
            totalPages: 4,
            sort: 'imported_desc',
        },
    }));
    render(<App/>);

    await user.selectOptions(
        await screen.findByRole('combobox', {name: 'Sort stories'}),
        'name_asc',
    );
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({sort: 'name_asc', page: 1}),
    ));
    await user.click(screen.getByRole('button', {name: 'Next →'}));
    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({sort: 'name_asc', page: 2}),
    ));
});

test('restores focus to the tag-manager opener when the modal closes', async () => {
    const user = userEvent.setup();
    render(<App/>);
    const opener = await screen.findByRole('button', {name: '＋ Manage tags'});

    await user.click(opener);
    const close = await screen.findByRole('button', {name: 'Close tag manager'});
    await waitFor(() => expect(
        screen.getByRole('textbox', {name: 'Key'}),
    ).toHaveFocus());
    await user.click(close);

    expect(opener).toHaveFocus();
});

test('freezes assignment targets when a tag change removes stories from the filter', async () => {
    const user = userEvent.setup();
    let assigned = false;
    queryStories.mockImplementation(async () => new app.LibraryPageResponse({
        page: {
            stories: assigned ? [stories[1]] : stories,
            page: 1,
            pageSize: 12,
            totalItems: assigned ? 1 : 2,
            totalPages: 1,
            sort: 'imported_desc',
        },
    }));
    setBooleanTag.mockImplementation(async () => {
        assigned = true;
        return new app.TagAssignmentResponse({
            result: {
                requestedStories: 1,
                changedStories: 1,
                assignmentsAdded: 1,
                assignmentsRemoved: 0,
            },
        });
    });
    loadTagAssignmentWorkspace.mockImplementation(async (storyIDs) => (
        new app.TagAssignmentWorkspaceResponse({
            workspace: {
                catalog: {
                    definitions: [{
                        id: 1,
                        key: 'favorite',
                        label: 'Favorite',
                        color: '#ff705c',
                        kind: 'boolean',
                        source: 'user',
                        protected: true,
                        values: [],
                    }],
                },
                requestedStories: storyIDs.length,
                states: [{
                    definitionId: 1,
                    assignedStories: 0,
                    values: [],
                }],
            },
        })
    ));
    render(<App/>);
    await screen.findByRole('heading', {name: 'Clockwork Forest'});

    await user.click(screen.getByRole('button', {name: 'Open details'}));
    await user.click(screen.getByRole('button', {name: 'Edit tags'}));
    const tagEditor = await screen.findByRole('dialog', {name: 'Edit story tags'});
    const broken = await within(tagEditor).findByRole('checkbox', {name: 'Favorite'});
    await user.click(broken);
    expect(await screen.findByRole('heading', {name: 'Moonlit Workshop'}))
        .toBeInTheDocument();
    await user.click(broken);

    expect(setBooleanTag).toHaveBeenNthCalledWith(1, [1], 1, true);
    expect(setBooleanTag).toHaveBeenNthCalledWith(2, [1], 1, true);
});

test('ignores superseded query responses without repeating application bootstrap', async () => {
    let resolveOld: ((value: app.LibraryPageResponse) => void) | undefined;
    let resolveNew: ((value: app.LibraryPageResponse) => void) | undefined;
    queryStories.mockImplementation(async (request) => {
        if (request.name === 'old') {
            return new Promise((resolve) => {
                resolveOld = resolve;
            });
        }
        if (request.name === 'new') {
            return new Promise((resolve) => {
                resolveNew = resolve;
            });
        }
        return new app.LibraryPageResponse({
            page: {
                stories,
                page: 1,
                pageSize: 12,
                totalItems: 2,
                totalPages: 1,
                sort: 'imported_desc',
            },
        });
    });
    render(<App/>);
    const search = await screen.findByRole('searchbox', {name: 'Search stories'});

    fireEvent.change(search, {target: {value: 'old'}});
    await waitFor(() => expect(resolveOld).toBeTypeOf('function'));
    fireEvent.change(search, {target: {value: 'new'}});
    await waitFor(() => expect(resolveNew).toBeTypeOf('function'));

    resolveNew?.(new app.LibraryPageResponse({
        page: {
            stories: [stories[1]],
            page: 1,
            pageSize: 12,
            totalItems: 1,
            totalPages: 1,
            sort: 'imported_desc',
        },
    }));
    expect(await screen.findByRole('heading', {name: 'Moonlit Workshop'}))
        .toBeInTheDocument();
    resolveOld?.(new app.LibraryPageResponse({
        page: {
            stories: [stories[0]],
            page: 1,
            pageSize: 12,
            totalItems: 1,
            totalPages: 1,
            sort: 'imported_desc',
        },
    }));

    await waitFor(() => expect(
        screen.getByRole('heading', {name: 'Moonlit Workshop'}),
    ).toBeInTheDocument());
    expect(applicationStatus).toHaveBeenCalledTimes(1);
    expect(loadTagCatalog).toHaveBeenCalledTimes(1);
    expect(activeOperations).toHaveBeenCalledTimes(1);
});

test('does not let an older view-all expansion overwrite a newer query', async () => {
    let resolveExpansion: ((value: app.LibraryPageResponse) => void) | undefined;
    queryStories.mockImplementation(async (request) => {
        if (request.pageSize === 50) {
            return new Promise((resolve) => {
                resolveExpansion = resolve;
            });
        }
        const visible = request.name === 'moon' ? [stories[1]] : stories;
        return new app.LibraryPageResponse({
            page: {
                stories: visible,
                page: 1,
                pageSize: 12,
                totalItems: request.name === 'moon' ? 1 : 48,
                totalPages: request.name === 'moon' ? 1 : 4,
                sort: 'imported_desc',
            },
        });
    });
    render(<App/>);
    await screen.findByRole('heading', {name: 'Clockwork Forest'});

    fireEvent.click(screen.getByRole('button', {name: 'View all →'}));
    await waitFor(() => expect(resolveExpansion).toBeTypeOf('function'));
    fireEvent.change(screen.getByRole('searchbox', {name: 'Search stories'}), {
        target: {value: 'moon'},
    });
    expect(await screen.findByRole('heading', {name: 'Moonlit Workshop'}))
        .toBeInTheDocument();
    resolveExpansion?.(new app.LibraryPageResponse({
        page: {
            stories: [stories[0]],
            page: 1,
            pageSize: 50,
            totalItems: 1,
            totalPages: 1,
            sort: 'imported_desc',
        },
    }));

    await waitFor(() => expect(
        screen.getByRole('heading', {name: 'Moonlit Workshop'}),
    ).toBeInTheDocument());
    expect(screen.queryByRole('heading', {name: 'Clockwork Forest'}))
        .not.toBeInTheDocument();
});

test('selects a cover and loads its detail drawer without losing the collection', async () => {
    const user = userEvent.setup();
    render(<App/>);

    const moonlit = await screen.findByRole('button', {
        name: 'Moonlit Workshop',
    });
    await user.click(moonlit);

    expect(moonlit).toHaveAttribute('aria-pressed', 'true');
    expect(await screen.findByRole('complementary', {name: 'Moonlit Workshop details'}))
        .toBeInTheDocument();
    expect(screen.getByRole('heading', {name: 'My story shelves'})).toBeInTheDocument();
    expect(loadStoryDetail).toHaveBeenLastCalledWith(2);
});

test('shows the story artwork and readable description in the details dialog', async () => {
    const user = userEvent.setup();
    loadStoryDetail.mockResolvedValue(new app.StoryDetailResponse({
        detail: {
            story: {
                ...stories[0],
                artworkId: 'rainbow-cover',
                description: 'Un dragon !<br/><br/>Aide <b>Lou</b> &amp; ses amis.<span class="SF_splitter"></span>',
            },
            archive: {
                originalFilename: 'clockwork-forest.zip',
                sha256: 'a'.repeat(64),
                verification: 'compatible',
            },
        },
    }));
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    const dialog = await screen.findByRole('dialog', {name: 'Clockwork Forest'});

    expect(within(dialog).getByRole('img', {name: 'Clockwork Forest artwork'}))
        .toHaveAttribute('src', '/artwork/rainbow-cover');
    expect(within(dialog).getByText('Un dragon ! Aide Lou & ses amis.'))
        .toBeInTheDocument();
    expect(dialog).not.toHaveTextContent('<br');
    expect(dialog).not.toHaveTextContent('SF_splitter');
    fireEvent.error(within(dialog).getByRole('img', {name: 'Clockwork Forest artwork'}));
    expect(within(dialog).queryByRole('img')).not.toBeInTheDocument();
    expect(within(dialog).getByRole('heading', {name: 'Clockwork Forest'}))
        .toBeInTheDocument();
});

test('keeps technical story details behind a show and hide toggle', async () => {
    const user = userEvent.setup();
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    const dialog = screen.getByRole('dialog', {name: 'Clockwork Forest'});
    expect(within(dialog).queryByText(stories[0].uuid)).not.toBeInTheDocument();
    expect(within(dialog).queryByText('Story UUID')).not.toBeInTheDocument();
    expect(within(dialog).queryByText('a'.repeat(64))).not.toBeInTheDocument();

    await user.click(within(dialog).getByRole('button', {name: 'Show technical details'}));
    expect(within(dialog).getByText(stories[0].uuid)).toBeInTheDocument();
    expect(within(dialog).getByText('clockwork-forest.zip')).toBeInTheDocument();
    expect(within(dialog).getByText('Verified')).toBeInTheDocument();
    expect(within(dialog).getByText('a'.repeat(64))).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', {name: 'Hide technical details'}));
    expect(within(dialog).queryByText(stories[0].uuid)).not.toBeInTheDocument();
});

test('asks for confirmation only after choosing to remove a story', async () => {
    const user = userEvent.setup();
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    const dialog = screen.getByRole('dialog', {name: 'Clockwork Forest'});
    expect(within(dialog).queryByText('Remove from this library?')).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', {name: 'Move to trash'})).not.toBeInTheDocument();

    await user.click(within(dialog).getByRole('button', {name: 'Remove from library…'}));
    expect(within(dialog).getByText('Remove from this library?')).toBeInTheDocument();
    expect(removeStory).not.toHaveBeenCalled();
    const cancel = within(dialog).getByRole('button', {name: 'Keep story'});
    await waitFor(() => expect(cancel).toHaveFocus());
    await user.click(cancel);
    expect(within(dialog).queryByRole('button', {name: 'Move to trash'})).not.toBeInTheDocument();
    expect(removeStory).not.toHaveBeenCalled();
    expect(within(dialog).getByRole('button', {name: 'Remove from library…'})).toHaveFocus();

    await user.click(within(dialog).getByRole('button', {name: 'Remove from library…'}));
    await user.keyboard('{Escape}');
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).queryByRole('button', {name: 'Move to trash'})).not.toBeInTheDocument();
    expect(removeStory).not.toHaveBeenCalled();
});

test('dismisses story details only when clicking outside the popup', async () => {
    const user = userEvent.setup();
    render(<App/>);
    const opener = await screen.findByRole('button', {name: 'Open details'});
    await user.click(opener);
    const dialog = screen.getByRole('dialog', {name: 'Clockwork Forest'});
    await user.click(within(dialog).getByRole('heading', {name: 'Clockwork Forest'}));
    expect(dialog).toBeInTheDocument();
    await user.click(dialog.closest('.dialog-backdrop')!);
    expect(dialog).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
    expect(removeStory).not.toHaveBeenCalled();
});

test('shows tags and their edit action in details and returns from the tag editor', async () => {
    const user = userEvent.setup();
    loadTagAssignmentWorkspace.mockResolvedValue(new app.TagAssignmentWorkspaceResponse({
        workspace: {
            catalog: {definitions: [{id: 2, key: 'mood', label: 'Mood', kind: 'choice',
                color: '#405cf5', values: [{id: 20, label: 'Calm'}]}]},
            requestedStories: 1,
            states: [{definitionId: 2, assignedStories: 0, values: [{valueId: 20, assignedStories: 1}]}],
        },
    }));
    render(<App/>);
    expect(screen.queryByRole('button', {name: 'Edit tags'})).not.toBeInTheDocument();
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    const dialog = screen.getByRole('dialog', {name: 'Clockwork Forest'});
    expect(await within(dialog).findByText('Mood · Calm')).toBeInTheDocument();
    expect(loadTagAssignmentWorkspace).toHaveBeenCalledWith([1]);
    await user.click(within(dialog).getByRole('button', {name: 'Edit tags'}));
    const editor = await screen.findByRole('dialog', {name: 'Edit story tags'});
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
    expect(await within(editor).findByRole('checkbox', {name: 'Calm'})).toBeChecked();
    await user.click(within(editor).getByRole('button', {name: 'Done'}));
    const reopened = await screen.findByRole('dialog', {name: 'Clockwork Forest'});
    expect(await within(reopened).findByText('Mood · Calm')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('button', {name: 'Edit tags'})).not.toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Open details'})).toHaveFocus();
});

test('collapses choice tag filters without clearing their selection', async () => {
    const user = userEvent.setup();
    render(<App/>);
    const mood = await screen.findByRole('button', {name: 'Mood'});
    await user.click(screen.getByRole('checkbox', {name: 'Calm'}));
    await user.click(mood);
    expect(mood).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('checkbox', {name: 'Calm'})).not.toBeInTheDocument();
    await user.click(mood);
    expect(screen.getByRole('checkbox', {name: 'Calm'})).toBeChecked();
});

test('always displays flagged covers without a visibility selector', async () => {
    queryStories.mockResolvedValue(new app.LibraryPageResponse({page: {stories: [{...stories[0], broken: true}, stories[1]], page: 1, pageSize: 12, totalItems: 2, totalPages: 1, sort: 'imported_desc'}}));
    render(<App/>);
    const broken = await screen.findByRole('button', {name: 'Clockwork Forest'});
    expect(within(broken).getByText('Broken')).toBeInTheDocument();
    expect(broken.closest('.story-card')).toHaveClass('is-broken');
    expect(screen.queryByRole('combobox', {name: 'Broken stories'})).not.toBeInTheDocument();
});

test('keeps removal pending in the dialog and allows retry after a failure', async () => {
    const user = userEvent.setup();
    let rejectRemoval: ((reason?: unknown) => void) | undefined;
    removeStory.mockImplementationOnce(() => new Promise((_, reject) => {
        rejectRemoval = reject;
    }));
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    const dialog = screen.getByRole('dialog', {name: 'Clockwork Forest'});
    await user.click(within(dialog).getByRole('button', {name: 'Remove from library…'}));
    await user.click(within(dialog).getByRole('button', {name: 'Move to trash'}));
    expect(within(dialog).getByRole('button', {name: 'Moving to trash…'})).toBeDisabled();
    expect(within(dialog).getByRole('button', {name: 'Keep story'})).toBeDisabled();
    await user.keyboard('{Escape}');
    expect(dialog).toBeInTheDocument();
    rejectRemoval?.(new Error('Disconnected'));
    expect(await within(dialog).findByText(
        'The story could not be moved to trash. Please try again.',
    )).toBeInTheDocument();
    expect(within(dialog).getByRole('button', {name: 'Move to trash'})).toBeEnabled();
    expect(screen.getByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
});

test('traps detail-dialog focus, closes with Escape, and restores its opener', async () => {
    const user = userEvent.setup();
    render(<App/>);

    const opener = await screen.findByRole('button', {name: 'Open details'});
    await user.click(opener);
    const dialog = await screen.findByRole('dialog', {name: 'Clockwork Forest'});
    const cancel = within(dialog).getByRole('button', {name: 'Close'});
    const remove = within(dialog).getByRole('button', {name: 'Remove from library…'});
    await waitFor(() => expect(cancel).toHaveFocus());

    await user.tab({shift: true});
    expect(remove).toHaveFocus();
    await user.tab();
    expect(cancel).toHaveFocus();
    await user.tab();
    expect(within(dialog).getByRole('button', {name: 'Mark as broken'})).toHaveFocus();
    await user.tab();
    expect(within(dialog).getByRole('button', {name: 'Edit tags'})).toHaveFocus();

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', {name: 'Clockwork Forest'}))
        .not.toBeInTheDocument();
    expect(opener).toHaveFocus();
});

test('keeps bulk selection in the collection and exposes updated tag chips', async () => {
    const user = userEvent.setup();
    loadTagAssignmentWorkspace.mockImplementation(async (storyIDs) => (
        new app.TagAssignmentWorkspaceResponse({
            workspace: {
                catalog: {
                    definitions: [{
                        id: 1,
                        key: 'favorite',
                        label: 'Favorite',
                        color: '#ff705c',
                        kind: 'boolean',
                        source: 'user',
                        protected: true,
                        values: [],
                    }],
                },
                requestedStories: storyIDs.length,
                states: [{
                    definitionId: 1,
                    assignedStories: storyIDs.length,
                    values: [],
                }],
            },
        })
    ));
    render(<App/>);

    const first = await screen.findByRole('button', {name: 'Clockwork Forest'});
    const second = screen.getByRole('button', {
        name: 'Moonlit Workshop',
    });
    expect((await screen.findAllByText('Favorite')).length).toBeGreaterThan(0);

    await user.keyboard('{Control>}');
    await user.click(second);
    await user.keyboard('{/Control}');

    expect(first).toHaveAttribute('aria-pressed', 'true');
    expect(second).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', {name: 'Open details'}));
    await user.click(screen.getByRole('button', {name: 'Edit tags for 2 stories'}));
    expect(await screen.findByRole('heading', {name: 'Edit tags for 2 stories'}))
        .toBeInTheDocument();
});

test('shows the empty-library import action in the canonical shell', async () => {
    queryStories.mockResolvedValue(new app.LibraryPageResponse({
        page: {
            stories: [],
            page: 1,
            pageSize: 12,
            totalItems: 0,
            totalPages: 0,
            sort: 'imported_desc',
        },
    }));

    render(<App/>);

    expect(await screen.findByRole('heading', {name: 'Build your local story archive'}))
        .toBeInTheDocument();
    expect(screen.getByText('Stories grouped by your tags · 0 archives'))
        .toBeInTheDocument();
});

test('shows never-synced metadata and runs a cancellable refresh with matched freshness', async () => {
    const user = userEvent.setup();
    refreshOfficialMetadata.mockResolvedValue(new app.OperationResponse({
        operation: {
            id: '00112233-4455-4677-8899-aabbccddeef0',
            kind: 'metadata_sync',
            status: 'queued',
            completedItems: 0,
            totalItems: 1,
            cancelRequested: false,
            createdAt: '2026-07-25T16:00:00Z',
            items: [{
                id: 50,
                sourceName: 'en-GB',
                status: 'pending',
                completedBytes: 0,
                totalBytes: 1,
            }],
        },
    }));
    render(<App/>);

    expect(await screen.findByRole('heading', {
        name: 'Add covers and richer story details',
    })).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Sync with Lunii'}));
    expect(refreshOfficialMetadata).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole('heading', {
        name: 'Refreshing official metadata',
    })).toBeInTheDocument();
    expect(screen.getByText('0 of 1 refresh phases complete. Your local collection remains available.'))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Syncing with Lunii…'})).toBeDisabled();
    await user.click(screen.getByRole('button', {name: 'Cancel refresh'}));
    expect(cancelOperation).toHaveBeenCalledWith('00112233-4455-4677-8899-aabbccddeef0');

    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: new metadata.CatalogStatus({
            state: 'fresh',
            locale: 'en-GB',
            matchedStoryCount: 1,
            activatedAt: '2026-07-25T16:01:00Z',
        }),
    }));
    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeef0',
        kind: 'metadata_sync',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T16:00:00Z',
        startedAt: '2026-07-25T16:00:01Z',
        finishedAt: '2026-07-25T16:01:00Z',
        items: [{
            id: 50,
            sourceName: 'en-GB',
            status: 'succeeded',
            outcomeCode: 'metadata_refreshed',
            outcomeMessage: 'Official metadata refreshed; 1 local story matched.',
            completedBytes: 1,
            totalBytes: 1,
        }],
    });
    expect(await screen.findByRole('heading', {
        name: 'Official metadata refreshed',
    })).toBeInTheDocument();
    await waitFor(() => expect(
        screen.getByText(/1 local story matched\./),
    ).toBeInTheDocument());
    const sync = screen.getByRole('region', {name: 'Official Lunii sync'});
    expect(sync).toHaveAttribute('data-state', 'lunii-sync-status');
    expect(sync.querySelector('time')).toHaveAttribute('dateTime', '2026-07-25T16:01:00Z');
});

test('shows a stale-cache state without blocking the collection', async () => {
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: new metadata.CatalogStatus({
            state: 'stale_cache',
            locale: 'en-GB',
            matchedStoryCount: 2,
            activatedAt: '2026-07-24T16:00:00Z',
            errorCode: 'catalog_fetch_failed',
            errorMessage: 'Official metadata could not be downloaded.',
        }),
    }));
    render(<App/>);

    expect(await screen.findByRole('heading', {
        name: 'Using saved Lunii story details',
    })).toBeInTheDocument();
    expect(await screen.findByRole('heading', {name: 'Clockwork Forest'}))
        .toBeInTheDocument();
    expect(screen.getByText(/Official metadata could not be downloaded\./))
        .toBeInTheDocument();
});

test('restores metadata freshness after a later import becomes the visible operation', async () => {
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: new metadata.CatalogStatus({
            state: 'fresh',
            locale: 'en-GB',
            matchedStoryCount: 1,
            activatedAt: '2026-07-25T16:01:00Z',
        }),
    }));
    render(<App/>);

    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeef0',
        kind: 'metadata_sync',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T16:00:00Z',
        finishedAt: '2026-07-25T16:01:00Z',
        items: [],
    });
    expect(await screen.findByRole('heading', {
        name: 'Official metadata refreshed',
    })).toBeInTheDocument();

    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T17:00:00Z',
        finishedAt: '2026-07-25T17:01:00Z',
        items: [],
    });

    expect(await screen.findByRole('heading', {
        name: 'Lunii covers and story details',
    })).toBeInTheDocument();
});

test('shows nonblocking import progress and terminal validation feedback', async () => {
    const user = userEvent.setup();
    selectAndImportStories.mockResolvedValue(new app.OperationResponse({
        operation: {
            id: '00112233-4455-4677-8899-aabbccddeeff',
            kind: 'import',
            status: 'queued',
            completedItems: 0,
            totalItems: 1,
            cancelRequested: false,
            createdAt: '2026-07-25T09:00:00Z',
            items: [{
                id: 1,
                sourceName: 'broken.zip',
                status: 'pending',
                completedBytes: 0,
                totalBytes: 100,
            }],
        },
    }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'Import'}));
    expect(screen.getByRole('heading', {name: 'Importing 1 story'})).toBeInTheDocument();

    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'failed',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            sourceName: 'broken.zip',
            status: 'failed',
            outcomeCode: 'invalid_container',
            outcomeMessage: 'The file is not a supported story archive.',
            completedBytes: 0,
            totalBytes: 100,
        }],
    });

    expect(await screen.findByRole('heading', {
        name: 'Unsupported or invalid story archive',
    })).toBeInTheDocument();
    expect(screen.getByText('The file is not a supported story archive.')).toBeInTheDocument();
});

test('restores and reconciles an active import after the frontend reloads', async () => {
    const running = {
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'running',
        completedItems: 0,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            sourceName: 'clockwork.zip',
            status: 'running',
            completedBytes: 512,
            totalBytes: 1024,
        }],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({
        operations: [running],
    }));
    operationSnapshot.mockResolvedValue(new app.OperationResponse({
        operation: {
            ...running,
            status: 'succeeded',
            completedItems: 1,
            items: [{
                ...running.items[0],
                status: 'succeeded',
                outcomeCode: 'imported',
                outcomeMessage: 'Story imported.',
                completedBytes: 1024,
            }],
        },
    }));

    render(<App/>);

    expect(await screen.findByRole('heading', {name: '1 story imported'}))
        .toBeInTheDocument();
    expect(activeOperations).toHaveBeenCalledTimes(1);
    expect(operationSnapshot)
        .toHaveBeenCalledWith('00112233-4455-4677-8899-aabbccddeeff');
});

test('restores persisted export progress and reconciles missed events', async () => {
    const user = userEvent.setup();
    const running = {
        id: 'export-after-reload',
        kind: 'export',
        status: 'running',
        exportSourceType: 'current_query',
        completedItems: 1,
        totalItems: 2,
        totalBytes: 2097152,
        cancelRequested: false,
        createdAt: '2026-07-25T20:00:00Z',
        items: [{
            id: 1,
            storyId: 1,
            sourceName: 'first.zip',
            outputName: 'first.zip',
            status: 'succeeded',
            completedBytes: 1048576,
            totalBytes: 1048576,
        }, {
            id: 2,
            storyId: 2,
            sourceName: 'second.zip',
            outputName: 'second.zip',
            status: 'running',
            completedBytes: 524288,
            totalBytes: 1048576,
        }],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({
        operations: [running],
    }));
    operationSnapshot.mockResolvedValue(new app.OperationResponse({
        operation: running,
    }));
    render(<App/>);

    expect(await screen.findByRole('heading', {name: 'Exporting story archives'}))
        .toBeInTheDocument();
    expect(screen.getByText(
        '1/2 finished · 1.5 MB of 2.0 MB. Progress is saved locally.',
    )).toBeInTheDocument();
    const storyProgress = screen.getByRole('list', {
        name: 'Export story progress',
    });
    expect(storyProgress).toHaveTextContent('first.zip');
    expect(storyProgress).toHaveTextContent('1.0 MB of 1.0 MB · succeeded');
    expect(storyProgress).toHaveTextContent('second.zip');
    expect(storyProgress).toHaveTextContent('512 KB of 1.0 MB · running');
    await waitFor(() => expect(operationSnapshot).toHaveBeenCalledWith(
        'export-after-reload',
    ));
    await user.click(screen.getByRole('button', {name: 'Cancel export'}));
    expect(cancelOperation).toHaveBeenCalledWith('export-after-reload');
});

test('groups final export outcomes and reveals the retained destination', async () => {
    const user = userEvent.setup();
    const finishedExport = {
        id: 'finished-export',
        kind: 'export',
        status: 'partially_succeeded',
        exportSourceType: 'collections',
        sourceCollectionIds: [7, 8],
        sourceCollectionNames: ['Moon', 'Forest'],
        destination: 'Lunii export',
        completedItems: 5,
        totalItems: 5,
        totalBytes: 5242880,
        cancelRequested: false,
        createdAt: '2026-07-25T20:00:00Z',
        finishedAt: '2026-07-25T20:01:00Z',
        items: [
            {
                id: 1,
                storyTitle: 'Exported story',
                sourceName: 'exported.zip',
                outputName: 'exported.zip',
                status: 'succeeded',
                outcomeMessage: 'Story archive exported.',
                completedBytes: 1048576,
                totalBytes: 1048576,
            },
            {
                id: 2,
                storyTitle: 'Skipped story',
                sourceName: 'skipped.zip',
                outputName: 'skipped.zip',
                status: 'skipped',
                outcomeMessage: 'Managed archive bytes are missing.',
                completedBytes: 0,
                totalBytes: 1048576,
            },
            {
                id: 3,
                storyTitle: 'Conflicted story',
                sourceName: 'conflicted.zip',
                outputName: 'conflicted.zip',
                status: 'conflicted',
                outcomeMessage: 'A file with this name already exists.',
                completedBytes: 0,
                totalBytes: 1048576,
            },
            {
                id: 4,
                storyTitle: 'Failed story',
                sourceName: 'failed.zip',
                outputName: 'failed.zip',
                status: 'failed',
                outcomeMessage: 'The copied archive failed checksum verification.',
                completedBytes: 0,
                totalBytes: 1048576,
            },
            {
                id: 5,
                storyTitle: 'Cancelled story',
                sourceName: 'cancelled.zip',
                outputName: 'cancelled.zip',
                status: 'cancelled',
                outcomeMessage: 'Story export cancelled.',
                completedBytes: 0,
                totalBytes: 1048576,
            },
        ],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({
        operations: [finishedExport],
    }));
    render(<App/>);

    const report = await screen.findByRole('region', {name: 'Export report'});
    expect(report).toHaveTextContent("Source collections · Moon, Forest");
    expect(report).toHaveTextContent('Destination · Lunii export');
    for (const label of [
        'Exported · 1',
        'Skipped · 1',
        'Conflicted · 1',
        'Failed · 1',
        'Cancelled · 1',
    ]) {
        expect(within(report).getByRole('heading', {name: label}))
            .toBeInTheDocument();
    }
    expect(report).toHaveTextContent(
        'The copied archive failed checksum verification.',
    );
    operationChanged?.({
        id: 'newer-import',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T20:02:00Z',
        finishedAt: '2026-07-25T20:03:00Z',
        items: [{
            id: 6,
            sourceName: 'newer.zip',
            status: 'succeeded',
            outcomeCode: 'imported',
            outcomeMessage: 'Story imported.',
            completedBytes: 1,
            totalBytes: 1,
        }],
    });
    expect(await screen.findByRole('heading', {name: '1 story imported'}))
        .toBeInTheDocument();
    expect(screen.getByRole('region', {name: 'Export report'}))
        .toBeInTheDocument();
    await user.click(within(report).getByRole('button', {
        name: 'Reveal destination',
    }));
    expect(revealExportDestination).toHaveBeenCalledWith('finished-export');
});

test('restores, displays, and polls every concurrent active import', async () => {
    const first = {
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'running',
        completedItems: 0,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            sourceName: 'clockwork.zip',
            status: 'running',
            completedBytes: 512,
            totalBytes: 1024,
        }],
    };
    const second = {
        ...first,
        id: '11112233-4455-4677-8899-aabbccddeeff',
        createdAt: '2026-07-25T09:01:00Z',
        items: [{
            ...first.items[0],
            id: 2,
            sourceName: 'forest.zip',
        }],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({
        operations: [second, first],
    }));
    operationSnapshot.mockImplementation(async (operationID) => (
        new app.OperationResponse({
            operation: operationID === first.id ? first : second,
        })
    ));

    render(<App/>);

    expect(await screen.findAllByRole('heading', {name: 'Importing 1 story'}))
        .toHaveLength(2);
    expect(operationSnapshot).toHaveBeenCalledWith(first.id);
    expect(operationSnapshot).toHaveBeenCalledWith(second.id);
    expect(screen.getAllByRole('button', {name: 'Cancel import'})).toHaveLength(2);
});

test('reloads selected story detail when an operation refreshes the collection', async () => {
    const user = userEvent.setup();
    loadStoryDetail
        .mockResolvedValueOnce(new app.StoryDetailResponse({
            detail: {
                story: stories[0],
                archive: {
                    originalFilename: 'before-refresh.zip',
                    detectedFormat: 'zip',
                    sha256: 'a'.repeat(64),
                    byteSize: stories[0].byteSize,
                    verification: 'compatible',
                },
            },
        }))
        .mockResolvedValue(new app.StoryDetailResponse({
            detail: {
                story: stories[0],
                archive: {
                    originalFilename: 'after-refresh.zip',
                    detectedFormat: 'zip',
                    sha256: 'b'.repeat(64),
                    byteSize: stories[0].byteSize,
                    verification: 'compatible',
                },
            },
        }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'Clockwork Forest'}));
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    await user.click(screen.getByRole('button', {name: 'Show technical details'}));
    expect(await screen.findByText('before-refresh.zip'))
        .toBeInTheDocument();
    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            storyId: 1,
            sourceName: 'clockwork.zip',
            status: 'succeeded',
            outcomeCode: 'imported',
            outcomeMessage: 'Story imported.',
            completedBytes: 1024,
            totalBytes: 1024,
        }],
    });

    expect(await screen.findByText('after-refresh.zip'))
        .toBeInTheDocument();
    expect(loadStoryDetail).toHaveBeenCalledTimes(2);
});

test('cancels story removal without changing the collection', async () => {
    const user = userEvent.setup();
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    expect(screen.getByRole('dialog', {name: 'Clockwork Forest'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Close'}));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(removeStory).not.toHaveBeenCalled();
    expect(screen.getByRole('button', {name: 'Clockwork Forest'}))
        .toBeInTheDocument();
});

test('confirms removal, refreshes the collection, and reports trash custody', async () => {
    const user = userEvent.setup();
    queryStories
        .mockResolvedValueOnce(new app.LibraryPageResponse({
            page: {
                stories,
                page: 1,
                pageSize: 12,
                totalItems: 2,
                totalPages: 1,
                sort: 'imported_desc',
            },
        }))
        .mockResolvedValue(new app.LibraryPageResponse({
            page: {
                stories: [stories[1]],
                page: 1,
                pageSize: 12,
                totalItems: 1,
                totalPages: 1,
                sort: 'imported_desc',
            },
        }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    await user.click(screen.getByRole('button', {name: 'Remove from library…'}));
    await user.click(screen.getByRole('button', {name: 'Move to trash'}));

    expect(removeStory).toHaveBeenCalledWith(1);
    expect(await screen.findByRole('heading', {
        name: 'Story removed from this collection',
    })).toBeInTheDocument();
    expect(screen.getByText('Clockwork Forest was moved to application trash.'))
        .toBeInTheDocument();
    expect(screen.queryByRole('button', {name: 'Clockwork Forest'}))
        .not.toBeInTheDocument();
});

test('completes the import, list, select, detail, and remove workflow', async () => {
    const user = userEvent.setup();
    const emptyPage = new app.LibraryPageResponse({
        page: {
            stories: [],
            page: 1,
            pageSize: 12,
            totalItems: 0,
            totalPages: 0,
            sort: 'imported_desc',
        },
    });
    const importedPage = new app.LibraryPageResponse({
        page: {
            stories: [stories[0]],
            page: 1,
            pageSize: 12,
            totalItems: 1,
            totalPages: 1,
            sort: 'imported_desc',
        },
    });
    queryStories
        .mockResolvedValueOnce(emptyPage)
        .mockResolvedValueOnce(importedPage)
        .mockResolvedValue(emptyPage);
    selectAndImportStories.mockResolvedValue(new app.OperationResponse({
        operation: {
            id: '00112233-4455-4677-8899-aabbccddeeff',
            kind: 'import',
            status: 'queued',
            completedItems: 0,
            totalItems: 1,
            cancelRequested: false,
            createdAt: '2026-07-25T09:00:00Z',
            items: [{
                id: 1,
                sourceName: 'clockwork.zip',
                status: 'pending',
                completedBytes: 0,
                totalBytes: 1048576,
            }],
        },
    }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {
        name: '＋ Import your first stories',
    }));
    operationChanged?.({
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: 'succeeded',
        completedItems: 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            storyId: 1,
            sourceName: 'clockwork.zip',
            status: 'succeeded',
            outcomeCode: 'imported',
            outcomeMessage: 'Story imported.',
            completedBytes: 1048576,
            totalBytes: 1048576,
        }],
    });

    const cover = await screen.findByRole('button', {name: 'Clockwork Forest'});
    expect(cover).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', {name: 'Open details'})).toBeEnabled();
    await user.click(screen.getByRole('button', {name: 'Open details'}));
    expect(screen.getByRole('dialog', {name: 'Clockwork Forest'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Remove from library…'}));
    await user.click(screen.getByRole('button', {name: 'Move to trash'}));

    expect(removeStory).toHaveBeenCalledWith(1);
    expect(await screen.findByRole('heading', {name: 'Build your local story archive'}))
        .toBeInTheDocument();
    expect(screen.getByText('Clockwork Forest was moved to application trash.'))
        .toBeInTheDocument();
});

test('loads every result when a story is beyond the initial collection page', async () => {
    const user = userEvent.setup();
    const firstBatchStory = new library.StorySummary({
        ...stories[1],
        id: 12,
        uuid: '12223333-4444-4555-8666-777788889999',
        title: 'The Twelfth Story',
    });
    const extraStory = new library.StorySummary({
        ...stories[1],
        id: 13,
        uuid: '22223333-4444-4555-8666-777788889999',
        title: 'The Thirteenth Story',
    });
    const initial = new app.LibraryPageResponse({
        page: {
            stories,
            page: 1,
            pageSize: 12,
            totalItems: 3,
            totalPages: 1,
            sort: 'imported_desc',
        },
    });
    let resolveSecondBatch: (
        response: app.LibraryPageResponse,
    ) => void = () => undefined;
    const secondBatch = new Promise<app.LibraryPageResponse>((resolve) => {
        resolveSecondBatch = resolve;
    });
    queryStories
        .mockResolvedValueOnce(initial)
        .mockResolvedValueOnce(new app.LibraryPageResponse({
            page: {
                stories: [...stories, firstBatchStory],
                page: 1,
                pageSize: 50,
                totalItems: 4,
                totalPages: 2,
                sort: 'imported_desc',
            },
        }))
        .mockReturnValueOnce(secondBatch);
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'View all →'}));

    expect(await screen.findByRole('button', {
        name: 'The Twelfth Story',
    })).toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Loading…'})).toBeDisabled();
    resolveSecondBatch(new app.LibraryPageResponse({
            page: {
                stories: [extraStory],
                page: 2,
                pageSize: 50,
                totalItems: 4,
                totalPages: 2,
                sort: 'imported_desc',
            },
        }));

    expect(await screen.findByRole('button', {
        name: 'The Thirteenth Story',
    })).toBeInTheDocument();
    expect(queryStories).toHaveBeenCalledWith(expect.objectContaining({
        page: 2,
        pageSize: 50,
        sort: 'imported_desc',
        name: '',
        booleanFilters: [],
        choiceFilters: [],
    }));
});


test('dismisses the Lunii panel and reopens sync without starting a refresh', async () => {
    const user = userEvent.setup();
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {state: 'fresh', activatedAt: '2026-07-25T16:01:00Z'},
    }));
    const view = render(<App/>);

    const panel = await screen.findByRole('region', {name: 'Official Lunii sync'});
    expect(within(panel).getByRole('heading', {name: 'Lunii covers and story details'}))
        .toBeInTheDocument();
    await user.click(within(panel).getByRole('button', {name: 'Close Lunii sync'}));
    expect(screen.queryByRole('region', {name: 'Official Lunii sync'})).not.toBeInTheDocument();
    const reopen = screen.getByRole('button', {name: 'Show Lunii sync'});
    expect(reopen).toHaveFocus();
    view.unmount();
    render(<App/>);
    await screen.findByRole('heading', {name: 'Clockwork Forest'});
    expect(screen.queryByRole('region', {name: 'Official Lunii sync'})).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Show Lunii sync'}));
    expect(await screen.findByRole('region', {name: 'Official Lunii sync'}))
        .toHaveTextContent('Last successful sync:');
    expect(refreshOfficialMetadata).not.toHaveBeenCalled();
});

test('keeps a dismissed export hidden after reopening the application', async () => {
    const user = userEvent.setup();
    activeOperations.mockResolvedValue(new app.OperationListResponse({operations: [{
        id: 'persisted-export', kind: 'export', status: 'succeeded',
        completedItems: 1, totalItems: 1, totalBytes: 100,
        createdAt: '2026-07-25T09:00:00Z', finishedAt: '2026-07-25T09:01:00Z',
        cancelRequested: false, items: [],
    }]}));
    const view = render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Close Export complete'}));
    view.unmount();
    render(<App/>);
    await screen.findByRole('heading', {name: 'Clockwork Forest'});
    expect(screen.queryByRole('heading', {name: 'Export complete'})).not.toBeInTheDocument();
});

test('dismisses a finished export without restoring it on later events or hiding new exports', async () => {
    const user = userEvent.setup();
    const finished = {
        id: 'dismissed-export', kind: 'export', status: 'succeeded',
        completedItems: 1, totalItems: 1, totalBytes: 100,
        createdAt: '2026-07-25T09:00:00Z', finishedAt: '2026-07-25T09:01:00Z',
        cancelRequested: false, items: [],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({operations: [finished]}));
    render(<App/>);

    expect(await screen.findByRole('heading', {name: 'Export complete'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Close Export complete'}));
    expect(screen.queryByRole('region', {name: 'Export report'})).not.toBeInTheDocument();
    expect(cancelOperation).not.toHaveBeenCalled();
    expect(screen.getByRole('searchbox', {name: 'Search stories'})).toHaveFocus();

    operationChanged?.(finished);
    operationChanged?.({
        ...finished, id: 'later-import', kind: 'import',
        items: [{id: 1, sourceName: 'later.zip', status: 'succeeded',
            outcomeCode: 'imported', completedBytes: 100, totalBytes: 100}],
    });
    expect(await screen.findByRole('heading', {name: '1 story imported'})).toBeInTheDocument();
    expect(screen.queryByRole('heading', {name: 'Export complete'})).not.toBeInTheDocument();
    operationChanged?.({...finished, id: 'new-export'});
    expect(await screen.findByRole('heading', {name: 'Export complete'})).toBeInTheDocument();
});


test('opens persisted history, including dismissed exports and older activity', async () => {
    const user = userEvent.setup();
    const exported = {
        id: 'history-export', kind: 'export', status: 'succeeded', destination: 'Lunii export',
        completedItems: 1, totalItems: 1, totalBytes: 100,
        createdAt: '2026-10-19T09:00:00Z', finishedAt: '2026-10-19T09:01:00Z',
        cancelRequested: false, items: [{id: 1, sourceName: 'forest.zip', status: 'succeeded',
            outcomeMessage: 'Story archive exported.', completedBytes: 100, totalBytes: 100}],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({operations: [exported]}));
    operationHistory.mockResolvedValueOnce(new app.OperationHistoryResponse({page: {
        operations: [exported], nextBefore: 'history-export',
    }})).mockResolvedValueOnce(new app.OperationHistoryResponse({page: {
        operations: [{...exported, id: 'older-import', kind: 'import', status: 'failed',
            createdAt: '2026-10-18T09:00:00Z', errorMessage: 'Unsupported archive.'}],
    }}));
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Close Export complete'}));
    const historyButton = screen.getByRole('button', {name: 'History'});
    await user.click(historyButton);
    const dialog = screen.getByRole('dialog', {name: 'History'});
    await user.click(await within(dialog).findByText('Export', {exact: true}));
    const report = within(dialog).getByRole('region', {name: 'Export report'});
    expect(report).toHaveTextContent('forest.zip');
    await user.click(within(report).getByRole('button', {name: 'Reveal destination'}));
    expect(revealExportDestination).toHaveBeenCalledWith('history-export');
    await user.click(within(dialog).getByRole('button', {name: 'Load older activity'}));
    expect(await within(dialog).findByText('Import', {exact: true})).toBeInTheDocument();
    expect(operationHistory.mock.calls).toEqual([[''], ['history-export']]);
    expect(within(dialog).queryByRole('button', {name: 'Load older activity'})).not.toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', {name: 'History'})).not.toBeInTheDocument();
    expect(historyButton).toHaveFocus();
    expect(screen.queryByRole('heading', {name: 'Export complete'})).not.toBeInTheDocument();
});

test('shows an actionable history error and retries to the empty state', async () => {
    const user = userEvent.setup();
    operationHistory.mockRejectedValueOnce(new Error('offline'));
    render(<App/>);
    await screen.findByRole('heading', {name: 'Clockwork Forest'});
    await user.click(screen.getByRole('button', {name: 'History'}));
    const dialog = screen.getByRole('dialog', {name: 'History'});
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('History could not be loaded.');
    await user.click(within(dialog).getByRole('button', {name: 'Try again'}));
    expect(await within(dialog).findByText(/No activity yet/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', {name: 'Close history'}));
    expect(screen.queryByRole('dialog', {name: 'History'})).not.toBeInTheDocument();
});


test('keeps saved terminal history newer than stale progress and explains reveal failures', async () => {
    const user = userEvent.setup();
    const operation = {
        id: 'recent-export', kind: 'export', completedItems: 1, totalItems: 1,
        totalBytes: 100, cancelRequested: false, createdAt: '2026-10-19T09:00:00Z', items: [],
    };
    activeOperations.mockResolvedValue(new app.OperationListResponse({operations: [
        {...operation, status: 'running'},
    ]}));
    operationHistory.mockResolvedValue(new app.OperationHistoryResponse({page: {operations: [
        {...operation, status: 'succeeded', finishedAt: '2026-10-19T09:01:00Z'},
    ]}}));
    revealExportDestination.mockResolvedValue(new app.MutationResponse({
        error: {code: 'not_found', message: 'The export folder is no longer available.'},
    }));
    render(<App/>);
    await screen.findByRole('heading', {name: 'Exporting story archives'});
    await user.click(screen.getByRole('button', {name: 'History'}));
    const dialog = screen.getByRole('dialog', {name: 'History'});
    expect(await within(dialog).findByText('Completed', {exact: true})).toBeInTheDocument();
    await user.click(within(dialog).getByText('Export', {exact: true}));
    await user.click(within(dialog).getByRole('button', {name: 'Reveal destination'}));
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('The export folder is no longer available.');
});


test('switches interface language without changing the selected story or query', async () => {
 const user = userEvent.setup();
 render(<App/>);
 const story = await screen.findByRole('button', {name: 'Clockwork Forest'});
 await user.click(story);
 const query = window.location.hash;
 const refreshCalls = refreshOfficialMetadata.mock.calls.length;
 act(() => setLanguage('fr'));
 expect(screen.getByRole('navigation', {name: 'Navigation principale'})).toBeInTheDocument();
 expect(screen.getByRole('button', {name: 'Clockwork Forest'})).toHaveAttribute('aria-pressed','true');
 expect(window.location.hash).toBe(query);
 expect(document.documentElement.lang).toBe('fr');
 expect(refreshOfficialMetadata.mock.calls.length).toBe(refreshCalls);
 act(() => setLanguage('en'));
});

test('updates the recently added heading when switching to French', async () => {
    render(<App/>);
    expect(await screen.findByRole('heading', {name: 'Recently added'})).toBeInTheDocument();
    act(() => setLanguage('fr'));
    expect(screen.getByRole('heading', {name: 'Ajout récent'})).toBeInTheDocument();
});

test('saves the union of selected collections instead of only the displayed collection', async () => {
    const user = userEvent.setup();
    listCollections.mockResolvedValue(new app.CollectionListResponse({collections: [
        {id: 7, name: 'arc', position: 0, validity: 'valid', count: 1},
        {id: 8, name: 'aqua', position: 1, validity: 'valid', count: 1},
    ]}));
    openCollection.mockImplementation(async (id) => new app.CollectionEvaluationResponse({evaluation: {
        collection: {id, name: id === 7 ? 'arc' : 'aqua', validity: 'valid'},
        query: {name: id === 7 ? 'forest' : 'moon'},
        page: {stories: [stories[id === 7 ? 0 : 1]], page: 1, pageSize: 12, totalItems: 1, totalPages: 1, sort: 'imported_desc'},
    }}));
    createCollection.mockResolvedValue(new app.CollectionResponse({collection: {id: 9, name: 't2', validity: 'valid'}}));
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'arc, 1 story'}));
    await screen.findByRole('heading', {name: 'arc'});
    await user.click(screen.getByRole('checkbox', {name: "Select arc for combined collection preview"}));
    await user.click(screen.getByRole('checkbox', {name: "Select aqua for combined collection preview"}));
    await user.click(screen.getByRole('button', {name: '＋ Save current query'}));
    await user.type(screen.getByRole('textbox', {name: "Collection name"}), 't2');
    await user.click(screen.getByRole('button', {name: "Save collection"}));
    await waitFor(() => expect(createCollection).toHaveBeenCalledWith('t2', expect.objectContaining({
        anyOf: [expect.objectContaining({name: 'forest'}), expect.objectContaining({name: 'moon'})],
    })));
});

type FlagFixtureQuery = {
    brokenVisibility?: string;
    brokenState?: string;
    anyOf?: FlagFixtureQuery[];
    name?: string;
    page?: number;
    pageSize?: number;
};

function installStoryFlagsFixture(initialFlags: number[] = [], fixtureStories = stories) {
    const state = new Map(fixtureStories.map((story) => [story.id, initialFlags.includes(story.id)]));
    const summaries = () => fixtureStories.map((story) => new library.StorySummary({...story, broken: state.get(story.id)}));
    const matchesCriteria = (story: library.StorySummary, query: FlagFixtureQuery): boolean => (
        (!query.name || story.title.toLowerCase().includes(query.name.toLowerCase())) &&
        (!query.anyOf?.length || query.anyOf.some((branch) => matchesCriteria(story, branch)))
    );
    const pageFor = (request: FlagFixtureQuery) => {
        const matches = summaries().filter((story) => matchesCriteria(story, request));
        const size = request.pageSize || 12;
        const page = request.page || 1;
        return {stories: matches.slice((page - 1) * size, page * size), page, pageSize: size,
            totalItems: matches.length, totalPages: Math.ceil(matches.length / size), sort: 'imported_desc'};
    };
    queryStories.mockImplementation(async (query) => new app.LibraryPageResponse({page: pageFor(query)}));
    loadStoryDetail.mockImplementation(async (id) => new app.StoryDetailResponse({detail: {
        story: summaries().find((story) => story.id === id),
        archive: {originalFilename: 'story.zip', detectedFormat: 'zip', verification: 'compatible', sha256: 'a'.repeat(64), byteSize: 10},
    }}));
    vi.mocked(SetStoriesBroken).mockImplementation(async (ids, broken) => {
        const unique = [...new Set(ids)];
        const changes = unique.filter((id) => state.get(id) !== broken)
            .map((id) => ({storyId: id, previous: !!state.get(id), current: broken}));
        for (const change of changes) state.set(change.storyId, change.current);
        return new app.StoryFlagsResponse({result: {requestedStories: unique.length, changedStories: changes.length, changes}});
    });
    vi.mocked(RestoreStoryFlags).mockImplementation(async (changes) => {
        const reversed = changes.map((change) => ({storyId: change.storyId, previous: change.current, current: change.previous}));
        for (const change of reversed) state.set(change.storyId, change.current);
        return new app.StoryFlagsResponse({result: {requestedStories: reversed.length, changedStories: reversed.length, changes: reversed}});
    });
    return {state, pageFor};
}

test.each(['List', 'Small covers', 'Large covers'])('flags and undoes directly from the %s index', async (view) => {
    installStoryFlagsFixture();
    const user = userEvent.setup();
    render(<App/>);
    await screen.findByRole('button', {name: 'Clockwork Forest'});
    await user.click(screen.getByRole('button', {name: view}));
    const action = screen.getByRole('button', {name: 'Mark Clockwork Forest as broken'});
    expect(action.closest('.story')).toBeNull();
    await user.click(action);
    expect(SetStoriesBroken).toHaveBeenCalledWith([1], true);
    await waitFor(() => expect(screen.getAllByRole('button', {name: 'Clockwork Forest'}).length).toBeGreaterThan(0));
    expect(screen.getByRole('status')).toHaveTextContent('1 of 1 stories updated.');
    expect(screen.getByRole('button', {name: 'Clear broken flag for Clockwork Forest'})).toHaveFocus();
    await user.click(screen.getByRole('button', {name: 'Undo'}));
    expect(await screen.findByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(RestoreStoryFlags).toHaveBeenCalledWith([expect.objectContaining({storyId: 1, previous: false, current: true})]);
});

test('clears and restores flagged stories without changing visible membership', async () => {
    installStoryFlagsFixture([1]);
    const user = userEvent.setup(); render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Clear broken flag for Clockwork Forest'}));
    expect(await screen.findByRole('button', {name: 'Mark Clockwork Forest as broken'})).toHaveFocus();
    expect(screen.getByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Undo'}));
    expect(await screen.findByRole('button', {name: 'Clear broken flag for Clockwork Forest'})).toBeInTheDocument();
});

test('keeps individual flag actions separate from mixed bulk selection and restores only changed flags', async () => {
    const {state} = installStoryFlagsFixture();
    const user = userEvent.setup();
    render(<App/>);
    await screen.findByRole('button', {name: 'Clockwork Forest'});
    const second = await screen.findByRole('button', {name: 'Moonlit Workshop'});
    await user.keyboard('{Control>}');
    await user.click(second);
    await user.keyboard('{/Control}');
    await user.click(screen.getByRole('button', {name: 'Mark Clockwork Forest as broken'}));
    expect(SetStoriesBroken).toHaveBeenLastCalledWith([1], true);
    expect(await screen.findByText('2 selected · 1 flagged')).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Mark selected as broken'}));
    expect(SetStoriesBroken).toHaveBeenLastCalledWith([1, 2], true);
    expect(await screen.findByText('2 selected · 2 flagged')).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Undo'}));
    await waitFor(() => expect(state.get(2)).toBe(false));
    expect(state.get(1)).toBe(true);
    expect(RestoreStoryFlags).toHaveBeenLastCalledWith([expect.objectContaining({storyId: 2, previous: false, current: true})]);
});

test('keeps inspected and rendered stories available after flagging', async () => {
    installStoryFlagsFixture();
    const user = userEvent.setup();
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    const dialog = await screen.findByRole('dialog', {name: 'Clockwork Forest'});
    await user.click(within(dialog).getByRole('button', {name: 'Mark as broken'}));
    await waitFor(() => expect(screen.getAllByRole('button', {name: 'Clockwork Forest'}).length).toBeGreaterThan(0));
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByRole('region', {name: 'Story warning'})).toHaveTextContent('Broken');
    await user.click(within(dialog).getByRole('button', {name: 'Clear broken flag'}));
    expect(await screen.findByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(SetStoriesBroken).toHaveBeenLastCalledWith([1], false);
});

test('prevents repeated pending flag actions and preserves results after a failed write', async () => {
    installStoryFlagsFixture();
    let finish: ((response: app.StoryFlagsResponse) => void) | undefined;
    vi.mocked(SetStoriesBroken).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const user = userEvent.setup();
    render(<App/>);
    const action = await screen.findByRole('button', {name: 'Mark Clockwork Forest as broken'});
    await user.click(action);
    expect(action).toBeDisabled();
    await user.click(action);
    expect(SetStoriesBroken).toHaveBeenCalledTimes(1);
    await act(async () => finish?.(new app.StoryFlagsResponse({error: {code: 'internal', message: 'Flag update failed.'}})));
    expect(await screen.findByRole('alert')).toHaveTextContent('Flag update failed.');
    expect(screen.getByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(screen.queryByRole('button', {name: 'Undo'})).not.toBeInTheDocument();
});

test('reports no-op flag writes without offering Undo', async () => {
    installStoryFlagsFixture();
    vi.mocked(SetStoriesBroken).mockResolvedValue(new app.StoryFlagsResponse({result: {requestedStories: 1, changedStories: 0, changes: []}}));
    const user = userEvent.setup();
    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Mark Clockwork Forest as broken'}));
    expect(await screen.findByRole('status')).toHaveTextContent('No broken flags changed.');
    expect(screen.queryByRole('button', {name: 'Undo'})).not.toBeInTheDocument();
});

test('flagging retains the current final page and focused control', async () => {
    installStoryFlagsFixture();
    window.history.replaceState(null, '', '/#/library?v=2&page=2&size=1');
    const user = userEvent.setup(); render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Mark Moonlit Workshop as broken'}));
    expect(await screen.findByRole('button', {name: 'Clear broken flag for Moonlit Workshop'})).toHaveFocus();
    expect(queryStories).toHaveBeenLastCalledWith(expect.objectContaining({page: 2, pageSize: 1}));
    expect(screen.getByRole('button', {name: 'Moonlit Workshop'})).toBeInTheDocument();
});

test('normalizes retired tag URL criteria without a Broken filter chip', async () => {
    installStoryFlagsFixture([1]);
    vi.mocked(LibraryCompatibility).mockResolvedValue(new app.LibraryCompatibilityResponse({compatibility: {legacyBrokenTagId: 9}}));
    window.history.replaceState(null, '', '/#/library?v=1&includeBroken=true&bool=9:true');
    render(<App/>);
    await screen.findByRole('button', {name: 'Clockwork Forest'});
    await waitFor(() => expect(window.location.hash).toContain('v=3'));
    expect(window.location.hash).not.toContain('bool=');
    expect(queryStories.mock.lastCall?.[0]).toHaveProperty('brokenState', undefined);
    expect(screen.queryByRole('button', {name: 'Remove filter Broken flag: set'})).not.toBeInTheDocument();
});

test('updates duplicate story copies in saved collection previews', async () => {
    const {pageFor} = installStoryFlagsFixture();
    listCollections.mockImplementation(async () => new app.CollectionListResponse({collections: [{id: 7, name: 'Favorites', position: 0, validity: 'valid', count: pageFor({}).totalItems}]}));
    openCollection.mockImplementation(async (_id, request) => new app.CollectionEvaluationResponse({evaluation: {
        collection: {id: 7, name: 'Favorites', normalizedName: 'favorites', position: 0, queryVersion: 4, queryPayload: '{}', validity: 'valid'},
        query: {}, page: pageFor(request),
    }}));
    const user = userEvent.setup();
    render(<App/>);
    await waitFor(() => expect(screen.getAllByRole('button', {name: 'Mark Clockwork Forest as broken'})).toHaveLength(2));
    await user.click(screen.getAllByRole('button', {name: 'Mark Clockwork Forest as broken'})[1]);
    await waitFor(() => expect(screen.getAllByRole('button', {name: 'Clockwork Forest'}).length).toBeGreaterThan(0));
    expect(await screen.findByRole('button', {name: 'Favorites, 2 stories'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Undo'}));
    await waitFor(() => expect(screen.getAllByRole('button', {name: 'Clockwork Forest'})).toHaveLength(2));
});

test.each(['true', 'false'] as const)('legacy brokenState=%s URLs retain both results and focus after flag changes', async (brokenState) => {
    installStoryFlagsFixture(brokenState === 'true' ? [1, 2] : []);
    window.history.replaceState(null, '', `/#/library?v=2&brokenVisibility=include&brokenState=${brokenState}`);
    const user = userEvent.setup(); render(<App/>);
    const action = await screen.findByRole('button', {name: brokenState === 'true' ? 'Clear broken flag for Clockwork Forest' : 'Mark Clockwork Forest as broken'});
    action.focus(); await user.keyboard('{Enter}');
    expect(await screen.findByRole('button', {name: brokenState === 'true' ? 'Mark Clockwork Forest as broken' : 'Clear broken flag for Clockwork Forest'})).toHaveFocus();
    expect(screen.getByRole('button', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Moonlit Workshop'})).toBeInTheDocument();
});

test('normalizes nested legacy flag alternatives while preserving nonflag criteria', async () => {
    installStoryFlagsFixture([1, 2]);
    const branch = {...DEFAULT_COLLECTION_QUERY, brokenVisibility: 'include', brokenState: 'true'};
    window.history.replaceState(null, '', `/#/library?v=2&any=${encodeURIComponent(JSON.stringify([{...branch, anyOf: [branch]}, {...DEFAULT_COLLECTION_QUERY, name: 'Clockwork Forest'}]))}`);
    const user = userEvent.setup(); render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Clear broken flag for Clockwork Forest'}));
    expect(await screen.findByRole('button', {name: 'Mark Clockwork Forest as broken'})).toHaveFocus();
    expect(screen.getByRole('button', {name: 'Moonlit Workshop'})).toBeInTheDocument();
    expect(queryStories.mock.lastCall?.[0].anyOf?.[0].anyOf?.[0]).toHaveProperty('brokenState', undefined);
});


test('restores a curated Shelf without opening a same-id migrated Collection', async () => {
    vi.mocked(ListShelves).mockResolvedValue(new app.ShelfListResponse({shelves: [{id: 7, name: 'Curated', count: 0}]}));
    vi.mocked(OpenShelf).mockResolvedValue(new app.ShelfEvaluationResponse({evaluation: {shelf: {id: 7, name: 'Curated', count: 0}, members: [], sources: [], exclusions: [], page: 1, totalPages: 0}}));
    const hash = encodeCollectionQuery({...DEFAULT_COLLECTION_QUERY, name: 'refined'});
    window.history.replaceState({collectionQuery: {hash, group: {kind: 'shelf', id: 7}, scope: 'shelf'}}, '', `/${hash}`);
    render(<App/>);
    expect(await screen.findByRole('heading', {name: 'Curated'})).toBeInTheDocument();
    expect(screen.getByRole('searchbox', {name: 'Refine this view'})).toHaveValue('refined');
    expect(OpenShelf).toHaveBeenCalledWith(7, expect.objectContaining({name: 'refined'}));
    expect(OpenCollection).not.toHaveBeenCalled();
    expect(window.history.state.collectionQuery.group).toEqual(expect.objectContaining({kind: 'shelf', id: 7}));
});

test('adds the selected story UUID explicitly to an existing Shelf', async () => {
    vi.mocked(ListShelves).mockResolvedValue(new app.ShelfListResponse({shelves: [{id: 7, name: 'Curated', count: 0}]}));
    vi.mocked(AddStoriesToShelf).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    const user = userEvent.setup(); render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Add to Shelf…'}));
    await user.selectOptions(await screen.findByLabelText('Add selection to Shelf'), '7');
    expect(AddStoriesToShelf).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', {name: 'Add selected stories'}));
    await waitFor(() => expect(AddStoriesToShelf).toHaveBeenCalledWith(7, [stories[0].uuid]));
});
