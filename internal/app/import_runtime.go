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
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/storage"
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
	if err := manager.Start(ctx); err != nil {
		return err
	}
	r.manager = manager
	r.query = library.NewQuery(database, nil)
	r.removal = removalService
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

func (r *ImportRuntime) Close() error {
	r.mu.Lock()
	manager := r.manager
	r.manager = nil
	r.query = nil
	r.removal = nil
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

var _ OperationPort = (*ImportRuntime)(nil)
var _ LibraryPort = (*ImportRuntime)(nil)
var _ RemovalPort = (*ImportRuntime)(nil)
