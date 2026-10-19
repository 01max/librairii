import {outcomeText} from './outcome-message';
import {t, currentLanguage} from './i18n';
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
        return t("No successful refresh has been recorded.");
    }
    const parsed = new Date(value);
    if (Number.isNaN(parsed.valueOf())) {
        return t("Last updated {0}.", value);
    }
    return t("Last updated {0}.", parsed.toLocaleString(currentLanguage()));
}

function matched(status: metadata.CatalogStatus | null): string {
    const count = status?.matchedStoryCount ?? 0;
    return t("{0} local {1} matched.", count, count === 1 ? 'story' : 'stories');
}

export function describeMetadataStatus(
    status: metadata.CatalogStatus,
): MetadataNotice {
    switch (status.state) {
        case 'fresh':
            return {
                state: 'metadata-fresh',
                tone: 'success',
                title: t("Official metadata is available"),
                message: `${matched(status)} ${freshness(status)}`,
            };
        case 'stale_cache':
            return {
                state: 'metadata-stale-cache',
                tone: 'warning',
                title: t("Using saved official metadata"),
                message: `${(status.errorMessage ? outcomeText(status.errorMessage) : null) || t("The latest refresh failed.")} ${freshness(status)}`,
            };
        default:
            return {
                state: 'metadata-never-synced',
                tone: 'neutral',
                title: t("Official metadata has not been synced"),
                message: t("Your collection continues to use embedded and local fallback details."),
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
            title: t("Refreshing official metadata"),
            message: t("{0} of {1} refresh phases complete. Your local collection remains available.", Math.min(operation.completedItems, operation.totalItems), operation.totalItems),
        };
    }
    if (operation.status === 'succeeded') {
        return {
            state: 'metadata-refresh-success',
            tone: 'success',
            title: t("Official metadata refreshed"),
            message: `${matched(status)} ${freshness(status)}`,
        };
    }
    if (operation.status === 'cancelled') {
        return {
            state: 'metadata-refresh-cancelled',
            tone: 'neutral',
            title: t("Official metadata refresh cancelled"),
            message: status?.state === 'never_synced'
                ? 'No official catalog is active; embedded and fallback details remain available.'
                : t("The existing saved catalog was kept. {0}", freshness(status)),
        };
    }
    if (status?.state === 'stale_cache') {
        return {
            state: 'metadata-refresh-stale-cache',
            tone: 'warning',
            title: t("Refresh failed — using saved metadata"),
            message: `${(operation.errorMessage ? outcomeText(operation.errorMessage) : null) || (status.errorMessage ? outcomeText(status.errorMessage) : null) || t("Official metadata could not be refreshed.")} ${freshness(status)}`,
        };
    }
    return {
        state: 'metadata-refresh-failed',
        tone: 'error',
        title: t("Official metadata is unavailable"),
        message: (operation.errorMessage ? outcomeText(operation.errorMessage) : null) ||
            t("The refresh failed. Embedded and fallback details remain available."),
    };
}
