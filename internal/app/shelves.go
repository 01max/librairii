package app

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func (a *Application) ListShelves(ctx context.Context, includeBroken ...library.BrokenVisibility) ShelfListResponse {
	summaries, err := a.collections.ListShelves(ctx, includeBroken...)
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
	shelf, err := a.collections.CreateShelf(ctx, name, query)
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
	evaluation, err := a.collections.OpenShelf(ctx, shelfID, request)
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
	shelf, err := a.collections.RenameShelf(ctx, shelfID, name)
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
	shelf, err := a.collections.DuplicateShelf(ctx, shelfID, name)
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
	shelf, err := a.collections.ReplaceShelfQuery(ctx, shelfID, query)
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
	if _, err := a.collections.ReorderShelves(ctx, orderedIDs); err != nil {
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
	if err := a.collections.DeleteShelf(ctx, shelfID); err != nil {
		return MutationResponse{Error: shelfAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) PreviewShelves(
	ctx context.Context,
	shelfIDs []int64,
	includeBroken ...library.BrokenVisibility,
) ShelfSelectionPreviewResponse {
	preview, err := a.collections.PreviewShelves(ctx, shelfIDs, includeBroken...)
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
	case errors.Is(err, collections.ErrInvalidShelfName),
		errors.Is(err, collections.ErrDuplicateShelfName),
		errors.Is(err, collections.ErrInvalidShelfOrder),
		errors.Is(err, collections.ErrInvalidShelfSelection),
		errors.Is(err, collections.ErrShelfCriteriaUnavailable),
		errors.Is(err, collections.ErrInvalidSavedLibraryQuery),
		errors.Is(err, collections.ErrUnsupportedSavedQueryVersion):
		return NewAPIError(ErrorInvalidInput, "The saved shelf could not be validated.")
	case errors.Is(err, collections.ErrShelfNotFound):
		return NewAPIError(ErrorConflict, "The saved shelf no longer exists.")
	case errors.Is(err, collections.ErrShelfNeedsAttention):
		return NewAPIError(ErrorConflict, "The saved shelf needs attention before it can be opened.")
	default:
		return NewAPIError(ErrorInternal, "The saved shelf action could not be completed.")
	}
}
