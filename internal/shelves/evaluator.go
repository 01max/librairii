package shelves

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/library"
)

var ErrMissingLibraryQuery = errors.New("story library query is required")

type storyLibrarySearcher interface {
	Search(context.Context, library.StoryLibraryQuery) (library.Page, error)
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
	evaluation, err := e.Evaluate(ctx, shelfID, library.ListRequest{
		Page:     1,
		PageSize: 1,
		Sort:     library.SortNameAscending,
	})
	if err != nil {
		return ShelfCount{}, err
	}
	return ShelfCount{
		ShelfID: shelfID,
		Count:   evaluation.Page.TotalItems,
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
