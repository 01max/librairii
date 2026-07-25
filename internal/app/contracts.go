package app

import (
	"context"
	"errors"
	"time"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
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

type Dependencies struct {
	Clock      Clock
	Dialogs    DialogPort
	Events     EventPort
	Readiness  ReadinessPort
	Operations OperationPort
	Library    LibraryPort
	Removal    RemovalPort
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
