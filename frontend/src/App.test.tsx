import {fireEvent, render, screen, waitFor, within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
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
    SetBooleanTag,
    StoryDetail as LoadStoryDetail,
    TagAssignmentWorkspace as LoadTagAssignmentWorkspace,
    TagCatalog as LoadTagCatalog,
} from '../wailsjs/go/main/App';
import {app, library, metadata, shelves} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import App from './App';

vi.mock('../wailsjs/go/main/App', () => ({
    ActiveOperations: vi.fn(),
    ApplicationStatus: vi.fn(),
    CancelOperation: vi.fn(),
    CreateShelf: vi.fn(),
    DeleteShelf: vi.fn(),
    DuplicateShelf: vi.fn(),
    ListShelves: vi.fn(),
    OfficialMetadataStatus: vi.fn(),
    OpenShelf: vi.fn(),
    OperationSnapshot: vi.fn(),
    PreviewShelves: vi.fn(),
    QueryStories: vi.fn(),
    RenameShelf: vi.fn(),
    ReorderShelves: vi.fn(),
    ReplaceShelfQuery: vi.fn(),
    RefreshOfficialMetadata: vi.fn(),
    RemoveStory: vi.fn(),
    SelectAndImportStories: vi.fn(),
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
const createShelf = vi.mocked(CreateShelf);
const deleteShelf = vi.mocked(DeleteShelf);
const duplicateShelf = vi.mocked(DuplicateShelf);
const listShelves = vi.mocked(ListShelves);
const officialMetadataStatus = vi.mocked(OfficialMetadataStatus);
const openShelf = vi.mocked(OpenShelf);
const operationSnapshot = vi.mocked(OperationSnapshot);
const previewShelves = vi.mocked(PreviewShelves);
const queryStories = vi.mocked(QueryStories);
const renameShelf = vi.mocked(RenameShelf);
const reorderShelves = vi.mocked(ReorderShelves);
const replaceShelfQuery = vi.mocked(ReplaceShelfQuery);
const refreshOfficialMetadata = vi.mocked(RefreshOfficialMetadata);
const removeStory = vi.mocked(RemoveStory);
const selectAndImportStories = vi.mocked(SelectAndImportStories);
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
    window.history.replaceState(null, '', '/');
    activeOperations.mockReset();
    applicationStatus.mockReset();
    cancelOperation.mockReset();
    createShelf.mockReset();
    deleteShelf.mockReset();
    duplicateShelf.mockReset();
    listShelves.mockReset();
    officialMetadataStatus.mockReset();
    openShelf.mockReset();
    operationSnapshot.mockReset();
    previewShelves.mockReset();
    queryStories.mockReset();
    renameShelf.mockReset();
    reorderShelves.mockReset();
    replaceShelfQuery.mockReset();
    refreshOfficialMetadata.mockReset();
    removeStory.mockReset();
    selectAndImportStories.mockReset();
    setBooleanTag.mockReset();
    loadStoryDetail.mockReset();
    loadTagAssignmentWorkspace.mockReset();
    loadTagCatalog.mockReset();
    eventsOn.mockReset();
    operationChanged = undefined;

    applicationStatus.mockResolvedValue(new app.StatusResponse({
        status: {
            state: 'ready',
            mutationsAllowed: true,
        },
    }));
    activeOperations.mockResolvedValue(new app.OperationListResponse({
        operations: [],
    }));
    listShelves.mockResolvedValue(new app.ShelfListResponse({
        shelves: [],
    }));
    officialMetadataStatus.mockResolvedValue(new app.MetadataStatusResponse({
        status: {
            state: 'never_synced',
            locale: 'en-GB',
            matchedStoryCount: 0,
        },
    }));
    operationSnapshot.mockResolvedValue(new app.OperationResponse({}));
    previewShelves.mockResolvedValue(new app.ShelfSelectionPreviewResponse({}));
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
                key: 'broken',
                normalizedKey: 'broken',
                label: 'Broken',
                color: '#ff705c',
                kind: 'boolean',
                source: 'builtin',
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

test('renders the canonical collection shell from typed library data', async () => {
    render(<App/>);

    expect(await screen.findByRole('heading', {name: 'Clockwork Forest'})).toBeInTheDocument();
    expect(screen.getByRole('heading', {name: 'My story shelves'})).toBeInTheDocument();
    expect(screen.getByRole('navigation', {name: 'Primary navigation'})).toBeInTheDocument();
    expect(screen.getByText('Stories in your local archive · 2 archives')).toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Clockwork Forest Lunii'}))
        .toHaveAttribute('aria-pressed', 'true');
    expect(await screen.findByText('clockwork-forest.zip · 1.0 MB · Verified'))
        .toBeInTheDocument();
});

test('renders saved shelf preview rows and opens them from view all', async () => {
    const user = userEvent.setup();
    const summaries = [
        new shelves.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        }),
        new shelves.Summary({
            id: 8,
            name: 'Adventures',
            position: 1,
            validity: 'valid',
            count: 1,
        }),
    ];
    listShelves.mockResolvedValue(new app.ShelfListResponse({shelves: summaries}));
    openShelf.mockImplementation(async (shelfID) => {
        const bedtime = shelfID === 7;
        return new app.ShelfEvaluationResponse({
            evaluation: {
                shelf: {
                    id: shelfID,
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
    expect(within(bedtimeRow!).getByText('1 story · Saved shelf'))
        .toBeInTheDocument();
    await waitFor(() => expect(within(adventuresRow!).getByText('Clockwork Forest'))
        .toBeInTheDocument());
    await waitFor(() => expect(openShelf).toHaveBeenCalledWith(
        7,
        expect.objectContaining({page: 1, pageSize: 6, sort: 'imported_desc'}),
    ));

    await user.click(within(bedtimeRow!).getByRole('button', {name: 'View all →'}));
    await waitFor(() => expect(openShelf).toHaveBeenCalledWith(
        7,
        expect.objectContaining({page: 1, pageSize: 12, sort: 'imported_desc'}),
    ));
    expect(screen.getByRole('button', {name: 'Bedtime, 1 story'}))
        .toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('moon');
});

test('opens a dynamic saved shelf and updates it from the current query', async () => {
    const user = userEvent.setup();
    const summary = new shelves.Summary({
        id: 7,
        name: 'Bedtime',
        position: 0,
        validity: 'valid',
        count: 1,
    });
    listShelves.mockResolvedValue(new app.ShelfListResponse({
        shelves: [summary],
    }));
    openShelf.mockResolvedValue(new app.ShelfEvaluationResponse({
        evaluation: {
            shelf: {
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
    replaceShelfQuery.mockResolvedValue(new app.ShelfResponse({
        shelf: {
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
    await waitFor(() => expect(openShelf).toHaveBeenCalledWith(
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
    await waitFor(() => expect(replaceShelfQuery).toHaveBeenCalledWith(
        7,
        expect.objectContaining({name: 'moon', page: 1}),
    ));
});

test('restores active shelf identity across back, forward, and reload', async () => {
    const user = userEvent.setup();
    const summaries = [
        new shelves.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        }),
        new shelves.Summary({
            id: 8,
            name: 'Adventures',
            position: 1,
            validity: 'valid',
            count: 1,
        }),
    ];
    listShelves.mockResolvedValue(new app.ShelfListResponse({shelves: summaries}));
    openShelf.mockImplementation(async (shelfID) => {
        const bedtime = shelfID === 7;
        const name = bedtime ? 'moon' : 'forest';
        const shelfName = bedtime ? 'Bedtime' : 'Adventures';
        return new app.ShelfEvaluationResponse({
            evaluation: {
                shelf: {
                    id: shelfID,
                    name: shelfName,
                    normalizedName: shelfName.toLowerCase(),
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

test('saves the current query as a named active shelf with its live count', async () => {
    const user = userEvent.setup();
    const saved = new shelves.Summary({
        id: 8,
        name: 'Moon shelf',
        position: 0,
        validity: 'valid',
        count: 2,
    });
    listShelves
        .mockResolvedValueOnce(new app.ShelfListResponse({shelves: []}))
        .mockResolvedValue(new app.ShelfListResponse({shelves: [saved]}));
    createShelf.mockResolvedValue(new app.ShelfResponse({
        shelf: {
            id: 8,
            name: 'Moon shelf',
            normalizedName: 'moon shelf',
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
    const shelfName = screen.getByRole('textbox', {name: 'Shelf name'});
    await waitFor(() => expect(shelfName).toHaveFocus());
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('heading', {name: 'Save the current query'}))
        .not.toBeInTheDocument();
    expect(saveOpener).toHaveFocus();

    await user.click(saveOpener);
    await user.type(screen.getByRole('textbox', {name: 'Shelf name'}), 'Moon shelf');
    await user.click(screen.getByRole('button', {name: 'Save shelf'}));

    await waitFor(() => expect(createShelf).toHaveBeenCalledWith(
        'Moon shelf',
        expect.objectContaining({name: 'moon', page: 1}),
    ));
    expect(await screen.findByRole('heading', {name: 'Moon shelf'})).toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Moon shelf, 2 stories'}))
        .toHaveClass('active');
});

test('renames, reorders, duplicates, and deletes saved shelves', async () => {
    const user = userEvent.setup();
    let summaries = [
        new shelves.Summary({
            id: 7,
            name: 'Bedtime',
            position: 0,
            validity: 'valid',
            count: 1,
        }),
        new shelves.Summary({
            id: 8,
            name: 'Adventures',
            position: 1,
            validity: 'valid',
            count: 2,
        }),
    ];
    listShelves.mockImplementation(async () => new app.ShelfListResponse({
        shelves: summaries,
    }));
    openShelf.mockImplementation(async (shelfID) => {
        const shelf = summaries.find((candidate) => candidate.id === shelfID);
        return new app.ShelfEvaluationResponse({
            evaluation: {
                shelf: {
                    id: shelfID,
                    name: shelf?.name ?? 'Shelf',
                    normalizedName: shelf?.name.toLowerCase() ?? 'shelf',
                    position: shelf?.position ?? 0,
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
    renameShelf.mockImplementation(async (shelfID, name) => {
        summaries = summaries.map((shelf) => shelf.id === shelfID
            ? new shelves.Summary({...shelf, name})
            : shelf);
        return new app.ShelfResponse({
            shelf: {
                id: shelfID,
                name,
                normalizedName: name.toLowerCase(),
                position: 0,
                queryVersion: 2,
                queryPayload: '{}',
                validity: 'valid',
            },
        });
    });
    reorderShelves.mockImplementation(async (orderedIDs) => {
        summaries = orderedIDs.map((id, position) => new shelves.Summary({
            ...summaries.find((shelf) => shelf.id === id),
            position,
        }));
        return new app.ShelfListResponse({shelves: summaries});
    });
    duplicateShelf.mockImplementation(async (_shelfID, name) => {
        const duplicate = new shelves.Summary({
            id: 9,
            name,
            position: summaries.length,
            validity: 'valid',
            count: 1,
        });
        summaries = [...summaries, duplicate];
        return new app.ShelfResponse({
            shelf: {
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
    deleteShelf.mockImplementation(async (shelfID) => {
        summaries = summaries.filter((shelf) => shelf.id !== shelfID);
        return new app.MutationResponse({success: true});
    });

    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Bedtime, 1 story'}));

    await user.click(screen.getByRole('button', {name: 'Rename Bedtime'}));
    const name = screen.getByRole('textbox', {name: 'Shelf name'});
    await user.clear(name);
    await user.type(name, 'Evening');
    await user.click(screen.getByRole('button', {name: 'Rename shelf'}));
    await waitFor(() => expect(renameShelf).toHaveBeenCalledWith(7, 'Evening'));

    await user.click(await screen.findByRole('button', {name: 'Move Evening down'}));
    await waitFor(() => expect(reorderShelves).toHaveBeenCalledWith([8, 7]));

    await user.click(screen.getByRole('button', {name: 'Duplicate Evening'}));
    await user.type(screen.getByRole('textbox', {name: 'Shelf name'}), 'Evening copy');
    await user.click(screen.getByRole('button', {name: 'Duplicate shelf'}));
    await waitFor(() => expect(duplicateShelf).toHaveBeenCalledWith(7, 'Evening copy'));
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
    await user.click(screen.getByRole('button', {name: 'Delete shelf'}));
    await waitFor(() => expect(deleteShelf).toHaveBeenCalledWith(9));
    expect(screen.queryByRole('button', {name: 'Evening copy, 1 story'}))
        .not.toBeInTheDocument();
});

test('keeps a deleted active shelf query as custom instead of labeling all stories', async () => {
    const user = userEvent.setup();
    let summaries = [new shelves.Summary({
        id: 7,
        name: 'Bedtime',
        position: 0,
        validity: 'valid',
        count: 1,
    })];
    listShelves.mockImplementation(async () => new app.ShelfListResponse({
        shelves: summaries,
    }));
    openShelf.mockResolvedValue(new app.ShelfEvaluationResponse({
        evaluation: {
            shelf: {
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
    deleteShelf.mockImplementation(async () => {
        summaries = [];
        return new app.MutationResponse({success: true});
    });

    render(<App/>);
    await user.click(await screen.findByRole('button', {name: 'Bedtime, 1 story'}));
    await user.click(screen.getByRole('button', {name: 'Delete Bedtime'}));
    await user.click(screen.getByRole('button', {name: 'Delete shelf'}));

    await waitFor(() => expect(deleteShelf).toHaveBeenCalledWith(7));
    expect(screen.getByRole('searchbox', {name: 'Search stories'}))
        .toHaveValue('moon');
    expect(screen.getByRole('button', {name: /All stories/}))
        .not.toHaveClass('active');
    expect(screen.queryByRole('button', {name: '↻ Update “Bedtime”'}))
        .not.toBeInTheDocument();
});

test('blocks unsafe shelf evaluation until its query is explicitly replaced', async () => {
    const user = userEvent.setup();
    const invalid = new shelves.Summary({
        id: 12,
        name: 'Old moods',
        position: 0,
        validity: 'needs_attention',
        attentionReason: 'missing_criteria',
        count: 0,
    });
    const repaired = new shelves.Summary({
        id: 12,
        name: 'Old moods',
        position: 0,
        validity: 'valid',
        count: 2,
    });
    listShelves
        .mockResolvedValueOnce(new app.ShelfListResponse({shelves: [invalid]}))
        .mockResolvedValue(new app.ShelfListResponse({shelves: [repaired]}));
    replaceShelfQuery.mockResolvedValue(new app.ShelfResponse({
        shelf: {
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

    expect(openShelf).not.toHaveBeenCalled();
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
    await waitFor(() => expect(replaceShelfQuery).toHaveBeenCalledWith(
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

test('requires explicit removal of unavailable criteria before shelf repair', async () => {
    const user = userEvent.setup();
    window.history.replaceState(
        null,
        '',
        '/#/library?bool=999%3Atrue&size=12&sort=imported_desc&v=1',
    );
    const invalid = new shelves.Summary({
        id: 15,
        name: 'Deleted tag shelf',
        position: 0,
        validity: 'needs_attention',
        attentionReason: 'missing_criteria',
        count: 0,
    });
    const repaired = new shelves.Summary({
        id: 15,
        name: 'Deleted tag shelf',
        position: 0,
        validity: 'valid',
        count: 2,
    });
    listShelves
        .mockResolvedValueOnce(new app.ShelfListResponse({shelves: [invalid]}))
        .mockResolvedValue(new app.ShelfListResponse({shelves: [repaired]}));
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
    replaceShelfQuery.mockResolvedValue(new app.ShelfResponse({
        shelf: {
            id: 15,
            name: 'Deleted tag shelf',
            normalizedName: 'deleted tag shelf',
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
        name: 'Deleted tag shelf, needs attention',
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

    await waitFor(() => expect(replaceShelfQuery).toHaveBeenCalledWith(
        15,
        expect.objectContaining({booleanFilters: []}),
    ));
});

test('moves an active shelf into repair when refreshed validity becomes unsafe', async () => {
    const user = userEvent.setup();
    const valid = new shelves.Summary({
        id: 16,
        name: 'Mood',
        position: 0,
        validity: 'valid',
        count: 1,
    });
    const invalid = new shelves.Summary({
        id: 16,
        name: 'Mood',
        position: 0,
        validity: 'needs_attention',
        attentionReason: 'missing_criteria',
        count: 0,
    });
    listShelves
        .mockResolvedValueOnce(new app.ShelfListResponse({shelves: [valid]}))
        .mockResolvedValue(new app.ShelfListResponse({shelves: [invalid]}));
    openShelf.mockResolvedValue(new app.ShelfEvaluationResponse({
        evaluation: {
            shelf: {
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

test('previews a multi-shelf union with per-shelf and overlap counts', async () => {
    const user = userEvent.setup();
    listShelves.mockResolvedValue(new app.ShelfListResponse({
        shelves: [
            new shelves.Summary({
                id: 7,
                name: 'Moon',
                position: 0,
                validity: 'valid',
                count: 2,
            }),
            new shelves.Summary({
                id: 8,
                name: 'Forest',
                position: 1,
                validity: 'valid',
                count: 2,
            }),
        ],
    }));
    previewShelves.mockImplementation(async (shelfIDs) => (
        new app.ShelfSelectionPreviewResponse({
            preview: shelfIDs.length === 1
                ? {
                    shelves: [{id: 7, name: 'Moon', count: 2}],
                    sourceShelfNames: ['Moon'],
                    uniqueStoryCount: 2,
                    overlapCount: 0,
                }
                : {
                    shelves: [
                        {id: 7, name: 'Moon', count: 2},
                        {id: 8, name: 'Forest', count: 2},
                    ],
                    sourceShelfNames: ['Moon', 'Forest'],
                    uniqueStoryCount: 3,
                    overlapCount: 1,
                },
        })
    ));

    render(<App/>);
    await user.click(await screen.findByRole('checkbox', {
        name: 'Select Moon for combined shelf preview',
    }));
    await waitFor(() => expect(previewShelves).toHaveBeenLastCalledWith([7]));
    await user.click(screen.getByRole('checkbox', {
        name: 'Select Forest for combined shelf preview',
    }));
    await waitFor(() => expect(previewShelves).toHaveBeenLastCalledWith([7, 8]));

    const preview = await screen.findByRole('region', {name: 'Combined shelf preview'});
    expect(preview).toHaveTextContent('Moon2');
    expect(preview).toHaveTextContent('Forest2');
    expect(preview).toHaveTextContent('3 unique stories');
    expect(preview).toHaveTextContent('1 overlapping membership collapsed.');
    expect(preview).toHaveTextContent('Sources: Moon, Forest');

    listShelves.mockResolvedValue(new app.ShelfListResponse({
        shelves: [
            new shelves.Summary({
                id: 7,
                name: 'Moon',
                position: 0,
                validity: 'needs_attention',
                attentionReason: 'missing_criteria',
                count: 0,
            }),
            new shelves.Summary({
                id: 8,
                name: 'Forest',
                position: 1,
                validity: 'valid',
                count: 2,
            }),
        ],
    }));
    previewShelves.mockResolvedValue(new app.ShelfSelectionPreviewResponse({
        error: {
            code: 'conflict',
            message: 'Repair the selected shelf before continuing.',
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
        name: 'Select Moon for combined shelf preview',
    });
    await waitFor(() => expect(invalidSelection).toBeDisabled());
    expect(invalidSelection).toBeChecked();
    await waitFor(() => expect(previewShelves).toHaveBeenLastCalledWith([7, 8]));
    expect(await screen.findByText('Repair the selected shelf before continuing.'))
        .toBeInTheDocument();
});

test('keeps a valid empty saved shelf active with edit and import recovery actions', async () => {
    const user = userEvent.setup();
    listShelves.mockResolvedValue(new app.ShelfListResponse({
        shelves: [new shelves.Summary({
            id: 14,
            name: 'Quiet',
            position: 0,
            validity: 'valid',
            count: 0,
        })],
    }));
    openShelf.mockResolvedValue(new app.ShelfEvaluationResponse({
        evaluation: {
            shelf: {
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
    expect(screen.getByRole('button', {name: 'Edit shelf query'})).toBeInTheDocument();
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
    await user.click(screen.getByRole('checkbox', {name: 'Broken'}));
    await user.click(screen.getByRole('checkbox', {name: 'Calm'}));
    await user.click(screen.getByRole('checkbox', {name: 'Bold'}));

    await waitFor(() => expect(queryStories).toHaveBeenLastCalledWith(
        expect.objectContaining({
            booleanFilters: [{definitionId: 1, state: 'true'}],
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
                color: '#ff705c',
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
    await user.click(screen.getByRole('checkbox', {name: 'English'}));
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
                color: '#ff705c',
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

    expect(await screen.findByRole('heading', {name: 'Using saved official metadata'}))
        .toBeInTheDocument();
    expect(await screen.findByRole('heading', {name: 'The Clockwork Mountain'}))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {
        name: `Unmatched local story ${unmatched.uuid}`,
    }))
        .toBeInTheDocument();
    expect(await screen.findByRole('img', {name: 'The Clockwork Mountain artwork'}))
        .toHaveAttribute('src', `/artwork/${artworkID}`);
    expect(screen.getByText(/Official metadata from Lunii catalog/))
        .toBeInTheDocument();
    expect(screen.getByText('54 min')).toBeInTheDocument();
    expect(screen.getByText('System-derived · Age · 3–5 years'))
        .toBeInTheDocument();

    await user.click(screen.getByRole('button', {
        name: `Unmatched local story ${unmatched.uuid}`,
    }));
    expect(await screen.findByText('Local story · embedded metadata'))
        .toBeInTheDocument();
    await user.click(screen.getByRole('button', {
        name: 'The Clockwork Mountain A. Example',
    }));

    await user.type(screen.getByRole('searchbox', {name: 'Search stories'}), 'Clockwork');
    await user.click(screen.getByRole('checkbox', {name: 'English'}));
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
                        key: 'broken',
                        label: 'Broken',
                        color: '#ff705c',
                        kind: 'boolean',
                        source: 'builtin',
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

    await user.click(screen.getByRole('button', {name: 'Edit tags'}));
    const broken = await screen.findByRole('checkbox', {name: 'Broken warning'});
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
        if (request.pageSize === 100) {
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
            pageSize: 100,
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
        name: 'Moonlit Workshop 11112222-3333-4444-8555-666677778888',
    });
    await user.click(moonlit);

    expect(moonlit).toHaveAttribute('aria-pressed', 'true');
    expect(await screen.findByRole('complementary', {name: 'Moonlit Workshop details'}))
        .toBeInTheDocument();
    expect(screen.getByRole('heading', {name: 'My story shelves'})).toBeInTheDocument();
    expect(loadStoryDetail).toHaveBeenLastCalledWith(2);
});

test('keeps bulk selection in the collection and exposes updated tag chips', async () => {
    const user = userEvent.setup();
    loadTagAssignmentWorkspace.mockImplementation(async (storyIDs) => (
        new app.TagAssignmentWorkspaceResponse({
            workspace: {
                catalog: {
                    definitions: [{
                        id: 1,
                        key: 'broken',
                        label: 'Broken',
                        color: '#ff705c',
                        kind: 'boolean',
                        source: 'builtin',
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

    const first = await screen.findByRole('button', {name: 'Clockwork Forest Lunii'});
    const second = screen.getByRole('button', {
        name: 'Moonlit Workshop 11112222-3333-4444-8555-666677778888',
    });
    expect((await screen.findAllByText('Broken')).length).toBeGreaterThan(1);

    await user.keyboard('{Control>}');
    await user.click(second);
    await user.keyboard('{/Control}');

    expect(first).toHaveAttribute('aria-pressed', 'true');
    expect(second).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', {name: 'Edit tags'}));
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
    expect(screen.getByText('Stories in your local archive · 0 archives')).toBeInTheDocument();
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
        name: 'Official metadata has not been synced',
    })).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: '↻ Sync'}));
    expect(refreshOfficialMetadata).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole('heading', {
        name: 'Refreshing official metadata',
    })).toBeInTheDocument();
    expect(screen.getByText('0 of 1 refresh phases complete. Your local collection remains available.'))
        .toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Refreshing…'})).toBeDisabled();
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
        name: 'Using saved official metadata',
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
        name: 'Official metadata is available',
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

    await user.click(await screen.findByRole('button', {name: '＋ Import stories'}));
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

    expect(await screen.findByText('before-refresh.zip · 1.0 MB · Verified'))
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

    expect(await screen.findByText('after-refresh.zip · 1.0 MB · Verified'))
        .toBeInTheDocument();
    expect(loadStoryDetail).toHaveBeenCalledTimes(2);
});

test('cancels story removal without changing the collection', async () => {
    const user = userEvent.setup();
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'Open details'}));
    expect(screen.getByRole('dialog', {name: 'Clockwork Forest'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Cancel'}));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(removeStory).not.toHaveBeenCalled();
    expect(screen.getByRole('button', {name: 'Clockwork Forest Lunii'}))
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
    await user.click(screen.getByRole('button', {name: 'Move to trash'}));

    expect(removeStory).toHaveBeenCalledWith(1);
    expect(await screen.findByRole('heading', {
        name: 'Story removed from this collection',
    })).toBeInTheDocument();
    expect(screen.getByText('Clockwork Forest was moved to application trash.'))
        .toBeInTheDocument();
    expect(screen.queryByRole('button', {name: 'Clockwork Forest Lunii'}))
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

    const cover = await screen.findByRole('button', {name: 'Clockwork Forest Lunii'});
    expect(cover).toHaveAttribute('aria-pressed', 'true');
    expect(await screen.findByText('clockwork-forest.zip · 1.0 MB · Verified'))
        .toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Open details'}));
    expect(screen.getByRole('dialog', {name: 'Clockwork Forest'})).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Move to trash'}));

    expect(removeStory).toHaveBeenCalledWith(1);
    expect(await screen.findByRole('heading', {name: 'Build your local story archive'}))
        .toBeInTheDocument();
    expect(screen.getByText('Clockwork Forest was moved to application trash.'))
        .toBeInTheDocument();
});

test('loads every result when a story is beyond the initial collection page', async () => {
    const user = userEvent.setup();
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
    queryStories
        .mockResolvedValueOnce(initial)
        .mockResolvedValueOnce(new app.LibraryPageResponse({
            page: {
                stories,
                page: 1,
                pageSize: 100,
                totalItems: 3,
                totalPages: 2,
                sort: 'imported_desc',
            },
        }))
        .mockResolvedValueOnce(new app.LibraryPageResponse({
            page: {
                stories: [extraStory],
                page: 2,
                pageSize: 100,
                totalItems: 3,
                totalPages: 2,
                sort: 'imported_desc',
            },
        }));
    render(<App/>);

    await user.click(await screen.findByRole('button', {name: 'View all →'}));

    expect(await screen.findByRole('button', {
        name: 'The Thirteenth Story 22223333-4444-4555-8666-777788889999',
    })).toBeInTheDocument();
    expect(queryStories).toHaveBeenCalledWith(expect.objectContaining({
        page: 2,
        pageSize: 100,
        sort: 'imported_desc',
        name: '',
        booleanFilters: [],
        choiceFilters: [],
    }));
});
