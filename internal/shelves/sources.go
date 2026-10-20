package shelves

import (
	"context"
	"database/sql"
	"encoding/json"
)

func uniqueCollectionIDs(ids []int64) ([]int64, error) {
	result := make([]int64, 0, len(ids))
	seen := map[int64]bool{}
	for _, id := range ids {
		if id <= 0 {
			return nil, ErrInvalidInput
		}
		if !seen[id] {
			seen[id] = true
			result = append(result, id)
		}
	}
	return result, nil
}

func attachSources(ctx context.Context, tx *sql.Tx, shelfID int64, ids []int64) (bool, error) {
	ids, err := uniqueCollectionIDs(ids)
	if err != nil {
		return false, err
	}
	encoded, err := json.Marshal(ids)
	if err != nil {
		return false, err
	}
	var count int
	if err := tx.QueryRowContext(ctx, `SELECT count(*) FROM collections WHERE id IN(SELECT value FROM json_each(?))`, string(encoded)).Scan(&count); err != nil {
		return false, err
	}
	if count != len(ids) {
		return false, ErrNotFound
	}
	anyChanged := false
	for _, id := range ids {
		inserted, err := changed(tx.ExecContext(ctx, `INSERT INTO shelf_sources(shelf_id,collection_id,collection_name)
		 SELECT ?,id,name FROM collections WHERE id=? ON CONFLICT DO NOTHING`, shelfID, id))
		if err != nil {
			return false, err
		}
		anyChanged = anyChanged || inserted
	}
	return anyChanged, nil
}

func (s *Service) AttachSources(ctx context.Context, id int64, collectionIDs []int64) (Shelf, error) {
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) { return attachSources(ctx, tx, id, collectionIDs) })
}

func recordOrigins(ctx context.Context, tx *sql.Tx, id int64, storyUUID string, sources []Source) error {
	for _, source := range sources {
		if _, err := tx.ExecContext(ctx, `INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(?,?,?) ON CONFLICT DO NOTHING`, id, storyUUID, source.ID); err != nil {
			return err
		}
	}
	return nil
}
