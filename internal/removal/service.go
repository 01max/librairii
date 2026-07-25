package removal

import (
	"context"
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/catalog"
)

var ErrInvalidStoryID = errors.New("story id must be positive")

type ErrorCode string

const (
	ErrorLookup  ErrorCode = "removal_lookup_failed"
	ErrorTrash   ErrorCode = "removal_trash_failed"
	ErrorDelete  ErrorCode = "removal_delete_failed"
	ErrorRestore ErrorCode = "removal_restore_failed"
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
	MoveToTrash(string) (string, error)
	RestoreFromTrash(string, string) error
}

type Service struct {
	catalog  storyCatalog
	archives archiveStore
}

func NewService(catalog storyCatalog, archives archiveStore) (*Service, error) {
	if catalog == nil || archives == nil {
		return nil, errors.New("removal service dependency is nil")
	}
	return &Service{catalog: catalog, archives: archives}, nil
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
	if err := ctx.Err(); err != nil {
		return Result{}, err
	}

	trashPath, err := s.archives.MoveToTrash(storyArchive.ManagedPath)
	if err != nil {
		return Result{}, &Error{Code: ErrorTrash, Cause: err}
	}
	if err := ctx.Err(); err != nil {
		return Result{}, s.restore(trashPath, storyArchive.ManagedPath, err)
	}
	if err := s.catalog.Delete(ctx, storyID); err != nil {
		return Result{}, s.restore(
			trashPath,
			storyArchive.ManagedPath,
			&Error{Code: ErrorDelete, Cause: err},
		)
	}
	return Result{StoryID: story.ID, UUID: story.UUID}, nil
}

func (s *Service) restore(
	trashPath string,
	managedPath string,
	cause error,
) error {
	if err := s.archives.RestoreFromTrash(trashPath, managedPath); err != nil {
		return errors.Join(cause, &Error{Code: ErrorRestore, Cause: err})
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
