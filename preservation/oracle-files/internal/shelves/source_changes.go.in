package shelves

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
)

type SourceChangeReview struct {
	ShelfID    int64           `json:"shelfId"`
	ShelfName  string          `json:"shelfName"`
	Token      string          `json:"token"`
	Sources    []Source        `json:"sources"`
	Detached   []Source        `json:"detached"`
	Affected   []Candidate     `json:"affected"`
	Warnings   []ReviewWarning `json:"warnings"`
	Incomplete bool            `json:"incomplete"`
}

type sourceChangeEvaluation struct {
	Plan       SourceChangeReview
	Before     reviewEvaluation
	After      reviewEvaluation
	DesiredIDs []int64
}

func (s *Service) evaluateSourceChange(ctx context.Context, tx *sql.Tx, id int64, desiredIDs []int64) (sourceChangeEvaluation, error) {
	desiredIDs, err := uniqueCollectionIDs(desiredIDs)
	if err != nil {
		return sourceChangeEvaluation{}, err
	}
	before, err := s.evaluateReview(ctx, tx, id)
	if err != nil {
		return sourceChangeEvaluation{}, err
	}
	desired, err := potentialSources(ctx, tx, desiredIDs)
	if err != nil {
		return sourceChangeEvaluation{}, err
	}
	current := map[int64]Source{}
	wanted := map[int64]bool{}
	for _, source := range before.Sources {
		current[source.Source.CollectionID] = source.Source
	}
	for i, source := range desired {
		wanted[source.CollectionID] = true
		if existing, ok := current[source.CollectionID]; ok {
			desired[i] = existing
		}
	}
	after, err := s.evaluateReviewWithSources(ctx, tx, id, &desired)
	if err != nil {
		return sourceChangeEvaluation{}, err
	}
	plan := SourceChangeReview{ShelfID: id, ShelfName: before.Shelf.Name, Sources: desired, Detached: make([]Source, 0), Affected: make([]Candidate, 0), Warnings: after.Review.Warnings, Incomplete: after.Review.Incomplete}
	for _, source := range before.Sources {
		if !wanted[source.Source.CollectionID] {
			plan.Detached = append(plan.Detached, source.Source)
		}
	}
	covered := map[string]bool{}
	for _, source := range after.Sources {
		for _, storyUUID := range source.Matches {
			covered[storyUUID] = true
		}
	}
	origins, err := readOrigins(ctx, tx, id)
	if err != nil {
		return sourceChangeEvaluation{}, err
	}
	if len(plan.Detached) > 0 {
		for _, member := range before.Members {
			if member.Protected || covered[member.UUID] {
				continue
			}
			title := member.CachedTitle
			story := after.Stories[member.UUID]
			if story != nil {
				title = story.Title
			}
			reason := "No remaining Collection is known to match this member."
			if len(desired) == 0 {
				reason = "This Shelf will have no Collection sources."
			}
			plan.Affected = append(plan.Affected, Candidate{UUID: member.UUID, Title: title, Story: story, Origins: origins[member.UUID], Reason: reason})
		}
	}
	return sourceChangeEvaluation{Plan: plan, Before: before, After: after, DesiredIDs: desiredIDs}, nil
}

func sourceChangeFingerprint(e sourceChangeEvaluation) ([32]byte, error) {
	before, err := fingerprint(e.Before)
	if err != nil {
		return [32]byte{}, err
	}
	after, err := fingerprint(e.After)
	if err != nil {
		return [32]byte{}, err
	}
	type heading struct {
		UUID, Title string
		Origins     []Source
	}
	headings := make([]heading, 0, len(e.Plan.Affected))
	for _, candidate := range e.Plan.Affected {
		headings = append(headings, heading{candidate.UUID, candidate.Title, candidate.Origins})
	}
	data, err := json.Marshal(struct {
		Before, After [32]byte
		Desired       []int64
		Headings      []heading
	}{before, after, e.DesiredIDs, headings})
	if err != nil {
		return [32]byte{}, err
	}
	return sha256.Sum256(data), nil
}

func validateSourceActions(plan SourceChangeReview, keepAll bool, decisions []Decision) (map[string]string, error) {
	actions := map[string]string{}
	if keepAll {
		if len(decisions) > 0 {
			return nil, ErrInvalidInput
		}
		for _, candidate := range plan.Affected {
			actions[candidate.UUID] = "keep"
		}
		return actions, nil
	}
	known := map[string]Candidate{}
	for _, candidate := range plan.Affected {
		known[candidate.UUID] = candidate
	}
	if len(decisions) != len(known) {
		return nil, ErrInvalidInput
	}
	for _, decision := range decisions {
		candidate, exists := known[decision.UUID]
		if !exists || actions[decision.UUID] != "" || (decision.Action != "keep" && decision.Action != "remove") {
			return nil, ErrInvalidInput
		}
		if decision.Action == "remove" && (plan.Incomplete || candidate.Story == nil) {
			return nil, ErrInvalidInput
		}
		actions[decision.UUID] = decision.Action
	}
	return actions, nil
}

func applySourceActions(ctx context.Context, tx *sql.Tx, e sourceChangeEvaluation, actions map[string]string) (bool, error) {
	id := e.Plan.ShelfID
	anyChanged, err := attachSources(ctx, tx, id, e.DesiredIDs)
	if err != nil {
		return false, err
	}
	for _, source := range e.Plan.Detached {
		retired, err := changed(tx.ExecContext(ctx, `UPDATE shelf_sources SET active=0,collection_name=? WHERE id=? AND shelf_id=? AND active=1`, source.Name, source.ID, id))
		if err != nil {
			return false, err
		}
		anyChanged = anyChanged || retired
	}
	for _, candidate := range e.Plan.Affected {
		switch actions[candidate.UUID] {
		case "keep":
			kept, err := changed(tx.ExecContext(ctx, `UPDATE shelf_stories SET protected=1 WHERE shelf_id=? AND story_uuid=? AND protected=0`, id, candidate.UUID))
			if err != nil {
				return false, err
			}
			anyChanged = anyChanged || kept
		case "remove":
			removed, err := removeMember(ctx, tx, id, candidate.UUID)
			if err != nil {
				return false, err
			}
			anyChanged = anyChanged || removed
		default:
			return false, ErrInvalidInput
		}
	}
	return anyChanged, nil
}

func (s *Service) PreviewSourceChange(ctx context.Context, id int64, desiredIDs []int64) (SourceChangeReview, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return SourceChangeReview{}, err
	}
	defer tx.Rollback()
	evaluation, err := s.evaluateSourceChange(ctx, tx, id, desiredIDs)
	if err != nil {
		return SourceChangeReview{}, err
	}
	hash, err := sourceChangeFingerprint(evaluation)
	if err != nil {
		return SourceChangeReview{}, err
	}
	if err := tx.Commit(); err != nil {
		return SourceChangeReview{}, err
	}
	evaluation.Plan.Token = s.issueTicket("sources", id, hash, reviewArguments{CollectionIDs: desiredIDs})
	return evaluation.Plan, nil
}

func (s *Service) ApplySourceChange(ctx context.Context, id int64, token string, keepAll bool, decisions []Decision) (Shelf, error) {
	result, err := s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		ticket, err := s.ticket(token, "sources", id)
		if err != nil {
			return false, err
		}
		evaluation, err := s.evaluateSourceChange(ctx, tx, id, ticket.Arguments.CollectionIDs)
		if err != nil {
			return false, err
		}
		hash, err := sourceChangeFingerprint(evaluation)
		if err != nil {
			return false, err
		}
		if hash != ticket.Fingerprint {
			return false, ErrStaleReview
		}
		actions, err := validateSourceActions(evaluation.Plan, keepAll, decisions)
		if err != nil {
			return false, err
		}
		return applySourceActions(ctx, tx, evaluation, actions)
	})
	if err == nil {
		s.consumeTicket(token)
	}
	return result, err
}
