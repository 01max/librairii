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
	maxDefinitionKeyRunes   = 64
	maxDefinitionLabelRunes = 80
	reorderPositionPadding  = 1
)

var (
	ErrInvalidDefinition   = errors.New("tag definition is invalid")
	ErrDuplicateDefinition = errors.New("tag definition key already exists")
	ErrDefinitionNotFound  = errors.New("tag definition does not exist")
	ErrProtectedDefinition = errors.New("tag definition is protected")
	ErrInvalidOrder        = errors.New("tag definition order is invalid")
	ErrDeletePlanStale     = errors.New("tag definition deletion plan is stale")
)

type CreateDefinition struct {
	Key   string `json:"key"`
	Label string `json:"label"`
	Color string `json:"color"`
	Kind  Kind   `json:"kind"`
}

type DefinitionDeletionPlan struct {
	Definition         Definition `json:"definition"`
	ValueCount         int        `json:"valueCount"`
	AssignmentCount    int        `json:"assignmentCount"`
	AffectedShelfCount int        `json:"affectedShelfCount"`
	AffectedShelfIDs   []int64    `json:"affectedShelfIds"`
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

func (s *Service) CreateDefinition(
	ctx context.Context,
	input CreateDefinition,
) (Definition, error) {
	normalizedKey, err := normalizeDefinitionKey(input.Key)
	if err != nil {
		return Definition{}, err
	}
	label, err := normalizeDefinitionLabel(input.Label)
	if err != nil {
		return Definition{}, err
	}
	color, err := normalizeAccessibleColor(input.Color)
	if err != nil {
		return Definition{}, err
	}
	if input.Kind != KindBoolean && input.Kind != KindChoice {
		return Definition{}, fmt.Errorf("%w: unsupported kind", ErrInvalidDefinition)
	}
	if normalizedKey == BrokenKey {
		return Definition{}, ErrDuplicateDefinition
	}

	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Definition{}, fmt.Errorf("begin tag definition creation: %w", err)
	}
	defer transaction.Rollback()

	var position int
	if err := transaction.QueryRowContext(
		ctx,
		`SELECT COALESCE(MAX(position), -1) + 1
		 FROM tag_definitions
		 WHERE source = 'user'`,
	).Scan(&position); err != nil {
		return Definition{}, fmt.Errorf("choose tag definition position: %w", err)
	}
	result, err := transaction.ExecContext(
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
		) VALUES (?, ?, ?, ?, ?, 'user', 'default', ?, 0)`,
		normalizedKey,
		normalizedKey,
		label,
		color,
		input.Kind,
		position,
	)
	if err != nil {
		if isDefinitionKeyConflict(err) {
			return Definition{}, ErrDuplicateDefinition
		}
		return Definition{}, fmt.Errorf("insert tag definition: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return Definition{}, fmt.Errorf("read tag definition id: %w", err)
	}
	definition, err := readDefinition(ctx, transaction, id)
	if err != nil {
		return Definition{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Definition{}, fmt.Errorf("commit tag definition creation: %w", err)
	}
	return definition, nil
}

func (s *Service) RenameDefinition(
	ctx context.Context,
	definitionID int64,
	label string,
) (Definition, error) {
	label, err := normalizeDefinitionLabel(label)
	if err != nil {
		return Definition{}, err
	}
	return s.updateUserDefinition(
		ctx,
		definitionID,
		"label = ?, updated_at = CURRENT_TIMESTAMP",
		label,
	)
}

func (s *Service) RecolorDefinition(
	ctx context.Context,
	definitionID int64,
	color string,
) (Definition, error) {
	color, err := normalizeAccessibleColor(color)
	if err != nil {
		return Definition{}, err
	}
	return s.updateUserDefinition(
		ctx,
		definitionID,
		"color = ?, updated_at = CURRENT_TIMESTAMP",
		color,
	)
}

func (s *Service) ReorderDefinitions(
	ctx context.Context,
	orderedIDs []int64,
) ([]Definition, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin tag definition reorder: %w", err)
	}
	defer transaction.Rollback()

	currentIDs, maxPosition, err := userDefinitionIDs(ctx, transaction)
	if err != nil {
		return nil, err
	}
	if !sameDefinitionIDs(currentIDs, orderedIDs) {
		return nil, ErrInvalidOrder
	}
	if len(orderedIDs) > 0 {
		offset := maxPosition + len(orderedIDs) + reorderPositionPadding
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE tag_definitions
			 SET position = position + ?,
			     updated_at = CURRENT_TIMESTAMP
			 WHERE source = 'user'`,
			offset,
		); err != nil {
			return nil, fmt.Errorf("stage tag definition reorder: %w", err)
		}
		for position, definitionID := range orderedIDs {
			result, err := transaction.ExecContext(
				ctx,
				`UPDATE tag_definitions
				 SET position = ?,
				     updated_at = CURRENT_TIMESTAMP
				 WHERE id = ? AND source = 'user'`,
				position,
				definitionID,
			)
			if err := transitionOne(result, err); err != nil {
				return nil, fmt.Errorf("position tag definition: %w", err)
			}
		}
	}
	definitions, err := listDefinitions(ctx, transaction)
	if err != nil {
		return nil, err
	}
	if err := transaction.Commit(); err != nil {
		return nil, fmt.Errorf("commit tag definition reorder: %w", err)
	}
	return definitions, nil
}

func (s *Service) ListDefinitions(ctx context.Context) ([]Definition, error) {
	return listDefinitions(ctx, s.database)
}

func (s *Service) PlanDefinitionDeletion(
	ctx context.Context,
	definitionID int64,
) (DefinitionDeletionPlan, error) {
	definition, err := readDefinition(ctx, s.database, definitionID)
	if err != nil {
		return DefinitionDeletionPlan{}, err
	}
	if definition.Source != SourceUser || definition.Protected {
		return DefinitionDeletionPlan{}, ErrProtectedDefinition
	}
	plan := DefinitionDeletionPlan{Definition: definition}
	if err := s.database.QueryRowContext(
		ctx,
		`SELECT
			(SELECT COUNT(*) FROM tag_values WHERE definition_id = ?),
			(SELECT COUNT(*) FROM story_tag_assignments WHERE definition_id = ?)`,
		definitionID,
		definitionID,
	).Scan(&plan.ValueCount, &plan.AssignmentCount); err != nil {
		return DefinitionDeletionPlan{}, fmt.Errorf("plan tag definition deletion: %w", err)
	}
	plan.AffectedShelfIDs, err = shelvesReferencingDefinition(
		ctx,
		s.database,
		definitionID,
	)
	if err != nil {
		return DefinitionDeletionPlan{}, err
	}
	plan.AffectedShelfCount = len(plan.AffectedShelfIDs)
	return plan, nil
}

func (s *Service) DeleteDefinition(
	ctx context.Context,
	plan DefinitionDeletionPlan,
) error {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tag definition deletion: %w", err)
	}
	defer transaction.Rollback()

	definition, err := readDefinition(ctx, transaction, plan.Definition.ID)
	if err != nil {
		return err
	}
	if definition.Source != SourceUser || definition.Protected {
		return ErrProtectedDefinition
	}
	var valueCount int
	var assignmentCount int
	if err := transaction.QueryRowContext(
		ctx,
		`SELECT
			(SELECT COUNT(*) FROM tag_values WHERE definition_id = ?),
			(SELECT COUNT(*) FROM story_tag_assignments WHERE definition_id = ?)`,
		definition.ID,
		definition.ID,
	).Scan(&valueCount, &assignmentCount); err != nil {
		return fmt.Errorf("validate tag definition deletion: %w", err)
	}
	affectedShelfIDs, err := shelvesReferencingDefinition(
		ctx,
		transaction,
		definition.ID,
	)
	if err != nil {
		return err
	}
	if definition != plan.Definition ||
		valueCount != plan.ValueCount ||
		assignmentCount != plan.AssignmentCount ||
		len(affectedShelfIDs) != plan.AffectedShelfCount ||
		!sameShelfReferenceIDs(affectedShelfIDs, plan.AffectedShelfIDs) {
		return ErrDeletePlanStale
	}
	if err := markShelvesNeedingAttention(
		ctx,
		transaction,
		affectedShelfIDs,
		ErrDeletePlanStale,
	); err != nil {
		return err
	}
	result, err := transaction.ExecContext(
		ctx,
		"DELETE FROM tag_definitions WHERE id = ? AND source = 'user'",
		definition.ID,
	)
	if err := transitionOne(result, err); err != nil {
		return fmt.Errorf("delete tag definition: %w", err)
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit tag definition deletion: %w", err)
	}
	return nil
}

func (s *Service) updateUserDefinition(
	ctx context.Context,
	definitionID int64,
	setClause string,
	value string,
) (Definition, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Definition{}, fmt.Errorf("begin tag definition update: %w", err)
	}
	defer transaction.Rollback()
	result, err := transaction.ExecContext(
		ctx,
		"UPDATE tag_definitions SET "+setClause+" WHERE id = ? AND source = 'user'",
		value,
		definitionID,
	)
	if err != nil {
		return Definition{}, fmt.Errorf("update tag definition: %w", err)
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return Definition{}, fmt.Errorf("read tag definition update count: %w", err)
	}
	if rows != 1 {
		return Definition{}, classifyDefinitionMutation(ctx, transaction, definitionID)
	}
	definition, err := readDefinition(ctx, transaction, definitionID)
	if err != nil {
		return Definition{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Definition{}, fmt.Errorf("commit tag definition update: %w", err)
	}
	return definition, nil
}

type rowQuerier interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

type definitionQueryer interface {
	rowQuerier
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
}

func readDefinition(
	ctx context.Context,
	querier rowQuerier,
	definitionID int64,
) (Definition, error) {
	var definition Definition
	var protected int
	err := querier.QueryRowContext(
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
		 WHERE id = ?`,
		definitionID,
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
	if errors.Is(err, sql.ErrNoRows) {
		return Definition{}, ErrDefinitionNotFound
	}
	if err != nil {
		return Definition{}, fmt.Errorf("read tag definition: %w", err)
	}
	definition.Protected = protected == 1
	return definition, nil
}

func listDefinitions(
	ctx context.Context,
	querier definitionQueryer,
) ([]Definition, error) {
	rows, err := querier.QueryContext(
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
		 ORDER BY CASE source
			WHEN 'builtin' THEN 0
			WHEN 'user' THEN 1
			ELSE 2
		 END, position, id`,
	)
	if err != nil {
		return nil, fmt.Errorf("list tag definitions: %w", err)
	}
	defer rows.Close()
	var definitions []Definition
	for rows.Next() {
		var definition Definition
		var protected int
		if err := rows.Scan(
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
		); err != nil {
			return nil, err
		}
		definition.Protected = protected == 1
		definitions = append(definitions, definition)
	}
	return definitions, rows.Err()
}

func userDefinitionIDs(
	ctx context.Context,
	querier definitionQueryer,
) ([]int64, int, error) {
	rows, err := querier.QueryContext(
		ctx,
		`SELECT id, position
		 FROM tag_definitions
		 WHERE source = 'user'
		 ORDER BY position, id`,
	)
	if err != nil {
		return nil, 0, fmt.Errorf("list user tag definition positions: %w", err)
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

func sameDefinitionIDs(current []int64, ordered []int64) bool {
	if len(current) != len(ordered) {
		return false
	}
	seen := make(map[int64]struct{}, len(ordered))
	for _, id := range ordered {
		if _, duplicate := seen[id]; duplicate {
			return false
		}
		seen[id] = struct{}{}
	}
	for _, id := range current {
		if _, ok := seen[id]; !ok {
			return false
		}
	}
	return true
}

func classifyDefinitionMutation(
	ctx context.Context,
	querier rowQuerier,
	definitionID int64,
) error {
	definition, err := readDefinition(ctx, querier, definitionID)
	if err != nil {
		return err
	}
	if definition.Source != SourceUser || definition.Protected {
		return ErrProtectedDefinition
	}
	return ErrDefinitionNotFound
}

func normalizeDefinitionKey(value string) (string, error) {
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
	if normalized == "" || utf8.RuneCountInString(normalized) > maxDefinitionKeyRunes {
		return "", fmt.Errorf("%w: key", ErrInvalidDefinition)
	}
	return normalized, nil
}

func normalizeDefinitionLabel(value string) (string, error) {
	value = strings.TrimSpace(value)
	if value == "" || utf8.RuneCountInString(value) > maxDefinitionLabelRunes {
		return "", fmt.Errorf("%w: label", ErrInvalidDefinition)
	}
	return value, nil
}

func isDefinitionKeyConflict(err error) bool {
	return isUniqueConstraint(err)
}

func transitionOne(result sql.Result, err error) error {
	if err != nil {
		return err
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if rows != 1 {
		return ErrDefinitionNotFound
	}
	return nil
}
