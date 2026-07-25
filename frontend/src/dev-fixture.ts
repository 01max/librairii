type FixtureStory = {
    id: number;
    uuid: string;
    title: string;
    author: string;
    sources: {
        title: string;
        description: string;
        author: string;
        artwork: string;
    };
    detectedFormat: string;
    compatibility: string;
    byteSize: number;
    importedAt: string;
};

const titles = [
    ['The Little Prince', 'Antoine de Saint-Exupéry'],
    ['The Secret Forest', 'Gallimard'],
    ['Milo and the Moon', 'Bayard'],
    ['Night Train North', 'Didier'],
    ['Cloud Collectors', 'École loisirs'],
    ['The Golden Acorn', 'Lunii'],
    ['Boat of Stars', 'Bayard'],
    ['The Sleepy Giant', 'Flammarion'],
    ['Down to the Sea', 'Lunii'],
    ['The Violet Door', 'Nathan'],
    ['Map of the Wind', 'Lunii'],
    ['Fox and Fern', 'Bayard'],
] as const;

const stories: FixtureStory[] = titles.map(([title, author], index) => ({
    id: index + 1,
    uuid: `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
    title,
    author,
    sources: {
        title: 'official',
        description: 'official',
        author: 'official',
        artwork: 'fallback',
    },
    detectedFormat: index % 2 === 0 ? 'v2_pk' : 'zip',
    compatibility: 'compatible',
    byteSize: (index + 72) * 1024 * 1024,
    importedAt: `2026-07-${String(25 - index).padStart(2, '0')}T09:00:00Z`,
}));

export function installCollectionFixture() {
    const fixtureApp = {
        ApplicationStatus: async () => ({
            status: {state: 'ready', mutationsAllowed: true},
        }),
        ListStories: async () => ({
            page: {
                stories,
                page: 1,
                pageSize: 12,
                totalItems: 48,
                totalPages: 4,
                sort: 'imported_desc',
            },
        }),
        StoryDetail: async (storyID: number) => {
            const story = stories.find((candidate) => candidate.id === storyID) ?? stories[0];
            return {
                detail: {
                    story,
                    archive: {
                        originalFilename: `${story.title.toLowerCase().replaceAll(' ', '-')}.v2.pk`,
                        detectedFormat: story.detectedFormat,
                        sha256: 'a'.repeat(64),
                        byteSize: story.byteSize,
                        verification: story.compatibility,
                    },
                },
            };
        },
    };
    (window as typeof window & {go: unknown}).go = {main: {App: fixtureApp}};
}
