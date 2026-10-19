import {outcomeText} from './outcome-message';
import {t} from './i18n';
import {operations} from '../wailsjs/go/models';

export type ExportNotice = {
    state: string;
    tone: 'working' | 'success' | 'warning' | 'error' | 'neutral';
    title: string;
    message: string;
};

export function describeExport(snapshot: operations.Snapshot): ExportNotice {
    const completedBytes = snapshot.items.reduce(
        (total, item) => total + item.completedBytes,
        0,
    );
    const progress = t("{0}/{1} finished", snapshot.completedItems, snapshot.totalItems) +
        (snapshot.totalBytes > 0
            ? ` · ${formatBytes(completedBytes)} of ${formatBytes(snapshot.totalBytes)}`
            : '');
    if (snapshot.cancelRequested && !snapshot.finishedAt) {
        return {
            state: 'export-cancelling',
            tone: 'working',
            title: t("Cancelling export safely"),
            message: t("{0}. Completed files will remain in the destination.", progress),
        };
    }
    switch (snapshot.status) {
        case 'queued':
            return {
                state: 'export-queued',
                tone: 'working',
                title: t("Export queued"),
                message: t("{0} {1} waiting for a bounded worker.", snapshot.totalItems, snapshot.totalItems === 1 ? 'story is' : 'stories are'),
            };
        case 'running':
            return {
                state: 'export-running',
                tone: 'working',
                title: t("Exporting story archives"),
                message: t("{0}. Progress is saved locally.", progress),
            };
        case 'succeeded':
            return {
                state: 'export-succeeded',
                tone: 'success',
                title: t("Export complete"),
                message: t("{0} {1} exported.", snapshot.completedItems, snapshot.completedItems === 1 ? 'story was' : 'stories were'),
            };
        case 'partially_succeeded':
            return {
                state: 'export-partial',
                tone: 'warning',
                title: t("Export completed with exceptions"),
                message: 'Completed files are available; review the stories not exported.',
            };
        case 'cancelled':
            return {
                state: 'export-cancelled',
                tone: 'neutral',
                title: t("Export cancelled"),
                message: t("Completed files were preserved and incomplete temporary files removed."),
            };
        default:
            return {
                state: 'export-failed',
                tone: 'error',
                title: t("Export could not finish"),
                message: (snapshot.errorMessage ? outcomeText(snapshot.errorMessage) : null) || t("No destination files were overwritten."),
            };
    }
}

function formatBytes(byteSize: number): string {
    if (byteSize < 1024 * 1024) {
        return `${Math.max(0, Math.round(byteSize / 1024))} KB`;
    }
    return `${(byteSize / (1024 * 1024)).toFixed(1)} MB`;
}
