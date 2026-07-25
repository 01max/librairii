import {
    OperationSnapshot,
    PackagedAcceptanceMode,
    QueryStories,
    RecordPackagedAcceptance,
    RevealExportDestination,
    SelectAndImportStories,
    SelectAndPreflightExport,
    StartPreparedExport,
} from '../wailsjs/go/main/App';
import {exporter, library, type operations} from '../wailsjs/go/models';
import {DEFAULT_COLLECTION_QUERY} from './query-codec';

const pollIntervalMilliseconds = 50;
const operationTimeoutMilliseconds = 15_000;

export async function runPackagedAcceptance(): Promise<void> {
    if (!await PackagedAcceptanceMode()) {
        return;
    }
    try {
        await checkpoint('scenario_started');
        const imported = await SelectAndImportStories();
        const importOperation = requiredOperation(imported, 'import');
        await checkpoint('import_queued');
        await waitForSuccess(importOperation.id, 'import');
        await checkpoint('import_succeeded');

        const query = new library.StoryLibraryQuery({
            ...DEFAULT_COLLECTION_QUERY,
            page: 1,
            pageSize: 24,
            sort: 'name_asc',
        });
        const collection = await QueryStories(query);
        if (collection.error || collection.page?.totalItems !== 1) {
            throw new Error('packaged collection did not contain the imported story');
        }
        await checkpoint('collection_loaded');

        const prepared = await SelectAndPreflightExport(new exporter.PreflightRequest({
            sourceType: 'current_query',
            query,
            storyIds: [],
            shelfIds: [],
        }));
        if (
            prepared.error ||
            !prepared.preflight?.canExport ||
            !prepared.preflight.preparationId
        ) {
            throw new Error('packaged export preflight was not ready');
        }
        await checkpoint('export_prepared');

        const exported = await StartPreparedExport(
            prepared.preflight.preparationId,
        );
        const exportOperation = requiredOperation(exported, 'export');
        await checkpoint('export_queued');
        await waitForSuccess(exportOperation.id, 'export');
        await checkpoint('export_succeeded');

        const revealed = await RevealExportDestination(exportOperation.id);
        if (revealed.error || !revealed.success) {
            throw new Error('packaged reveal binding failed');
        }
        await checkpoint('reveal_succeeded');
        await checkpoint('complete');
    } catch {
        await checkpoint('failed');
    }
}

function requiredOperation(
    response: {operation?: operations.Snapshot; error?: {message: string}},
    label: string,
): operations.Snapshot {
    if (response.error || !response.operation?.id) {
        throw new Error(`packaged ${label} did not start`);
    }
    return response.operation;
}

async function waitForSuccess(operationID: string, label: string): Promise<void> {
    const deadline = Date.now() + operationTimeoutMilliseconds;
    while (Date.now() < deadline) {
        const response = await OperationSnapshot(operationID);
        if (response.error || !response.operation) {
            throw new Error(`packaged ${label} progress could not be read`);
        }
        switch (response.operation.status) {
            case 'succeeded':
                return;
            case 'failed':
            case 'cancelled':
            case 'interrupted':
                throw new Error(`packaged ${label} ended as ${response.operation.status}`);
        }
        await delay(pollIntervalMilliseconds);
    }
    throw new Error(`packaged ${label} timed out`);
}

async function checkpoint(name: string): Promise<void> {
    const response = await RecordPackagedAcceptance(name);
    if (response.error || !response.success) {
        throw new Error(`packaged acceptance checkpoint ${name} failed`);
    }
}

function delay(milliseconds: number): Promise<void> {
    return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}
