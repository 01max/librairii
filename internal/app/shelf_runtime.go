package app

import (
	"context"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelves"
)

func (r *ImportRuntime) currentShelves() (*shelves.Service, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.shelves == nil {
		return nil, ErrImportRuntimeNotReady
	}
	return r.shelves, nil
}

var _ ShelfPort = (*ImportRuntime)(nil)

func (r *ImportRuntime) ListShelves(ctx context.Context) ([]shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return nil, err
	}
	return service.List(ctx)
}

func (r *ImportRuntime) CreateShelf(ctx context.Context, name string, storyUUIDs []string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.CreateManual(ctx, name, storyUUIDs)
}

func (r *ImportRuntime) OpenShelf(ctx context.Context, shelfID int64, query library.StoryLibraryQuery) (shelves.Evaluation, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Evaluation{}, err
	}
	return service.Open(ctx, shelfID, query)
}

func (r *ImportRuntime) RenameShelf(ctx context.Context, shelfID int64, name string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.Rename(ctx, shelfID, name)
}

func (r *ImportRuntime) DuplicateShelf(ctx context.Context, shelfID int64, name string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.Duplicate(ctx, shelfID, name)
}

func (r *ImportRuntime) DeleteShelf(ctx context.Context, shelfID int64) error {
	service, err := r.currentShelves()
	if err != nil {
		return err
	}
	return service.Delete(ctx, shelfID)
}

func (r *ImportRuntime) ReorderShelves(ctx context.Context, orderedIDs []int64) error {
	service, err := r.currentShelves()
	if err != nil {
		return err
	}
	return service.Reorder(ctx, orderedIDs)
}

func (r *ImportRuntime) AddStoriesToShelf(ctx context.Context, shelfID int64, storyUUIDs []string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.AddStories(ctx, shelfID, storyUUIDs)
}

func (r *ImportRuntime) RemoveStoriesFromShelf(ctx context.Context, shelfID int64, storyUUIDs []string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.RemoveStories(ctx, shelfID, storyUUIDs)
}

func (r *ImportRuntime) SetShelfStoryProtected(ctx context.Context, shelfID int64, storyUUID string, protected bool) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.SetProtected(ctx, shelfID, storyUUID, protected)
}

func (r *ImportRuntime) ReorderShelfStories(ctx context.Context, shelfID int64, orderedUUIDs []string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.ReorderMembers(ctx, shelfID, orderedUUIDs)
}

func (r *ImportRuntime) ListShelfExclusions(ctx context.Context, shelfID int64) ([]shelves.Exclusion, error) {
	service, err := r.currentShelves()
	if err != nil {
		return nil, err
	}
	return service.Exclusions(ctx, shelfID)
}

func (r *ImportRuntime) ClearShelfExclusion(ctx context.Context, shelfID int64, storyUUID string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.ClearExclusion(ctx, shelfID, storyUUID)
}

func (r *ImportRuntime) AttachShelfSources(ctx context.Context, shelfID int64, collectionIDs []int64) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.AttachSources(ctx, shelfID, collectionIDs)
}
func (r *ImportRuntime) PreviewShelfUpdate(ctx context.Context, shelfID int64) (shelves.Review, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Review{}, err
	}
	return service.Preview(ctx, shelfID)
}
func (r *ImportRuntime) ApplyShelfUpdate(ctx context.Context, shelfID int64, token string, decisions []shelves.Decision) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.Apply(ctx, shelfID, token, decisions)
}

func (r *ImportRuntime) PreviewShelfCreation(ctx context.Context, request shelves.CreationRequest) (shelves.Review, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Review{}, err
	}
	return service.PreviewCreation(ctx, request)
}
func (r *ImportRuntime) CreateShelfFromReview(ctx context.Context, token string, selectedUUIDs []string) (shelves.Shelf, error) {
	service, err := r.currentShelves()
	if err != nil {
		return shelves.Shelf{}, err
	}
	return service.CreateFromReview(ctx, token, selectedUUIDs)
}
