package app

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelves"
)

func (a *Application) ListShelves(ctx context.Context) ShelfListResponse {
	summaries, err := a.shelves.ListShelves(ctx)
	if err != nil {
		return ShelfListResponse{Error: shelfAPIError(err)}
	}
	return ShelfListResponse{Shelves: summaries}
}

func (a *Application) CreateShelf(
	ctx context.Context,
	name string,
	query library.StoryLibraryQuery,
) ShelfResponse {
	if response := a.requireShelfMutation(); response != nil {
		return *response
	}
	shelf, err := a.shelves.CreateShelf(ctx, name, query)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &shelf}
}

func (a *Application) OpenShelf(
	ctx context.Context,
	shelfID int64,
	request library.ListRequest,
) ShelfEvaluationResponse {
	evaluation, err := a.shelves.OpenShelf(ctx, shelfID, request)
	if err != nil {
		return ShelfEvaluationResponse{Error: shelfAPIError(err)}
	}
	return ShelfEvaluationResponse{Evaluation: &evaluation}
}

func (a *Application) RenameShelf(
	ctx context.Context,
	shelfID int64,
	name string,
) ShelfResponse {
	if response := a.requireShelfMutation(); response != nil {
		return *response
	}
	shelf, err := a.shelves.RenameShelf(ctx, shelfID, name)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &shelf}
}

func (a *Application) DuplicateShelf(
	ctx context.Context,
	shelfID int64,
	name string,
) ShelfResponse {
	if response := a.requireShelfMutation(); response != nil {
		return *response
	}
	shelf, err := a.shelves.DuplicateShelf(ctx, shelfID, name)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &shelf}
}

func (a *Application) ReplaceShelfQuery(
	ctx context.Context,
	shelfID int64,
	query library.StoryLibraryQuery,
) ShelfResponse {
	if response := a.requireShelfMutation(); response != nil {
		return *response
	}
	shelf, err := a.shelves.ReplaceShelfQuery(ctx, shelfID, query)
	if err != nil {
		return ShelfResponse{Error: shelfAPIError(err)}
	}
	return ShelfResponse{Shelf: &shelf}
}

func (a *Application) ReorderShelves(
	ctx context.Context,
	orderedIDs []int64,
) ShelfListResponse {
	if !a.Status().MutationsAllowed {
		return ShelfListResponse{Error: NewAPIError(
			ErrorNotReady,
			"Saved shelves are unavailable until storage is ready.",
		)}
	}
	if _, err := a.shelves.ReorderShelves(ctx, orderedIDs); err != nil {
		return ShelfListResponse{Error: shelfAPIError(err)}
	}
	return a.ListShelves(ctx)
}

func (a *Application) DeleteShelf(
	ctx context.Context,
	shelfID int64,
) MutationResponse {
	if !a.Status().MutationsAllowed {
		return MutationResponse{Error: NewAPIError(
			ErrorNotReady,
			"Saved shelves are unavailable until storage is ready.",
		)}
	}
	if err := a.shelves.DeleteShelf(ctx, shelfID); err != nil {
		return MutationResponse{Error: shelfAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) PreviewShelves(
	ctx context.Context,
	shelfIDs []int64,
) ShelfSelectionPreviewResponse {
	preview, err := a.shelves.PreviewShelves(ctx, shelfIDs)
	if err != nil {
		return ShelfSelectionPreviewResponse{Error: shelfAPIError(err)}
	}
	return ShelfSelectionPreviewResponse{Preview: &preview}
}

func (a *Application) requireShelfMutation() *ShelfResponse {
	if a.Status().MutationsAllowed {
		return nil
	}
	return &ShelfResponse{Error: NewAPIError(
		ErrorNotReady,
		"Saved shelves are unavailable until storage is ready.",
	)}
}

func shelfAPIError(err error) *APIError {
	switch {
	case errors.Is(err, shelves.ErrInvalidShelfName),
		errors.Is(err, shelves.ErrDuplicateShelfName),
		errors.Is(err, shelves.ErrInvalidShelfOrder),
		errors.Is(err, shelves.ErrInvalidShelfSelection),
		errors.Is(err, shelves.ErrShelfCriteriaUnavailable),
		errors.Is(err, shelves.ErrInvalidSavedLibraryQuery),
		errors.Is(err, shelves.ErrUnsupportedSavedQueryVersion):
		return NewAPIError(ErrorInvalidInput, "The saved shelf could not be validated.")
	case errors.Is(err, shelves.ErrShelfNotFound):
		return NewAPIError(ErrorConflict, "The saved shelf no longer exists.")
	case errors.Is(err, shelves.ErrShelfNeedsAttention):
		return NewAPIError(ErrorConflict, "The saved shelf needs attention before it can be opened.")
	default:
		return NewAPIError(ErrorInternal, "The saved shelf action could not be completed.")
	}
}
