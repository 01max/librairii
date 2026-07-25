package app

import (
	"context"
	"database/sql"
	"errors"
	"sync"
	"time"

	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
)

const EventApplicationReady = "application:ready"

var ErrMissingDependency = errors.New("application dependency is required")

type Application struct {
	mu         sync.RWMutex
	clock      Clock
	dialogs    DialogPort
	events     EventPort
	readiness  ReadinessPort
	operations OperationPort
	library    LibraryPort
	removal    RemovalPort
	tags       TaggingPort
	shelves    ShelfPort
	resources  []ResourcePort
	state      LifecycleState
	startedAt  time.Time
	ready      bool
	lastError  *APIError
}

func New(deps Dependencies) (*Application, error) {
	if deps.Clock == nil ||
		deps.Dialogs == nil ||
		deps.Events == nil ||
		deps.Readiness == nil ||
		deps.Operations == nil ||
		deps.Library == nil ||
		deps.Removal == nil ||
		deps.Tags == nil ||
		deps.Shelves == nil {
		return nil, ErrMissingDependency
	}

	resources := append([]ResourcePort(nil), deps.Resources...)
	resources = append(resources, deps.Operations)
	return &Application{
		clock:      deps.Clock,
		dialogs:    deps.Dialogs,
		events:     deps.Events,
		readiness:  deps.Readiness,
		operations: deps.Operations,
		library:    deps.Library,
		removal:    deps.Removal,
		tags:       deps.Tags,
		shelves:    deps.Shelves,
		resources:  resources,
		state:      StateInitializing,
	}, nil
}

func (a *Application) RemoveStory(
	ctx context.Context,
	storyID int64,
) RemovalResponse {
	if !a.Status().MutationsAllowed {
		return RemovalResponse{
			Error: NewAPIError(ErrorNotReady, "Removal is unavailable until storage is ready."),
		}
	}
	result, err := a.removal.Remove(ctx, storyID)
	if err != nil {
		return RemovalResponse{Error: removalAPIError(err)}
	}
	return RemovalResponse{Result: &result}
}

func (a *Application) ListStories(
	ctx context.Context,
	request library.ListRequest,
) LibraryPageResponse {
	page, err := a.library.List(ctx, request)
	if err != nil {
		return LibraryPageResponse{Error: libraryAPIError(err)}
	}
	return LibraryPageResponse{Page: &page}
}

func (a *Application) QueryStories(
	ctx context.Context,
	request library.StoryLibraryQuery,
) LibraryPageResponse {
	page, err := a.library.Search(ctx, request)
	if err != nil {
		return LibraryPageResponse{Error: libraryAPIError(err)}
	}
	return LibraryPageResponse{Page: &page}
}

func (a *Application) StoryDetail(
	ctx context.Context,
	storyID int64,
) StoryDetailResponse {
	detail, err := a.library.Detail(ctx, storyID)
	if err != nil {
		return StoryDetailResponse{Error: libraryAPIError(err)}
	}
	return StoryDetailResponse{Detail: &detail}
}

func (a *Application) Start(ctx context.Context) error {
	report, err := a.readiness.Check(ctx)
	if err == nil && report.MutationsAllowed {
		err = a.operations.Start(ctx)
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	if a.state == StateReady || a.state == StateRecovery {
		return nil
	}

	a.startedAt = a.clock.Now().UTC()
	if err != nil {
		a.state = StateRecovery
		a.ready = false
		a.lastError = NewAPIError(ErrorStorageUnavailable, "Application storage is unavailable.")
		return nil
	}
	if !report.MutationsAllowed {
		a.state = StateRecovery
		a.ready = false
		a.lastError = NewAPIError(ErrorStorageUnavailable, "Application storage requires recovery.")
		return nil
	}

	a.state = StateReady
	a.ready = true
	a.lastError = nil
	return nil
}

func (a *Application) SelectAndStartImport(ctx context.Context) OperationResponse {
	if !a.Status().MutationsAllowed {
		return OperationResponse{
			Error: NewAPIError(ErrorNotReady, "Imports are unavailable until storage is ready."),
		}
	}
	paths, err := a.dialogs.OpenFiles(ctx, FileDialogRequest{
		Title: "Import story archives",
		Extensions: []string{
			".plain.pk",
			".v1.pk",
			".v2.pk",
			".pk",
			".zip",
			".7z",
		},
		Multiple: true,
	})
	if err != nil {
		return OperationResponse{
			Error: NewAPIError(ErrorInternal, "The story archive picker could not be opened."),
		}
	}
	if len(paths) == 0 {
		return OperationResponse{Cancelled: true}
	}
	snapshot, err := a.operations.StartImport(ctx, paths)
	if err != nil {
		return OperationResponse{Error: operationAPIError(err)}
	}
	return OperationResponse{Operation: &snapshot}
}

func (a *Application) SelectAndPreflightExport(
	ctx context.Context,
	request exporter.PreflightRequest,
) ExportPreflightResponse {
	if !a.Status().MutationsAllowed {
		return ExportPreflightResponse{
			Error: NewAPIError(
				ErrorNotReady,
				"Exports are unavailable until storage is ready.",
			),
		}
	}
	destination, err := a.dialogs.OpenDirectory(ctx, "Export story archives")
	if err != nil {
		return ExportPreflightResponse{
			Error: NewAPIError(
				ErrorInternal,
				"The export destination picker could not be opened.",
			),
		}
	}
	if destination == "" {
		return ExportPreflightResponse{Cancelled: true}
	}
	preflight, err := a.operations.PrepareExport(ctx, request, destination)
	if err != nil {
		if errors.Is(err, exporter.ErrInvalidScope) {
			return ExportPreflightResponse{
				Error: NewAPIError(ErrorInvalidInput, "The export scope is invalid."),
			}
		}
		return ExportPreflightResponse{
			Error: NewAPIError(ErrorInternal, "The export could not be prepared."),
		}
	}
	return ExportPreflightResponse{Preflight: &preflight}
}

func (a *Application) StartPreparedExport(
	ctx context.Context,
	preparationID string,
) OperationResponse {
	if !a.Status().MutationsAllowed {
		return OperationResponse{
			Error: NewAPIError(
				ErrorNotReady,
				"Exports are unavailable until storage is ready.",
			),
		}
	}
	snapshot, err := a.operations.StartPreparedExport(ctx, preparationID)
	if err != nil {
		return OperationResponse{Error: operationAPIError(err)}
	}
	return OperationResponse{Operation: &snapshot}
}

func (a *Application) RevealExportDestination(
	ctx context.Context,
	operationID string,
) MutationResponse {
	snapshot, err := a.operations.Snapshot(ctx, operationID)
	if err != nil {
		return MutationResponse{Error: operationAPIError(err)}
	}
	if snapshot.Kind != operations.KindExport ||
		!snapshot.Terminal() ||
		snapshot.Destination == "" {
		return MutationResponse{
			Error: NewAPIError(
				ErrorInvalidInput,
				"The export destination is not available yet.",
			),
		}
	}
	if err := a.dialogs.RevealDirectory(ctx, snapshot.Destination); err != nil {
		return MutationResponse{
			Error: NewAPIError(
				ErrorInternal,
				"The export destination could not be revealed.",
			),
		}
	}
	return MutationResponse{Success: true}
}

func (a *Application) RefreshOfficialMetadata(
	ctx context.Context,
) OperationResponse {
	if !a.Status().MutationsAllowed {
		return OperationResponse{
			Error: NewAPIError(
				ErrorNotReady,
				"Official metadata refresh is unavailable until storage is ready.",
			),
		}
	}
	snapshot, err := a.operations.StartMetadataRefresh(ctx, metadata.DefaultLocale)
	if err != nil {
		return OperationResponse{Error: operationAPIError(err)}
	}
	return OperationResponse{Operation: &snapshot}
}

func (a *Application) OfficialMetadataStatus(
	ctx context.Context,
) MetadataStatusResponse {
	status, err := a.operations.MetadataStatus(ctx, metadata.DefaultLocale)
	if err != nil {
		return MetadataStatusResponse{
			Error: NewAPIError(ErrorInternal, "Official metadata status could not be loaded."),
		}
	}
	return MetadataStatusResponse{Status: status}
}

func (a *Application) CancelOperation(
	ctx context.Context,
	operationID string,
) OperationResponse {
	if !a.Status().MutationsAllowed {
		return OperationResponse{
			Error: NewAPIError(ErrorNotReady, "Operations are unavailable until storage is ready."),
		}
	}
	snapshot, err := a.operations.Cancel(ctx, operationID)
	if err != nil {
		return OperationResponse{Error: operationAPIError(err)}
	}
	return OperationResponse{Operation: &snapshot}
}

func (a *Application) OperationSnapshot(
	ctx context.Context,
	operationID string,
) OperationResponse {
	snapshot, err := a.operations.Snapshot(ctx, operationID)
	if err != nil {
		return OperationResponse{Error: operationAPIError(err)}
	}
	return OperationResponse{Operation: &snapshot}
}

func (a *Application) ActiveOperations(ctx context.Context) OperationListResponse {
	snapshots, err := a.operations.Active(ctx)
	if err != nil {
		return OperationListResponse{Error: operationAPIError(err)}
	}
	return OperationListResponse{Operations: snapshots}
}

func (a *Application) AnnounceReady(ctx context.Context) {
	status := a.Status()
	if status.State == StateReady {
		a.events.Emit(ctx, EventApplicationReady, status)
	}
}

func (a *Application) Stop(_ context.Context) {
	a.mu.Lock()
	a.state = StateStopped
	a.ready = false
	resources := append([]ResourcePort(nil), a.resources...)
	a.mu.Unlock()

	for index := len(resources) - 1; index >= 0; index-- {
		_ = resources[index].Close()
	}
}

func (a *Application) Status() Status {
	a.mu.RLock()
	defer a.mu.RUnlock()

	return a.statusLocked()
}

func (a *Application) StatusResponse() StatusResponse {
	a.mu.RLock()
	defer a.mu.RUnlock()

	return StatusResponse{Status: a.statusLocked(), Error: a.lastError}
}

func (a *Application) statusLocked() Status {
	status := Status{
		State:            a.state,
		MutationsAllowed: a.ready,
	}
	if !a.startedAt.IsZero() {
		status.StartedAt = a.startedAt.Format(time.RFC3339Nano)
	}
	return status
}

func operationAPIError(err error) *APIError {
	switch {
	case errors.Is(err, context.Canceled):
		return NewAPIError(ErrorCancelled, "The operation was cancelled.")
	case errors.Is(err, operations.ErrInvalidRequest):
		return NewAPIError(ErrorInvalidInput, "The operation request is invalid.")
	case errors.Is(err, operations.ErrOperationNotActive):
		return NewAPIError(ErrorConflict, "The operation is no longer active.")
	case errors.Is(err, operations.ErrOperationActive):
		return NewAPIError(ErrorConflict, "Official metadata is already being refreshed.")
	case errors.Is(err, sql.ErrNoRows):
		return NewAPIError(ErrorInvalidInput, "The operation does not exist.")
	default:
		return NewAPIError(ErrorInternal, "The operation could not be completed.")
	}
}

func libraryAPIError(err error) *APIError {
	switch {
	case errors.Is(err, library.ErrInvalidListRequest),
		errors.Is(err, library.ErrInvalidStoryLibraryQuery):
		return NewAPIError(ErrorInvalidInput, "The library request is invalid.")
	case errors.Is(err, sql.ErrNoRows):
		return NewAPIError(ErrorInvalidInput, "The story does not exist.")
	default:
		return NewAPIError(ErrorInternal, "The library could not be loaded.")
	}
}

func removalAPIError(err error) *APIError {
	switch {
	case errors.Is(err, context.Canceled):
		return NewAPIError(ErrorCancelled, "Story removal was cancelled.")
	case errors.Is(err, removal.ErrInvalidStoryID):
		return NewAPIError(ErrorInvalidInput, "The story removal request is invalid.")
	case errors.Is(err, sql.ErrNoRows):
		return NewAPIError(ErrorInvalidInput, "The story does not exist.")
	default:
		return NewAPIError(ErrorInternal, "The story could not be moved to trash.")
	}
}
