import {expect, test} from 'vitest';
import {operations} from '../wailsjs/go/models';
import {describeExport} from './export-state';

test('describes persisted export progress and safe cancellation', () => {
    const running = new operations.Snapshot({
        id: 'export',
        kind: 'export',
        status: 'running',
        completedItems: 1,
        totalItems: 2,
        totalBytes: 2097152,
        cancelRequested: false,
        createdAt: '2026-07-25T10:00:00Z',
        items: [{
            id: 1,
            sourceName: 'first.zip',
            status: 'succeeded',
            completedBytes: 1048576,
            totalBytes: 1048576,
        }, {
            id: 2,
            sourceName: 'second.zip',
            status: 'running',
            completedBytes: 524288,
            totalBytes: 1048576,
        }],
    });
    expect(describeExport(running)).toMatchObject({
        state: 'export-running',
        title: 'Exporting story archives',
        message: '1/2 finished · 1.5 MB of 2.0 MB. Progress is saved locally.',
    });
    expect(describeExport(new operations.Snapshot({
        ...running,
        cancelRequested: true,
    }))).toMatchObject({
        state: 'export-cancelling',
        title: 'Cancelling export safely',
    });
});
