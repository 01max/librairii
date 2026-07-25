package shelves

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/library"
)

var ErrMissingLibraryQuery = errors.New("story library query is required")
var ErrInvalidShelfSelection = errors.New("shelf selection is invalid")

type storyLibrarySearcher interface {
	Search(context.Context, library.StoryLibraryQuery) (library.Page, error)
}

type storyLibraryCounter interface {
	Count(context.Context, library.StoryLibraryQuery) (int, error)
}

type Evaluation struct {
	Shelf Shelf             `json:"shelf"`
	Query SavedLibraryQuery `json:"query"`
	Page  library.Page      `json:"page"`
}

type ShelfCount struct {
	ShelfID int64 `json:"shelfId"`
	Count   int   `json:"count"`
}

type Summary struct {
	ID              int64           `json:"id"`
	Name            string          `json:"name"`
	Position        int             `json:"position"`
	Validity        Validity        `json:"validity"`
	AttentionReason AttentionReason `json:"attentionReason,omitempty"`
	Count           int             `json:"count"`
}

type SelectedShelfPreview struct {
	ID    int64  `json:"id"`
	Name  string `json:"name"`
	Count int    `json:"count"`
}

type SelectionPreview struct {
	Shelves          []SelectedShelfPreview `json:"shelves"`
	SourceShelfNames []string               `json:"sourceShelfNames"`
	UniqueStoryCount int                    `json:"uniqueStoryCount"`
	OverlapCount     int                    `json:"overlapCount"`
}

type Evaluator struct {
	shelves *Service
	library storyLibrarySearcher
}

func NewEvaluator(
	shelves *Service,
	libraryQuery storyLibrarySearcher,
) (*Evaluator, error) {
	if shelves == nil {
		return nil, ErrMissingDatabase
	}
	if libraryQuery == nil {
		return nil, ErrMissingLibraryQuery
	}
	return &Evaluator{shelves: shelves, library: libraryQuery}, nil
}

func (e *Evaluator) Evaluate(
	ctx context.Context,
	shelfID int64,
	request library.ListRequest,
) (Evaluation, error) {
	opened, err := e.shelves.Open(ctx, shelfID)
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
		Shelf: opened.Shelf,
		Query: opened.Query,
		Page:  page,
	}, nil
}

func (e *Evaluator) Count(ctx context.Context, shelfID int64) (ShelfCount, error) {
	opened, err := e.shelves.Open(ctx, shelfID)
	if err != nil {
		return ShelfCount{}, err
	}
	if counter, ok := e.library.(storyLibraryCounter); ok {
		count, err := counter.Count(ctx, opened.Query.StoryLibraryQuery())
		if err != nil {
			return ShelfCount{}, err
		}
		return ShelfCount{ShelfID: shelfID, Count: count}, nil
	}
	query := opened.Query.StoryLibraryQuery()
	query.Page = 1
	query.PageSize = 1
	query.Sort = library.SortNameAscending
	page, err := e.library.Search(ctx, query)
	if err != nil {
		return ShelfCount{}, err
	}
	return ShelfCount{
		ShelfID: shelfID,
		Count:   page.TotalItems,
	}, nil
}

func (e *Evaluator) Counts(ctx context.Context) ([]ShelfCount, error) {
	shelves, err := e.shelves.List(ctx)
	if err != nil {
		return nil, err
	}
	counts := make([]ShelfCount, 0, len(shelves))
	for _, shelf := range shelves {
		count, err := e.Count(ctx, shelf.ID)
		if err != nil {
			return nil, err
		}
		counts = append(counts, count)
	}
	return counts, nil
}

func (e *Evaluator) Summaries(ctx context.Context) ([]Summary, error) {
	shelves, err := e.shelves.List(ctx)
	if err != nil {
		return nil, err
	}
	summaries := make([]Summary, 0, len(shelves))
	for _, shelf := range shelves {
		inspection, err := e.shelves.Inspect(ctx, shelf.ID)
		if err != nil {
			return nil, err
		}
		shelf = inspection.Shelf
		if shelf.Validity != ValidityValid {
			summaries = append(summaries, Summary{
				ID:              shelf.ID,
				Name:            shelf.Name,
				Position:        shelf.Position,
				Validity:        shelf.Validity,
				AttentionReason: inspection.AttentionReason,
			})
			continue
		}
		count, err := e.Count(ctx, shelf.ID)
		if err != nil {
			return nil, err
		}
		summaries = append(summaries, Summary{
			ID:       shelf.ID,
			Name:     shelf.Name,
			Position: shelf.Position,
			Validity: shelf.Validity,
			Count:    count.Count,
		})
	}
	return summaries, nil
}

func (e *Evaluator) PreviewSelection(
	ctx context.Context,
	shelfIDs []int64,
) (SelectionPreview, error) {
	if len(shelfIDs) == 0 {
		return SelectionPreview{}, ErrInvalidShelfSelection
	}
	seenShelves := make(map[int64]struct{}, len(shelfIDs))
	union := make(map[int64]struct{})
	preview := SelectionPreview{
		Shelves:          make([]SelectedShelfPreview, 0, len(shelfIDs)),
		SourceShelfNames: make([]string, 0, len(shelfIDs)),
	}
	totalMemberships := 0
	for _, shelfID := range shelfIDs {
		if shelfID <= 0 {
			return SelectionPreview{}, ErrInvalidShelfSelection
		}
		if _, duplicate := seenShelves[shelfID]; duplicate {
			return SelectionPreview{}, ErrInvalidShelfSelection
		}
		seenShelves[shelfID] = struct{}{}

		opened, err := e.shelves.Open(ctx, shelfID)
		if err != nil {
			return SelectionPreview{}, err
		}
		query := opened.Query.StoryLibraryQuery()
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
		preview.Shelves = append(preview.Shelves, SelectedShelfPreview{
			ID:    opened.Shelf.ID,
			Name:  opened.Shelf.Name,
			Count: count,
		})
		preview.SourceShelfNames = append(
			preview.SourceShelfNames,
			opened.Shelf.Name,
		)
	}
	preview.UniqueStoryCount = len(union)
	preview.OverlapCount = totalMemberships - preview.UniqueStoryCount
	return preview, nil
}
