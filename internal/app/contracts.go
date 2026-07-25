package app

import (
	"context"
	"errors"
	"time"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/tagging"
)

type LifecycleState string

const (
	StateInitializing LifecycleState = "initializing"
	StateReady        LifecycleState = "ready"
	StateRecovery     LifecycleState = "recovery"
	StateStopping     LifecycleState = "stopping"
	StateStopped      LifecycleState = "stopped"
)

type ErrorCode string

const (
	ErrorCancelled          ErrorCode = "cancelled"
	ErrorConflict           ErrorCode = "conflict"
	ErrorInternal           ErrorCode = "internal"
	ErrorInvalidInput       ErrorCode = "invalid_input"
	ErrorNotReady           ErrorCode = "not_ready"
	ErrorStorageUnavailable ErrorCode = "storage_unavailable"
)

type APIError struct {
	Code    ErrorCode         `json:"code"`
	Message string            `json:"message"`
	Details map[string]string `json:"details,omitempty"`
}

func (e *APIError) Error() string {
	return e.Message
}

func NewAPIError(code ErrorCode, message string) *APIError {
	return &APIError{Code: code, Message: message}
}

func AsAPIError(err error) *APIError {
	if err == nil {
		return nil
	}

	var apiErr *APIError
	if errors.As(err, &apiErr) {
		return apiErr
	}

	return NewAPIError(ErrorInternal, "An unexpected application error occurred.")
}

type Status struct {
	State            LifecycleState `json:"state"`
	StartedAt        string         `json:"startedAt,omitempty"`
	MutationsAllowed bool           `json:"mutationsAllowed"`
}

type StatusResponse struct {
	Status Status    `json:"status"`
	Error  *APIError `json:"error,omitempty"`
}

type Clock interface {
	Now() time.Time
}

type FileDialogRequest struct {
	Title      string
	Extensions []string
	Multiple   bool
}

type DialogPort interface {
	OpenFiles(context.Context, FileDialogRequest) ([]string, error)
	OpenDirectory(context.Context, string) (string, error)
}

type EventPort interface {
	Emit(context.Context, string, any)
}

type ReadinessIssue struct {
	Code string `json:"code"`
}

type ReadinessReport struct {
	MutationsAllowed bool
	Issues           []ReadinessIssue
}

type ReadinessPort interface {
	Check(context.Context) (ReadinessReport, error)
}

type ResourcePort interface {
	Close() error
}

type OperationPort interface {
	Start(context.Context) error
	StartImport(context.Context, []string) (operations.Snapshot, error)
	StartMetadataRefresh(context.Context, string) (operations.Snapshot, error)
	MetadataStatus(context.Context, string) (metadata.CatalogStatus, error)
	Cancel(context.Context, string) (operations.Snapshot, error)
	Snapshot(context.Context, string) (operations.Snapshot, error)
	Active(context.Context) ([]operations.Snapshot, error)
	Close() error
}

type LibraryPort interface {
	List(context.Context, library.ListRequest) (library.Page, error)
	Search(context.Context, library.StoryLibraryQuery) (library.Page, error)
	Detail(context.Context, int64) (library.StoryDetail, error)
}

type RemovalPort interface {
	Remove(context.Context, int64) (removal.Result, error)
}

type TaggingPort interface {
	Catalog(context.Context) (tagging.Catalog, error)
	CreateDefinition(context.Context, tagging.CreateDefinition) (tagging.Definition, error)
	RenameDefinition(context.Context, int64, string) (tagging.Definition, error)
	RecolorDefinition(context.Context, int64, string) (tagging.Definition, error)
	ReorderDefinitions(context.Context, []int64) ([]tagging.Definition, error)
	PlanDefinitionDeletion(context.Context, int64) (tagging.DefinitionDeletionPlan, error)
	DeleteDefinition(context.Context, tagging.DefinitionDeletionPlan) error
	CreateValue(context.Context, tagging.CreateValue) (tagging.Value, error)
	RenameValue(context.Context, int64, string) (tagging.Value, error)
	ReorderValues(context.Context, int64, []int64) ([]tagging.Value, error)
	PlanValueDeletion(context.Context, int64) (tagging.ValueDeletionPlan, error)
	DeleteValue(context.Context, tagging.ValueDeletionPlan) error
	AssignmentWorkspace(context.Context, []int64) (tagging.AssignmentWorkspace, error)
	SetBulkBoolean(context.Context, []int64, int64, bool) (tagging.AssignmentResult, error)
	SetBulkChoiceValues(context.Context, []int64, int64, []int64) (tagging.AssignmentResult, error)
	SetBulkChoiceValue(context.Context, []int64, int64, int64, bool) (tagging.AssignmentResult, error)
}

type ShelfPort interface {
	ListShelves(context.Context) ([]shelves.Summary, error)
	CreateShelf(context.Context, string, library.StoryLibraryQuery) (shelves.Shelf, error)
	OpenShelf(context.Context, int64, library.ListRequest) (shelves.Evaluation, error)
	RenameShelf(context.Context, int64, string) (shelves.Shelf, error)
	DuplicateShelf(context.Context, int64, string) (shelves.Shelf, error)
	ReplaceShelfQuery(
		context.Context,
		int64,
		library.StoryLibraryQuery,
	) (shelves.Shelf, error)
	ReorderShelves(context.Context, []int64) ([]shelves.Shelf, error)
	DeleteShelf(context.Context, int64) error
	PreviewShelves(context.Context, []int64) (shelves.SelectionPreview, error)
}

type Dependencies struct {
	Clock      Clock
	Dialogs    DialogPort
	Events     EventPort
	Readiness  ReadinessPort
	Operations OperationPort
	Library    LibraryPort
	Removal    RemovalPort
	Tags       TaggingPort
	Shelves    ShelfPort
	Resources  []ResourcePort
}

type OperationResponse struct {
	Operation *operations.Snapshot `json:"operation,omitempty"`
	Cancelled bool                 `json:"cancelled,omitempty"`
	Error     *APIError            `json:"error,omitempty"`
}

type OperationListResponse struct {
	Operations []operations.Snapshot `json:"operations"`
	Error      *APIError             `json:"error,omitempty"`
}

type MetadataStatusResponse struct {
	Status metadata.CatalogStatus `json:"status"`
	Error  *APIError              `json:"error,omitempty"`
}

type LibraryPageResponse struct {
	Page  *library.Page `json:"page,omitempty"`
	Error *APIError     `json:"error,omitempty"`
}

type StoryDetailResponse struct {
	Detail *library.StoryDetail `json:"detail,omitempty"`
	Error  *APIError            `json:"error,omitempty"`
}

type RemovalResponse struct {
	Result *removal.Result `json:"result,omitempty"`
	Error  *APIError       `json:"error,omitempty"`
}

type TagCatalogResponse struct {
	Catalog *tagging.Catalog `json:"catalog,omitempty"`
	Error   *APIError        `json:"error,omitempty"`
}

type TagDefinitionResponse struct {
	Definition *tagging.Definition `json:"definition,omitempty"`
	Error      *APIError           `json:"error,omitempty"`
}

type TagValueResponse struct {
	Value *tagging.Value `json:"value,omitempty"`
	Error *APIError      `json:"error,omitempty"`
}

type TagDefinitionDeletionPlanResponse struct {
	Plan  *tagging.DefinitionDeletionPlan `json:"plan,omitempty"`
	Error *APIError                       `json:"error,omitempty"`
}

type TagValueDeletionPlanResponse struct {
	Plan  *tagging.ValueDeletionPlan `json:"plan,omitempty"`
	Error *APIError                  `json:"error,omitempty"`
}

type MutationResponse struct {
	Success bool      `json:"success"`
	Error   *APIError `json:"error,omitempty"`
}

type TagAssignmentWorkspaceResponse struct {
	Workspace *tagging.AssignmentWorkspace `json:"workspace,omitempty"`
	Error     *APIError                    `json:"error,omitempty"`
}

type TagAssignmentResponse struct {
	Result *tagging.AssignmentResult `json:"result,omitempty"`
	Error  *APIError                 `json:"error,omitempty"`
}

type ShelfListResponse struct {
	Shelves []shelves.Summary `json:"shelves"`
	Error   *APIError         `json:"error,omitempty"`
}

type ShelfResponse struct {
	Shelf *shelves.Shelf `json:"shelf,omitempty"`
	Error *APIError      `json:"error,omitempty"`
}

type ShelfEvaluationResponse struct {
	Evaluation *shelves.Evaluation `json:"evaluation,omitempty"`
	Error      *APIError           `json:"error,omitempty"`
}

type ShelfSelectionPreviewResponse struct {
	Preview *shelves.SelectionPreview `json:"preview,omitempty"`
	Error   *APIError                 `json:"error,omitempty"`
}
