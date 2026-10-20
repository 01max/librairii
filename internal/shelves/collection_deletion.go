package shelves

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
	"errors"

	"github.com/01max/librairii/internal/collections"
)

type CollectionDeletionReview struct {
	CollectionID int64                `json:"collectionId"`
	Name         string               `json:"name"`
	Token        string               `json:"token"`
	Shelves      []SourceChangeReview `json:"shelves"`
}

type ShelfDecisions struct {
	ShelfID   int64      `json:"shelfId"`
	Decisions []Decision `json:"decisions"`
}

type collectionDeletionEvaluation struct {
	Plan       CollectionDeletionReview
	Definition collections.Collection
	Shelves    []sourceChangeEvaluation
}

func (s *Service) evaluateCollectionDeletion(ctx context.Context, tx *sql.Tx, id int64) (collectionDeletionEvaluation, error) {
	definition, err := collections.ReadInTransaction(ctx, tx, id)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return collectionDeletionEvaluation{}, ErrNotFound
		}
		return collectionDeletionEvaluation{}, err
	}
	rows, err := tx.QueryContext(ctx, `SELECT DISTINCT shelf_id FROM shelf_sources WHERE collection_id=? AND active=1 ORDER BY shelf_id`, id)
	if err != nil {
		return collectionDeletionEvaluation{}, err
	}
	ids := []int64{}
	for rows.Next() {
		var id int64
		if err := rows.Scan(&id); err != nil {
			rows.Close()
			return collectionDeletionEvaluation{}, err
		}
		ids = append(ids, id)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return collectionDeletionEvaluation{}, err
	}
	evaluation := collectionDeletionEvaluation{Plan: CollectionDeletionReview{CollectionID: id, Name: definition.Name, Shelves: make([]SourceChangeReview, 0)}, Definition: definition, Shelves: make([]sourceChangeEvaluation, 0)}
	for _, shelfID := range ids {
		sources, err := readSources(ctx, tx, shelfID, true)
		if err != nil {
			return collectionDeletionEvaluation{}, err
		}
		desired := []int64{}
		for _, source := range sources {
			if source.CollectionID != id {
				desired = append(desired, source.CollectionID)
			}
		}
		change, err := s.evaluateSourceChange(ctx, tx, shelfID, desired)
		if err != nil {
			return collectionDeletionEvaluation{}, err
		}
		evaluation.Shelves = append(evaluation.Shelves, change)
		evaluation.Plan.Shelves = append(evaluation.Plan.Shelves, change.Plan)
	}
	return evaluation, nil
}

func collectionDeletionFingerprint(e collectionDeletionEvaluation) ([32]byte, error) {
	hashes := make([][32]byte, 0, len(e.Shelves))
	for _, shelf := range e.Shelves {
		hash, err := sourceChangeFingerprint(shelf)
		if err != nil {
			return [32]byte{}, err
		}
		hashes = append(hashes, hash)
	}
	data, err := json.Marshal(struct {
		Definition collections.Collection
		Shelves    [][32]byte
	}{e.Definition, hashes})
	if err != nil {
		return [32]byte{}, err
	}
	return sha256.Sum256(data), nil
}

func (s *Service) PreviewCollectionDeletion(ctx context.Context, id int64) (CollectionDeletionReview, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return CollectionDeletionReview{}, err
	}
	defer tx.Rollback()
	evaluation, err := s.evaluateCollectionDeletion(ctx, tx, id)
	if err != nil {
		return CollectionDeletionReview{}, err
	}
	hash, err := collectionDeletionFingerprint(evaluation)
	if err != nil {
		return CollectionDeletionReview{}, err
	}
	if err := tx.Commit(); err != nil {
		return CollectionDeletionReview{}, err
	}
	evaluation.Plan.Token = s.issueTicket("delete_collection", 0, hash, reviewArguments{CollectionID: id})
	return evaluation.Plan, nil
}

func (s *Service) ApplyCollectionDeletion(ctx context.Context, token string, keepAll bool, decisions []ShelfDecisions) error {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	ticket, err := s.ticket(token, "delete_collection", 0)
	if err != nil {
		return err
	}
	evaluation, err := s.evaluateCollectionDeletion(ctx, tx, ticket.Arguments.CollectionID)
	if err != nil {
		return err
	}
	hash, err := collectionDeletionFingerprint(evaluation)
	if err != nil {
		return err
	}
	if hash != ticket.Fingerprint {
		return ErrStaleReview
	}
	if keepAll && len(decisions) > 0 {
		return ErrInvalidInput
	}
	byShelf := map[int64][]Decision{}
	known := map[int64]bool{}
	for _, shelf := range evaluation.Shelves {
		known[shelf.Plan.ShelfID] = true
	}
	for _, group := range decisions {
		if _, duplicate := byShelf[group.ShelfID]; duplicate || !known[group.ShelfID] {
			return ErrInvalidInput
		}
		byShelf[group.ShelfID] = group.Decisions
	}
	actions := map[int64]map[string]string{}
	for _, shelf := range evaluation.Shelves {
		validated, err := validateSourceActions(shelf.Plan, keepAll, byShelf[shelf.Plan.ShelfID])
		if err != nil {
			return err
		}
		actions[shelf.Plan.ShelfID] = validated
	}
	for _, shelf := range evaluation.Shelves {
		changed, err := applySourceActions(ctx, tx, shelf, actions[shelf.Plan.ShelfID])
		if err != nil {
			return err
		}
		if changed {
			if _, err := tx.ExecContext(ctx, `UPDATE shelves SET revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?`, shelf.Plan.ShelfID); err != nil {
				return err
			}
		}
	}
	if _, err := tx.ExecContext(ctx, `UPDATE shelf_sources SET collection_id=NULL WHERE collection_id=? AND active=0`, evaluation.Plan.CollectionID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM collections WHERE id=?`, evaluation.Plan.CollectionID); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	s.consumeTicket(token)
	return nil
}
