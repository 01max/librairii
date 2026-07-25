package shelves

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

type criteriaQueryer interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func validateSavedCriteria(
	ctx context.Context,
	queryer criteriaQueryer,
	query SavedLibraryQuery,
) error {
	for _, filter := range query.BooleanFilters {
		if err := requireDefinitionKind(
			ctx,
			queryer,
			filter.DefinitionID,
			"boolean",
		); err != nil {
			return err
		}
	}
	for _, filter := range query.ChoiceFilters {
		if err := requireDefinitionKind(
			ctx,
			queryer,
			filter.DefinitionID,
			"choice",
		); err != nil {
			return err
		}
		for _, valueID := range filter.ValueIDs {
			var definitionID int64
			err := queryer.QueryRowContext(
				ctx,
				"SELECT definition_id FROM tag_values WHERE id = ?",
				valueID,
			).Scan(&definitionID)
			if errors.Is(err, sql.ErrNoRows) {
				return fmt.Errorf(
					"%w: choice value %d",
					ErrShelfCriteriaUnavailable,
					valueID,
				)
			}
			if err != nil {
				return fmt.Errorf("read saved shelf choice value: %w", err)
			}
			if definitionID != filter.DefinitionID {
				return fmt.Errorf(
					"%w: choice value %d",
					ErrShelfCriteriaUnavailable,
					valueID,
				)
			}
		}
	}
	return nil
}

func requireDefinitionKind(
	ctx context.Context,
	queryer criteriaQueryer,
	definitionID int64,
	expected string,
) error {
	var kind string
	err := queryer.QueryRowContext(
		ctx,
		"SELECT kind FROM tag_definitions WHERE id = ?",
		definitionID,
	).Scan(&kind)
	if errors.Is(err, sql.ErrNoRows) {
		return fmt.Errorf(
			"%w: tag definition %d",
			ErrShelfCriteriaUnavailable,
			definitionID,
		)
	}
	if err != nil {
		return fmt.Errorf("read saved shelf tag definition: %w", err)
	}
	if kind != expected {
		return fmt.Errorf(
			"%w: tag definition %d",
			ErrShelfCriteriaUnavailable,
			definitionID,
		)
	}
	return nil
}
