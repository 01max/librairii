package collections

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

var ErrMissingDatabase = errors.New("collection database is required")

type Collection struct {
	Revision       int64    `json:"revision"`
	ID             int64    `json:"id"`
	Name           string   `json:"name"`
	NormalizedName string   `json:"normalizedName"`
	Position       int      `json:"position"`
	QueryVersion   int      `json:"queryVersion"`
	QueryPayload   string   `json:"queryPayload"`
	Validity       Validity `json:"validity"`
}

type NewCollection struct {
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
	input NewCollection,
) (Collection, error) {
	return insertCollection(ctx, r.database, input)
}

func insertCollection(
	ctx context.Context,
	executor collectionExecutor,
	input NewCollection,
) (Collection, error) {
	validity := input.Validity
	if validity == "" {
		validity = ValidityValid
	}
	result, err := executor.ExecContext(
		ctx,
		`INSERT INTO collections (
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
		return Collection{}, fmt.Errorf("insert collection: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return Collection{}, fmt.Errorf("read collection id: %w", err)
	}
	return Collection{
		ID:             id,
		Name:           input.Name,
		NormalizedName: input.NormalizedName,
		Position:       input.Position,
		QueryVersion:   input.QueryVersion,
		QueryPayload:   input.QueryPayload,
		Validity:       validity,
		Revision:       1,
	}, nil
}

func (r *Repository) List(ctx context.Context) ([]Collection, error) {
	return listCollections(ctx, r.database)
}

func (r *Repository) Collection(ctx context.Context, collectionID int64) (Collection, error) {
	return readCollection(ctx, r.database, collectionID)
}

type collectionExecutor interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

type collectionQueryer interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func listCollections(ctx context.Context, queryer collectionQueryer) ([]Collection, error) {
	rows, err := queryer.QueryContext(
		ctx,
		`SELECT
			id,
			name,
			normalized_name,
			position,
			query_version,
			query_payload,
			validity_state,
 revision
		 FROM collections
		 ORDER BY position, id`,
	)
	if err != nil {
		return nil, fmt.Errorf("list collections: %w", err)
	}
	defer rows.Close()

	collections := make([]Collection, 0)
	for rows.Next() {
		var collection Collection
		if err := rows.Scan(
			&collection.ID,
			&collection.Name,
			&collection.NormalizedName,
			&collection.Position,
			&collection.QueryVersion,
			&collection.QueryPayload,
			&collection.Validity,
			&collection.Revision,
		); err != nil {
			return nil, fmt.Errorf("scan collection: %w", err)
		}
		collections = append(collections, collection)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate collections: %w", err)
	}
	return collections, nil
}

func readCollection(
	ctx context.Context,
	queryer collectionQueryer,
	collectionID int64,
) (Collection, error) {
	var collection Collection
	err := queryer.QueryRowContext(
		ctx,
		`SELECT
			id,
			name,
			normalized_name,
			position,
			query_version,
			query_payload,
			validity_state,
 revision
		 FROM collections
		 WHERE id = ?`,
		collectionID,
	).Scan(
		&collection.ID,
		&collection.Name,
		&collection.NormalizedName,
		&collection.Position,
		&collection.QueryVersion,
		&collection.QueryPayload,
		&collection.Validity,
		&collection.Revision,
	)
	if err != nil {
		return Collection{}, err
	}
	return collection, nil
}
