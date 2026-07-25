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
    const progress = `${snapshot.completedItems}/${snapshot.totalItems} finished` +
        (snapshot.totalBytes > 0
            ? ` · ${formatBytes(completedBytes)} of ${formatBytes(snapshot.totalBytes)}`
            : '');
    if (snapshot.cancelRequested && !snapshot.finishedAt) {
        return {
            state: 'export-cancelling',
            tone: 'working',
            title: 'Cancelling export safely',
            message: `${progress}. Completed files will remain in the destination.`,
        };
    }
    switch (snapshot.status) {
        case 'queued':
            return {
                state: 'export-queued',
                tone: 'working',
                title: 'Export queued',
                message: `${snapshot.totalItems} ${
                    snapshot.totalItems === 1 ? 'story is' : 'stories are'
                } waiting for a bounded worker.`,
            };
        case 'running':
            return {
                state: 'export-running',
                tone: 'working',
                title: 'Exporting story archives',
                message: `${progress}. Progress is saved locally.`,
            };
        case 'succeeded':
            return {
                state: 'export-succeeded',
                tone: 'success',
                title: 'Export complete',
                message: `${snapshot.completedItems} ${
                    snapshot.completedItems === 1 ? 'story was' : 'stories were'
                } exported.`,
            };
        case 'partially_succeeded':
            return {
                state: 'export-partial',
                tone: 'warning',
                title: 'Export completed with exceptions',
                message: 'Completed files are available; review the stories not exported.',
            };
        case 'cancelled':
            return {
                state: 'export-cancelled',
                tone: 'neutral',
                title: 'Export cancelled',
                message: 'Completed files were preserved and incomplete temporary files removed.',
            };
        default:
            return {
                state: 'export-failed',
                tone: 'error',
                title: 'Export could not finish',
                message: snapshot.errorMessage || 'No destination files were overwritten.',
            };
    }
}

function formatBytes(byteSize: number): string {
    if (byteSize < 1024 * 1024) {
        return `${Math.max(0, Math.round(byteSize / 1024))} KB`;
    }
    return `${(byteSize / (1024 * 1024)).toFixed(1)} MB`;
}
