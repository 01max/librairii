package exporter

import (
	"context"
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/collections"
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

type collectionResolver interface {
	Open(context.Context, int64) (collections.OpenedCollection, error)
}

type Scope struct {
	Source           operations.ExportSource
	Stories          []library.ExportStory
	CollapsedOverlap int
}

type Resolver struct {
	library     libraryResolver
	collections collectionResolver
	shelves     curatedShelfResolver
}

func NewResolver(
	libraryQuery libraryResolver,
	shelfService collectionResolver,
	curated ...curatedShelfResolver,
) (*Resolver, error) {
	if libraryQuery == nil {
		return nil, ErrMissingLibrary
	}
	if shelfService == nil {
		return nil, ErrMissingShelves
	}
	result := &Resolver{
		library:     libraryQuery,
		collections: shelfService,
	}
	if len(curated) > 1 {
		return nil, ErrInvalidScope
	}
	if len(curated) == 1 {
		result.shelves = curated[0]
	}
	return result, nil
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

func (r *Resolver) ResolveCollection(
	ctx context.Context,
	shelfID int64,
	includeBroken ...library.BrokenVisibility,
) (Scope, error) {
	if shelfID <= 0 {
		return Scope{}, ErrInvalidScope
	}
	opened, err := r.collections.Open(ctx, shelfID)
	if err != nil {
		return Scope{}, fmt.Errorf("open export shelf %d: %w", shelfID, err)
	}
	stories, err := r.library.ExportQuery(ctx, exportVisibility(opened.Query.StoryLibraryQuery(), includeBroken))
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
			Type:            operations.ExportSourceCollection,
			CollectionIDs:   []int64{opened.Collection.ID},
			CollectionNames: []string{opened.Collection.Name},
		},
		Stories: stories,
	}, nil
}

func (r *Resolver) ResolveCollections(
	ctx context.Context,
	shelfIDs []int64,
	includeBroken ...library.BrokenVisibility,
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
	union := make(map[string]struct{})
	var stories []library.ExportStory
	source := operations.ExportSource{
		Type:            operations.ExportSourceCollections,
		CollectionIDs:   make([]int64, 0, len(shelfIDs)),
		CollectionNames: make([]string, 0, len(shelfIDs)),
	}
	totalMemberships := 0
	for _, shelfID := range shelfIDs {
		opened, err := r.collections.Open(ctx, shelfID)
		if err != nil {
			return Scope{}, fmt.Errorf("open export shelf %d: %w", shelfID, err)
		}
		members, err := r.library.ExportQuery(ctx, exportVisibility(opened.Query.StoryLibraryQuery(), includeBroken))
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
			if _, duplicate := union[story.UUID]; duplicate {
				continue
			}
			union[story.UUID] = struct{}{}
			stories = append(stories, story)
		}
		source.CollectionIDs = append(source.CollectionIDs, opened.Collection.ID)
		source.CollectionNames = append(source.CollectionNames, opened.Collection.Name)
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

func exportVisibility(query library.StoryLibraryQuery, override []library.BrokenVisibility) library.StoryLibraryQuery {
	return query
}

type curatedShelfResolver interface {
	Get(context.Context, int64) (shelves.Shelf, error)
	Members(context.Context, int64) ([]shelves.Member, error)
}
type uuidLibraryResolver interface {
	ExportUUIDs(context.Context, []string, library.StoryLibraryQuery) ([]library.ExportStory, error)
}

func (r *Resolver) ResolveShelf(ctx context.Context, id int64) (Scope, error) {
	return r.resolveCurated(ctx, []int64{id}, nil)
}
func (r *Resolver) ResolveShelves(ctx context.Context, ids []int64) (Scope, error) {
	if len(ids) < 2 {
		return Scope{}, ErrInvalidScope
	}
	return r.resolveCurated(ctx, ids, nil)
}
func (r *Resolver) ResolveShelfResult(ctx context.Context, id int64, query library.StoryLibraryQuery) (Scope, error) {
	scope, err := r.resolveCurated(ctx, []int64{id}, &query)
	if err != nil {
		return Scope{}, err
	}
	scope.Source = operations.ExportSource{Type: operations.ExportSourceCurrentQuery}
	return scope, nil
}
func (r *Resolver) resolveCurated(ctx context.Context, ids []int64, refinement *library.StoryLibraryQuery) (Scope, error) {
	if r.shelves == nil {
		return Scope{}, ErrMissingShelves
	}
	resolver, ok := r.library.(uuidLibraryResolver)
	if !ok {
		return Scope{}, ErrMissingLibrary
	}
	unique, err := uniqueIDs(ids)
	if err != nil || len(unique) != len(ids) || len(ids) == 0 {
		return Scope{}, ErrInvalidScope
	}
	source := operations.ExportSource{Type: operations.ExportSourceCuratedShelf, ShelfIDs: ids, ShelfNames: make([]string, 0, len(ids))}
	if len(ids) > 1 {
		source.Type = operations.ExportSourceCuratedShelves
	}
	scope := Scope{Source: source, Stories: make([]library.ExportStory, 0)}
	seen := map[string]bool{}
	for _, id := range ids {
		shelf, err := r.shelves.Get(ctx, id)
		if err != nil {
			return Scope{}, err
		}
		members, err := r.shelves.Members(ctx, id)
		if err != nil {
			return Scope{}, err
		}
		scope.Source.ShelfNames = append(scope.Source.ShelfNames, shelf.Name)
		uuids := make([]string, 0, len(members))
		for _, m := range members {
			uuids = append(uuids, m.UUID)
		}
		available, err := resolver.ExportUUIDs(ctx, uuids, library.StoryLibraryQuery{})
		if err != nil {
			return Scope{}, err
		}
		byUUID := map[string]library.ExportStory{}
		for _, story := range available {
			byUUID[story.UUID] = story
		}
		var matching map[string]bool
		filtered := false
		if refinement != nil {
			canonical, err := library.CanonicalStoryLibraryMembershipQuery(*refinement)
			if err != nil {
				return Scope{}, err
			}
			filtered = canonical.Name != "" || len(canonical.AnyOf) > 0 || len(canonical.Languages) > 0 || len(canonical.Compatibilities) > 0 || len(canonical.BooleanFilters) > 0 || len(canonical.ChoiceFilters) > 0
			results, err := resolver.ExportUUIDs(ctx, uuids, *refinement)
			if err != nil {
				return Scope{}, err
			}
			matching = map[string]bool{}
			for _, story := range results {
				matching[story.UUID] = true
			}
		}
		for _, m := range members {
			story, exists := byUUID[m.UUID]
			if refinement != nil && ((exists && !matching[m.UUID]) || (!exists && filtered)) {
				continue
			}
			if seen[m.UUID] {
				scope.CollapsedOverlap++
				continue
			}
			seen[m.UUID] = true
			if !exists {
				title := m.CachedTitle
				if title == "" {
					title = m.UUID
				}
				story = library.ExportStory{UUID: m.UUID, Title: title}
			}
			scope.Stories = append(scope.Stories, story)
		}
	}
	return scope, nil
}
