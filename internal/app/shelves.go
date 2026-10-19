package app

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelves"
)

type ShelfPort interface {
	ListShelves(context.Context) ([]shelves.Shelf, error)
	CreateShelf(context.Context, string, []string) (shelves.Shelf, error)
	OpenShelf(context.Context, int64, library.StoryLibraryQuery) (shelves.Evaluation, error)
	RenameShelf(context.Context, int64, string) (shelves.Shelf, error)
	DuplicateShelf(context.Context, int64, string) (shelves.Shelf, error)
	DeleteShelf(context.Context, int64) error
	ReorderShelves(context.Context, []int64) error
	AddStoriesToShelf(context.Context, int64, []string) (shelves.Shelf, error)
	RemoveStoriesFromShelf(context.Context, int64, []string) (shelves.Shelf, error)
	SetShelfStoryProtected(context.Context, int64, string, bool) (shelves.Shelf, error)
	ReorderShelfStories(context.Context, int64, []string) (shelves.Shelf, error)
	ListShelfExclusions(context.Context, int64) ([]shelves.Exclusion, error)
	ClearShelfExclusion(context.Context, int64, string) (shelves.Shelf, error)
	AttachShelfSources(context.Context, int64, []int64) (shelves.Shelf, error)
	PreviewShelfUpdate(context.Context, int64) (shelves.Review, error)
	ApplyShelfUpdate(context.Context, int64, string, []shelves.Decision) (shelves.Shelf, error)
	PreviewShelfCreation(context.Context, shelves.CreationRequest) (shelves.Review, error)
	CreateShelfFromReview(context.Context, string, []string) (shelves.Shelf, error)
}
type ShelfListResponse struct {
	Shelves []shelves.Shelf `json:"shelves"`
	Error   *APIError       `json:"error,omitempty"`
}
type ShelfResponse struct {
	Shelf *shelves.Shelf `json:"shelf,omitempty"`
	Error *APIError      `json:"error,omitempty"`
}
type ShelfEvaluationResponse struct {
	Evaluation *shelves.Evaluation `json:"evaluation,omitempty"`
	Error      *APIError           `json:"error,omitempty"`
}
type ShelfExclusionsResponse struct {
	Exclusions []shelves.Exclusion `json:"exclusions"`
	Error      *APIError           `json:"error,omitempty"`
}

func shelfAPIError(err error) *APIError {
	switch {
	case errors.Is(err, ErrImportRuntimeNotReady):
		return NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")
	case errors.Is(err, shelves.ErrInvalidInput), errors.Is(err, library.ErrInvalidStoryLibraryQuery):
		return NewAPIError(ErrorInvalidInput, "The shelf request is invalid.")
	case errors.Is(err, shelves.ErrNotFound), errors.Is(err, shelves.ErrConflict):
		return NewAPIError(ErrorConflict, "The shelf changed. Refresh and try again.")
	default:
		return NewAPIError(ErrorInternal, "The shelf action could not be completed.")
	}
}
func (a *Application) ListShelves(ctx context.Context) ShelfListResponse {
	if a.shelves == nil {
		return ShelfListResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.ListShelves(ctx)
	if err != nil {
		return ShelfListResponse{Error: shelfAPIError(err)}
	}
	return ShelfListResponse{Shelves: result}
}

func (a *Application) CreateShelf(ctx context.Context, name string, storyUUIDs []string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.CreateShelf(ctx, name, storyUUIDs)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) OpenShelf(ctx context.Context, shelfID int64, query library.StoryLibraryQuery) ShelfEvaluationResponse {
	if a.shelves == nil {
		return ShelfEvaluationResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.OpenShelf(ctx, shelfID, query)
	if err != nil {
		return ShelfEvaluationResponse{Error: shelfAPIError(err)}
	}
	return ShelfEvaluationResponse{Evaluation: &result}
}

func (a *Application) RenameShelf(ctx context.Context, shelfID int64, name string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.RenameShelf(ctx, shelfID, name)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) DuplicateShelf(ctx context.Context, shelfID int64, name string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.DuplicateShelf(ctx, shelfID, name)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) DeleteShelf(ctx context.Context, shelfID int64) MutationResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return MutationResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	if err := a.shelves.DeleteShelf(ctx, shelfID); err != nil {
		return MutationResponse{Error: shelfAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) ReorderShelves(ctx context.Context, orderedIDs []int64) MutationResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return MutationResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	if err := a.shelves.ReorderShelves(ctx, orderedIDs); err != nil {
		return MutationResponse{Error: shelfAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) AddStoriesToShelf(ctx context.Context, shelfID int64, storyUUIDs []string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.AddStoriesToShelf(ctx, shelfID, storyUUIDs)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) RemoveStoriesFromShelf(ctx context.Context, shelfID int64, storyUUIDs []string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.RemoveStoriesFromShelf(ctx, shelfID, storyUUIDs)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) SetShelfStoryProtected(ctx context.Context, shelfID int64, storyUUID string, protected bool) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.SetShelfStoryProtected(ctx, shelfID, storyUUID, protected)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) ReorderShelfStories(ctx context.Context, shelfID int64, orderedUUIDs []string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.ReorderShelfStories(ctx, shelfID, orderedUUIDs)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) ListShelfExclusions(ctx context.Context, shelfID int64) ShelfExclusionsResponse {
	if a.shelves == nil {
		return ShelfExclusionsResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.ListShelfExclusions(ctx, shelfID)
	if err != nil {
		return ShelfExclusionsResponse{Error: shelfAPIError(err)}
	}
	return ShelfExclusionsResponse{Exclusions: result}
}

func (a *Application) ClearShelfExclusion(ctx context.Context, shelfID int64, storyUUID string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.ClearShelfExclusion(ctx, shelfID, storyUUID)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

type ShelfReviewResponse struct {
	Review *shelves.Review `json:"review,omitempty"`
	Error  *APIError       `json:"error,omitempty"`
}

func (a *Application) AttachShelfSources(ctx context.Context, shelfID int64, collectionIDs []int64) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.AttachShelfSources(ctx, shelfID, collectionIDs)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}
func (a *Application) PreviewShelfUpdate(ctx context.Context, shelfID int64) ShelfReviewResponse {
	if a.shelves == nil {
		return ShelfReviewResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.PreviewShelfUpdate(ctx, shelfID)
	if err != nil {
		return ShelfReviewResponse{Error: shelfAPIError(err)}
	}
	return ShelfReviewResponse{Review: &result}
}
func (a *Application) ApplyShelfUpdate(ctx context.Context, shelfID int64, token string, decisions []shelves.Decision) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.ApplyShelfUpdate(ctx, shelfID, token, decisions)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}

func (a *Application) PreviewShelfCreation(ctx context.Context, request shelves.CreationRequest) ShelfReviewResponse {
	if a.shelves == nil {
		return ShelfReviewResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.PreviewShelfCreation(ctx, request)
	if err != nil {
		return ShelfReviewResponse{Error: shelfAPIError(err)}
	}
	return ShelfReviewResponse{Review: &result}
}
func (a *Application) CreateShelfFromReview(ctx context.Context, token string, selectedUUIDs []string) ShelfResponse {
	if a.shelves == nil || !a.Status().MutationsAllowed {
		return ShelfResponse{Error: NewAPIError(ErrorNotReady, "Shelves are unavailable until storage is ready.")}
	}
	result, err := a.shelves.CreateShelfFromReview(ctx, token, selectedUUIDs)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &result}
}
