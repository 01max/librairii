package tagging

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"sort"
	"strings"
)

const maxBulkAssignmentStories = 1_000

var (
	ErrInvalidAssignment = errors.New("tag assignment request is invalid")
	ErrStoryNotFound     = errors.New("tag assignment story does not exist")
	ErrAssignmentKind    = errors.New("tag assignment does not match definition kind")
	ErrDerivedAssignment = errors.New("derived tags cannot be assigned manually")
)

type AssignmentResult struct {
	RequestedStories   int `json:"requestedStories"`
	ChangedStories     int `json:"changedStories"`
	AssignmentsAdded   int `json:"assignmentsAdded"`
	AssignmentsRemoved int `json:"assignmentsRemoved"`
}

type ValueAssignmentState struct {
	ValueID         int64 `json:"valueId"`
	AssignedStories int   `json:"assignedStories"`
}

type DefinitionAssignmentState struct {
	DefinitionID    int64                  `json:"definitionId"`
	AssignedStories int                    `json:"assignedStories"`
	Values          []ValueAssignmentState `json:"values"`
}

type AssignmentWorkspace struct {
	Catalog          Catalog                     `json:"catalog"`
	RequestedStories int                         `json:"requestedStories"`
	States           []DefinitionAssignmentState `json:"states"`
}

func (s *Service) AssignmentWorkspace(
	ctx context.Context,
	storyIDs []int64,
) (AssignmentWorkspace, error) {
	storyIDs, err := normalizeStoryIDs(storyIDs)
	if err != nil {
		return AssignmentWorkspace{}, err
	}
	if err := requireStories(ctx, s.database, storyIDs); err != nil {
		return AssignmentWorkspace{}, err
	}
	catalog, err := s.Catalog(ctx)
	if err != nil {
		return AssignmentWorkspace{}, err
	}
	workspace := AssignmentWorkspace{
		Catalog:          catalog,
		RequestedStories: len(storyIDs),
		States:           make([]DefinitionAssignmentState, 0, len(catalog.Definitions)),
	}
	for _, definition := range catalog.Definitions {
		state, err := s.assignmentState(ctx, storyIDs, definition)
		if err != nil {
			return AssignmentWorkspace{}, err
		}
		workspace.States = append(workspace.States, state)
	}
	return workspace, nil
}

func (s *Service) assignmentState(
	ctx context.Context,
	storyIDs []int64,
	definition DefinitionWithValues,
) (DefinitionAssignmentState, error) {
	placeholders := strings.TrimSuffix(strings.Repeat("?,", len(storyIDs)), ",")
	arguments := make([]any, 0, len(storyIDs)+1)
	arguments = append(arguments, definition.ID)
	for _, storyID := range storyIDs {
		arguments = append(arguments, storyID)
	}
	state := DefinitionAssignmentState{
		DefinitionID: definition.ID,
		Values:       []ValueAssignmentState{},
	}
	if definition.Kind == KindBoolean {
		err := s.database.QueryRowContext(
			ctx,
			`SELECT COUNT(DISTINCT story_id)
			 FROM story_tag_assignments
			 WHERE definition_id = ?
			   AND value_id IS NULL
			   AND story_id IN (`+placeholders+`)`,
			arguments...,
		).Scan(&state.AssignedStories)
		if err != nil {
			return DefinitionAssignmentState{}, fmt.Errorf("count boolean assignments: %w", err)
		}
		return state, nil
	}
	rows, err := s.database.QueryContext(
		ctx,
		`SELECT value_id, COUNT(DISTINCT story_id)
		 FROM story_tag_assignments
		 WHERE definition_id = ?
		   AND value_id IS NOT NULL
		   AND story_id IN (`+placeholders+`)
		 GROUP BY value_id`,
		arguments...,
	)
	if err != nil {
		return DefinitionAssignmentState{}, fmt.Errorf("count choice assignments: %w", err)
	}
	defer rows.Close()
	counts := make(map[int64]int)
	for rows.Next() {
		var valueID int64
		var count int
		if err := rows.Scan(&valueID, &count); err != nil {
			return DefinitionAssignmentState{}, err
		}
		counts[valueID] = count
	}
	if err := rows.Err(); err != nil {
		return DefinitionAssignmentState{}, err
	}
	for _, value := range definition.Values {
		state.Values = append(state.Values, ValueAssignmentState{
			ValueID:         value.ID,
			AssignedStories: counts[value.ID],
		})
	}
	return state, nil
}

func (s *Service) SetStoryBoolean(
	ctx context.Context,
	storyID int64,
	definitionID int64,
	assigned bool,
) (AssignmentResult, error) {
	return s.SetBulkBoolean(ctx, []int64{storyID}, definitionID, assigned)
}

func (s *Service) SetBulkBoolean(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	assigned bool,
) (AssignmentResult, error) {
	storyIDs, err := normalizeStoryIDs(storyIDs)
	if err != nil {
		return AssignmentResult{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return AssignmentResult{}, fmt.Errorf("begin boolean tag assignment: %w", err)
	}
	defer transaction.Rollback()
	definition, err := readDefinition(ctx, transaction, definitionID)
	if err != nil {
		return AssignmentResult{}, err
	}
	if err := requireManualDefinition(definition, KindBoolean); err != nil {
		return AssignmentResult{}, err
	}
	if err := requireStories(ctx, transaction, storyIDs); err != nil {
		return AssignmentResult{}, err
	}

	result := AssignmentResult{RequestedStories: len(storyIDs)}
	for _, storyID := range storyIDs {
		var statementResult sql.Result
		if assigned {
			statementResult, err = transaction.ExecContext(
				ctx,
				`INSERT INTO story_tag_assignments (
					story_id, definition_id, value_id, source
				) VALUES (?, ?, NULL, 'manual')
				ON CONFLICT DO NOTHING`,
				storyID,
				definition.ID,
			)
		} else {
			statementResult, err = transaction.ExecContext(
				ctx,
				`DELETE FROM story_tag_assignments
				 WHERE story_id = ?
				   AND definition_id = ?
				   AND value_id IS NULL
				   AND source = 'manual'`,
				storyID,
				definition.ID,
			)
		}
		if err != nil {
			return AssignmentResult{}, fmt.Errorf("set boolean tag assignment: %w", err)
		}
		changed, err := statementResult.RowsAffected()
		if err != nil {
			return AssignmentResult{}, fmt.Errorf("count boolean tag assignment changes: %w", err)
		}
		if changed == 1 {
			result.ChangedStories++
			if assigned {
				result.AssignmentsAdded++
			} else {
				result.AssignmentsRemoved++
			}
		}
	}
	if err := transaction.Commit(); err != nil {
		return AssignmentResult{}, fmt.Errorf("commit boolean tag assignment: %w", err)
	}
	return result, nil
}

func (s *Service) SetStoryChoiceValues(
	ctx context.Context,
	storyID int64,
	definitionID int64,
	valueIDs []int64,
) (AssignmentResult, error) {
	return s.SetBulkChoiceValues(ctx, []int64{storyID}, definitionID, valueIDs)
}

func (s *Service) SetBulkChoiceValue(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	valueID int64,
	assigned bool,
) (AssignmentResult, error) {
	storyIDs, err := normalizeStoryIDs(storyIDs)
	if err != nil {
		return AssignmentResult{}, err
	}
	if valueID <= 0 {
		return AssignmentResult{}, ErrInvalidAssignment
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return AssignmentResult{}, fmt.Errorf("begin choice tag toggle: %w", err)
	}
	defer transaction.Rollback()
	definition, err := readDefinition(ctx, transaction, definitionID)
	if err != nil {
		return AssignmentResult{}, err
	}
	if err := requireManualDefinition(definition, KindChoice); err != nil {
		return AssignmentResult{}, err
	}
	if err := requireStories(ctx, transaction, storyIDs); err != nil {
		return AssignmentResult{}, err
	}
	if err := requireDefinitionValues(ctx, transaction, definition.ID, []int64{valueID}); err != nil {
		return AssignmentResult{}, err
	}
	result := AssignmentResult{RequestedStories: len(storyIDs)}
	for _, storyID := range storyIDs {
		var statementResult sql.Result
		if assigned {
			statementResult, err = transaction.ExecContext(
				ctx,
				`INSERT INTO story_tag_assignments (
					story_id, definition_id, value_id, source
				) VALUES (?, ?, ?, 'manual')
				ON CONFLICT DO NOTHING`,
				storyID,
				definition.ID,
				valueID,
			)
		} else {
			statementResult, err = transaction.ExecContext(
				ctx,
				`DELETE FROM story_tag_assignments
				 WHERE story_id = ?
				   AND definition_id = ?
				   AND value_id = ?
				   AND source = 'manual'`,
				storyID,
				definition.ID,
				valueID,
			)
		}
		if err != nil {
			return AssignmentResult{}, fmt.Errorf("toggle choice tag assignment: %w", err)
		}
		changed, err := statementResult.RowsAffected()
		if err != nil {
			return AssignmentResult{}, fmt.Errorf("count choice tag assignment changes: %w", err)
		}
		if changed == 1 {
			result.ChangedStories++
			if assigned {
				result.AssignmentsAdded++
			} else {
				result.AssignmentsRemoved++
			}
		}
	}
	if err := transaction.Commit(); err != nil {
		return AssignmentResult{}, fmt.Errorf("commit choice tag toggle: %w", err)
	}
	return result, nil
}

func (s *Service) SetBulkChoiceValues(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	valueIDs []int64,
) (AssignmentResult, error) {
	storyIDs, err := normalizeStoryIDs(storyIDs)
	if err != nil {
		return AssignmentResult{}, err
	}
	valueIDs, err = normalizeValueIDs(valueIDs)
	if err != nil {
		return AssignmentResult{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return AssignmentResult{}, fmt.Errorf("begin choice tag assignment: %w", err)
	}
	defer transaction.Rollback()
	definition, err := readDefinition(ctx, transaction, definitionID)
	if err != nil {
		return AssignmentResult{}, err
	}
	if err := requireManualDefinition(definition, KindChoice); err != nil {
		return AssignmentResult{}, err
	}
	if err := requireStories(ctx, transaction, storyIDs); err != nil {
		return AssignmentResult{}, err
	}
	if err := requireDefinitionValues(ctx, transaction, definition.ID, valueIDs); err != nil {
		return AssignmentResult{}, err
	}

	desired := make(map[int64]struct{}, len(valueIDs))
	for _, valueID := range valueIDs {
		desired[valueID] = struct{}{}
	}
	result := AssignmentResult{RequestedStories: len(storyIDs)}
	for _, storyID := range storyIDs {
		current, err := manualChoiceValueIDs(ctx, transaction, storyID, definition.ID)
		if err != nil {
			return AssignmentResult{}, err
		}
		changed := false
		for valueID := range current {
			if _, keep := desired[valueID]; keep {
				continue
			}
			statementResult, err := transaction.ExecContext(
				ctx,
				`DELETE FROM story_tag_assignments
				 WHERE story_id = ?
				   AND definition_id = ?
				   AND value_id = ?
				   AND source = 'manual'`,
				storyID,
				definition.ID,
				valueID,
			)
			if err := transitionOne(statementResult, err); err != nil {
				return AssignmentResult{}, fmt.Errorf("remove choice tag assignment: %w", err)
			}
			result.AssignmentsRemoved++
			changed = true
		}
		for _, valueID := range valueIDs {
			if _, exists := current[valueID]; exists {
				continue
			}
			statementResult, err := transaction.ExecContext(
				ctx,
				`INSERT INTO story_tag_assignments (
					story_id, definition_id, value_id, source
				) VALUES (?, ?, ?, 'manual')`,
				storyID,
				definition.ID,
				valueID,
			)
			if err := transitionOne(statementResult, err); err != nil {
				return AssignmentResult{}, fmt.Errorf("add choice tag assignment: %w", err)
			}
			result.AssignmentsAdded++
			changed = true
		}
		if changed {
			result.ChangedStories++
		}
	}
	if err := transaction.Commit(); err != nil {
		return AssignmentResult{}, fmt.Errorf("commit choice tag assignment: %w", err)
	}
	return result, nil
}

func requireManualDefinition(definition Definition, expectedKind Kind) error {
	if definition.Kind != expectedKind {
		return ErrAssignmentKind
	}
	if definition.Source == SourceDerived {
		return ErrDerivedAssignment
	}
	return nil
}

func normalizeStoryIDs(storyIDs []int64) ([]int64, error) {
	if len(storyIDs) == 0 || len(storyIDs) > maxBulkAssignmentStories {
		return nil, ErrInvalidAssignment
	}
	unique := make(map[int64]struct{}, len(storyIDs))
	for _, storyID := range storyIDs {
		if storyID <= 0 {
			return nil, ErrInvalidAssignment
		}
		unique[storyID] = struct{}{}
	}
	normalized := make([]int64, 0, len(unique))
	for storyID := range unique {
		normalized = append(normalized, storyID)
	}
	sort.Slice(normalized, func(i, j int) bool { return normalized[i] < normalized[j] })
	return normalized, nil
}

func normalizeValueIDs(valueIDs []int64) ([]int64, error) {
	unique := make(map[int64]struct{}, len(valueIDs))
	for _, valueID := range valueIDs {
		if valueID <= 0 {
			return nil, ErrInvalidAssignment
		}
		unique[valueID] = struct{}{}
	}
	normalized := make([]int64, 0, len(unique))
	for valueID := range unique {
		normalized = append(normalized, valueID)
	}
	sort.Slice(normalized, func(i, j int) bool { return normalized[i] < normalized[j] })
	return normalized, nil
}

func requireStories(
	ctx context.Context,
	querier rowQuerier,
	storyIDs []int64,
) error {
	for _, storyID := range storyIDs {
		var exists int
		if err := querier.QueryRowContext(
			ctx,
			"SELECT EXISTS(SELECT 1 FROM stories WHERE id = ?)",
			storyID,
		).Scan(&exists); err != nil {
			return fmt.Errorf("check tag assignment story: %w", err)
		}
		if exists != 1 {
			return ErrStoryNotFound
		}
	}
	return nil
}

func requireDefinitionValues(
	ctx context.Context,
	querier rowQuerier,
	definitionID int64,
	valueIDs []int64,
) error {
	for _, valueID := range valueIDs {
		var exists int
		if err := querier.QueryRowContext(
			ctx,
			`SELECT EXISTS(
				SELECT 1
				FROM tag_values
				WHERE id = ? AND definition_id = ?
			)`,
			valueID,
			definitionID,
		).Scan(&exists); err != nil {
			return fmt.Errorf("check tag assignment value: %w", err)
		}
		if exists != 1 {
			return ErrValueNotFound
		}
	}
	return nil
}

func manualChoiceValueIDs(
	ctx context.Context,
	querier definitionQueryer,
	storyID int64,
	definitionID int64,
) (map[int64]struct{}, error) {
	rows, err := querier.QueryContext(
		ctx,
		`SELECT value_id
		 FROM story_tag_assignments
		 WHERE story_id = ?
		   AND definition_id = ?
		   AND value_id IS NOT NULL
		   AND source = 'manual'`,
		storyID,
		definitionID,
	)
	if err != nil {
		return nil, fmt.Errorf("list choice tag assignments: %w", err)
	}
	defer rows.Close()
	values := make(map[int64]struct{})
	for rows.Next() {
		var valueID int64
		if err := rows.Scan(&valueID); err != nil {
			return nil, err
		}
		values[valueID] = struct{}{}
	}
	return values, rows.Err()
}
