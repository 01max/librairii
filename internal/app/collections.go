package app

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func (a *Application) ListCollections(ctx context.Context, includeBroken ...library.BrokenVisibility) CollectionListResponse {
	summaries, err := a.collections.ListCollections(ctx, includeBroken...)
	if err != nil {
		return CollectionListResponse{Error: collectionAPIError(err)}
	}
	return CollectionListResponse{Collections: summaries}
}

func (a *Application) CreateCollection(
	ctx context.Context,
	name string,
	query library.StoryLibraryQuery,
) CollectionResponse {
	if response := a.requireCollectionMutation(); response != nil {
		return *response
	}
	shelf, err := a.collections.CreateCollection(ctx, name, query)
	if err != nil {
		return CollectionResponse{Error: collectionAPIError(err)}
	}
	return CollectionResponse{Collection: &shelf}
}

func (a *Application) OpenCollection(
	ctx context.Context,
	shelfID int64,
	request library.ListRequest,
) CollectionEvaluationResponse {
	evaluation, err := a.collections.OpenCollection(ctx, shelfID, request)
	if err != nil {
		return CollectionEvaluationResponse{Error: collectionAPIError(err)}
	}
	return CollectionEvaluationResponse{Evaluation: &evaluation}
}

func (a *Application) RenameCollection(
	ctx context.Context,
	shelfID int64,
	name string,
) CollectionResponse {
	if response := a.requireCollectionMutation(); response != nil {
		return *response
	}
	shelf, err := a.collections.RenameCollection(ctx, shelfID, name)
	if err != nil {
		return CollectionResponse{Error: collectionAPIError(err)}
	}
	return CollectionResponse{Collection: &shelf}
}

func (a *Application) DuplicateCollection(
	ctx context.Context,
	shelfID int64,
	name string,
) CollectionResponse {
	if response := a.requireCollectionMutation(); response != nil {
		return *response
	}
	shelf, err := a.collections.DuplicateCollection(ctx, shelfID, name)
	if err != nil {
		return CollectionResponse{Error: collectionAPIError(err)}
	}
	return CollectionResponse{Collection: &shelf}
}

func (a *Application) ReplaceCollectionQuery(
	ctx context.Context,
	shelfID int64,
	query library.StoryLibraryQuery,
) CollectionResponse {
	if response := a.requireCollectionMutation(); response != nil {
		return *response
	}
	shelf, err := a.collections.ReplaceCollectionQuery(ctx, shelfID, query)
	if err != nil {
		return CollectionResponse{Error: collectionAPIError(err)}
	}
	return CollectionResponse{Collection: &shelf}
}

func (a *Application) ReorderCollections(
	ctx context.Context,
	orderedIDs []int64,
) CollectionListResponse {
	if !a.Status().MutationsAllowed {
		return CollectionListResponse{Error: NewAPIError(
			ErrorNotReady,
			"Saved shelves are unavailable until storage is ready.",
		)}
	}
	if _, err := a.collections.ReorderCollections(ctx, orderedIDs); err != nil {
		return CollectionListResponse{Error: collectionAPIError(err)}
	}
	return a.ListCollections(ctx)
}

func (a *Application) DeleteCollection(
	ctx context.Context,
	shelfID int64,
) MutationResponse {
	if !a.Status().MutationsAllowed {
		return MutationResponse{Error: NewAPIError(
			ErrorNotReady,
			"Saved shelves are unavailable until storage is ready.",
		)}
	}
	if err := a.collections.DeleteCollection(ctx, shelfID); err != nil {
		return MutationResponse{Error: collectionAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) PreviewCollections(
	ctx context.Context,
	shelfIDs []int64,
	includeBroken ...library.BrokenVisibility,
) CollectionSelectionPreviewResponse {
	preview, err := a.collections.PreviewCollections(ctx, shelfIDs, includeBroken...)
	if err != nil {
		return CollectionSelectionPreviewResponse{Error: collectionAPIError(err)}
	}
	return CollectionSelectionPreviewResponse{Preview: &preview}
}

func (a *Application) requireCollectionMutation() *CollectionResponse {
	if a.Status().MutationsAllowed {
		return nil
	}
	return &CollectionResponse{Error: NewAPIError(
		ErrorNotReady,
		"Saved shelves are unavailable until storage is ready.",
	)}
}

func collectionAPIError(err error) *APIError {
	switch {
	case errors.Is(err, collections.ErrCollectionInUse):
		return NewAPIError(ErrorConflict, "This Collection supplies Shelves. Review its removal first.")
	case errors.Is(err, collections.ErrInvalidCollectionName),
		errors.Is(err, collections.ErrDuplicateCollectionName),
		errors.Is(err, collections.ErrInvalidCollectionOrder),
		errors.Is(err, collections.ErrInvalidCollectionSelection),
		errors.Is(err, collections.ErrCollectionCriteriaUnavailable),
		errors.Is(err, collections.ErrInvalidSavedLibraryQuery),
		errors.Is(err, collections.ErrUnsupportedSavedQueryVersion):
		return NewAPIError(ErrorInvalidInput, "The saved shelf could not be validated.")
	case errors.Is(err, collections.ErrCollectionNotFound):
		return NewAPIError(ErrorConflict, "The saved shelf no longer exists.")
	case errors.Is(err, collections.ErrCollectionNeedsAttention):
		return NewAPIError(ErrorConflict, "The saved shelf needs attention before it can be opened.")
	default:
		return NewAPIError(ErrorInternal, "The saved shelf action could not be completed.")
	}
}
