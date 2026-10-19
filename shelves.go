package main

import (
	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelves"
)

func (a *App) ListShelves() coreapp.ShelfListResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfListResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ListShelves(a.runtimeContext())
}

func (a *App) CreateShelf(name string, storyUUIDs []string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.CreateShelf(a.runtimeContext(), name, storyUUIDs)
}

func (a *App) OpenShelf(shelfID int64, query library.StoryLibraryQuery) coreapp.ShelfEvaluationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfEvaluationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.OpenShelf(a.runtimeContext(), shelfID, query)
}

func (a *App) RenameShelf(shelfID int64, name string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RenameShelf(a.runtimeContext(), shelfID, name)
}

func (a *App) DuplicateShelf(shelfID int64, name string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.DuplicateShelf(a.runtimeContext(), shelfID, name)
}

func (a *App) DeleteShelf(shelfID int64) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.DeleteShelf(a.runtimeContext(), shelfID)
}

func (a *App) ReorderShelves(orderedIDs []int64) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ReorderShelves(a.runtimeContext(), orderedIDs)
}

func (a *App) AddStoriesToShelf(shelfID int64, storyUUIDs []string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.AddStoriesToShelf(a.runtimeContext(), shelfID, storyUUIDs)
}

func (a *App) RemoveStoriesFromShelf(shelfID int64, storyUUIDs []string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RemoveStoriesFromShelf(a.runtimeContext(), shelfID, storyUUIDs)
}

func (a *App) SetShelfStoryProtected(shelfID int64, storyUUID string, protected bool) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.SetShelfStoryProtected(a.runtimeContext(), shelfID, storyUUID, protected)
}

func (a *App) ReorderShelfStories(shelfID int64, orderedUUIDs []string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ReorderShelfStories(a.runtimeContext(), shelfID, orderedUUIDs)
}

func (a *App) ListShelfExclusions(shelfID int64) coreapp.ShelfExclusionsResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfExclusionsResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ListShelfExclusions(a.runtimeContext(), shelfID)
}

func (a *App) ClearShelfExclusion(shelfID int64, storyUUID string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ClearShelfExclusion(a.runtimeContext(), shelfID, storyUUID)
}

func (a *App) AttachShelfSources(shelfID int64, collectionIDs []int64) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.AttachShelfSources(a.runtimeContext(), shelfID, collectionIDs)
}
func (a *App) PreviewShelfUpdate(shelfID int64) coreapp.ShelfReviewResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfReviewResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.PreviewShelfUpdate(a.runtimeContext(), shelfID)
}
func (a *App) ApplyShelfUpdate(shelfID int64, token string, decisions []shelves.Decision) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ApplyShelfUpdate(a.runtimeContext(), shelfID, token, decisions)
}

func (a *App) PreviewShelfCreation(request shelves.CreationRequest) coreapp.ShelfReviewResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfReviewResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.PreviewShelfCreation(a.runtimeContext(), request)
}
func (a *App) CreateShelfFromReview(token string, selectedUUIDs []string) coreapp.ShelfResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ShelfResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.CreateShelfFromReview(a.runtimeContext(), token, selectedUUIDs)
}
