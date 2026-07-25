import {render, screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {
    ApplicationStatus,
    ListStories,
    StoryDetail as LoadStoryDetail,
} from '../wailsjs/go/main/App';
import {app, library} from '../wailsjs/go/models';
import App from './App';

vi.mock('../wailsjs/go/main/App', () => ({
    ApplicationStatus: vi.fn(),
    ListStories: vi.fn(),
    StoryDetail: vi.fn(),
}));

const applicationStatus = vi.mocked(ApplicationStatus);
const listStories = vi.mocked(ListStories);
const loadStoryDetail = vi.mocked(LoadStoryDetail);

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
    applicationStatus.mockReset();
    listStories.mockReset();
    loadStoryDetail.mockReset();

    applicationStatus.mockResolvedValue(new app.StatusResponse({
        status: {
            state: 'ready',
            mutationsAllowed: true,
        },
    }));
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
