package tagging

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"slices"

	"github.com/01max/librairii/internal/collectionquery"
)

var ErrSavedCollectionQueryInvalid = errors.New("saved shelf query cannot be inspected")

type shelfReferenceQueryer interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
}

type shelfReferenceExecutor interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func collectionsReferencingDefinition(
	ctx context.Context,
	queryer shelfReferenceQueryer,
	definitionID int64,
) ([]int64, error) {
	return matchingCollectionIDs(ctx, queryer, func(query collectionquery.References) bool {
		for _, filter := range query.BooleanFilters {
			if filter.DefinitionID == definitionID && filter.State != "ignored" {
				return true
			}
		}
		for _, filter := range query.ChoiceFilters {
			if filter.DefinitionID == definitionID {
				return true
			}
		}
		return false
	})
}

func collectionsReferencingValue(
	ctx context.Context,
	queryer shelfReferenceQueryer,
	valueID int64,
) ([]int64, error) {
	return matchingCollectionIDs(ctx, queryer, func(query collectionquery.References) bool {
		for _, filter := range query.ChoiceFilters {
			if slices.Contains(filter.ValueIDs, valueID) {
				return true
			}
		}
		return false
	})
}

func matchingCollectionIDs(
	ctx context.Context,
	queryer shelfReferenceQueryer,
	matches func(collectionquery.References) bool,
) ([]int64, error) {
	rows, err := queryer.QueryContext(
		ctx,
		`SELECT id, query_version, query_payload, validity_state,
            COALESCE((SELECT definition_id FROM legacy_broken_tag WHERE singleton=1), 0)
		 FROM collections
		 ORDER BY id`,
	)
	if err != nil {
		return nil, fmt.Errorf("list saved shelf references: %w", err)
	}
	defer rows.Close()

	var matched []int64
	for rows.Next() {
		var shelfID int64
		var version int
		var payload string
		var validity string
		var legacyID int64
		if err := rows.Scan(&shelfID, &version, &payload, &validity, &legacyID); err != nil {
			return nil, fmt.Errorf("scan saved shelf reference: %w", err)
		}
		query, err := collectionquery.DecodeReferences(version, payload)
		if err != nil {
			if validity == "needs_attention" {
				continue
			}
			return nil, fmt.Errorf(
				"%w: shelf %d: %v",
				ErrSavedCollectionQueryInvalid,
				shelfID,
				err,
			)
		}
		if version < 4 && legacyID > 0 {
			filters := query.BooleanFilters[:0]
			for _, filter := range query.BooleanFilters {
				if filter.DefinitionID != legacyID {
					filters = append(filters, filter)
				}
			}
			query.BooleanFilters = filters
		}
		if matches(query) {
			matched = append(matched, shelfID)
		}
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate saved shelf references: %w", err)
	}
	return matched, nil
}

func markCollectionsNeedingAttention(
	ctx context.Context,
	executor shelfReferenceExecutor,
	shelfIDs []int64,
	staleError error,
) error {
	for _, shelfID := range shelfIDs {
		result, err := executor.ExecContext(
			ctx,
			`UPDATE collections
			 SET validity_state = 'needs_attention',
			     updated_at = CURRENT_TIMESTAMP
			 WHERE id = ? AND validity_state = 'valid'`,
			shelfID,
		)
		if err != nil {
			return fmt.Errorf("mark saved shelf %d as needing attention: %w", shelfID, err)
		}
		updated, err := result.RowsAffected()
		if err != nil {
			return fmt.Errorf("read saved shelf %d update count: %w", shelfID, err)
		}
		if updated != 1 {
			var validity string
			err := executor.QueryRowContext(
				ctx,
				"SELECT validity_state FROM collections WHERE id = ?",
				shelfID,
			).Scan(&validity)
			if errors.Is(err, sql.ErrNoRows) {
				return staleError
			}
			if err != nil {
				return fmt.Errorf("read saved shelf %d validity: %w", shelfID, err)
			}
			if validity != "needs_attention" {
				return staleError
			}
		}
	}
	return nil
}

func sameCollectionReferenceIDs(current []int64, planned []int64) bool {
	return slices.Equal(current, planned)
}
