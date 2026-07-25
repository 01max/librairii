package tagging

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

const (
	BrokenKey      = "broken"
	BrokenLabel    = "Broken"
	BrokenColor    = "#FF705C"
	BrokenPosition = 0
)

type Kind string

const (
	KindBoolean Kind = "boolean"
	KindChoice  Kind = "choice"
)

type Source string

const (
	SourceUser    Source = "user"
	SourceBuiltIn Source = "builtin"
	SourceDerived Source = "derived"
)

type Presentation string

const (
	PresentationDefault Presentation = "default"
	PresentationWarning Presentation = "warning"
	PresentationSystem  Presentation = "system"
)

var (
	ErrBuiltInDrift    = errors.New("built-in tag identity drifted")
	ErrMissingDatabase = errors.New("tag database is required")
)

type Definition struct {
	ID            int64
	Key           string
	NormalizedKey string
	Label         string
	Color         string
	Kind          Kind
	Source        Source
	Presentation  Presentation
	Position      int
	Protected     bool
}

func SeedBuiltIns(ctx context.Context, database *sql.DB) (Definition, error) {
	if database == nil {
		return Definition{}, ErrMissingDatabase
	}
	transaction, err := database.BeginTx(ctx, nil)
	if err != nil {
		return Definition{}, fmt.Errorf("begin built-in tag seed: %w", err)
	}
	defer transaction.Rollback()

	if _, err := transaction.ExecContext(
		ctx,
		`INSERT INTO tag_definitions (
			key,
			normalized_key,
			label,
			color,
			kind,
			source,
			presentation,
			position,
			is_protected
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)
		ON CONFLICT(normalized_key) DO NOTHING`,
		BrokenKey,
		BrokenKey,
		BrokenLabel,
		BrokenColor,
		KindBoolean,
		SourceBuiltIn,
		PresentationWarning,
		BrokenPosition,
	); err != nil {
		return Definition{}, fmt.Errorf("seed broken tag: %w", err)
	}

	var definition Definition
	var protected int
	err = transaction.QueryRowContext(
		ctx,
		`SELECT
			id,
			key,
			normalized_key,
			label,
			color,
			kind,
			source,
			presentation,
			position,
			is_protected
		 FROM tag_definitions
		 WHERE normalized_key = ? COLLATE NOCASE`,
		BrokenKey,
	).Scan(
		&definition.ID,
		&definition.Key,
		&definition.NormalizedKey,
		&definition.Label,
		&definition.Color,
		&definition.Kind,
		&definition.Source,
		&definition.Presentation,
		&definition.Position,
		&protected,
	)
	if err != nil {
		return Definition{}, fmt.Errorf("read broken tag: %w", err)
	}
	definition.Protected = protected == 1
	if !definition.canonicalBroken() {
		return Definition{}, ErrBuiltInDrift
	}

	var values int
	if err := transaction.QueryRowContext(
		ctx,
		"SELECT COUNT(*) FROM tag_values WHERE definition_id = ?",
		definition.ID,
	).Scan(&values); err != nil {
		return Definition{}, fmt.Errorf("count broken tag values: %w", err)
	}
	if values != 0 {
		return Definition{}, ErrBuiltInDrift
	}
	if err := transaction.Commit(); err != nil {
		return Definition{}, fmt.Errorf("commit built-in tag seed: %w", err)
	}
	return definition, nil
}

func (d Definition) canonicalBroken() bool {
	return d.Key == BrokenKey &&
		d.NormalizedKey == BrokenKey &&
		d.Label == BrokenLabel &&
		d.Color == BrokenColor &&
		d.Kind == KindBoolean &&
		d.Source == SourceBuiltIn &&
		d.Presentation == PresentationWarning &&
		d.Position == BrokenPosition &&
		d.Protected
}
