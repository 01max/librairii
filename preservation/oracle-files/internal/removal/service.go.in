package removal

import (
	"context"
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/catalog"
	"github.com/google/uuid"
)

var ErrInvalidStoryID = errors.New("story id must be positive")

type ErrorCode string

const (
	ErrorLookup    ErrorCode = "removal_lookup_failed"
	ErrorIntent    ErrorCode = "removal_intent_failed"
	ErrorTrash     ErrorCode = "removal_trash_failed"
	ErrorDelete    ErrorCode = "removal_delete_failed"
	ErrorRestore   ErrorCode = "removal_restore_failed"
	ErrorReconcile ErrorCode = "removal_reconcile_failed"
)

type Error struct {
	Code  ErrorCode
	Cause error
}

func (e *Error) Error() string {
	return fmt.Sprintf("%s: %v", e.Code, e.Cause)
}

func (e *Error) Unwrap() error {
	return e.Cause
}

type Result struct {
	StoryID int64  `json:"storyId"`
	UUID    string `json:"uuid"`
}

type storyCatalog interface {
	FindByID(context.Context, int64) (catalog.Story, catalog.StoryArchive, error)
	Delete(context.Context, int64) error
}

type archiveStore interface {
	PlanRemovalTrash(string, string) (string, error)
	MoveToTrashAt(string, string) error
	RestoreFromTrash(string, string) error
	Exists(string) (bool, error)
}

type intentStore interface {
	Create(context.Context, Intent) error
	Delete(context.Context, string) error
	List(context.Context) ([]Intent, error)
}

type Service struct {
	catalog  storyCatalog
	archives archiveStore
	intents  intentStore
}

func NewService(
	catalog storyCatalog,
	archives archiveStore,
	intents intentStore,
) (*Service, error) {
	if catalog == nil || archives == nil || intents == nil {
		return nil, errors.New("removal service dependency is nil")
	}
	return &Service{catalog: catalog, archives: archives, intents: intents}, nil
}

func (s *Service) Remove(ctx context.Context, storyID int64) (Result, error) {
	if storyID <= 0 {
		return Result{}, ErrInvalidStoryID
	}
	if err := ctx.Err(); err != nil {
		return Result{}, err
	}
	story, storyArchive, err := s.catalog.FindByID(ctx, storyID)
	if err != nil {
		return Result{}, &Error{Code: ErrorLookup, Cause: err}
	}
	intent := Intent{
		ID:          uuid.NewString(),
		StoryID:     story.ID,
		ManagedPath: storyArchive.ManagedPath,
	}
	intent.TrashPath, err = s.archives.PlanRemovalTrash(intent.ID, intent.ManagedPath)
	if err != nil {
		return Result{}, &Error{Code: ErrorTrash, Cause: err}
	}
	if err := s.intents.Create(ctx, intent); err != nil {
		return Result{}, &Error{Code: ErrorIntent, Cause: err}
	}
	if err := ctx.Err(); err != nil {
		return Result{}, s.clearIntent(context.WithoutCancel(ctx), intent.ID, err)
	}
	if err := s.archives.MoveToTrashAt(intent.ManagedPath, intent.TrashPath); err != nil {
		return Result{}, s.clearIntent(
			context.WithoutCancel(ctx),
			intent.ID,
			&Error{Code: ErrorTrash, Cause: err},
		)
	}
	if err := ctx.Err(); err != nil {
		return Result{}, s.restoreAndClear(
			context.WithoutCancel(ctx),
			intent,
			err,
		)
	}
	if err := s.catalog.Delete(ctx, storyID); err != nil {
		return Result{}, s.restoreAndClear(
			context.WithoutCancel(ctx),
			intent,
			&Error{Code: ErrorDelete, Cause: err},
		)
	}
	return Result{StoryID: story.ID, UUID: story.UUID}, nil
}

// Reconcile rolls back removals interrupted after their intent was persisted
// but before the story deletion transaction completed.
func (s *Service) Reconcile(ctx context.Context) error {
	intents, err := s.intents.List(ctx)
	if err != nil {
		return &Error{Code: ErrorReconcile, Cause: err}
	}
	for _, intent := range intents {
		managedExists, err := s.archives.Exists(intent.ManagedPath)
		if err != nil {
			return &Error{Code: ErrorReconcile, Cause: err}
		}
		trashExists, err := s.archives.Exists(intent.TrashPath)
		if err != nil {
			return &Error{Code: ErrorReconcile, Cause: err}
		}
		switch {
		case managedExists && !trashExists:
			if err := s.intents.Delete(ctx, intent.ID); err != nil {
				return &Error{Code: ErrorReconcile, Cause: err}
			}
		case !managedExists && trashExists:
			if err := s.archives.RestoreFromTrash(
				intent.TrashPath,
				intent.ManagedPath,
			); err != nil {
				return &Error{Code: ErrorReconcile, Cause: err}
			}
			if err := s.intents.Delete(ctx, intent.ID); err != nil {
				return &Error{Code: ErrorReconcile, Cause: err}
			}
		default:
			return &Error{
				Code: ErrorReconcile,
				Cause: fmt.Errorf(
					"removal intent %s has ambiguous archive custody",
					intent.ID,
				),
			}
		}
	}
	return nil
}

func (s *Service) restoreAndClear(
	ctx context.Context,
	intent Intent,
	cause error,
) error {
	if err := s.archives.RestoreFromTrash(
		intent.TrashPath,
		intent.ManagedPath,
	); err != nil {
		return errors.Join(cause, &Error{Code: ErrorRestore, Cause: err})
	}
	return s.clearIntent(ctx, intent.ID, cause)
}

func (s *Service) clearIntent(
	ctx context.Context,
	intentID string,
	cause error,
) error {
	if err := s.intents.Delete(ctx, intentID); err != nil {
		return errors.Join(cause, &Error{Code: ErrorIntent, Cause: err})
	}
	return cause
}

func ErrorHasCode(err error, code ErrorCode) bool {
	var removalError *Error
	if errors.As(err, &removalError) && removalError.Code == code {
		return true
	}
	if joined, ok := err.(interface{ Unwrap() []error }); ok {
		for _, child := range joined.Unwrap() {
			if ErrorHasCode(child, code) {
				return true
			}
		}
	}
	return false
}
