import {metadata, operations} from '../wailsjs/go/models';
import {operationIsActive} from './import-state';

export type MetadataNoticeTone = 'working' | 'success' | 'warning' | 'error' | 'neutral';

export type MetadataNotice = {
    state: string;
    tone: MetadataNoticeTone;
    title: string;
    message: string;
};

function freshness(status: metadata.CatalogStatus | null): string {
    const value = status?.activatedAt || status?.fetchedAt;
    if (!value) {
        return 'No successful refresh has been recorded.';
    }
    const parsed = new Date(value);
    if (Number.isNaN(parsed.valueOf())) {
        return `Last updated ${value}.`;
    }
    return `Last updated ${parsed.toLocaleString()}.`;
}

function matched(status: metadata.CatalogStatus | null): string {
    const count = status?.matchedStoryCount ?? 0;
    return `${count} local ${count === 1 ? 'story' : 'stories'} matched.`;
}

export function describeMetadataStatus(
    status: metadata.CatalogStatus,
): MetadataNotice {
    switch (status.state) {
        case 'fresh':
            return {
                state: 'metadata-fresh',
                tone: 'success',
                title: 'Official metadata is available',
                message: `${matched(status)} ${freshness(status)}`,
            };
        case 'stale_cache':
            return {
                state: 'metadata-stale-cache',
                tone: 'warning',
                title: 'Using saved official metadata',
                message: `${status.errorMessage || 'The latest refresh failed.'} ${freshness(status)}`,
            };
        default:
            return {
                state: 'metadata-never-synced',
                tone: 'neutral',
                title: 'Official metadata has not been synced',
                message: 'Your collection continues to use embedded and local fallback details.',
            };
    }
}

export function describeMetadataRefresh(
    operation: operations.Snapshot,
    status: metadata.CatalogStatus | null,
): MetadataNotice {
    if (operationIsActive(operation)) {
        return {
            state: 'metadata-refreshing',
            tone: 'working',
            title: 'Refreshing official metadata',
            message: `${Math.min(operation.completedItems, operation.totalItems)} of ${operation.totalItems} refresh phases complete. Your local collection remains available.`,
        };
    }
    if (operation.status === 'succeeded') {
        return {
            state: 'metadata-refresh-success',
            tone: 'success',
            title: 'Official metadata refreshed',
            message: `${matched(status)} ${freshness(status)}`,
        };
    }
    if (operation.status === 'cancelled') {
        return {
            state: 'metadata-refresh-cancelled',
            tone: 'neutral',
            title: 'Official metadata refresh cancelled',
            message: status?.state === 'never_synced'
                ? 'No official catalog is active; embedded and fallback details remain available.'
                : `The existing saved catalog was kept. ${freshness(status)}`,
        };
    }
    if (status?.state === 'stale_cache') {
        return {
            state: 'metadata-refresh-stale-cache',
            tone: 'warning',
            title: 'Refresh failed — using saved metadata',
            message: `${operation.errorMessage || status.errorMessage || 'Official metadata could not be refreshed.'} ${freshness(status)}`,
        };
    }
    return {
        state: 'metadata-refresh-failed',
        tone: 'error',
        title: 'Official metadata is unavailable',
        message: operation.errorMessage ||
            'The refresh failed. Embedded and fallback details remain available.',
    };
}
