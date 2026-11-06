package shelves

import (
	"context"
	"database/sql"
	"errors"
	"strings"
	"sync"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/searchtext"
)

var (
	ErrInvalidInput = errors.New("invalid shelf input")
	ErrNotFound     = errors.New("shelf or member no longer exists")
	ErrConflict     = errors.New("shelf state changed; refresh and try again")
)

type Shelf struct {
	ID               int64  `json:"id"`
	Name             string `json:"name"`
	Position         int    `json:"position"`
	Revision         int64  `json:"revision"`
	Count            int    `json:"count"`
	BrokenCount      int    `json:"brokenCount"`
	UnavailableCount int    `json:"unavailableCount"`
	SourceCount      int    `json:"sourceCount"`
}

type Member struct {
	UUID        string                `json:"uuid"`
	Position    int                   `json:"position"`
	Protected   bool                  `json:"protected"`
	CachedTitle string                `json:"cachedTitle"`
	Story       *library.StorySummary `json:"story,omitempty"`
	Origins     []Source              `json:"origins"`
}

type Service struct {
	database  *sql.DB
	library   *library.Query
	reviewsMu sync.Mutex
	reviews   map[string]reviewTicket
	now       func() time.Time
}

func NewService(database *sql.DB, query *library.Query) (*Service, error) {
	if database == nil || query == nil {
		return nil, ErrInvalidInput
	}
	return &Service{database: database, library: query, now: time.Now, reviews: make(map[string]reviewTicket)}, nil
}

type reader interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

const shelfSelect = `SELECT s.id,s.name,s.position,s.revision,
 (SELECT count(*) FROM shelf_stories m WHERE m.shelf_id=s.id),
 (SELECT count(*) FROM shelf_stories m JOIN stories st ON st.uuid=m.story_uuid WHERE m.shelf_id=s.id AND st.is_broken=1),
 (SELECT count(*) FROM shelf_stories m WHERE m.shelf_id=s.id AND NOT EXISTS
   (SELECT 1 FROM stories st JOIN story_archives a ON a.story_id=st.id WHERE st.uuid=m.story_uuid)),
 (SELECT count(*) FROM shelf_sources src WHERE src.shelf_id=s.id AND src.active=1)
 FROM shelves s`

func scanShelf(row interface{ Scan(...any) error }) (Shelf, error) {
	var shelf Shelf
	err := row.Scan(&shelf.ID, &shelf.Name, &shelf.Position, &shelf.Revision, &shelf.Count, &shelf.BrokenCount, &shelf.UnavailableCount, &shelf.SourceCount)
	if errors.Is(err, sql.ErrNoRows) {
		return Shelf{}, ErrNotFound
	}
	return shelf, err
}

func readShelf(ctx context.Context, q reader, id int64) (Shelf, error) {
	if id <= 0 {
		return Shelf{}, ErrInvalidInput
	}
	return scanShelf(q.QueryRowContext(ctx, shelfSelect+` WHERE s.id=?`, id))
}

func (s *Service) Get(ctx context.Context, id int64) (Shelf, error) {
	return readShelf(ctx, s.database, id)
}

func (s *Service) List(ctx context.Context) ([]Shelf, error) {
	rows, err := s.database.QueryContext(ctx, shelfSelect+` ORDER BY s.position,s.id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make([]Shelf, 0)
	for rows.Next() {
		shelf, err := scanShelf(rows)
		if err != nil {
			return nil, err
		}
		result = append(result, shelf)
	}
	return result, rows.Err()
}

func normalizeName(value string) (string, string, error) {
	if !utf8.ValidString(value) {
		return "", "", ErrInvalidInput
	}
	for _, r := range value {
		if unicode.IsControl(r) && !unicode.IsSpace(r) {
			return "", "", ErrInvalidInput
		}
	}
	name := strings.Join(strings.Fields(value), " ")
	normalized := searchtext.Normalize(name)
	if name == "" || normalized == "" || utf8.RuneCountInString(name) > 80 || utf8.RuneCountInString(normalized) > 80 {
		return "", "", ErrInvalidInput
	}
	return name, normalized, nil
}

func createShelf(ctx context.Context, tx *sql.Tx, name string) (int64, error) {
	name, normalized, err := normalizeName(name)
	if err != nil {
		return 0, err
	}
	result, err := tx.ExecContext(ctx, `INSERT INTO shelves(name,normalized_name,position) VALUES(?,?,(SELECT coalesce(max(position),-1)+1 FROM shelves))`, name, normalized)
	if err != nil {
		if strings.Contains(err.Error(), "shelves.normalized_name") {
			return 0, ErrInvalidInput
		}
		return 0, err
	}
	return result.LastInsertId()
}

func (s *Service) Create(ctx context.Context, name string) (Shelf, error) {
	return s.CreateManual(ctx, name, nil)
}

func (s *Service) mutate(ctx context.Context, id int64, fn func(*sql.Tx) (bool, error)) (Shelf, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, err
	}
	defer tx.Rollback()
	if _, err = readShelf(ctx, tx, id); err != nil {
		return Shelf{}, err
	}
	changed, err := fn(tx)
	if err != nil {
		return Shelf{}, err
	}
	if changed {
		if _, err = tx.ExecContext(ctx, `UPDATE shelves SET revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?`, id); err != nil {
			return Shelf{}, err
		}
	}
	result, err := readShelf(ctx, tx, id)
	if err != nil {
		return Shelf{}, err
	}
	return result, tx.Commit()
}

func (s *Service) Rename(ctx context.Context, id int64, name string) (Shelf, error) {
	name, normalized, err := normalizeName(name)
	if err != nil {
		return Shelf{}, err
	}
	return s.mutate(ctx, id, func(tx *sql.Tx) (bool, error) {
		result, err := tx.ExecContext(ctx, `UPDATE shelves SET name=?,normalized_name=? WHERE id=? AND name!=?`, name, normalized, id, name)
		if err != nil {
			if strings.Contains(err.Error(), "shelves.normalized_name") {
				return false, ErrInvalidInput
			}
			return false, err
		}
		n, err := result.RowsAffected()
		return n > 0, err
	})
}

func (s *Service) Delete(ctx context.Context, id int64) error {
	if id <= 0 {
		return ErrInvalidInput
	}
	result, err := s.database.ExecContext(ctx, `DELETE FROM shelves WHERE id=?`, id)
	if err != nil {
		return err
	}
	n, err := result.RowsAffected()
	if err == nil && n == 0 {
		return ErrNotFound
	}
	return err
}

func readMembers(ctx context.Context, q reader, id int64) ([]Member, error) {
	rows, err := q.QueryContext(ctx, `SELECT story_uuid,position,protected,cached_title FROM shelf_stories WHERE shelf_id=? ORDER BY position,story_uuid`, id)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	members := make([]Member, 0)
	for rows.Next() {
		var member Member
		if err := rows.Scan(&member.UUID, &member.Position, &member.Protected, &member.CachedTitle); err != nil {
			return nil, err
		}
		members = append(members, member)
	}
	return members, rows.Err()
}

func (s *Service) Members(ctx context.Context, id int64) ([]Member, error) {
	if _, err := s.Get(ctx, id); err != nil {
		return nil, err
	}
	return readMembers(ctx, s.database, id)
}

// Reorder requires the complete current identity set so stale lists cannot
// silently drop or duplicate a group.
func (s *Service) Reorder(ctx context.Context, ids []int64) error {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	rows, err := tx.QueryContext(ctx, `SELECT id FROM shelves`)
	if err != nil {
		return err
	}
	known := map[int64]bool{}
	for rows.Next() {
		var id int64
		if err := rows.Scan(&id); err != nil {
			rows.Close()
			return err
		}
		known[id] = true
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return err
	}
	if len(known) != len(ids) {
		return ErrConflict
	}
	for _, id := range ids {
		if !known[id] {
			return ErrInvalidInput
		}
		delete(known, id)
	}
	var padding int
	if err := tx.QueryRowContext(ctx, `SELECT coalesce(max(position),0)+1 FROM shelves`).Scan(&padding); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE shelves SET position=position+?`, padding); err != nil {
		return err
	}
	for i, id := range ids {
		if _, err := tx.ExecContext(ctx, `UPDATE shelves SET position=? WHERE id=?`, i, id); err != nil {
			return err
		}
	}
	return tx.Commit()
}
