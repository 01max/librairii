import {expect, test} from 'vitest';
import {metadata, operations} from '../wailsjs/go/models';
import {
    describeMetadataRefresh,
    describeMetadataStatus,
} from './metadata-state';

test('describes never-synced, fresh, and stale catalog states explicitly', () => {
    expect(describeMetadataStatus(new metadata.CatalogStatus({
        state: 'never_synced',
        locale: 'en-GB',
        matchedStoryCount: 0,
    })).state).toBe('metadata-never-synced');
    expect(describeMetadataStatus(new metadata.CatalogStatus({
        state: 'fresh',
        locale: 'en-GB',
        matchedStoryCount: 1,
        activatedAt: '2026-07-25T16:00:00Z',
    }))).toMatchObject({
        state: 'metadata-fresh',
        tone: 'success',
        title: 'Official metadata is available',
    });
    expect(describeMetadataStatus(new metadata.CatalogStatus({
        state: 'stale_cache',
        locale: 'en-GB',
        matchedStoryCount: 4,
        activatedAt: '2026-07-25T16:00:00Z',
        errorMessage: 'Official metadata could not be downloaded.',
    }))).toMatchObject({
        state: 'metadata-stale-cache',
        tone: 'warning',
        title: 'Using saved official metadata',
    });
});

test('describes metadata operation progress, cancellation, and last-known-good failure', () => {
    const running = new operations.Snapshot({
        id: 'sync',
        kind: 'metadata_sync',
        status: 'running',
        completedItems: 0,
        totalItems: 1,
        cancelRequested: false,
        items: [],
    });
    expect(describeMetadataRefresh(running, null)).toMatchObject({
        state: 'metadata-refreshing',
        tone: 'working',
    });

    const stale = new metadata.CatalogStatus({
        state: 'stale_cache',
        locale: 'en-GB',
        matchedStoryCount: 2,
        activatedAt: '2026-07-25T16:00:00Z',
    });
    expect(describeMetadataRefresh(new operations.Snapshot({
        ...running,
        status: 'failed',
        errorMessage: 'Official metadata could not be downloaded.',
    }), stale)).toMatchObject({
        state: 'metadata-refresh-stale-cache',
        tone: 'warning',
    });
    expect(describeMetadataRefresh(new operations.Snapshot({
        ...running,
        status: 'cancelled',
    }), stale)).toMatchObject({
        state: 'metadata-refresh-cancelled',
        tone: 'neutral',
    });
});
