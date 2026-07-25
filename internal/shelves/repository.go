package shelves

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

type Validity string

const (
	ValidityValid          Validity = "valid"
	ValidityNeedsAttention Validity = "needs_attention"
)

var ErrMissingDatabase = errors.New("shelf database is required")

type Shelf struct {
	ID             int64    `json:"id"`
	Name           string   `json:"name"`
	NormalizedName string   `json:"normalizedName"`
	Position       int      `json:"position"`
	QueryVersion   int      `json:"queryVersion"`
	QueryPayload   string   `json:"queryPayload"`
	Validity       Validity `json:"validity"`
}

type NewShelf struct {
	Name           string
	NormalizedName string
	Position       int
	QueryVersion   int
	QueryPayload   string
	Validity       Validity
}

type Repository struct {
	database *sql.DB
}

func NewRepository(database *sql.DB) (*Repository, error) {
	if database == nil {
		return nil, ErrMissingDatabase
	}
	return &Repository{database: database}, nil
}

func (r *Repository) Insert(
	ctx context.Context,
	input NewShelf,
) (Shelf, error) {
	return insertShelf(ctx, r.database, input)
}

func insertShelf(
	ctx context.Context,
	executor shelfExecutor,
	input NewShelf,
) (Shelf, error) {
	validity := input.Validity
	if validity == "" {
		validity = ValidityValid
	}
	result, err := executor.ExecContext(
		ctx,
		`INSERT INTO shelves (
			name,
			normalized_name,
			position,
			query_version,
			query_payload,
			validity_state
		 ) VALUES (?, ?, ?, ?, ?, ?)`,
		input.Name,
		input.NormalizedName,
		input.Position,
		input.QueryVersion,
		input.QueryPayload,
		validity,
	)
	if err != nil {
		return Shelf{}, fmt.Errorf("insert shelf: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return Shelf{}, fmt.Errorf("read shelf id: %w", err)
	}
	return Shelf{
		ID:             id,
		Name:           input.Name,
		NormalizedName: input.NormalizedName,
		Position:       input.Position,
		QueryVersion:   input.QueryVersion,
		QueryPayload:   input.QueryPayload,
		Validity:       validity,
	}, nil
}

func (r *Repository) List(ctx context.Context) ([]Shelf, error) {
	return listShelves(ctx, r.database)
}

func (r *Repository) Shelf(ctx context.Context, shelfID int64) (Shelf, error) {
	return readShelf(ctx, r.database, shelfID)
}

type shelfExecutor interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

type shelfQueryer interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func listShelves(ctx context.Context, queryer shelfQueryer) ([]Shelf, error) {
	rows, err := queryer.QueryContext(
		ctx,
		`SELECT
			id,
			name,
			normalized_name,
			position,
			query_version,
			query_payload,
			validity_state
		 FROM shelves
		 ORDER BY position, id`,
	)
	if err != nil {
		return nil, fmt.Errorf("list shelves: %w", err)
	}
	defer rows.Close()

	shelves := make([]Shelf, 0)
	for rows.Next() {
		var shelf Shelf
		if err := rows.Scan(
			&shelf.ID,
			&shelf.Name,
			&shelf.NormalizedName,
			&shelf.Position,
			&shelf.QueryVersion,
			&shelf.QueryPayload,
			&shelf.Validity,
		); err != nil {
			return nil, fmt.Errorf("scan shelf: %w", err)
		}
		shelves = append(shelves, shelf)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate shelves: %w", err)
	}
	return shelves, nil
}

func readShelf(
	ctx context.Context,
	queryer shelfQueryer,
	shelfID int64,
) (Shelf, error) {
	var shelf Shelf
	err := queryer.QueryRowContext(
		ctx,
		`SELECT
			id,
			name,
			normalized_name,
			position,
			query_version,
			query_payload,
			validity_state
		 FROM shelves
		 WHERE id = ?`,
		shelfID,
	).Scan(
		&shelf.ID,
		&shelf.Name,
		&shelf.NormalizedName,
		&shelf.Position,
		&shelf.QueryVersion,
		&shelf.QueryPayload,
		&shelf.Validity,
	)
	if err != nil {
		return Shelf{}, err
	}
	return shelf, nil
}
