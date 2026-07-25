package tagging

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/unicode/norm"
)

const (
	maxValueKeyRunes   = 64
	maxValueLabelRunes = 80
)

var (
	ErrInvalidValue         = errors.New("tag value is invalid")
	ErrDuplicateValue       = errors.New("tag value key already exists")
	ErrValueNotFound        = errors.New("tag value does not exist")
	ErrValuesNotAllowed     = errors.New("tag definition does not accept values")
	ErrInvalidValueOrder    = errors.New("tag value order is invalid")
	ErrValueDeletePlanStale = errors.New("tag value deletion plan is stale")
)

type Value struct {
	ID            int64
	DefinitionID  int64
	Key           string
	NormalizedKey string
	Label         string
	Position      int
}

type CreateValue struct {
	DefinitionID int64
	Key          string
	Label        string
}

type ValueDeletionPlan struct {
	Value              Value
	AssignmentCount    int
	AffectedShelfCount int
}

func (s *Service) CreateValue(
	ctx context.Context,
	input CreateValue,
) (Value, error) {
	key, err := normalizeValueKey(input.Key)
	if err != nil {
		return Value{}, err
	}
	label, err := normalizeValueLabel(input.Label)
	if err != nil {
		return Value{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Value{}, fmt.Errorf("begin tag value creation: %w", err)
	}
	defer transaction.Rollback()

	definition, err := readDefinition(ctx, transaction, input.DefinitionID)
	if err != nil {
		return Value{}, err
	}
	if err := requireUserChoiceDefinition(definition); err != nil {
		return Value{}, err
	}
	var position int
	if err := transaction.QueryRowContext(
		ctx,
		`SELECT COALESCE(MAX(position), -1) + 1
		 FROM tag_values
		 WHERE definition_id = ?`,
		definition.ID,
	).Scan(&position); err != nil {
		return Value{}, fmt.Errorf("choose tag value position: %w", err)
	}
	result, err := transaction.ExecContext(
		ctx,
		`INSERT INTO tag_values (
			definition_id,
			key,
			normalized_key,
			label,
			position
		) VALUES (?, ?, ?, ?, ?)`,
		definition.ID,
		key,
		key,
		label,
		position,
	)
	if err != nil {
		if isValueKeyConflict(err) {
			return Value{}, ErrDuplicateValue
		}
		return Value{}, fmt.Errorf("insert tag value: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return Value{}, fmt.Errorf("read tag value id: %w", err)
	}
	value, err := readValue(ctx, transaction, id)
	if err != nil {
		return Value{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Value{}, fmt.Errorf("commit tag value creation: %w", err)
	}
	return value, nil
}

func (s *Service) RenameValue(
	ctx context.Context,
	valueID int64,
	label string,
) (Value, error) {
	label, err := normalizeValueLabel(label)
	if err != nil {
		return Value{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Value{}, fmt.Errorf("begin tag value rename: %w", err)
	}
	defer transaction.Rollback()
	value, err := readValue(ctx, transaction, valueID)
	if err != nil {
		return Value{}, err
	}
	definition, err := readDefinition(ctx, transaction, value.DefinitionID)
	if err != nil {
		return Value{}, err
	}
	if err := requireUserChoiceDefinition(definition); err != nil {
		return Value{}, err
	}
	result, err := transaction.ExecContext(
		ctx,
		`UPDATE tag_values
		 SET label = ?, updated_at = CURRENT_TIMESTAMP
		 WHERE id = ?`,
		label,
		value.ID,
	)
	if err := transitionOne(result, err); err != nil {
		return Value{}, fmt.Errorf("rename tag value: %w", err)
	}
	value, err = readValue(ctx, transaction, value.ID)
	if err != nil {
		return Value{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Value{}, fmt.Errorf("commit tag value rename: %w", err)
	}
	return value, nil
}

func (s *Service) ReorderValues(
	ctx context.Context,
	definitionID int64,
	orderedIDs []int64,
) ([]Value, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin tag value reorder: %w", err)
	}
	defer transaction.Rollback()
	definition, err := readDefinition(ctx, transaction, definitionID)
	if err != nil {
		return nil, err
	}
	if err := requireUserChoiceDefinition(definition); err != nil {
		return nil, err
	}
	currentIDs, maxPosition, err := valueIDs(ctx, transaction, definition.ID)
	if err != nil {
		return nil, err
	}
	if !sameDefinitionIDs(currentIDs, orderedIDs) {
		return nil, ErrInvalidValueOrder
	}
	if len(orderedIDs) > 0 {
		offset := maxPosition + len(orderedIDs) + reorderPositionPadding
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE tag_values
			 SET position = position + ?,
			     updated_at = CURRENT_TIMESTAMP
			 WHERE definition_id = ?`,
			offset,
			definition.ID,
		); err != nil {
			return nil, fmt.Errorf("stage tag value reorder: %w", err)
		}
		for position, valueID := range orderedIDs {
			result, err := transaction.ExecContext(
				ctx,
				`UPDATE tag_values
				 SET position = ?,
				     updated_at = CURRENT_TIMESTAMP
				 WHERE id = ? AND definition_id = ?`,
				position,
				valueID,
				definition.ID,
			)
			if err := transitionOne(result, err); err != nil {
				return nil, fmt.Errorf("position tag value: %w", err)
			}
		}
	}
	values, err := listValues(ctx, transaction, definition.ID)
	if err != nil {
		return nil, err
	}
	if err := transaction.Commit(); err != nil {
		return nil, fmt.Errorf("commit tag value reorder: %w", err)
	}
	return values, nil
}

func (s *Service) ListValues(
	ctx context.Context,
	definitionID int64,
) ([]Value, error) {
	if _, err := readDefinition(ctx, s.database, definitionID); err != nil {
		return nil, err
	}
	return listValues(ctx, s.database, definitionID)
}

func (s *Service) PlanValueDeletion(
	ctx context.Context,
	valueID int64,
) (ValueDeletionPlan, error) {
	value, err := readValue(ctx, s.database, valueID)
	if err != nil {
		return ValueDeletionPlan{}, err
	}
	definition, err := readDefinition(ctx, s.database, value.DefinitionID)
	if err != nil {
		return ValueDeletionPlan{}, err
	}
	if err := requireUserChoiceDefinition(definition); err != nil {
		return ValueDeletionPlan{}, err
	}
	plan := ValueDeletionPlan{Value: value}
	if err := s.database.QueryRowContext(
		ctx,
		"SELECT COUNT(*) FROM story_tag_assignments WHERE value_id = ?",
		value.ID,
	).Scan(&plan.AssignmentCount); err != nil {
		return ValueDeletionPlan{}, fmt.Errorf("plan tag value deletion: %w", err)
	}
	return plan, nil
}

func (s *Service) DeleteValue(ctx context.Context, plan ValueDeletionPlan) error {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tag value deletion: %w", err)
	}
	defer transaction.Rollback()
	value, err := readValue(ctx, transaction, plan.Value.ID)
	if err != nil {
		return err
	}
	definition, err := readDefinition(ctx, transaction, value.DefinitionID)
	if err != nil {
		return err
	}
	if err := requireUserChoiceDefinition(definition); err != nil {
		return err
	}
	var assignmentCount int
	if err := transaction.QueryRowContext(
		ctx,
		"SELECT COUNT(*) FROM story_tag_assignments WHERE value_id = ?",
		value.ID,
	).Scan(&assignmentCount); err != nil {
		return fmt.Errorf("validate tag value deletion: %w", err)
	}
	if value != plan.Value || assignmentCount != plan.AssignmentCount {
		return ErrValueDeletePlanStale
	}
	result, err := transaction.ExecContext(
		ctx,
		"DELETE FROM tag_values WHERE id = ?",
		value.ID,
	)
	if err := transitionOne(result, err); err != nil {
		return fmt.Errorf("delete tag value: %w", err)
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit tag value deletion: %w", err)
	}
	return nil
}

func requireUserChoiceDefinition(definition Definition) error {
	if definition.Kind != KindChoice {
		return ErrValuesNotAllowed
	}
	if definition.Source != SourceUser || definition.Protected {
		return ErrProtectedDefinition
	}
	return nil
}

func readValue(ctx context.Context, querier rowQuerier, valueID int64) (Value, error) {
	var value Value
	err := querier.QueryRowContext(
		ctx,
		`SELECT
			id,
			definition_id,
			key,
			normalized_key,
			label,
			position
		 FROM tag_values
		 WHERE id = ?`,
		valueID,
	).Scan(
		&value.ID,
		&value.DefinitionID,
		&value.Key,
		&value.NormalizedKey,
		&value.Label,
		&value.Position,
	)
	if errors.Is(err, sql.ErrNoRows) {
		return Value{}, ErrValueNotFound
	}
	if err != nil {
		return Value{}, fmt.Errorf("read tag value: %w", err)
	}
	return value, nil
}

func listValues(
	ctx context.Context,
	querier definitionQueryer,
	definitionID int64,
) ([]Value, error) {
	rows, err := querier.QueryContext(
		ctx,
		`SELECT
			id,
			definition_id,
			key,
			normalized_key,
			label,
			position
		 FROM tag_values
		 WHERE definition_id = ?
		 ORDER BY position, id`,
		definitionID,
	)
	if err != nil {
		return nil, fmt.Errorf("list tag values: %w", err)
	}
	defer rows.Close()
	var values []Value
	for rows.Next() {
		var value Value
		if err := rows.Scan(
			&value.ID,
			&value.DefinitionID,
			&value.Key,
			&value.NormalizedKey,
			&value.Label,
			&value.Position,
		); err != nil {
			return nil, err
		}
		values = append(values, value)
	}
	return values, rows.Err()
}

func valueIDs(
	ctx context.Context,
	querier definitionQueryer,
	definitionID int64,
) ([]int64, int, error) {
	rows, err := querier.QueryContext(
		ctx,
		`SELECT id, position
		 FROM tag_values
		 WHERE definition_id = ?
		 ORDER BY position, id`,
		definitionID,
	)
	if err != nil {
		return nil, 0, fmt.Errorf("list tag value positions: %w", err)
	}
	defer rows.Close()
	var ids []int64
	maxPosition := -1
	for rows.Next() {
		var id int64
		var position int
		if err := rows.Scan(&id, &position); err != nil {
			return nil, 0, err
		}
		ids = append(ids, id)
		if position > maxPosition {
			maxPosition = position
		}
	}
	return ids, maxPosition, rows.Err()
}

func normalizeValueKey(value string) (string, error) {
	value = strings.TrimSpace(value)
	var builder strings.Builder
	separator := false
	for _, character := range norm.NFKD.String(strings.ToLower(value)) {
		switch {
		case unicode.Is(unicode.Mn, character):
			continue
		case unicode.IsLetter(character) || unicode.IsDigit(character):
			if separator && builder.Len() > 0 {
				builder.WriteByte('-')
			}
			separator = false
			builder.WriteRune(character)
		default:
			separator = true
		}
	}
	normalized := builder.String()
	if normalized == "" || utf8.RuneCountInString(normalized) > maxValueKeyRunes {
		return "", fmt.Errorf("%w: key", ErrInvalidValue)
	}
	return normalized, nil
}

func normalizeValueLabel(value string) (string, error) {
	value = strings.TrimSpace(value)
	if value == "" || utf8.RuneCountInString(value) > maxValueLabelRunes {
		return "", fmt.Errorf("%w: label", ErrInvalidValue)
	}
	return value, nil
}

func isValueKeyConflict(err error) bool {
	return strings.Contains(
		err.Error(),
		"tag_values.definition_id, tag_values.normalized_key",
	)
}
