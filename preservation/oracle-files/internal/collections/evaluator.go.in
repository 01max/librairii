package collections

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/library"
)

var ErrMissingLibraryQuery = errors.New("story library query is required")
var ErrInvalidCollectionSelection = errors.New("collection selection is invalid")

type storyLibrarySearcher interface {
	Search(context.Context, library.StoryLibraryQuery) (library.Page, error)
}

type storyLibraryCounter interface {
	Count(context.Context, library.StoryLibraryQuery) (int, error)
}

type Evaluation struct {
	Collection Collection        `json:"collection"`
	Query      SavedLibraryQuery `json:"query"`
	Page       library.Page      `json:"page"`
}

type CollectionCount struct {
	CollectionID int64 `json:"collectionId"`
	Count        int   `json:"count"`
}

type Summary struct {
	ID              int64           `json:"id"`
	Name            string          `json:"name"`
	Position        int             `json:"position"`
	Validity        Validity        `json:"validity"`
	AttentionReason AttentionReason `json:"attentionReason,omitempty"`
	Count           int             `json:"count"`
}

type SelectedCollectionPreview struct {
	ID    int64  `json:"id"`
	Name  string `json:"name"`
	Count int    `json:"count"`
}

type SelectionPreview struct {
	Collections           []SelectedCollectionPreview `json:"collections"`
	SourceCollectionNames []string                    `json:"sourceCollectionNames"`
	UniqueStoryCount      int                         `json:"uniqueStoryCount"`
	OverlapCount          int                         `json:"overlapCount"`
}

type Evaluator struct {
	collections *Service
	library     storyLibrarySearcher
}

func NewEvaluator(
	collections *Service,
	libraryQuery storyLibrarySearcher,
) (*Evaluator, error) {
	if collections == nil {
		return nil, ErrMissingDatabase
	}
	if libraryQuery == nil {
		return nil, ErrMissingLibraryQuery
	}
	return &Evaluator{collections: collections, library: libraryQuery}, nil
}

func (e *Evaluator) Evaluate(
	ctx context.Context,
	collectionID int64,
	request library.ListRequest,
) (Evaluation, error) {
	opened, err := e.collections.Open(ctx, collectionID)
	if err != nil {
		return Evaluation{}, err
	}
	query := opened.Query.StoryLibraryQuery()
	query.Page = request.Page
	query.PageSize = request.PageSize
	query.Sort = request.Sort
	page, err := e.library.Search(ctx, query)
	if err != nil {
		return Evaluation{}, err
	}
	return Evaluation{
		Collection: opened.Collection,
		Query:      opened.Query,
		Page:       page,
	}, nil
}

func (e *Evaluator) Count(ctx context.Context, collectionID int64, includeBroken ...library.BrokenVisibility) (CollectionCount, error) {
	opened, err := e.collections.Open(ctx, collectionID)
	if err != nil {
		return CollectionCount{}, err
	}
	if counter, ok := e.library.(storyLibraryCounter); ok {
		count, err := counter.Count(ctx, withVisibility(opened.Query.StoryLibraryQuery(), includeBroken))
		if err != nil {
			return CollectionCount{}, err
		}
		return CollectionCount{CollectionID: collectionID, Count: count}, nil
	}
	query := withVisibility(opened.Query.StoryLibraryQuery(), includeBroken)
	query.Page = 1
	query.PageSize = 1
	query.Sort = library.SortNameAscending
	page, err := e.library.Search(ctx, query)
	if err != nil {
		return CollectionCount{}, err
	}
	return CollectionCount{
		CollectionID: collectionID,
		Count:        page.TotalItems,
	}, nil
}

func (e *Evaluator) Counts(ctx context.Context, includeBroken ...library.BrokenVisibility) ([]CollectionCount, error) {
	collections, err := e.collections.List(ctx)
	if err != nil {
		return nil, err
	}
	counts := make([]CollectionCount, 0, len(collections))
	for _, collection := range collections {
		count, err := e.Count(ctx, collection.ID, includeBroken...)
		if err != nil {
			return nil, err
		}
		counts = append(counts, count)
	}
	return counts, nil
}

func (e *Evaluator) Summaries(ctx context.Context, includeBroken ...library.BrokenVisibility) ([]Summary, error) {
	collections, err := e.collections.List(ctx)
	if err != nil {
		return nil, err
	}
	summaries := make([]Summary, 0, len(collections))
	for _, collection := range collections {
		inspection, err := e.collections.Inspect(ctx, collection.ID)
		if err != nil {
			return nil, err
		}
		collection = inspection.Collection
		if collection.Validity != ValidityValid {
			summaries = append(summaries, Summary{
				ID:              collection.ID,
				Name:            collection.Name,
				Position:        collection.Position,
				Validity:        collection.Validity,
				AttentionReason: inspection.AttentionReason,
			})
			continue
		}
		count, err := e.Count(ctx, collection.ID, includeBroken...)
		if err != nil {
			return nil, err
		}
		summaries = append(summaries, Summary{
			ID:       collection.ID,
			Name:     collection.Name,
			Position: collection.Position,
			Validity: collection.Validity,
			Count:    count.Count,
		})
	}
	return summaries, nil
}

func (e *Evaluator) PreviewSelection(
	ctx context.Context,
	collectionIDs []int64,
	includeBroken ...library.BrokenVisibility,
) (SelectionPreview, error) {
	if len(collectionIDs) == 0 {
		return SelectionPreview{}, ErrInvalidCollectionSelection
	}
	seenCollections := make(map[int64]struct{}, len(collectionIDs))
	union := make(map[int64]struct{})
	preview := SelectionPreview{
		Collections:           make([]SelectedCollectionPreview, 0, len(collectionIDs)),
		SourceCollectionNames: make([]string, 0, len(collectionIDs)),
	}
	totalMemberships := 0
	for _, collectionID := range collectionIDs {
		if collectionID <= 0 {
			return SelectionPreview{}, ErrInvalidCollectionSelection
		}
		if _, duplicate := seenCollections[collectionID]; duplicate {
			return SelectionPreview{}, ErrInvalidCollectionSelection
		}
		seenCollections[collectionID] = struct{}{}

		opened, err := e.collections.Open(ctx, collectionID)
		if err != nil {
			return SelectionPreview{}, err
		}
		query := withVisibility(opened.Query.StoryLibraryQuery(), includeBroken)
		query.Page = 1
		query.PageSize = library.MaxPageSize
		query.Sort = library.SortNameAscending
		members := make(map[int64]struct{})
		for {
			page, err := e.library.Search(ctx, query)
			if err != nil {
				return SelectionPreview{}, err
			}
			for _, story := range page.Stories {
				members[story.ID] = struct{}{}
				union[story.ID] = struct{}{}
			}
			if query.Page >= page.TotalPages {
				break
			}
			query.Page++
		}
		count := len(members)
		totalMemberships += count
		preview.Collections = append(preview.Collections, SelectedCollectionPreview{
			ID:    opened.Collection.ID,
			Name:  opened.Collection.Name,
			Count: count,
		})
		preview.SourceCollectionNames = append(
			preview.SourceCollectionNames,
			opened.Collection.Name,
		)
	}
	preview.UniqueStoryCount = len(union)
	preview.OverlapCount = totalMemberships - preview.UniqueStoryCount
	return preview, nil
}

func withVisibility(query library.StoryLibraryQuery, override []library.BrokenVisibility) library.StoryLibraryQuery {
	return query
}
