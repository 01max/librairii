import {beforeEach, expect, test, vi} from 'vitest';
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
import {app, exporter, library, operations} from '../wailsjs/go/models';
import {runPackagedAcceptance} from './packaged-acceptance';

vi.mock('../wailsjs/go/main/App', () => ({
    OperationSnapshot: vi.fn(),
    PackagedAcceptanceMode: vi.fn(),
    QueryStories: vi.fn(),
    RecordPackagedAcceptance: vi.fn(),
    RevealExportDestination: vi.fn(),
    SelectAndImportStories: vi.fn(),
    SelectAndPreflightExport: vi.fn(),
    StartPreparedExport: vi.fn(),
}));

const operationSnapshot = vi.mocked(OperationSnapshot);
const packagedAcceptanceMode = vi.mocked(PackagedAcceptanceMode);
const queryStories = vi.mocked(QueryStories);
const recordPackagedAcceptance = vi.mocked(RecordPackagedAcceptance);
const revealExportDestination = vi.mocked(RevealExportDestination);
const selectAndImportStories = vi.mocked(SelectAndImportStories);
const selectAndPreflightExport = vi.mocked(SelectAndPreflightExport);
const startPreparedExport = vi.mocked(StartPreparedExport);

beforeEach(() => {
    vi.clearAllMocks();
    packagedAcceptanceMode.mockResolvedValue(true);
    recordPackagedAcceptance.mockResolvedValue(new app.MutationResponse({
        success: true,
    }));
    selectAndImportStories.mockResolvedValue(new app.OperationResponse({
        operation: new operations.Snapshot({
            id: 'import-operation',
            status: 'queued',
        }),
    }));
    operationSnapshot.mockImplementation(async (operationID: string) => (
        new app.OperationResponse({
            operation: new operations.Snapshot({
                id: operationID,
                status: 'succeeded',
            }),
        })
    ));
    queryStories.mockResolvedValue(new app.LibraryPageResponse({
        page: new library.Page({totalItems: 1}),
    }));
    selectAndPreflightExport.mockResolvedValue(new app.ExportPreflightResponse({
        preflight: new exporter.PreflightReport({
            preparationId: 'export-preparation',
            canExport: true,
        }),
    }));
    startPreparedExport.mockResolvedValue(new app.OperationResponse({
        operation: new operations.Snapshot({
            id: 'export-operation',
            status: 'queued',
        }),
    }));
    revealExportDestination.mockResolvedValue(new app.MutationResponse({
        success: true,
    }));
});

test('does nothing outside explicit packaged acceptance', async () => {
    packagedAcceptanceMode.mockResolvedValue(false);

    await runPackagedAcceptance();

    expect(selectAndImportStories).not.toHaveBeenCalled();
    expect(recordPackagedAcceptance).not.toHaveBeenCalled();
});

test('drives import, progress, query, export, and reveal through Wails bindings', async () => {
    await runPackagedAcceptance();

    expect(operationSnapshot).toHaveBeenCalledWith('import-operation');
    expect(operationSnapshot).toHaveBeenCalledWith('export-operation');
    expect(queryStories).toHaveBeenCalledOnce();
    expect(selectAndPreflightExport).toHaveBeenCalledOnce();
    expect(revealExportDestination).toHaveBeenCalledWith('export-operation');
    expect(recordPackagedAcceptance.mock.calls.map(([checkpoint]) => checkpoint))
        .toEqual([
            'scenario_started',
            'import_queued',
            'import_succeeded',
            'collection_loaded',
            'export_prepared',
            'export_queued',
            'export_succeeded',
            'reveal_succeeded',
            'complete',
        ]);
});

test('records a terminal failure when a packaged binding rejects the flow', async () => {
    selectAndImportStories.mockResolvedValue(new app.OperationResponse({
        error: new app.APIError({
            code: 'internal',
            message: 'picker failed',
        }),
    }));

    await runPackagedAcceptance();

    expect(recordPackagedAcceptance).toHaveBeenLastCalledWith('failed');
    expect(queryStories).not.toHaveBeenCalled();
});
