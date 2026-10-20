package shelves

import (
	"context"
	"database/sql"
	"encoding/json"

	"github.com/01max/librairii/internal/library"
	"github.com/google/uuid"
)

func uniqueUUIDs(values []string) ([]string, error) {
	seen := map[string]bool{}
	result := make([]string, 0, len(values))
	for _, value := range values {
		parsed, err := uuid.Parse(value)
		if err != nil || parsed.String() != value {
			return nil, ErrInvalidInput
		}
		if !seen[value] {
			seen[value] = true
			result = append(result, value)
		}
	}
	return result, nil
}

func activeTitles(ctx context.Context, q reader, uuids []string) (map[string]string, error) {
	encoded, err := json.Marshal(uuids)
	if err != nil {
		return nil, err
	}
	rows, err := q.QueryContext(ctx, `SELECT s.uuid,coalesce(nullif(s.embedded_title,''),s.uuid)
	 FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.uuid IN (SELECT value FROM json_each(?))`, string(encoded))
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := map[string]string{}
	for rows.Next() {
		var id, title string
		if err := rows.Scan(&id, &title); err != nil {
			return nil, err
		}
		result[id] = title
	}
	return result, rows.Err()
}

func (s *Service) manualTitles(ctx context.Context, tx *sql.Tx, uuids []string) (map[string]string, error) {
	stories, err := s.library.WithTransaction(tx).StoriesInScope(ctx, uuids, library.StoryLibraryQuery{})
	if err != nil {
		return nil, err
	}
	titles := make(map[string]string, len(stories))
	for _, story := range stories {
		titles[story.UUID] = story.Title
	}
	return titles, nil
}

func changed(result sql.Result, err error) (bool, error) {
	if err != nil {
		return false, err
	}
	n, err := result.RowsAffected()
	return n > 0, err
}

func addMember(ctx context.Context, tx *sql.Tx, id int64, storyUUID, title string, protected bool) (bool, error) {
	// Promoting a source member to manual protection never demotes an existing
	// protected member when the same story is accepted through another source.
	result, err := tx.ExecContext(ctx, `INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title)
	 VALUES(?,?,(SELECT coalesce(max(position),-1)+1 FROM shelf_stories WHERE shelf_id=?),?,?)
	 ON CONFLICT(shelf_id,story_uuid) DO UPDATE SET protected=1
	 WHERE excluded.protected=1 AND shelf_stories.protected=0`, id, storyUUID, id, protected, title)
	added, err := changed(result, err)
	if err != nil {
		return false, err
	}
	cleared, err := changed(tx.ExecContext(ctx, `DELETE FROM shelf_exclusions WHERE shelf_id=? AND story_uuid=?`, id, storyUUID))
	return added || cleared, err
}

func exclude(ctx context.Context, tx *sql.Tx, id int64, storyUUID string) (bool, error) {
	return changed(tx.ExecContext(ctx, `INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?) ON CONFLICT DO NOTHING`, id, storyUUID))
}

func removeMember(ctx context.Context, tx *sql.Tx, id int64, storyUUID string) (bool, error) {
	removed, err := changed(tx.ExecContext(ctx, `DELETE FROM shelf_stories WHERE shelf_id=? AND story_uuid=?`, id, storyUUID))
	if err != nil {
		return false, err
	}
	excluded, err := exclude(ctx, tx, id, storyUUID)
	return removed || excluded, err
}

func (s *Service) AddStories(ctx context.Context, id int64, values []string) (Shelf, error) {
	uuids, err := uniqueUUIDs(values)
	if err != nil {
		return Shelf{}, err
	}
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		titles, err := s.manualTitles(ctx, tx, uuids)
		if err != nil {
			return false, err
		}
		if len(titles) != len(uuids) {
			return false, ErrNotFound
		}
		anyChanged := false
		for _, storyUUID := range uuids {
			added, err := addMember(ctx, tx, id, storyUUID, titles[storyUUID], true)
			if err != nil {
				return false, err
			}
			anyChanged = anyChanged || added
		}
		return anyChanged, nil
	})
}

func (s *Service) RemoveStories(ctx context.Context, id int64, values []string) (Shelf, error) {
	uuids, err := uniqueUUIDs(values)
	if err != nil {
		return Shelf{}, err
	}
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		anyChanged := false
		for _, storyUUID := range uuids {
			var known int
			if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM shelf_stories WHERE shelf_id=? AND story_uuid=?) OR EXISTS(SELECT 1 FROM shelf_exclusions WHERE shelf_id=? AND story_uuid=?)`, id, storyUUID, id, storyUUID).Scan(&known); err != nil {
				return false, err
			}
			if known == 0 {
				return false, ErrNotFound
			}
			removed, err := removeMember(ctx, tx, id, storyUUID)
			if err != nil {
				return false, err
			}
			anyChanged = anyChanged || removed
		}
		return anyChanged, nil
	})
}

func (s *Service) SetProtected(ctx context.Context, id int64, storyUUID string, protected bool) (Shelf, error) {
	if _, err := uniqueUUIDs([]string{storyUUID}); err != nil {
		return Shelf{}, err
	}
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		var current bool
		if err := tx.QueryRowContext(ctx, `SELECT protected FROM shelf_stories WHERE shelf_id=? AND story_uuid=?`, id, storyUUID).Scan(&current); err != nil {
			if err == sql.ErrNoRows {
				return false, ErrNotFound
			}
			return false, err
		}
		if current == protected {
			return false, nil
		}
		if !protected {
			var sources int
			if err := tx.QueryRowContext(ctx, `SELECT count(*) FROM shelf_sources WHERE shelf_id=? AND active=1`, id).Scan(&sources); err != nil {
				return false, err
			}
			if sources == 0 {
				return false, ErrInvalidInput
			}
		}
		return changed(tx.ExecContext(ctx, `UPDATE shelf_stories SET protected=? WHERE shelf_id=? AND story_uuid=?`, protected, id, storyUUID))
	})
}

type Exclusion struct {
	UUID  string `json:"uuid"`
	Title string `json:"title"`
}

func readExclusions(ctx context.Context, q reader, id int64) ([]Exclusion, error) {
	rows, err := q.QueryContext(ctx, `SELECT e.story_uuid,coalesce(nullif(s.embedded_title,''),e.story_uuid) FROM shelf_exclusions e LEFT JOIN stories s ON s.uuid=e.story_uuid WHERE shelf_id=? ORDER BY e.story_uuid`, id)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make([]Exclusion, 0)
	for rows.Next() {
		var item Exclusion
		if err := rows.Scan(&item.UUID, &item.Title); err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	return result, rows.Err()
}

func (s *Service) Exclusions(ctx context.Context, id int64) ([]Exclusion, error) {
	if _, err := s.Get(ctx, id); err != nil {
		return nil, err
	}
	return readExclusions(ctx, s.database, id)
}

func (s *Service) ClearExclusion(ctx context.Context, id int64, storyUUID string) (Shelf, error) {
	if _, err := uniqueUUIDs([]string{storyUUID}); err != nil {
		return Shelf{}, err
	}
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		return changed(tx.ExecContext(ctx, `DELETE FROM shelf_exclusions WHERE shelf_id=? AND story_uuid=?`, id, storyUUID))
	})
}

func (s *Service) ReorderMembers(ctx context.Context, id int64, values []string) (Shelf, error) {
	uuids, err := uniqueUUIDs(values)
	if err != nil || len(uuids) != len(values) {
		return Shelf{}, ErrInvalidInput
	}
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		members, err := readMembers(ctx, tx, id)
		if err != nil {
			return false, err
		}
		if len(members) != len(uuids) {
			return false, ErrConflict
		}
		known := map[string]bool{}
		same := true
		maxPosition := 0
		for i, m := range members {
			known[m.UUID] = true
			same = same && m.UUID == uuids[i]
			if m.Position > maxPosition {
				maxPosition = m.Position
			}
		}
		for _, storyUUID := range uuids {
			if !known[storyUUID] {
				return false, ErrConflict
			}
		}
		if same {
			return false, nil
		}
		if _, err := tx.ExecContext(ctx, `UPDATE shelf_stories SET position=position+? WHERE shelf_id=?`, maxPosition+1, id); err != nil {
			return false, err
		}
		for i, storyUUID := range uuids {
			if _, err := tx.ExecContext(ctx, `UPDATE shelf_stories SET position=? WHERE shelf_id=? AND story_uuid=?`, i, id, storyUUID); err != nil {
				return false, err
			}
		}
		return true, nil
	})
}
