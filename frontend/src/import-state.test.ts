import {describe, expect, test} from 'vitest';
import {operations} from '../wailsjs/go/models';
import {describeImport} from './import-state';

function snapshot(
    status: string,
    outcomeCode: string,
    outcomeMessage = '',
): operations.Snapshot {
    return new operations.Snapshot({
        id: '00112233-4455-4677-8899-aabbccddeeff',
        kind: 'import',
        status,
        completedItems: status === 'running' ? 0 : 1,
        totalItems: 1,
        cancelRequested: false,
        createdAt: '2026-07-25T09:00:00Z',
        items: [{
            id: 1,
            sourceName: 'story.zip',
            status: status === 'running' ? 'running' : 'failed',
            outcomeCode,
            outcomeMessage,
            completedBytes: 0,
            totalBytes: 100,
        }],
    });
}

describe.each([
    ['running', '', 'importing'],
    ['succeeded', 'imported', 'success'],
    ['succeeded', 'duplicate_checksum', 'duplicate-checksum'],
    ['failed', 'uuid_conflict', 'uuid-conflict'],
    ['failed', 'invalid_container', 'invalid-format'],
    ['failed', 'import_failed', 'failed-import'],
])('%s import with %s', (status, outcomeCode, expectedState) => {
    test(`maps to the ${expectedState} UI state`, () => {
        expect(describeImport(snapshot(status, outcomeCode)).state).toBe(expectedState);
    });
});
