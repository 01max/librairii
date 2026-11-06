package shelves

import "context"

type Source struct {
	ID           int64  `json:"id"`
	CollectionID int64  `json:"collectionId"`
	Name         string `json:"name"`
	Active       bool   `json:"active"`
}

func readSources(ctx context.Context, q reader, id int64, activeOnly bool) ([]Source, error) {
	statement := `SELECT src.id,coalesce(src.collection_id,0),CASE WHEN src.active=1 THEN coalesce(c.name,src.collection_name) ELSE src.collection_name END,src.active FROM shelf_sources src LEFT JOIN collections c ON c.id=src.collection_id WHERE src.shelf_id=?`
	if activeOnly {
		statement += ` AND src.active=1`
	}
	statement += ` ORDER BY src.id`
	rows, err := q.QueryContext(ctx, statement, id)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make([]Source, 0)
	for rows.Next() {
		var source Source
		if err := rows.Scan(&source.ID, &source.CollectionID, &source.Name, &source.Active); err != nil {
			return nil, err
		}
		result = append(result, source)
	}
	return result, rows.Err()
}

func (s *Service) Sources(ctx context.Context, id int64) ([]Source, error) {
	if _, err := s.Get(ctx, id); err != nil {
		return nil, err
	}
	return readSources(ctx, s.database, id, true)
}

func (s *Service) Duplicate(ctx context.Context, id int64, name string) (Shelf, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, err
	}
	defer tx.Rollback()
	if _, err := readShelf(ctx, tx, id); err != nil {
		return Shelf{}, err
	}
	sources, err := readSources(ctx, tx, id, false)
	if err != nil {
		return Shelf{}, err
	}
	newID, err := createShelf(ctx, tx, name)
	if err != nil {
		return Shelf{}, err
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title,added_at)
	 SELECT ?,story_uuid,position,protected,cached_title,added_at FROM shelf_stories WHERE shelf_id=?`, newID, id); err != nil {
		return Shelf{}, err
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO shelf_exclusions(shelf_id,story_uuid,created_at)
	 SELECT ?,story_uuid,created_at FROM shelf_exclusions WHERE shelf_id=?`, newID, id); err != nil {
		return Shelf{}, err
	}
	for _, source := range sources {
		result, err := tx.ExecContext(ctx, `INSERT INTO shelf_sources(shelf_id,collection_id,collection_name,active) VALUES(?,nullif(?,0),?,?)`, newID, source.CollectionID, source.Name, source.Active)
		if err != nil {
			return Shelf{}, err
		}
		newSourceID, err := result.LastInsertId()
		if err != nil {
			return Shelf{}, err
		}
		if _, err := tx.ExecContext(ctx, `INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id,accepted_at)
		 SELECT ?,story_uuid,?,accepted_at FROM shelf_story_origins WHERE shelf_id=? AND source_id=?`, newID, newSourceID, id, source.ID); err != nil {
			return Shelf{}, err
		}
	}
	result, err := readShelf(ctx, tx, newID)
	if err != nil {
		return Shelf{}, err
	}
	return result, tx.Commit()
}
