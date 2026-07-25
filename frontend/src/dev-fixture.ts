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

const outcomeByFixture: Record<string, {
    status: string;
    outcomeCode: string;
    outcomeMessage: string;
}> = {
    success: {
        status: 'succeeded',
        outcomeCode: 'imported',
        outcomeMessage: 'Story imported.',
    },
    invalid: {
        status: 'failed',
        outcomeCode: 'invalid_container',
        outcomeMessage: 'The file is not a supported story archive.',
    },
    duplicate: {
        status: 'succeeded',
        outcomeCode: 'duplicate_checksum',
        outcomeMessage: 'This archive is already in the collection.',
    },
    conflict: {
        status: 'failed',
        outcomeCode: 'uuid_conflict',
        outcomeMessage: 'Different archive bytes already use this story UUID.',
    },
    failed: {
        status: 'failed',
        outcomeCode: 'import_failed',
        outcomeMessage: 'The file could not be imported.',
    },
};

function fixtureOperation(fixture: string) {
    const outcome = outcomeByFixture[fixture];
    const running = fixture === 'importing';
    if (!outcome && !running) {
        return null;
    }
    return {
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status: running ? 'running' : outcome.status,
        completedItems: running ? 0 : 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            storyId: outcome?.outcomeCode === 'imported' ? 1 : undefined,
            sourceName: 'clockwork-forest.zip',
            status: running
                ? 'running'
                : outcome.outcomeCode === 'imported'
                    ? 'succeeded'
                    : 'failed',
            outcomeCode: outcome?.outcomeCode,
            outcomeMessage: outcome?.outcomeMessage,
            completedBytes: running ? 524288 : 1048576,
            totalBytes: 1048576,
        }],
    };
}

export function installCollectionFixture() {
    const fixture = new URLSearchParams(window.location.search).get('fixture') ?? 'collection';
    const visibleStories = fixture === 'empty' ? [] : stories;
    const totalItems = fixture === 'empty' ? 0 : 48;
    const fixtureSnapshot = fixtureOperation(fixture);
    const fixtureApp = {
        ActiveOperations: async () => ({
            operations: fixtureSnapshot?.status === 'running' ? [fixtureSnapshot] : [],
        }),
        ApplicationStatus: async () => ({
            status: {state: 'ready', mutationsAllowed: true},
        }),
        ListStories: async () => ({
            page: {
                stories: visibleStories,
                page: 1,
                pageSize: 12,
                totalItems,
                totalPages: totalItems === 0 ? 0 : 4,
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
        SelectAndImportStories: async () => ({cancelled: true}),
        CancelOperation: async () => ({cancelled: true}),
        RemoveStory: async (storyID: number) => ({
            result: {
                storyId: storyID,
                uuid: stories.find((story) => story.id === storyID)?.uuid ?? '',
            },
        }),
        OperationSnapshot: async () => ({}),
    };
    (window as typeof window & {go: unknown}).go = {main: {App: fixtureApp}};
    (window as typeof window & {runtime: unknown}).runtime = {
        EventsOnMultiple: (
            _eventName: string,
            callback: (value: unknown) => void,
        ) => {
            if (fixtureSnapshot) {
                queueMicrotask(() => callback(fixtureSnapshot));
            }
            return () => undefined;
        },
    };
}
