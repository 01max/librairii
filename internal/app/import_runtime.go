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
	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/storage"
	"github.com/01max/librairii/internal/tagging"
)

var ErrImportRuntimeNotReady = errors.New("import runtime storage is not ready")

type StorageProvider interface {
	Layout() storage.Layout
	SQL() *sql.DB
}

type ImportRuntime struct {
	mu      sync.RWMutex
	storage StorageProvider
	clock   Clock
	events  EventPort
	workers int
	manager *operations.Manager
	query   *library.Query
	removal *removal.Service
	tags    *tagging.Service
}

func NewImportRuntime(
	storage StorageProvider,
	clock Clock,
	events EventPort,
	workers int,
) (*ImportRuntime, error) {
	if storage == nil || clock == nil || events == nil || workers < 1 {
		return nil, ErrMissingDependency
	}
	return &ImportRuntime{
		storage: storage,
		clock:   clock,
		events:  events,
		workers: workers,
	}, nil
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
	if _, err := tagging.SeedBuiltIns(ctx, database); err != nil {
		return fmt.Errorf("seed built-in tags: %w", err)
	}
	if err := metadata.SeedDefaultDerivedFacets(ctx, database); err != nil {
		return fmt.Errorf("seed derived tags: %w", err)
	}
	if err := library.BackfillNormalizedDisplayNames(ctx, database); err != nil {
		return fmt.Errorf("backfill story display names: %w", err)
	}

	archiveRepository := archive.NewRepository(layout)
	catalogRepository := catalog.NewRepository(database)
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
	manager, err := operations.NewManager(operations.Dependencies{
		Repository:     operations.NewRepository(database),
		Imports:        importService,
		Events:         r.events,
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
		metadata.NewRepository(database),
		metadata.DefaultLocale,
	)
	if err != nil {
		_ = manager.Close()
		return fmt.Errorf("construct official metadata provider: %w", err)
	}
	r.manager = manager
	r.query = library.NewQuery(database, officialProvider)
	r.removal = removalService
	r.tags = tagService
	return nil
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

func (r *ImportRuntime) Close() error {
	r.mu.Lock()
	manager := r.manager
	r.manager = nil
	r.query = nil
	r.removal = nil
	r.tags = nil
	r.mu.Unlock()
	if manager == nil {
		return nil
	}
	return manager.Close()
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

var _ OperationPort = (*ImportRuntime)(nil)
var _ LibraryPort = (*ImportRuntime)(nil)
var _ RemovalPort = (*ImportRuntime)(nil)
var _ TaggingPort = (*ImportRuntime)(nil)
