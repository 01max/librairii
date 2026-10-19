import {
    CANONICAL_PARITY_FIXTURE,
    type FixtureStory,
} from './parity-fixture';
import {
    DEFAULT_COLLECTION_QUERY,
    encodeCollectionQuery,
} from './query-codec';
import performanceConfig from '../performance-config.json';

const parityAgeDefinition = {
    id: 10,
    key: 'age',
    normalizedKey: 'age',
    label: 'Age',
    color: '#267d5d',
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
            position: 0,
        },
        {
            id: 102,
            definitionId: 10,
            key: '6-8',
            normalizedKey: '6-8',
            label: '6–8 years',
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
    {length: performanceConfig.browserStoryCount},
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

const historyOperations = [{
    id: '00000000-0000-4000-8000-000000000901', kind: 'export', status: 'succeeded',
    completedItems: 1, totalItems: 1, totalBytes: 100, cancelRequested: false,
    createdAt: '2026-07-25T10:00:00Z', finishedAt: '2026-07-25T10:01:00Z',
    destination: 'Lunii export', sourceShelfNames: ['Bedtime'],
    items: [{id: 901, sourceName: 'moon.zip', storyTitle: 'Milo and the Moon',
        outputName: 'moon.zip', status: 'succeeded', outcomeMessage: 'Story archive exported.',
        completedBytes: 100, totalBytes: 100}],
}, {
    id: '00000000-0000-4000-8000-000000000900', kind: 'import', status: 'succeeded',
    completedItems: 1, totalItems: 1, totalBytes: 100, cancelRequested: false,
    createdAt: '2026-07-25T09:00:00Z', finishedAt: '2026-07-25T09:01:00Z',
    items: [{id: 900, sourceName: 'moon.zip', status: 'succeeded', outcomeCode: 'imported',
        outcomeMessage: 'Story imported.', completedBytes: 100, totalBytes: 100}],
}];

export function installCollectionFixture() {
    const fixture = new URLSearchParams(window.location.search).get('fixture') ?? 'collection';
    if (fixture === 'parity') {
        const query = {
            ...DEFAULT_COLLECTION_QUERY,
            pageSize: 12,
            sort: 'imported_desc' as const,
            choiceFilters: [{definitionId: 10, valueIds: [101]}],
        };
        const hash = encodeCollectionQuery(query);
        window.history.replaceState({
            collectionQuery: {
                hash,
                shelfId: null,
                scope: 'all',
            },
        }, '', `${window.location.pathname}${window.location.search}${hash}`);
    }
    const fixtureStories = fixture === 'empty'
        ? []
        : fixture === 'performance'
            ? performanceStories
            : fixture === 'parity'
                ? [...CANONICAL_PARITY_FIXTURE.stories]
            : fixture === 'broken-stories'
                ? stories.slice(0, 3).map((story, index) => ({...story, broken: index === 0}))
                : stories;
    const totalItems = fixture === 'collection' || fixture === 'parity'
        ? 48
        : fixtureStories.length;
    const fixtureFlags = new Map(fixtureStories.map((story) => [story.id, 'broken' in story && !!story.broken]));
    const currentStories = () => fixtureStories.map((story) => ({...story, broken: fixtureFlags.get(story.id) ?? false}));
    const matchesVisibility = (broken: boolean, mode = 'hide') => mode === 'include' || (mode === 'only' ? broken : !broken);
    const fixtureSnapshot = fixtureOperation(fixture);
    const page = (
        pageNumber: number,
        pageSize: number,
        sort: string,
        matchingItems = totalItems,
        source = fixtureStories,
    ) => ({
        stories: source.slice(
            (pageNumber - 1) * pageSize,
            pageNumber * pageSize,
        ),
        page: pageNumber,
        pageSize,
        totalItems: matchingItems,
        totalPages: matchingItems === 0 ? 0 : Math.ceil(matchingItems / pageSize),
        sort,
    });
    let settingsLanguage = new URLSearchParams(window.location.search).get('lang') === 'fr' ? 'fr' : 'en';
    let setupComplete = fixture !== 'setup';
    let movePhase = fixture === 'relocating' ? 'copying' : '';
    let pendingSettings=['settings','relocating'].includes(fixture);
    const settingsSnapshot = () => ({language:settingsLanguage,root:'~/Stories/Librairii',setupRequired:!setupComplete,isolated:false,recovery:false,
        pendingOpen:pendingSettings,
        move:movePhase ? {phase:movePhase,copied:512,total:1024,cancelable:movePhase==='copying'} : undefined});
    const fixtureApp = {
        Settings: async () => settingsSnapshot(),
        DismissSettings: async () => {pendingSettings=false;},
        SetUILanguage: async (language: string) => { settingsLanguage=language; return settingsSnapshot(); },
        SelectLibraryLocation: async () => ({...settingsSnapshot(),preparation:{id:'fixture-move',source:'~/Stories/Librairii',destination:'/Volumes/Histoires/Librairii',bytes:1024,libraryId:'fixture'}}),
        ConfirmLibrarySetup: async () => {setupComplete=true;return {success:true};},
        StartLibraryMove: async () => {movePhase='copying';return settingsSnapshot();},
        CancelLibraryMove: async () => {movePhase='cancelled';return settingsSnapshot();},
        RetryLibrary: async () => settingsSnapshot(),
        LocateLibrary: async () => settingsSnapshot(),
        ActiveOperations: async () => ({
            operations: fixture === 'history' ? [historyOperations[0]]
                : fixtureSnapshot?.status === 'running' ? [fixtureSnapshot] : [],
        }),
        LibraryCompatibility: async () => ({compatibility: {legacyBrokenTagId: 0}}),
        ApplicationStatus: async () => ({
            status: {
                state: setupComplete ? 'ready' : 'setup_required',
                mutationsAllowed: setupComplete,
                recoveryAvailable: false,
            },
        }),
        PackagedAcceptanceMode: async () => false,
        RecordPackagedAcceptance: async () => ({success: false}),
        RecoverStorage: async () => ({success: false}),
        ListShelves: async () => ({
            shelves: fixture === 'parity'
                ? CANONICAL_PARITY_FIXTURE.savedShelves.map(
                    (shelf, position) => ({
                        id: shelf.id,
                        name: shelf.name,
                        position,
                        validity: shelf.id === 3 ? 'sidebar_only' : 'valid',
                        count: shelf.count,
                    }),
                )
                : [],
        }),
        OfficialMetadataStatus: async () => ({
            status: {
                state: fixture === 'first-sync' ? 'never_synced' : 'fresh',
                locale: 'en-GB',
                storyCount: totalItems,
                matchedStoryCount: totalItems,
                activatedAt: fixture === 'first-sync' ? undefined : '2026-07-25T09:00:00Z',
            },
        }),
        ListStories: async () => ({
            page: page(1, 12, 'imported_desc'),
        }),
        QueryStories: async (query: {
            brokenVisibility?: string;
            brokenState?: string;
            page: number;
            pageSize: number;
            sort: string;
            choiceFilters?: Array<{definitionId: number; valueIds: number[]}>;
        }) => {
            window.dispatchEvent(new CustomEvent('librairii:fixture-query', {
                detail: query,
            }));
            const ageValueIDs = query.choiceFilters?.find(
                (filter) => filter.definitionId === 10,
            )?.valueIds ?? [];
            const matchingItems = ageValueIDs.includes(101)
                ? 12
                : ageValueIDs.includes(102)
                    ? 18
                    : totalItems;
            const visible = currentStories().filter((story) => matchesVisibility(story.broken, query.brokenVisibility) &&
                (!query.brokenState || (query.brokenState === 'true' ? story.broken : !story.broken)));
            return {
                page: page(
                    query.page,
                    query.pageSize,
                    query.sort,
                    fixture === 'broken-stories' ? visible.length : matchingItems,
                    visible,
                ),
            };
        },
        OpenShelf: async (
            shelfID: number,
            request: {page: number; pageSize: number; sort: string; brokenVisibility?: string},
        ) => {
            const preview = CANONICAL_PARITY_FIXTURE.mainShelves.find(
                (shelf) => shelf.sourceShelfID === shelfID,
            );
            const source = (preview?.stories ?? []).map((story) => ({...story, broken: fixtureFlags.get(story.id) ?? false})).filter((story) => matchesVisibility(story.broken, request.brokenVisibility));
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
                        choiceFilters: [{
                            definitionId: 11,
                            valueIds: [111],
                        }],
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
                },
            };
        },
        StoryDetail: async (storyID: number) => {
            const story = currentStories().find(
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
                definitions: [parityAgeDefinition],
            },
        }),
        TagAssignmentWorkspace: async (storyIDs: number[]) => ({
            workspace: {
                catalog: {
                    definitions: fixture === 'parity' ? parityAssignmentDefinitions : [parityAgeDefinition],
                },
                requestedStories: storyIDs.length,
                states: fixture === 'parity' ? [
                    {
                        definitionId: 10,
                        assignedStories: 0,
                        values: [{
                            valueId: 101,
                            assignedStories: storyIDs.length,
                        }],
                    },
                    {
                        definitionId: 11,
                        assignedStories: 0,
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
                ] : [],
            },
        }),
        SetStoriesBroken: async (storyIDs: number[], broken: boolean) => {
            const ids = [...new Set(storyIDs)];
            const changes = ids.filter((id) => fixtureFlags.get(id) !== broken).map((id) => ({storyId: id, previous: fixtureFlags.get(id) ?? false, current: broken}));
            changes.forEach((change) => fixtureFlags.set(change.storyId, change.current));
            return {result: {requestedStories: ids.length, changedStories: changes.length, changes}};
        },
        RestoreStoryFlags: async (changes: Array<{storyId: number; previous: boolean; current: boolean}>) => {
            if (changes.some((change) => fixtureFlags.get(change.storyId) !== change.current)) return {error: {message: 'These flags changed after this action. Refresh the collection before retrying.'}};
            const restored = changes.map((change) => ({storyId: change.storyId, previous: change.current, current: change.previous}));
            restored.forEach((change) => fixtureFlags.set(change.storyId, change.current));
            return {result: {requestedStories: restored.length, changedStories: restored.length, changes: restored}};
        },
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
        OperationHistory: async () => ({page: {
            operations: fixture === 'history' ? historyOperations : [],
        }}),
        RevealExportDestination: async () => ({success: true}),
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
