import {
    CANONICAL_PARITY_FIXTURE,
    type FixtureStory,
} from './parity-fixture';

const parityAgeDefinition = {
    id: 10,
    key: 'age',
    normalizedKey: 'age',
    label: 'Age',
    color: '#ff705c',
    kind: 'choice',
    source: 'derived',
    presentation: 'filter',
    position: 0,
    protected: true,
    values: [
        {
            id: 101,
            definitionId: 10,
            key: '3-5',
            normalizedKey: '3-5',
            label: '3–5 years',
            count: 12,
            initiallyChecked: true,
            position: 0,
        },
        {
            id: 102,
            definitionId: 10,
            key: '6-8',
            normalizedKey: '6-8',
            label: '6–8 years',
            count: 18,
            position: 1,
        },
    ],
};

const parityAssignmentDefinitions = [
    {
        ...parityAgeDefinition,
        values: [{
            ...parityAgeDefinition.values[0],
            label: '3–5',
        }],
    },
    {
        id: 11,
        key: 'mood',
        normalizedKey: 'mood',
        label: 'Mood',
        color: '#6779e8',
        kind: 'choice',
        source: 'user',
        presentation: 'filter',
        position: 1,
        protected: false,
        values: [{
            id: 111,
            definitionId: 11,
            key: 'bedtime',
            normalizedKey: 'bedtime',
            label: 'Bedtime',
            position: 0,
        }],
    },
    {
        id: 12,
        key: 'favorite',
        normalizedKey: 'favorite',
        label: 'Favorite',
        color: '#55b79a',
        kind: 'boolean',
        source: 'user',
        presentation: 'filter',
        position: 2,
        protected: false,
        values: [],
    },
] as const;

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

const performanceStories: FixtureStory[] = Array.from(
    {length: 5_000},
    (_, index) => {
        const storyNumber = index + 1;
        return {
            id: storyNumber,
            uuid: `00000000-0000-4000-8000-${String(storyNumber).padStart(12, '0')}`,
            title: `Synthetic Story ${String(storyNumber).padStart(5, '0')}`,
            author: `Fixture Author ${storyNumber % 24}`,
            sources: {
                title: 'official',
                description: 'official',
                author: 'official',
                artwork: 'fallback',
            },
            detectedFormat: storyNumber % 2 === 0 ? 'v2_pk' : 'zip',
            compatibility: storyNumber % 10 === 0 ? 'missing' : 'compatible',
            byteSize: (storyNumber % 128 + 1) * 1024 * 1024,
            importedAt: `2026-07-${String(storyNumber % 25 + 1).padStart(2, '0')}T09:00:00Z`,
        };
    },
);

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
    const fixtureStories = fixture === 'empty'
        ? []
        : fixture === 'performance'
            ? performanceStories
            : fixture === 'parity'
                ? [...CANONICAL_PARITY_FIXTURE.stories]
            : stories;
    const totalItems = fixture === 'collection' || fixture === 'parity'
        ? 48
        : fixtureStories.length;
    const fixtureSnapshot = fixtureOperation(fixture);
    const page = (pageNumber: number, pageSize: number, sort: string) => ({
        stories: fixtureStories.slice(
            (pageNumber - 1) * pageSize,
            pageNumber * pageSize,
        ),
        page: pageNumber,
        pageSize,
        totalItems,
        totalPages: totalItems === 0 ? 0 : Math.ceil(totalItems / pageSize),
        sort,
    });
    const fixtureApp = {
        ActiveOperations: async () => ({
            operations: fixtureSnapshot?.status === 'running' ? [fixtureSnapshot] : [],
        }),
        ApplicationStatus: async () => ({
            status: {state: 'ready', mutationsAllowed: true},
        }),
        ListShelves: async () => ({
            shelves: fixture === 'parity'
                ? CANONICAL_PARITY_FIXTURE.savedShelves.map(
                    (shelf, position) => ({
                        id: shelf.id,
                        name: shelf.name,
                        position,
                        validity: shelf.id === 3 ? 'sidebar_only' : 'valid',
                        count: shelf.count,
                        color: shelf.color,
                    }),
                )
                : [],
        }),
        OfficialMetadataStatus: async () => fixture === 'parity' ? {} : ({
            status: {
                state: 'fresh',
                locale: 'en-GB',
                storyCount: totalItems,
                matchedStoryCount: totalItems,
                activatedAt: '2026-07-25T09:00:00Z',
            },
        }),
        ListStories: async () => ({
            page: page(1, 12, 'imported_desc'),
        }),
        QueryStories: async (query: {page: number; pageSize: number; sort: string}) => ({
            page: page(query.page, query.pageSize, query.sort),
        }),
        OpenShelf: async (
            shelfID: number,
            request: {page: number; pageSize: number; sort: string},
        ) => {
            const preview = CANONICAL_PARITY_FIXTURE.mainShelves.find(
                (shelf) => shelf.sourceShelfID === shelfID,
            );
            const source = preview?.stories ?? [];
            return {
                evaluation: {
                    shelf: {
                        id: shelfID,
                        name: preview?.name ?? 'Favorites',
                        position: shelfID - 1,
                        queryVersion: 1,
                        queryPayload: '{}',
                        validity: 'valid',
                    },
                    query: {
                        version: 1,
                        name: '',
                        languages: [],
                        compatibilities: [],
                        booleanFilters: [],
                        choiceFilters: [],
                    },
                    page: {
                        stories: source.slice(0, request.pageSize),
                        page: request.page,
                        pageSize: request.pageSize,
                        totalItems: preview?.count ?? 0,
                        totalPages: preview
                            ? Math.ceil(preview.count / request.pageSize)
                            : 0,
                        sort: request.sort,
                    },
                    previewSource: preview?.source,
                },
            };
        },
        StoryDetail: async (storyID: number) => {
            const story = fixtureStories.find(
                (candidate) => candidate.id === storyID,
            ) ?? fixtureStories[0] ?? stories[0];
            const parityArchive = fixture === 'parity' && storyID === 1
                ? CANONICAL_PARITY_FIXTURE.selectedDetail.archive
                : null;
            return {
                detail: {
                    story,
                    archive: {
                        originalFilename: parityArchive?.originalFilename ??
                            `${story.title.toLowerCase().replaceAll(' ', '-')}.v2.pk`,
                        detectedFormat: parityArchive?.detectedFormat ??
                            story.detectedFormat,
                        sha256: parityArchive?.sha256 ?? 'a'.repeat(64),
                        byteSize: parityArchive?.byteSize ?? story.byteSize,
                        verification: parityArchive?.verification ??
                            story.compatibility,
                    },
                },
            };
        },
        TagCatalog: async () => ({
            catalog: {
                definitions: fixture === 'parity' ? [parityAgeDefinition] : [{
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
                }],
            },
        }),
        TagAssignmentWorkspace: async (storyIDs: number[]) => ({
            workspace: {
                catalog: {
                    definitions: fixture === 'parity'
                        ? parityAssignmentDefinitions
                        : [{
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
                    }],
                },
                requestedStories: storyIDs.length,
                states: fixture === 'parity' ? [
                    {
                        definitionId: 10,
                        assignedStories: storyIDs.length,
                        values: [{
                            valueId: 101,
                            assignedStories: storyIDs.length,
                        }],
                    },
                    {
                        definitionId: 11,
                        assignedStories: storyIDs.length,
                        values: [{
                            valueId: 111,
                            assignedStories: storyIDs.length,
                        }],
                    },
                    {
                        definitionId: 12,
                        assignedStories: storyIDs.length,
                        values: [],
                    },
                ] : [{
                    definitionId: 1,
                    assignedStories: 0,
                    values: [],
                }],
            },
        }),
        SetBooleanTag: async (storyIDs: number[]) => ({
            result: {
                requestedStories: storyIDs.length,
                changedStories: storyIDs.length,
                assignmentsAdded: storyIDs.length,
                assignmentsRemoved: 0,
            },
        }),
        SetChoiceTagValue: async (storyIDs: number[]) => ({
            result: {
                requestedStories: storyIDs.length,
                changedStories: storyIDs.length,
                assignmentsAdded: storyIDs.length,
                assignmentsRemoved: 0,
            },
        }),
        SelectAndImportStories: async () => ({cancelled: true}),
        CancelOperation: async () => ({cancelled: true}),
        RemoveStory: async (storyID: number) => ({
            result: {
                storyId: storyID,
                uuid: fixtureStories.find(
                    (story) => story.id === storyID,
                )?.uuid ?? '',
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
