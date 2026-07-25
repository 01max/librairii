package tagging

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"slices"

	"github.com/01max/librairii/internal/shelfquery"
)

var ErrSavedShelfQueryInvalid = errors.New("saved shelf query cannot be inspected")

type shelfReferenceQueryer interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
}

type shelfReferenceExecutor interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

func shelvesReferencingDefinition(
	ctx context.Context,
	queryer shelfReferenceQueryer,
	definitionID int64,
) ([]int64, error) {
	return matchingShelfIDs(ctx, queryer, func(query shelfquery.References) bool {
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

func shelvesReferencingValue(
	ctx context.Context,
	queryer shelfReferenceQueryer,
	valueID int64,
) ([]int64, error) {
	return matchingShelfIDs(ctx, queryer, func(query shelfquery.References) bool {
		for _, filter := range query.ChoiceFilters {
			if slices.Contains(filter.ValueIDs, valueID) {
				return true
			}
		}
		return false
	})
}

func matchingShelfIDs(
	ctx context.Context,
	queryer shelfReferenceQueryer,
	matches func(shelfquery.References) bool,
) ([]int64, error) {
	rows, err := queryer.QueryContext(
		ctx,
		`SELECT id, query_version, query_payload
		 FROM shelves
		 WHERE validity_state = 'valid'
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
		if err := rows.Scan(&shelfID, &version, &payload); err != nil {
			return nil, fmt.Errorf("scan saved shelf reference: %w", err)
		}
		query, err := shelfquery.DecodeReferences(version, payload)
		if err != nil {
			return nil, fmt.Errorf(
				"%w: shelf %d: %v",
				ErrSavedShelfQueryInvalid,
				shelfID,
				err,
			)
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

func markShelvesNeedingAttention(
	ctx context.Context,
	executor shelfReferenceExecutor,
	shelfIDs []int64,
	staleError error,
) error {
	for _, shelfID := range shelfIDs {
		result, err := executor.ExecContext(
			ctx,
			`UPDATE shelves
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
			return staleError
		}
	}
	return nil
}

func sameShelfReferenceIDs(current []int64, planned []int64) bool {
	return slices.Equal(current, planned)
}
