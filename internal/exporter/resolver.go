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
	ExportQuery(context.Context, library.StoryLibraryQuery) ([]library.ExportStory, error)
	ExportStories(context.Context, []int64) ([]library.ExportStory, error)
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
	stories, err := r.library.ExportStories(ctx, ids)
	if err != nil {
		return Scope{}, fmt.Errorf("%w: resolve selection: %v", ErrStoryUnavailable, err)
	}
	return Scope{
		Source:  operations.ExportSource{Type: operations.ExportSourceSelection},
		Stories: stories,
	}, nil
}

func (r *Resolver) ResolveCurrentQuery(
	ctx context.Context,
	query library.StoryLibraryQuery,
) (Scope, error) {
	stories, err := r.library.ExportQuery(ctx, query)
	if err != nil {
		return Scope{}, fmt.Errorf("%w: resolve current query: %v", ErrStoryUnavailable, err)
	}
	return Scope{
		Source:  operations.ExportSource{Type: operations.ExportSourceCurrentQuery},
		Stories: stories,
	}, nil
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
	stories, err := r.library.ExportQuery(ctx, opened.Query.StoryLibraryQuery())
	if err != nil {
		return Scope{}, fmt.Errorf(
			"%w: resolve export shelf %d: %v",
			ErrStoryUnavailable,
			shelfID,
			err,
		)
	}
	return Scope{
		Source: operations.ExportSource{
			Type:       operations.ExportSourceShelf,
			ShelfIDs:   []int64{opened.Shelf.ID},
			ShelfNames: []string{opened.Shelf.Name},
		},
		Stories: stories,
	}, nil
}

func (r *Resolver) ResolveShelves(
	ctx context.Context,
	shelfIDs []int64,
) (Scope, error) {
	if len(shelfIDs) < 2 {
		return Scope{}, ErrInvalidScope
	}
	seenShelves := make(map[int64]struct{}, len(shelfIDs))
	for _, shelfID := range shelfIDs {
		if shelfID <= 0 {
			return Scope{}, ErrInvalidScope
		}
		if _, duplicate := seenShelves[shelfID]; duplicate {
			return Scope{}, ErrInvalidScope
		}
		seenShelves[shelfID] = struct{}{}
	}
	union := make(map[int64]struct{})
	var stories []library.ExportStory
	source := operations.ExportSource{
		Type:       operations.ExportSourceShelves,
		ShelfIDs:   make([]int64, 0, len(shelfIDs)),
		ShelfNames: make([]string, 0, len(shelfIDs)),
	}
	totalMemberships := 0
	for _, shelfID := range shelfIDs {
		opened, err := r.shelves.Open(ctx, shelfID)
		if err != nil {
			return Scope{}, fmt.Errorf("open export shelf %d: %w", shelfID, err)
		}
		members, err := r.library.ExportQuery(ctx, opened.Query.StoryLibraryQuery())
		if err != nil {
			return Scope{}, fmt.Errorf(
				"%w: resolve export shelf %d: %v",
				ErrStoryUnavailable,
				shelfID,
				err,
			)
		}
		totalMemberships += len(members)
		for _, story := range members {
			if _, duplicate := union[story.ID]; duplicate {
				continue
			}
			union[story.ID] = struct{}{}
			stories = append(stories, story)
		}
		source.ShelfIDs = append(source.ShelfIDs, opened.Shelf.ID)
		source.ShelfNames = append(source.ShelfNames, opened.Shelf.Name)
	}
	return Scope{
		Source:           source,
		Stories:          stories,
		CollapsedOverlap: totalMemberships - len(stories),
	}, nil
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
