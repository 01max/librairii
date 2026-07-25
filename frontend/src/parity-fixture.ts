export const CANONICAL_PROTOTYPE = {
    path: 'openspec/ui-prototypes/05-archive-shelves.html',
    sha256: '19119b85ed820e1893020347ad5015bbed173ef8c8e6e1164405d83f1b5f00f9',
    normativeSelector: '.app',
    excludedSelector: '.back',
} as const;

export type FixtureStory = {
    id: number;
    uuid: string;
    title: string;
    author: string;
    publisher?: string;
    sources: {
        title: string;
        description: string;
        author: string;
        artwork: string;
    };
    official?: {
        locale: string;
        language: string;
        title: string;
        author: string;
        publisher: string;
        provenance: string;
        durationSeconds: number;
        fetchedAt: string;
        activatedAt: string;
    };
    detectedFormat: string;
    compatibility: string;
    byteSize: number;
    importedAt: string;
};

const sampleStories: Array<[
    string,
    string,
    number,
]> = [
    ['The Little Prince', 'Lunii', 54],
    ['The Secret Forest', 'Gallimard', 32],
    ['Milo and the Moon', 'Bayard', 24],
    ['Night Train North', 'Didier', 41],
    ['Cloud Collectors', 'École loisirs', 27],
    ['The Golden Acorn', 'Lunii', 35],
    ['Boat of Stars', 'Bayard', 19],
    ['The Sleepy Giant', 'Flammarion', 38],
    ['Down to the Sea', 'Lunii', 29],
    ['The Violet Door', 'Nathan', 44],
    ['Map of the Wind', 'Lunii', 36],
    ['Fox and Fern', 'Bayard', 21],
];

const stories: FixtureStory[] = sampleStories.map(
    ([title, publisher, durationMinutes], index) => ({
        id: index + 1,
        uuid: `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
        title,
        author: index === 0 ? 'Antoine de Saint-Exupéry' : publisher,
        publisher,
        sources: {
            title: 'official',
            description: 'official',
            author: 'official',
            artwork: 'fallback',
        },
        official: {
            locale: 'en-GB',
            language: 'en-GB',
            title,
            author: index === 0 ? 'Antoine de Saint-Exupéry' : publisher,
            publisher,
            provenance: 'canonical-parity-fixture',
            durationSeconds: durationMinutes * 60,
            fetchedAt: '2026-07-25T09:00:00Z',
            activatedAt: '2026-07-25T09:00:00Z',
        },
        detectedFormat: index === 0 ? 'v2_pk' : 'zip',
        compatibility: 'compatible',
        byteSize: index === 0 ? 90_596_966 : (index + 72) * 1024 * 1024,
        importedAt: `2026-07-${String(25 - index).padStart(2, '0')}T09:00:00Z`,
    }),
);

export const CANONICAL_PARITY_FIXTURE = {
    totalArchives: 48,
    savedShelves: [
        {id: 1, name: 'Bedtime', count: 12, color: '#ff705c'},
        {id: 2, name: 'Adventures', count: 16, color: '#55b79a'},
        {id: 3, name: 'Favorites', count: 9, color: '#f5bc41'},
    ],
    facets: {
        age: [
            {label: '3–5 years', count: 12, checked: true},
            {label: '6–8 years', count: 18, checked: false},
        ],
        collapsed: ['Language', 'Import status'],
    },
    mainShelves: [
        {
            sourceShelfID: 1,
            name: 'Bedtime',
            count: 9,
            source: 'Mood tag',
            stories: stories.slice(0, 6),
        },
        {
            sourceShelfID: 2,
            name: 'Weekend adventures',
            count: 12,
            source: 'Mood tag',
            stories: stories.slice(6, 12),
        },
    ],
    stories,
    selectedStoryID: 1,
    selectedDetail: {
        title: 'The Little Prince',
        subtitle: 'Antoine de Saint-Exupéry · metadata synced today',
        facts: [
            {label: 'Stories', value: '12'},
            {label: 'Duration', value: '54 min'},
            {label: 'Language', value: 'English'},
        ],
        tags: [
            {label: 'Age · 3–5', color: '#ff705c'},
            {label: 'Mood · Bedtime', color: '#6779e8'},
            {label: 'Favorite', color: '#55b79a'},
        ],
        archive: {
            originalFilename: 'the-little-prince.v2.pk',
            detectedFormat: 'v2_pk',
            sha256: 'a'.repeat(64),
            byteSize: 90_596_966,
            verification: 'compatible',
        },
    },
} as const;
