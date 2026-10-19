import {
    Settings, SetUILanguage, SelectLibraryLocation, ConfirmLibrarySetup, StartLibraryMove,
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
        const initialSettings=await Settings();
        const settingsScenario=!initialSettings.isolated;
        if (settingsScenario) {
            const language=await SetUILanguage('en');if(language.error)throw new Error(language.error);
            await SelectLibraryLocation();
            const setup=await ConfirmLibrarySetup();if(!setup.success)throw new Error('setup failed');
            await checkpoint('setup_completed');
            await checkpoint('settings_menu_selected');
        }
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
        if (settingsScenario) {
            const selected=await SelectLibraryLocation();if(!selected.preparation){await checkpoint('move_selection_failed');throw new Error(selected.error || 'move preparation missing');}
            const move=await StartLibraryMove(selected.preparation.id);if(move.error)throw new Error(move.error);
            const deadline=Date.now()+operationTimeoutMilliseconds;
            let moved=false;
            while(Date.now()<deadline){const state=await Settings();if(state.move?.phase==='succeeded'){moved=true;break;}if(['failed','cleanup_pending'].includes(state.move?.phase ?? ''))throw new Error(state.move?.error);await new Promise((resolve)=>setTimeout(resolve,pollIntervalMilliseconds));}
            if(!moved)throw new Error('library move timed out');
            await checkpoint('library_relocated');
        }

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
    } catch (error) {
        console.error('packaged acceptance failed', error);
        await checkpoint('failed');
    }
}

function requiredOperation(
    response: {operation?: operations.Snapshot; error?: {message: string}},
    label: string,
): operations.Snapshot {
    if (response.error || !response.operation?.id) {
        const detail = response.error?.message ?? 'operation was not returned';
        throw new Error(`packaged ${label} did not start: ${detail}`);
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
