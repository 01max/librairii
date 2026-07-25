package exporter

import (
	"context"
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
)

var (
	ErrMissingLibrary   = errors.New("export library resolver is required")
	ErrMissingShelves   = errors.New("export shelf resolver is required")
	ErrInvalidScope     = errors.New("export scope is invalid")
	ErrStoryUnavailable = errors.New("resolved export story is unavailable")
)

type libraryResolver interface {
	Search(context.Context, library.StoryLibraryQuery) (library.Page, error)
	ExportStory(context.Context, int64) (library.ExportStory, error)
}

type shelfResolver interface {
	Open(context.Context, int64) (shelves.OpenedShelf, error)
}

type Scope struct {
	Source           operations.ExportSource
	Stories          []library.ExportStory
	CollapsedOverlap int
}

type Resolver struct {
	library libraryResolver
	shelves shelfResolver
}

func NewResolver(
	libraryQuery libraryResolver,
	shelfService shelfResolver,
) (*Resolver, error) {
	if libraryQuery == nil {
		return nil, ErrMissingLibrary
	}
	if shelfService == nil {
		return nil, ErrMissingShelves
	}
	return &Resolver{
		library: libraryQuery,
		shelves: shelfService,
	}, nil
}

func (r *Resolver) ResolveSelection(
	ctx context.Context,
	storyIDs []int64,
) (Scope, error) {
	ids, err := uniqueIDs(storyIDs)
	if err != nil {
		return Scope{}, err
	}
	return r.materialize(ctx, operations.ExportSource{
		Type: operations.ExportSourceSelection,
	}, ids, 0)
}

func (r *Resolver) ResolveCurrentQuery(
	ctx context.Context,
	query library.StoryLibraryQuery,
) (Scope, error) {
	ids, err := r.queryStoryIDs(ctx, query)
	if err != nil {
		return Scope{}, err
	}
	return r.materialize(ctx, operations.ExportSource{
		Type: operations.ExportSourceCurrentQuery,
	}, ids, 0)
}

func (r *Resolver) ResolveShelf(
	ctx context.Context,
	shelfID int64,
) (Scope, error) {
	if shelfID <= 0 {
		return Scope{}, ErrInvalidScope
	}
	opened, err := r.shelves.Open(ctx, shelfID)
	if err != nil {
		return Scope{}, fmt.Errorf("open export shelf %d: %w", shelfID, err)
	}
	ids, err := r.queryStoryIDs(ctx, opened.Query.StoryLibraryQuery())
	if err != nil {
		return Scope{}, fmt.Errorf("resolve export shelf %d: %w", shelfID, err)
	}
	return r.materialize(ctx, operations.ExportSource{
		Type:       operations.ExportSourceShelf,
		ShelfIDs:   []int64{opened.Shelf.ID},
		ShelfNames: []string{opened.Shelf.Name},
	}, ids, 0)
}

func (r *Resolver) ResolveShelves(
	ctx context.Context,
	shelfIDs []int64,
) (Scope, error) {
	if len(shelfIDs) < 2 {
		return Scope{}, ErrInvalidScope
	}
	seenShelves := make(map[int64]struct{}, len(shelfIDs))
	union := make(map[int64]struct{})
	var orderedStoryIDs []int64
	source := operations.ExportSource{
		Type:       operations.ExportSourceShelves,
		ShelfIDs:   make([]int64, 0, len(shelfIDs)),
		ShelfNames: make([]string, 0, len(shelfIDs)),
	}
	totalMemberships := 0
	for _, shelfID := range shelfIDs {
		if shelfID <= 0 {
			return Scope{}, ErrInvalidScope
		}
		if _, duplicate := seenShelves[shelfID]; duplicate {
			return Scope{}, ErrInvalidScope
		}
		seenShelves[shelfID] = struct{}{}
		opened, err := r.shelves.Open(ctx, shelfID)
		if err != nil {
			return Scope{}, fmt.Errorf("open export shelf %d: %w", shelfID, err)
		}
		memberIDs, err := r.queryStoryIDs(ctx, opened.Query.StoryLibraryQuery())
		if err != nil {
			return Scope{}, fmt.Errorf(
				"resolve export shelf %d: %w",
				shelfID,
				err,
			)
		}
		totalMemberships += len(memberIDs)
		for _, storyID := range memberIDs {
			if _, duplicate := union[storyID]; duplicate {
				continue
			}
			union[storyID] = struct{}{}
			orderedStoryIDs = append(orderedStoryIDs, storyID)
		}
		source.ShelfIDs = append(source.ShelfIDs, opened.Shelf.ID)
		source.ShelfNames = append(source.ShelfNames, opened.Shelf.Name)
	}
	return r.materialize(
		ctx,
		source,
		orderedStoryIDs,
		totalMemberships-len(orderedStoryIDs),
	)
}

func (r *Resolver) queryStoryIDs(
	ctx context.Context,
	query library.StoryLibraryQuery,
) ([]int64, error) {
	query.Page = 1
	query.PageSize = library.MaxPageSize
	if query.Sort == "" {
		query.Sort = library.SortNameAscending
	}
	var ordered []int64
	seen := make(map[int64]struct{})
	for {
		page, err := r.library.Search(ctx, query)
		if err != nil {
			return nil, err
		}
		for _, story := range page.Stories {
			if _, duplicate := seen[story.ID]; duplicate {
				continue
			}
			seen[story.ID] = struct{}{}
			ordered = append(ordered, story.ID)
		}
		if query.Page >= page.TotalPages {
			return ordered, nil
		}
		query.Page++
	}
}

func (r *Resolver) materialize(
	ctx context.Context,
	source operations.ExportSource,
	storyIDs []int64,
	collapsedOverlap int,
) (Scope, error) {
	scope := Scope{
		Source:           source,
		Stories:          make([]library.ExportStory, 0, len(storyIDs)),
		CollapsedOverlap: collapsedOverlap,
	}
	for _, storyID := range storyIDs {
		story, err := r.library.ExportStory(ctx, storyID)
		if err != nil {
			return Scope{}, fmt.Errorf(
				"%w: story %d: %v",
				ErrStoryUnavailable,
				storyID,
				err,
			)
		}
		scope.Stories = append(scope.Stories, story)
	}
	return scope, nil
}

func uniqueIDs(ids []int64) ([]int64, error) {
	unique := make([]int64, 0, len(ids))
	seen := make(map[int64]struct{}, len(ids))
	for _, id := range ids {
		if id <= 0 {
			return nil, ErrInvalidScope
		}
		if _, duplicate := seen[id]; duplicate {
			continue
		}
		seen[id] = struct{}{}
		unique = append(unique, id)
	}
	return unique, nil
}
