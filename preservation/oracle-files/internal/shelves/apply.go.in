package shelves

import (
	"context"
	"database/sql"
)

func (s *Service) Apply(ctx context.Context, id int64, token string, decisions []Decision) (Shelf, error) {
	result, err := s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		evaluation, err := s.validatedReview(ctx, tx, id, token)
		if err != nil {
			return false, err
		}
		actions, err := validateDecisions(evaluation.Review, decisions)
		if err != nil {
			return false, err
		}
		anyChanged := false
		// Follow the preview's stable order, independently of client ordering.
		for _, candidate := range evaluation.Review.Additions {
			switch actions[candidate.UUID] {
			case "add":
				if len(candidate.Sources) == 0 {
					return false, ErrConflict
				}
				added, err := addMember(ctx, tx, id, candidate.UUID, candidate.Title, false)
				if err != nil {
					return false, err
				}
				if err := recordOrigins(ctx, tx, id, candidate.UUID, candidate.Sources); err != nil {
					return false, err
				}
				anyChanged = anyChanged || added
			case "exclude":
				excluded, err := exclude(ctx, tx, id, candidate.UUID)
				if err != nil {
					return false, err
				}
				anyChanged = anyChanged || excluded
			}
		}
		for _, candidate := range evaluation.Review.Removals {
			switch actions[candidate.UUID] {
			case "remove":
				removed, err := removeMember(ctx, tx, id, candidate.UUID)
				if err != nil {
					return false, err
				}
				anyChanged = anyChanged || removed
			case "keep":
				kept, err := changed(tx.ExecContext(ctx, `UPDATE shelf_stories SET protected=1 WHERE shelf_id=? AND story_uuid=? AND protected=0`, id, candidate.UUID))
				if err != nil {
					return false, err
				}
				anyChanged = anyChanged || kept
			}
		}
		return anyChanged, nil
	})
	if err == nil {
		s.consumeTicket(token)
	}
	return result, err
}
