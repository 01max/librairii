import {render, screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
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
import {app, library} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import App from './App';

vi.mock('../wailsjs/go/main/App', () => ({
    ActiveOperations: vi.fn(),
    ApplicationStatus: vi.fn(),
    CancelOperation: vi.fn(),
    ListStories: vi.fn(),
    OperationSnapshot: vi.fn(),
    RemoveStory: vi.fn(),
    SelectAndImportStories: vi.fn(),
    StoryDetail: vi.fn(),
}));

vi.mock('../wailsjs/runtime/runtime', () => ({
    EventsOn: vi.fn(),
}));

const activeOperations = vi.mocked(ActiveOperations);
const applicationStatus = vi.mocked(ApplicationStatus);
const cancelOperation = vi.mocked(CancelOperation);
const listStories = vi.mocked(ListStories);
const operationSnapshot = vi.mocked(OperationSnapshot);
const removeStory = vi.mocked(RemoveStory);
const selectAndImportStories = vi.mocked(SelectAndImportStories);
const loadStoryDetail = vi.mocked(LoadStoryDetail);
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
    activeOperations.mockReset();
    applicationStatus.mockReset();
    cancelOperation.mockReset();
    listStories.mockReset();
    operationSnapshot.mockReset();
    removeStory.mockReset();
    selectAndImportStories.mockReset();
    loadStoryDetail.mockReset();
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
    listStories.mockResolvedValue(new app.LibraryPageResponse({
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

test('shows the empty-library import action in the canonical shell', async () => {
    listStories.mockResolvedValue(new app.LibraryPageResponse({
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
    listStories
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
    listStories
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
    listStories
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
    expect(listStories).toHaveBeenCalledWith({
        page: 2,
        pageSize: 100,
        sort: 'imported_desc',
    });
});
