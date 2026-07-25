package app

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"sync"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/diagnostics"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
	"github.com/01max/librairii/internal/tagging"
	"github.com/google/uuid"
)

var ErrImportRuntimeNotReady = errors.New("import runtime storage is not ready")

const maxPreparedExports = 8

type StorageProvider interface {
	Layout() storage.Layout
	SQL() *sql.DB
}

type ImportRuntime struct {
	mu               sync.RWMutex
	storage          StorageProvider
	clock            Clock
	events           EventPort
	workers          int
	metadataFetcher  metadata.CatalogFetcher
	manager          *operations.Manager
	query            *library.Query
	removal          *removal.Service
	tags             *tagging.Service
	shelves          *shelves.Service
	shelfEvaluator   *shelves.Evaluator
	exportPreflight  *exporter.PreflightService
	diagnosticLogger *diagnostics.Logger
	diagnostics      *diagnostics.Service
	preparedExports  map[string]exporter.PreflightReport
	preparedOrder    []string
}

type ImportRuntimeOption func(*ImportRuntime)

func WithMetadataFetcher(fetcher metadata.CatalogFetcher) ImportRuntimeOption {
	return func(runtime *ImportRuntime) {
		runtime.metadataFetcher = fetcher
	}
}

func NewImportRuntime(
	storage StorageProvider,
	clock Clock,
	events EventPort,
	workers int,
	options ...ImportRuntimeOption,
) (*ImportRuntime, error) {
	if storage == nil || clock == nil || events == nil || workers < 1 {
		return nil, ErrMissingDependency
	}
	runtime := &ImportRuntime{
		storage:         storage,
		clock:           clock,
		events:          events,
		workers:         workers,
		preparedExports: make(map[string]exporter.PreflightReport),
	}
	for _, option := range options {
		if option != nil {
			option(runtime)
		}
	}
	if runtime.metadataFetcher == nil {
		defaultFetcher, err := lunii.NewCatalogClient(lunii.ProductionConfig())
		if err != nil {
			return nil, fmt.Errorf("construct metadata catalog client: %w", err)
		}
		runtime.metadataFetcher = defaultFetcher
	}
	return runtime, nil
}

func (r *ImportRuntime) Start(ctx context.Context) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.manager != nil {
		return nil
	}
	database := r.storage.SQL()
	layout := r.storage.Layout()
	if database == nil || layout.Root == "" {
		return ErrImportRuntimeNotReady
	}
	diagnosticLogger, err := diagnostics.NewLogger(
		layout.Logs,
		diagnostics.DefaultPolicy(),
		r.clock.Now,
	)
	if err != nil {
		return fmt.Errorf("open diagnostic logger: %w", err)
	}
	loggerOwned := true
	defer func() {
		if loggerOwned {
			_ = diagnosticLogger.Close()
		}
	}()
	diagnosticService, err := diagnostics.NewService(
		layout,
		database,
		diagnosticLogger,
	)
	if err != nil {
		return fmt.Errorf("construct diagnostic export service: %w", err)
	}
	if _, err := tagging.SeedBuiltIns(ctx, database); err != nil {
		return fmt.Errorf("seed built-in tags: %w", err)
	}
	if err := metadata.SeedDefaultDerivedFacets(ctx, database); err != nil {
		return fmt.Errorf("seed derived tags: %w", err)
	}
	if err := library.BackfillNormalizedDisplayNames(ctx, database); err != nil {
		return fmt.Errorf("backfill story display names: %w", err)
	}
	if err := metadata.BackfillNormalizedTitles(ctx, database); err != nil {
		return fmt.Errorf("backfill official display names: %w", err)
	}

	archiveRepository := archive.NewRepository(layout)
	metadataRepository := metadata.NewRepository(database)
	catalogProjector, err := metadata.NewCatalogProjector(
		database,
		metadata.DefaultCatalogProjectionConfig(),
	)
	if err != nil {
		return fmt.Errorf("construct metadata catalog projector: %w", err)
	}
	catalogRepository := catalog.NewRepository(database, catalogProjector)
	metadataRefresh, err := metadata.NewRefreshService(
		r.metadataFetcher,
		metadataRepository,
		metadata.NewRawSnapshotStore(layout),
	)
	if err != nil {
		return fmt.Errorf("construct metadata refresh service: %w", err)
	}
	importService, err := importer.NewService(
		archiveRepository,
		artwork.NewRepository(layout),
		catalogRepository,
		inspection.NewStoryInspector(),
		inspection.DefaultLimits(),
	)
	if err != nil {
		return fmt.Errorf("construct import service: %w", err)
	}
	exportCopier, err := exporter.NewCopier(layout)
	if err != nil {
		return fmt.Errorf("construct export copier: %w", err)
	}
	manager, err := operations.NewManager(operations.Dependencies{
		Repository:     operations.NewRepository(database),
		Imports:        importService,
		Exports:        exportCopier,
		ExportRecovery: exportCopier,
		Metadata:       metadataRefresh,
		Events: diagnosticEventPort{
			delegate: r.events,
			logger:   diagnosticLogger,
		},
		Clock:          r.clock,
		StagingCleaner: archiveRepository,
		Workers:        r.workers,
	})
	if err != nil {
		return fmt.Errorf("construct operation manager: %w", err)
	}
	removalService, err := removal.NewService(
		catalogRepository,
		archiveRepository,
		removal.NewRepository(database),
	)
	if err != nil {
		return fmt.Errorf("construct removal service: %w", err)
	}
	if err := removalService.Reconcile(ctx); err != nil {
		return fmt.Errorf("reconcile removals: %w", err)
	}
	tagService, err := tagging.NewService(database)
	if err != nil {
		return fmt.Errorf("construct tagging service: %w", err)
	}
	if err := manager.Start(ctx); err != nil {
		return err
	}
	officialProvider, err := metadata.NewLibraryProvider(
		metadataRepository,
		metadata.DefaultLocale,
	)
	if err != nil {
		_ = manager.Close()
		return fmt.Errorf("construct official metadata provider: %w", err)
	}
	libraryQuery := library.NewQuery(database, officialProvider)
	shelfService, err := shelves.NewService(database)
	if err != nil {
		_ = manager.Close()
		return fmt.Errorf("construct saved shelf service: %w", err)
	}
	shelfEvaluator, err := shelves.NewEvaluator(shelfService, libraryQuery)
	if err != nil {
		_ = manager.Close()
		return fmt.Errorf("construct saved shelf evaluator: %w", err)
	}
	exportResolver, err := exporter.NewResolver(libraryQuery, shelfService)
	if err != nil {
		_ = manager.Close()
		return fmt.Errorf("construct export resolver: %w", err)
	}
	exportPreflight, err := exporter.NewPreflightService(exportResolver, layout)
	if err != nil {
		_ = manager.Close()
		return fmt.Errorf("construct export preflight: %w", err)
	}
	if err := diagnosticLogger.Record(
		diagnostics.LevelInfo,
		diagnostics.EventRuntimeStarted,
		"ready",
	); err != nil {
		_ = manager.Close()
		return fmt.Errorf("record runtime startup: %w", err)
	}
	r.manager = manager
	r.query = libraryQuery
	r.removal = removalService
	r.tags = tagService
	r.shelves = shelfService
	r.shelfEvaluator = shelfEvaluator
	r.exportPreflight = exportPreflight
	r.diagnosticLogger = diagnosticLogger
	r.diagnostics = diagnosticService
	loggerOwned = false
	return nil
}

type diagnosticEventPort struct {
	delegate EventPort
	logger   *diagnostics.Logger
}

func (p diagnosticEventPort) Emit(ctx context.Context, name string, payload any) {
	p.delegate.Emit(ctx, name, payload)
	if name != operations.EventChanged {
		return
	}
	snapshot, ok := payload.(operations.Snapshot)
	if !ok {
		return
	}
	level := diagnostics.LevelInfo
	if snapshot.Status == operations.StatusFailed ||
		snapshot.Status == operations.StatusInterrupted {
		level = diagnostics.LevelWarn
	}
	_ = p.logger.Record(
		level,
		diagnostics.EventOperationChanged,
		string(snapshot.Kind)+":"+string(snapshot.Status),
	)
}

func (r *ImportRuntime) StartImport(
	ctx context.Context,
	paths []string,
) (operations.Snapshot, error) {
	manager, err := r.current()
	if err != nil {
		return operations.Snapshot{}, err
	}
	return manager.StartImport(ctx, paths)
}

func (r *ImportRuntime) StartMetadataRefresh(
	ctx context.Context,
	locale string,
) (operations.Snapshot, error) {
	manager, err := r.current()
	if err != nil {
		return operations.Snapshot{}, err
	}
	return manager.StartMetadataRefresh(ctx, locale)
}

func (r *ImportRuntime) PrepareExport(
	ctx context.Context,
	request exporter.PreflightRequest,
	destination string,
) (exporter.PreflightReport, error) {
	r.mu.RLock()
	preflight := r.exportPreflight
	r.mu.RUnlock()
	if preflight == nil {
		return exporter.PreflightReport{}, ErrImportRuntimeNotReady
	}
	report, err := preflight.Plan(ctx, request, destination)
	if err != nil || !report.CanExport {
		return report, err
	}
	report.PreparationID = uuid.NewString()
	r.mu.Lock()
	if r.exportPreflight != preflight {
		r.mu.Unlock()
		return exporter.PreflightReport{}, ErrImportRuntimeNotReady
	}
	r.storePreparedExportLocked(report)
	r.mu.Unlock()
	return report, nil
}

func (r *ImportRuntime) StartPreparedExport(
	ctx context.Context,
	preparationID string,
) (operations.Snapshot, error) {
	if _, err := uuid.Parse(preparationID); err != nil {
		return operations.Snapshot{}, operations.ErrInvalidRequest
	}
	manager, err := r.current()
	if err != nil {
		return operations.Snapshot{}, err
	}
	r.mu.Lock()
	report, found := r.takePreparedExportLocked(preparationID)
	r.mu.Unlock()
	if !found || !report.CanExport {
		return operations.Snapshot{}, operations.ErrInvalidRequest
	}
	snapshot, err := manager.StartExport(
		ctx,
		report.Source,
		report.Destination,
		report.OperationItems(),
	)
	if err != nil {
		r.mu.Lock()
		if r.manager == manager {
			r.storePreparedExportLocked(report)
		}
		r.mu.Unlock()
		return operations.Snapshot{}, err
	}
	return snapshot, nil
}

func (r *ImportRuntime) MetadataStatus(
	ctx context.Context,
	locale string,
) (metadata.CatalogStatus, error) {
	if _, err := r.current(); err != nil {
		return metadata.CatalogStatus{}, err
	}
	database := r.storage.SQL()
	if database == nil {
		return metadata.CatalogStatus{}, ErrImportRuntimeNotReady
	}
	return metadata.NewRepository(database).CatalogStatus(ctx, locale)
}

func (r *ImportRuntime) ExportDiagnostics(
	ctx context.Context,
	destination string,
) (diagnostics.Report, error) {
	r.mu.RLock()
	service := r.diagnostics
	r.mu.RUnlock()
	if service == nil {
		return diagnostics.Report{}, ErrImportRuntimeNotReady
	}
	return service.Export(ctx, destination)
}

func (r *ImportRuntime) Cancel(
	ctx context.Context,
	operationID string,
) (operations.Snapshot, error) {
	manager, err := r.current()
	if err != nil {
		return operations.Snapshot{}, err
	}
	return manager.Cancel(ctx, operationID)
}

func (r *ImportRuntime) Snapshot(
	ctx context.Context,
	operationID string,
) (operations.Snapshot, error) {
	manager, err := r.current()
	if err != nil {
		return operations.Snapshot{}, err
	}
	return manager.Snapshot(ctx, operationID)
}

func (r *ImportRuntime) Active(
	ctx context.Context,
) ([]operations.Snapshot, error) {
	manager, err := r.current()
	if err != nil {
		return nil, err
	}
	return manager.Active(ctx)
}

func (r *ImportRuntime) List(
	ctx context.Context,
	request library.ListRequest,
) (library.Page, error) {
	query, err := r.currentQuery()
	if err != nil {
		return library.Page{}, err
	}
	return query.List(ctx, request)
}

func (r *ImportRuntime) Search(
	ctx context.Context,
	request library.StoryLibraryQuery,
) (library.Page, error) {
	query, err := r.currentQuery()
	if err != nil {
		return library.Page{}, err
	}
	return query.Search(ctx, request)
}

func (r *ImportRuntime) Detail(
	ctx context.Context,
	storyID int64,
) (library.StoryDetail, error) {
	query, err := r.currentQuery()
	if err != nil {
		return library.StoryDetail{}, err
	}
	return query.Detail(ctx, storyID)
}

func (r *ImportRuntime) Remove(
	ctx context.Context,
	storyID int64,
) (removal.Result, error) {
	service, err := r.currentRemoval()
	if err != nil {
		return removal.Result{}, err
	}
	return service.Remove(ctx, storyID)
}

func (r *ImportRuntime) Catalog(ctx context.Context) (tagging.Catalog, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.Catalog{}, err
	}
	return service.Catalog(ctx)
}

func (r *ImportRuntime) CreateDefinition(
	ctx context.Context,
	input tagging.CreateDefinition,
) (tagging.Definition, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.Definition{}, err
	}
	return service.CreateDefinition(ctx, input)
}

func (r *ImportRuntime) RenameDefinition(
	ctx context.Context,
	definitionID int64,
	label string,
) (tagging.Definition, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.Definition{}, err
	}
	return service.RenameDefinition(ctx, definitionID, label)
}

func (r *ImportRuntime) RecolorDefinition(
	ctx context.Context,
	definitionID int64,
	color string,
) (tagging.Definition, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.Definition{}, err
	}
	return service.RecolorDefinition(ctx, definitionID, color)
}

func (r *ImportRuntime) ReorderDefinitions(
	ctx context.Context,
	orderedIDs []int64,
) ([]tagging.Definition, error) {
	service, err := r.currentTags()
	if err != nil {
		return nil, err
	}
	return service.ReorderDefinitions(ctx, orderedIDs)
}

func (r *ImportRuntime) PlanDefinitionDeletion(
	ctx context.Context,
	definitionID int64,
) (tagging.DefinitionDeletionPlan, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.DefinitionDeletionPlan{}, err
	}
	return service.PlanDefinitionDeletion(ctx, definitionID)
}

func (r *ImportRuntime) DeleteDefinition(
	ctx context.Context,
	plan tagging.DefinitionDeletionPlan,
) error {
	service, err := r.currentTags()
	if err != nil {
		return err
	}
	return service.DeleteDefinition(ctx, plan)
}

func (r *ImportRuntime) CreateValue(
	ctx context.Context,
	input tagging.CreateValue,
) (tagging.Value, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.Value{}, err
	}
	return service.CreateValue(ctx, input)
}

func (r *ImportRuntime) RenameValue(
	ctx context.Context,
	valueID int64,
	label string,
) (tagging.Value, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.Value{}, err
	}
	return service.RenameValue(ctx, valueID, label)
}

func (r *ImportRuntime) ReorderValues(
	ctx context.Context,
	definitionID int64,
	orderedIDs []int64,
) ([]tagging.Value, error) {
	service, err := r.currentTags()
	if err != nil {
		return nil, err
	}
	return service.ReorderValues(ctx, definitionID, orderedIDs)
}

func (r *ImportRuntime) PlanValueDeletion(
	ctx context.Context,
	valueID int64,
) (tagging.ValueDeletionPlan, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.ValueDeletionPlan{}, err
	}
	return service.PlanValueDeletion(ctx, valueID)
}

func (r *ImportRuntime) DeleteValue(
	ctx context.Context,
	plan tagging.ValueDeletionPlan,
) error {
	service, err := r.currentTags()
	if err != nil {
		return err
	}
	return service.DeleteValue(ctx, plan)
}

func (r *ImportRuntime) AssignmentWorkspace(
	ctx context.Context,
	storyIDs []int64,
) (tagging.AssignmentWorkspace, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.AssignmentWorkspace{}, err
	}
	return service.AssignmentWorkspace(ctx, storyIDs)
}

func (r *ImportRuntime) SetBulkBoolean(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	assigned bool,
) (tagging.AssignmentResult, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.AssignmentResult{}, err
	}
	return service.SetBulkBoolean(ctx, storyIDs, definitionID, assigned)
}

func (r *ImportRuntime) SetBulkChoiceValues(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	valueIDs []int64,
) (tagging.AssignmentResult, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.AssignmentResult{}, err
	}
	return service.SetBulkChoiceValues(ctx, storyIDs, definitionID, valueIDs)
}

func (r *ImportRuntime) SetBulkChoiceValue(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	valueID int64,
	assigned bool,
) (tagging.AssignmentResult, error) {
	service, err := r.currentTags()
	if err != nil {
		return tagging.AssignmentResult{}, err
	}
	return service.SetBulkChoiceValue(ctx, storyIDs, definitionID, valueID, assigned)
}

func (r *ImportRuntime) ListShelves(
	ctx context.Context,
) ([]shelves.Summary, error) {
	_, evaluator, err := r.currentShelves()
	if err != nil {
		return nil, err
	}
	return evaluator.Summaries(ctx)
}

func (r *ImportRuntime) CreateShelf(
	ctx context.Context,
	name string,
	query library.StoryLibraryQuery,
) (shelves.Shelf, error) {
	service, _, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.Create(ctx, name, query)
}

func (r *ImportRuntime) OpenShelf(
	ctx context.Context,
	shelfID int64,
	request library.ListRequest,
) (shelves.Evaluation, error) {
	_, evaluator, err := r.currentShelves()
	if err != nil {
		return shelves.Evaluation{}, err
	}
	return evaluator.Evaluate(ctx, shelfID, request)
}

func (r *ImportRuntime) RenameShelf(
	ctx context.Context,
	shelfID int64,
	name string,
) (shelves.Shelf, error) {
	service, _, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.Rename(ctx, shelfID, name)
}

func (r *ImportRuntime) DuplicateShelf(
	ctx context.Context,
	shelfID int64,
	name string,
) (shelves.Shelf, error) {
	service, _, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.Duplicate(ctx, shelfID, name)
}

func (r *ImportRuntime) ReplaceShelfQuery(
	ctx context.Context,
	shelfID int64,
	query library.StoryLibraryQuery,
) (shelves.Shelf, error) {
	service, _, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.ReplaceQuery(ctx, shelfID, query)
}

func (r *ImportRuntime) ReorderShelves(
	ctx context.Context,
	orderedIDs []int64,
) ([]shelves.Shelf, error) {
	service, _, err := r.currentShelves()
	if err != nil {
		return nil, err
	}
	return service.Reorder(ctx, orderedIDs)
}

func (r *ImportRuntime) DeleteShelf(
	ctx context.Context,
	shelfID int64,
) error {
	service, _, err := r.currentShelves()
	if err != nil {
		return err
	}
	return service.Delete(ctx, shelfID)
}

func (r *ImportRuntime) PreviewShelves(
	ctx context.Context,
	shelfIDs []int64,
) (shelves.SelectionPreview, error) {
	_, evaluator, err := r.currentShelves()
	if err != nil {
		return shelves.SelectionPreview{}, err
	}
	return evaluator.PreviewSelection(ctx, shelfIDs)
}

func (r *ImportRuntime) Close() error {
	r.mu.Lock()
	manager := r.manager
	diagnosticLogger := r.diagnosticLogger
	r.manager = nil
	r.query = nil
	r.removal = nil
	r.tags = nil
	r.shelves = nil
	r.shelfEvaluator = nil
	r.exportPreflight = nil
	r.diagnosticLogger = nil
	r.diagnostics = nil
	r.preparedExports = make(map[string]exporter.PreflightReport)
	r.preparedOrder = nil
	r.mu.Unlock()
	var closeErrors []error
	if manager != nil {
		closeErrors = append(closeErrors, manager.Close())
	}
	if diagnosticLogger != nil {
		if err := diagnosticLogger.Record(
			diagnostics.LevelInfo,
			diagnostics.EventRuntimeStopped,
			"stopped",
		); err != nil {
			closeErrors = append(closeErrors, err)
		}
		closeErrors = append(closeErrors, diagnosticLogger.Close())
	}
	return errors.Join(closeErrors...)
}

func (r *ImportRuntime) storePreparedExportLocked(report exporter.PreflightReport) {
	for len(r.preparedExports) >= maxPreparedExports {
		oldest := r.preparedOrder[0]
		r.preparedOrder = r.preparedOrder[1:]
		delete(r.preparedExports, oldest)
	}
	r.preparedExports[report.PreparationID] = report
	r.preparedOrder = append(r.preparedOrder, report.PreparationID)
}

func (r *ImportRuntime) takePreparedExportLocked(
	preparationID string,
) (exporter.PreflightReport, bool) {
	report, found := r.preparedExports[preparationID]
	if !found {
		return exporter.PreflightReport{}, false
	}
	delete(r.preparedExports, preparationID)
	for index, candidate := range r.preparedOrder {
		if candidate != preparationID {
			continue
		}
		copy(r.preparedOrder[index:], r.preparedOrder[index+1:])
		r.preparedOrder = r.preparedOrder[:len(r.preparedOrder)-1]
		break
	}
	return report, true
}

func (r *ImportRuntime) current() (*operations.Manager, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.manager == nil {
		return nil, ErrImportRuntimeNotReady
	}
	return r.manager, nil
}

func (r *ImportRuntime) currentQuery() (*library.Query, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.query == nil {
		return nil, ErrImportRuntimeNotReady
	}
	return r.query, nil
}

func (r *ImportRuntime) currentRemoval() (*removal.Service, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.removal == nil {
		return nil, ErrImportRuntimeNotReady
	}
	return r.removal, nil
}

func (r *ImportRuntime) currentTags() (*tagging.Service, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.tags == nil {
		return nil, ErrImportRuntimeNotReady
	}
	return r.tags, nil
}

func (r *ImportRuntime) currentShelves() (
	*shelves.Service,
	*shelves.Evaluator,
	error,
) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.shelves == nil || r.shelfEvaluator == nil {
		return nil, nil, ErrImportRuntimeNotReady
	}
	return r.shelves, r.shelfEvaluator, nil
}

var _ OperationPort = (*ImportRuntime)(nil)
var _ LibraryPort = (*ImportRuntime)(nil)
var _ RemovalPort = (*ImportRuntime)(nil)
var _ TaggingPort = (*ImportRuntime)(nil)
var _ ShelfPort = (*ImportRuntime)(nil)
