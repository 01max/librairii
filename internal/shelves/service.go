package shelves

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/searchtext"
)

const (
	maxShelfNameRunes   = 80
	reorderShelfPadding = 1
)

var (
	ErrInvalidShelfName    = errors.New("shelf name is invalid")
	ErrDuplicateShelfName  = errors.New("shelf name already exists")
	ErrShelfNotFound       = errors.New("shelf does not exist")
	ErrInvalidShelfOrder   = errors.New("shelf order is invalid")
	ErrShelfNeedsAttention = errors.New("shelf needs attention")
)

type OpenedShelf struct {
	Shelf Shelf             `json:"shelf"`
	Query SavedLibraryQuery `json:"query"`
}

type Service struct {
	database *sql.DB
}

func NewService(database *sql.DB) (*Service, error) {
	if database == nil {
		return nil, ErrMissingDatabase
	}
	return &Service{database: database}, nil
}

func (s *Service) Create(
	ctx context.Context,
	name string,
	query library.StoryLibraryQuery,
) (Shelf, error) {
	name, normalizedName, err := normalizeShelfName(name)
	if err != nil {
		return Shelf{}, err
	}
	serialized, err := EncodeSavedLibraryQuery(query)
	if err != nil {
		return Shelf{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, fmt.Errorf("begin shelf creation: %w", err)
	}
	defer transaction.Rollback()

	position, err := nextShelfPosition(ctx, transaction)
	if err != nil {
		return Shelf{}, err
	}
	shelf, err := insertShelf(ctx, transaction, NewShelf{
		Name:           name,
		NormalizedName: normalizedName,
		Position:       position,
		QueryVersion:   serialized.Version,
		QueryPayload:   serialized.Payload,
	})
	if isDuplicateShelfName(err) {
		return Shelf{}, ErrDuplicateShelfName
	}
	if err != nil {
		return Shelf{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Shelf{}, fmt.Errorf("commit shelf creation: %w", err)
	}
	return shelf, nil
}

func (s *Service) Open(ctx context.Context, shelfID int64) (OpenedShelf, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return OpenedShelf{}, fmt.Errorf("begin shelf open: %w", err)
	}
	defer transaction.Rollback()

	shelf, err := readShelf(ctx, transaction, shelfID)
	if errors.Is(err, sql.ErrNoRows) {
		return OpenedShelf{}, ErrShelfNotFound
	}
	if err != nil {
		return OpenedShelf{}, fmt.Errorf("read shelf: %w", err)
	}
	if shelf.Validity != ValidityValid {
		return OpenedShelf{}, ErrShelfNeedsAttention
	}
	migrated, err := MigrateSavedLibraryQuery(
		shelf.QueryVersion,
		shelf.QueryPayload,
	)
	if err != nil {
		return OpenedShelf{}, err
	}
	if shelf.QueryVersion != migrated.Version ||
		shelf.QueryPayload != migrated.Payload {
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE shelves
			 SET query_version = ?,
			     query_payload = ?,
			     updated_at = CURRENT_TIMESTAMP
			 WHERE id = ?`,
			migrated.Version,
			migrated.Payload,
			shelf.ID,
		); err != nil {
			return OpenedShelf{}, fmt.Errorf("migrate saved shelf query: %w", err)
		}
		shelf.QueryVersion = migrated.Version
		shelf.QueryPayload = migrated.Payload
	}
	query, err := DecodeSavedLibraryQuery(
		shelf.QueryVersion,
		shelf.QueryPayload,
	)
	if err != nil {
		return OpenedShelf{}, err
	}
	if err := transaction.Commit(); err != nil {
		return OpenedShelf{}, fmt.Errorf("commit shelf open: %w", err)
	}
	return OpenedShelf{Shelf: shelf, Query: query}, nil
}

func (s *Service) Rename(
	ctx context.Context,
	shelfID int64,
	name string,
) (Shelf, error) {
	name, normalizedName, err := normalizeShelfName(name)
	if err != nil {
		return Shelf{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, fmt.Errorf("begin shelf rename: %w", err)
	}
	defer transaction.Rollback()

	result, err := transaction.ExecContext(
		ctx,
		`UPDATE shelves
		 SET name = ?,
		     normalized_name = ?,
		     updated_at = CURRENT_TIMESTAMP
		 WHERE id = ?`,
		name,
		normalizedName,
		shelfID,
	)
	if isDuplicateShelfName(err) {
		return Shelf{}, ErrDuplicateShelfName
	}
	if err := requireShelfMutation(result, err); err != nil {
		return Shelf{}, err
	}
	shelf, err := readShelf(ctx, transaction, shelfID)
	if err != nil {
		return Shelf{}, fmt.Errorf("read renamed shelf: %w", err)
	}
	if err := transaction.Commit(); err != nil {
		return Shelf{}, fmt.Errorf("commit shelf rename: %w", err)
	}
	return shelf, nil
}

func (s *Service) Duplicate(
	ctx context.Context,
	shelfID int64,
	name string,
) (Shelf, error) {
	name, normalizedName, err := normalizeShelfName(name)
	if err != nil {
		return Shelf{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, fmt.Errorf("begin shelf duplication: %w", err)
	}
	defer transaction.Rollback()

	source, err := readShelf(ctx, transaction, shelfID)
	if errors.Is(err, sql.ErrNoRows) {
		return Shelf{}, ErrShelfNotFound
	}
	if err != nil {
		return Shelf{}, fmt.Errorf("read shelf to duplicate: %w", err)
	}
	if source.Validity != ValidityValid {
		return Shelf{}, ErrShelfNeedsAttention
	}
	migrated, err := MigrateSavedLibraryQuery(
		source.QueryVersion,
		source.QueryPayload,
	)
	if err != nil {
		return Shelf{}, err
	}
	position, err := nextShelfPosition(ctx, transaction)
	if err != nil {
		return Shelf{}, err
	}
	duplicate, err := insertShelf(ctx, transaction, NewShelf{
		Name:           name,
		NormalizedName: normalizedName,
		Position:       position,
		QueryVersion:   migrated.Version,
		QueryPayload:   migrated.Payload,
		Validity:       source.Validity,
	})
	if isDuplicateShelfName(err) {
		return Shelf{}, ErrDuplicateShelfName
	}
	if err != nil {
		return Shelf{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Shelf{}, fmt.Errorf("commit shelf duplication: %w", err)
	}
	return duplicate, nil
}

func (s *Service) ReplaceQuery(
	ctx context.Context,
	shelfID int64,
	query library.StoryLibraryQuery,
) (Shelf, error) {
	serialized, err := EncodeSavedLibraryQuery(query)
	if err != nil {
		return Shelf{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, fmt.Errorf("begin shelf query replacement: %w", err)
	}
	defer transaction.Rollback()

	result, err := transaction.ExecContext(
		ctx,
		`UPDATE shelves
		 SET query_version = ?,
		     query_payload = ?,
		     validity_state = 'valid',
		     updated_at = CURRENT_TIMESTAMP
		 WHERE id = ?`,
		serialized.Version,
		serialized.Payload,
		shelfID,
	)
	if err := requireShelfMutation(result, err); err != nil {
		return Shelf{}, err
	}
	shelf, err := readShelf(ctx, transaction, shelfID)
	if err != nil {
		return Shelf{}, fmt.Errorf("read updated shelf: %w", err)
	}
	if err := transaction.Commit(); err != nil {
		return Shelf{}, fmt.Errorf("commit shelf query replacement: %w", err)
	}
	return shelf, nil
}

func (s *Service) Reorder(
	ctx context.Context,
	orderedIDs []int64,
) ([]Shelf, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin shelf reorder: %w", err)
	}
	defer transaction.Rollback()

	current, err := listShelves(ctx, transaction)
	if err != nil {
		return nil, err
	}
	if !sameShelfIDs(current, orderedIDs) {
		return nil, ErrInvalidShelfOrder
	}
	if len(orderedIDs) > 0 {
		maxPosition := current[len(current)-1].Position
		offset := maxPosition + len(orderedIDs) + reorderShelfPadding
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE shelves
			 SET position = position + ?,
			     updated_at = CURRENT_TIMESTAMP`,
			offset,
		); err != nil {
			return nil, fmt.Errorf("stage shelf reorder: %w", err)
		}
		for position, shelfID := range orderedIDs {
			result, err := transaction.ExecContext(
				ctx,
				`UPDATE shelves
				 SET position = ?,
				     updated_at = CURRENT_TIMESTAMP
				 WHERE id = ?`,
				position,
				shelfID,
			)
			if err := requireShelfMutation(result, err); err != nil {
				return nil, fmt.Errorf("position shelf: %w", err)
			}
		}
	}
	reordered, err := listShelves(ctx, transaction)
	if err != nil {
		return nil, err
	}
	if err := transaction.Commit(); err != nil {
		return nil, fmt.Errorf("commit shelf reorder: %w", err)
	}
	return reordered, nil
}

func (s *Service) Delete(ctx context.Context, shelfID int64) error {
	result, err := s.database.ExecContext(
		ctx,
		"DELETE FROM shelves WHERE id = ?",
		shelfID,
	)
	return requireShelfMutation(result, err)
}

func (s *Service) List(ctx context.Context) ([]Shelf, error) {
	return listShelves(ctx, s.database)
}

func normalizeShelfName(value string) (string, string, error) {
	if !utf8.ValidString(value) {
		return "", "", ErrInvalidShelfName
	}
	for _, character := range value {
		if unicode.IsControl(character) && !unicode.IsSpace(character) {
			return "", "", ErrInvalidShelfName
		}
	}
	name := strings.Join(strings.Fields(value), " ")
	normalized := searchtext.Normalize(name)
	if name == "" ||
		normalized == "" ||
		utf8.RuneCountInString(name) > maxShelfNameRunes ||
		utf8.RuneCountInString(normalized) > maxShelfNameRunes {
		return "", "", ErrInvalidShelfName
	}
	return name, normalized, nil
}

func nextShelfPosition(ctx context.Context, queryer shelfQueryer) (int, error) {
	var position int
	if err := queryer.QueryRowContext(
		ctx,
		"SELECT COALESCE(MAX(position), -1) + 1 FROM shelves",
	).Scan(&position); err != nil {
		return 0, fmt.Errorf("read next shelf position: %w", err)
	}
	return position, nil
}

func requireShelfMutation(result sql.Result, err error) error {
	if err != nil {
		return err
	}
	affected, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if affected != 1 {
		return ErrShelfNotFound
	}
	return nil
}

func sameShelfIDs(shelves []Shelf, orderedIDs []int64) bool {
	if len(shelves) != len(orderedIDs) {
		return false
	}
	seen := make(map[int64]struct{}, len(orderedIDs))
	for _, shelf := range shelves {
		seen[shelf.ID] = struct{}{}
	}
	for _, shelfID := range orderedIDs {
		if _, exists := seen[shelfID]; !exists {
			return false
		}
		delete(seen, shelfID)
	}
	return len(seen) == 0
}
