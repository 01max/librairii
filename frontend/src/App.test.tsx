import {fireEvent, render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {
    ActiveOperations,
    ApplicationStatus,
    CancelOperation,
    OperationSnapshot,
    QueryStories,
    RemoveStory,
    SelectAndImportStories,
    SetBooleanTag,
    StoryDetail as LoadStoryDetail,
    TagAssignmentWorkspace as LoadTagAssignmentWorkspace,
    TagCatalog as LoadTagCatalog,
} from '../wailsjs/go/main/App';
import {app, library} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import App from './App';

vi.mock('../wailsjs/go/main/App', () => ({
    ActiveOperations: vi.fn(),
    ApplicationStatus: vi.fn(),
    CancelOperation: vi.fn(),
    OperationSnapshot: vi.fn(),
    QueryStories: vi.fn(),
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
const operationSnapshot = vi.mocked(OperationSnapshot);
const queryStories = vi.mocked(QueryStories);
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
    operationSnapshot.mockReset();
    queryStories.mockReset();
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
    operationSnapshot.mockResolvedValue(new app.OperationResponse({}));
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
            booleanFilters: [],
            choiceFilters: [],
        }),
    ));
    expect(search).toHaveFocus();
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
