package shelves

import (
	"context"
	"database/sql"
	"encoding/json"

	"github.com/01max/librairii/internal/library"
)

type CreationRequest struct {
	Name          string   `json:"name"`
	ManualUUIDs   []string `json:"manualUuids"`
	CollectionIDs []int64  `json:"collectionIds"`
}

func potentialSources(ctx context.Context, q reader, ids []int64) ([]Source, error) {
	ids, err := uniqueCollectionIDs(ids)
	if err != nil {
		return nil, err
	}
	encoded, err := json.Marshal(ids)
	if err != nil {
		return nil, err
	}
	rows, err := q.QueryContext(ctx, `SELECT id,name FROM collections WHERE id IN (SELECT value FROM json_each(?))`, string(encoded))
	if err != nil {
		return nil, err
	}
	names := map[int64]string{}
	for rows.Next() {
		var id int64
		var name string
		if err := rows.Scan(&id, &name); err != nil {
			rows.Close()
			return nil, err
		}
		names[id] = name
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return nil, err
	}
	if len(names) != len(ids) {
		return nil, ErrNotFound
	}
	result := make([]Source, 0, len(ids))
	for _, id := range ids {
		result = append(result, Source{CollectionID: id, Name: names[id], Active: true})
	}
	return result, nil
}

func (s *Service) evaluateCreation(ctx context.Context, tx *sql.Tx, request CreationRequest) (reviewEvaluation, error) {
	name, _, err := normalizeName(request.Name)
	if err != nil {
		return reviewEvaluation{}, err
	}
	manual, err := uniqueUUIDs(request.ManualUUIDs)
	if err != nil {
		return reviewEvaluation{}, err
	}
	sources, err := potentialSources(ctx, tx, request.CollectionIDs)
	if err != nil {
		return reviewEvaluation{}, err
	}
	titles, err := activeTitles(ctx, tx, manual)
	if err != nil {
		return reviewEvaluation{}, err
	}
	if len(titles) != len(manual) {
		return reviewEvaluation{}, ErrNotFound
	}
	query := s.library.WithTransaction(tx)
	states, union, err := evaluateSources(ctx, tx, query, sources)
	if err != nil {
		return reviewEvaluation{}, err
	}
	allIDs := map[string]bool{}
	isManual := map[string]bool{}
	for _, id := range manual {
		allIDs[id] = true
		isManual[id] = true
	}
	for id := range union {
		allIDs[id] = true
	}
	stories, err := query.StoriesInScope(ctx, sortedKeys(allIDs), library.StoryLibraryQuery{})
	if err != nil {
		return reviewEvaluation{}, err
	}
	byUUID := map[string]*library.StorySummary{}
	for i := range stories {
		byUUID[stories[i].UUID] = &stories[i]
	}
	order := append([]string(nil), manual...)
	for _, story := range stories {
		if !isManual[story.UUID] {
			order = append(order, story.UUID)
		}
	}
	review := Review{Additions: make([]Candidate, 0), Removals: make([]Candidate, 0), Warnings: make([]ReviewWarning, 0)}
	for _, state := range states {
		if state.Warning != "" {
			review.Incomplete = true
			review.Warnings = append(review.Warnings, ReviewWarning{CollectionID: state.Source.CollectionID, Name: state.Source.Name, Code: state.Warning, Message: "A source needs attention. Only known matching stories are shown."})
		}
	}
	for _, id := range order {
		story := byUUID[id]
		if story == nil {
			return reviewEvaluation{}, ErrConflict
		}
		reason := "Matches a linked Collection."
		if isManual[id] {
			reason = "Selected manually."
		}
		review.Additions = append(review.Additions, Candidate{UUID: id, Title: story.Title, Story: story, Sources: union[id], Manual: isManual[id], Reason: reason})
	}
	return reviewEvaluation{Review: review, Shelf: Shelf{Name: name, SourceCount: len(sources)}, Sources: states, Available: sortedKeys(titles)}, nil
}

func (s *Service) PreviewCreation(ctx context.Context, request CreationRequest) (Review, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Review{}, err
	}
	defer tx.Rollback()
	evaluation, err := s.evaluateCreation(ctx, tx, request)
	if err != nil {
		return Review{}, err
	}
	hash, err := fingerprint(evaluation)
	if err != nil {
		return Review{}, err
	}
	if err := tx.Commit(); err != nil {
		return Review{}, err
	}
	evaluation.Review.Token = s.issueTicket("create", 0, hash, reviewArguments{Name: request.Name, CollectionIDs: request.CollectionIDs, ManualUUIDs: request.ManualUUIDs})
	return evaluation.Review, nil
}

func (s *Service) CreateFromReview(ctx context.Context, token string, selectedUUIDs []string) (Shelf, error) {
	selected, err := uniqueUUIDs(selectedUUIDs)
	if err != nil {
		return Shelf{}, err
	}
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, err
	}
	defer tx.Rollback()
	ticket, err := s.ticket(token, "create", 0)
	if err != nil {
		return Shelf{}, err
	}
	request := CreationRequest{Name: ticket.Arguments.Name, ManualUUIDs: ticket.Arguments.ManualUUIDs, CollectionIDs: ticket.Arguments.CollectionIDs}
	evaluation, err := s.evaluateCreation(ctx, tx, request)
	if err != nil {
		return Shelf{}, err
	}
	hash, err := fingerprint(evaluation)
	if err != nil {
		return Shelf{}, err
	}
	if hash != ticket.Fingerprint {
		return Shelf{}, ErrStaleReview
	}
	known := map[string]bool{}
	for _, candidate := range evaluation.Review.Additions {
		known[candidate.UUID] = true
	}
	chosen := map[string]bool{}
	for _, id := range selected {
		if !known[id] {
			return Shelf{}, ErrInvalidInput
		}
		chosen[id] = true
	}
	id, err := createShelf(ctx, tx, request.Name)
	if err != nil {
		return Shelf{}, err
	}
	if _, err := attachSources(ctx, tx, id, request.CollectionIDs); err != nil {
		return Shelf{}, err
	}
	sources, err := readSources(ctx, tx, id, true)
	if err != nil {
		return Shelf{}, err
	}
	byCollection := map[int64]Source{}
	for _, source := range sources {
		byCollection[source.CollectionID] = source
	}
	for _, candidate := range evaluation.Review.Additions {
		if !chosen[candidate.UUID] {
			continue
		}
		if _, err := addMember(ctx, tx, id, candidate.UUID, candidate.Title, candidate.Manual); err != nil {
			return Shelf{}, err
		}
		origins := make([]Source, 0, len(candidate.Sources))
		for _, source := range candidate.Sources {
			origins = append(origins, byCollection[source.CollectionID])
		}
		if err := recordOrigins(ctx, tx, id, candidate.UUID, origins); err != nil {
			return Shelf{}, err
		}
	}
	result, err := readShelf(ctx, tx, id)
	if err != nil {
		return Shelf{}, err
	}
	if err := tx.Commit(); err != nil {
		return Shelf{}, err
	}
	s.consumeTicket(token)
	return result, nil
}
