package library

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"sort"
	"strings"

	"github.com/01max/librairii/internal/searchtext"
	"golang.org/x/text/language"
)

const (
	maxNameSearchRunes     = 200
	maxFilterGroups        = 50
	maxValuesPerFilter     = 100
	resolvedDisplayNameSQL = `COALESCE(
		NULLIF((
			SELECT official_name.title_normalized
			FROM official_story_metadata AS official_name
			JOIN catalog_snapshots AS official_snapshot
			  ON official_snapshot.id = official_name.snapshot_id
			WHERE official_name.story_uuid = s.uuid
			  AND official_snapshot.status = 'active'
			ORDER BY official_snapshot.activated_at DESC,
			         official_snapshot.id DESC
			LIMIT 1
		), ''),
		s.display_name_normalized
	)`
)

var ErrInvalidStoryLibraryQuery = errors.New("story library query is invalid")

type BooleanFilterState string

const (
	BooleanIgnored BooleanFilterState = "ignored"
	BooleanTrue    BooleanFilterState = "true"
	BooleanFalse   BooleanFilterState = "false"
)

type BooleanFilter struct {
	DefinitionID int64              `json:"definitionId"`
	State        BooleanFilterState `json:"state"`
}

type ChoiceFilter struct {
	DefinitionID int64   `json:"definitionId"`
	ValueIDs     []int64 `json:"valueIds"`
}

type StoryLibraryQuery struct {
	Name            string          `json:"name"`
	Languages       []string        `json:"languages"`
	Compatibilities []Compatibility `json:"compatibilities"`
	BooleanFilters  []BooleanFilter `json:"booleanFilters"`
	ChoiceFilters   []ChoiceFilter  `json:"choiceFilters"`
	Page            int             `json:"page"`
	PageSize        int             `json:"pageSize"`
	Sort            Sort            `json:"sort"`
}

func (q *Query) Search(
	ctx context.Context,
	request StoryLibraryQuery,
) (Page, error) {
	request, err := normalizeStoryLibraryQuery(request)
	if err != nil {
		return Page{}, err
	}
	if err := q.validateFilterDefinitions(ctx, request); err != nil {
		return Page{}, err
	}
	return q.searchNormalized(ctx, request)
}

func (q *Query) searchNormalized(
	ctx context.Context,
	request StoryLibraryQuery,
) (Page, error) {
	where, arguments := storyLibraryPredicate(request)
	totalItems, err := q.countLocalRecords(ctx, where, arguments)
	if err != nil {
		return Page{}, err
	}
	totalPages := 0
	if totalItems > 0 {
		totalPages = (totalItems + request.PageSize - 1) / request.PageSize
	}
	offset := (request.Page - 1) * request.PageSize
	var records []localRecord
	if offset < totalItems {
		records, err = q.localRecordPage(
			ctx,
			where,
			arguments,
			request.Sort,
			request.PageSize,
			offset,
		)
		if err != nil {
			return Page{}, err
		}
	}
	stories, err := q.summariesFromRecords(ctx, records)
	if err != nil {
		return Page{}, err
	}
	return Page{
		Stories:    stories,
		Page:       request.Page,
		PageSize:   request.PageSize,
		TotalItems: totalItems,
		TotalPages: totalPages,
		Sort:       request.Sort,
	}, nil
}

func BackfillNormalizedDisplayNames(ctx context.Context, database *sql.DB) error {
	if database == nil {
		return errors.New("library database is required")
	}
	rows, err := database.QueryContext(
		ctx,
		`SELECT id, uuid, COALESCE(embedded_title, '')
		 FROM stories
		 ORDER BY id`,
	)
	if err != nil {
		return fmt.Errorf("list story display names: %w", err)
	}
	type update struct {
		id         int64
		normalized string
	}
	var updates []update
	for rows.Next() {
		var id int64
		var uuid string
		var embeddedTitle string
		if err := rows.Scan(&id, &uuid, &embeddedTitle); err != nil {
			_ = rows.Close()
			return err
		}
		title := strings.TrimSpace(embeddedTitle)
		if title == "" {
			title = "Story " + uuid
		}
		updates = append(updates, update{
			id:         id,
			normalized: searchtext.Normalize(title),
		})
	}
	if err := rows.Close(); err != nil {
		return err
	}
	transaction, err := database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin display name backfill: %w", err)
	}
	defer transaction.Rollback()
	for _, update := range updates {
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE stories
			 SET display_name_normalized = ?
			 WHERE id = ?`,
			update.normalized,
			update.id,
		); err != nil {
			return fmt.Errorf("backfill story display name: %w", err)
		}
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit display name backfill: %w", err)
	}
	return nil
}

func normalizeStoryLibraryQuery(
	request StoryLibraryQuery,
) (StoryLibraryQuery, error) {
	listRequest, err := normalizeListRequest(ListRequest{
		Page:     request.Page,
		PageSize: request.PageSize,
		Sort:     request.Sort,
	})
	if err != nil {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
	}
	request.Page = listRequest.Page
	request.PageSize = listRequest.PageSize
	request.Sort = listRequest.Sort
	request.Name = searchtext.Normalize(request.Name)
	groupCount := len(request.BooleanFilters) + len(request.ChoiceFilters)
	if len(request.Languages) > 0 {
		groupCount++
	}
	if len(request.Compatibilities) > 0 {
		groupCount++
	}
	if len([]rune(request.Name)) > maxNameSearchRunes ||
		groupCount > maxFilterGroups ||
		len(request.Languages) > maxValuesPerFilter ||
		len(request.Compatibilities) > 3 {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
	}

	seenLanguages := make(map[string]struct{}, len(request.Languages))
	normalizedLanguages := make([]string, 0, len(request.Languages))
	for _, value := range request.Languages {
		locale, err := canonicalLanguage(value)
		if err != nil {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
		}
		if _, duplicate := seenLanguages[locale]; duplicate {
			continue
		}
		seenLanguages[locale] = struct{}{}
		normalizedLanguages = append(normalizedLanguages, locale)
	}
	sort.Strings(normalizedLanguages)
	request.Languages = normalizedLanguages

	seenCompatibilities := make(map[Compatibility]struct{}, len(request.Compatibilities))
	normalizedCompatibilities := make(
		[]Compatibility,
		0,
		len(request.Compatibilities),
	)
	for _, value := range request.Compatibilities {
		switch value {
		case CompatibilityCompatible, CompatibilityMissing, CompatibilityInvalid:
		default:
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
		}
		if _, duplicate := seenCompatibilities[value]; duplicate {
			continue
		}
		seenCompatibilities[value] = struct{}{}
		normalizedCompatibilities = append(normalizedCompatibilities, value)
	}
	sort.Slice(normalizedCompatibilities, func(i, j int) bool {
		return normalizedCompatibilities[i] < normalizedCompatibilities[j]
	})
	request.Compatibilities = normalizedCompatibilities

	definitions := make(map[int64]struct{})
	for index := range request.BooleanFilters {
		filter := &request.BooleanFilters[index]
		if filter.DefinitionID <= 0 ||
			(filter.State != BooleanIgnored &&
				filter.State != BooleanTrue &&
				filter.State != BooleanFalse) {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
		}
		if _, duplicate := definitions[filter.DefinitionID]; duplicate {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
		}
		definitions[filter.DefinitionID] = struct{}{}
	}
	for index := range request.ChoiceFilters {
		filter := &request.ChoiceFilters[index]
		if filter.DefinitionID <= 0 ||
			len(filter.ValueIDs) == 0 ||
			len(filter.ValueIDs) > maxValuesPerFilter {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
		}
		if _, duplicate := definitions[filter.DefinitionID]; duplicate {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
		}
		definitions[filter.DefinitionID] = struct{}{}
		seenValues := make(map[int64]struct{}, len(filter.ValueIDs))
		normalized := make([]int64, 0, len(filter.ValueIDs))
		for _, valueID := range filter.ValueIDs {
			if valueID <= 0 {
				return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
			}
			if _, duplicate := seenValues[valueID]; duplicate {
				continue
			}
			seenValues[valueID] = struct{}{}
			normalized = append(normalized, valueID)
		}
		filter.ValueIDs = normalized
	}
	return request, nil
}

func (q *Query) validateFilterDefinitions(
	ctx context.Context,
	request StoryLibraryQuery,
) error {
	for _, filter := range request.BooleanFilters {
		var kind string
		err := q.database.QueryRowContext(
			ctx,
			"SELECT kind FROM tag_definitions WHERE id = ?",
			filter.DefinitionID,
		).Scan(&kind)
		if errors.Is(err, sql.ErrNoRows) || (err == nil && kind != "boolean") {
			return ErrInvalidStoryLibraryQuery
		}
		if err != nil {
			return fmt.Errorf("validate boolean filter: %w", err)
		}
	}
	for _, filter := range request.ChoiceFilters {
		var kind string
		err := q.database.QueryRowContext(
			ctx,
			"SELECT kind FROM tag_definitions WHERE id = ?",
			filter.DefinitionID,
		).Scan(&kind)
		if errors.Is(err, sql.ErrNoRows) || (err == nil && kind != "choice") {
			return ErrInvalidStoryLibraryQuery
		}
		if err != nil {
			return fmt.Errorf("validate choice filter: %w", err)
		}
		for _, valueID := range filter.ValueIDs {
			var exists int
			if err := q.database.QueryRowContext(
				ctx,
				`SELECT EXISTS(
					SELECT 1
					FROM tag_values
					WHERE id = ? AND definition_id = ?
				)`,
				valueID,
				filter.DefinitionID,
			).Scan(&exists); err != nil {
				return fmt.Errorf("validate choice filter value: %w", err)
			}
			if exists != 1 {
				return ErrInvalidStoryLibraryQuery
			}
		}
	}
	return nil
}

func storyLibraryPredicate(request StoryLibraryQuery) (string, []any) {
	var predicates []string
	var arguments []any
	if request.Name != "" {
		predicates = append(
			predicates,
			"instr("+resolvedDisplayNameSQL+", ?) > 0",
		)
		arguments = append(arguments, request.Name)
	}
	if len(request.Languages) > 0 {
		placeholders := strings.TrimSuffix(
			strings.Repeat("?,", len(request.Languages)),
			",",
		)
		predicates = append(predicates, `EXISTS (
			SELECT 1
			FROM official_story_metadata AS official_language
			JOIN catalog_snapshots AS language_snapshot
			  ON language_snapshot.id = official_language.snapshot_id
			WHERE official_language.story_uuid = s.uuid
			  AND language_snapshot.status = 'active'
			  AND official_language.language IN (`+placeholders+`)
		)`)
		for _, locale := range request.Languages {
			arguments = append(arguments, locale)
		}
	}
	if len(request.Compatibilities) > 0 {
		placeholders := strings.TrimSuffix(
			strings.Repeat("?,", len(request.Compatibilities)),
			",",
		)
		predicates = append(
			predicates,
			"a.validation_state IN ("+placeholders+")",
		)
		for _, value := range request.Compatibilities {
			arguments = append(arguments, compatibilityValidationState(value))
		}
	}
	for _, filter := range request.BooleanFilters {
		if filter.State == BooleanIgnored {
			continue
		}
		exists := `EXISTS (
			SELECT 1
			FROM story_tag_assignments boolean_assignment
			WHERE boolean_assignment.story_id = s.id
			  AND boolean_assignment.definition_id = ?
			  AND boolean_assignment.value_id IS NULL
		)`
		if filter.State == BooleanFalse {
			exists = "NOT " + exists
		}
		predicates = append(predicates, exists)
		arguments = append(arguments, filter.DefinitionID)
	}
	for _, filter := range request.ChoiceFilters {
		placeholders := strings.TrimSuffix(
			strings.Repeat("?,", len(filter.ValueIDs)),
			",",
		)
		predicates = append(predicates, `EXISTS (
			SELECT 1
			FROM story_tag_assignments choice_assignment
			WHERE choice_assignment.story_id = s.id
			  AND choice_assignment.definition_id = ?
			  AND choice_assignment.value_id IN (`+placeholders+`)
		)`)
		arguments = append(arguments, filter.DefinitionID)
		for _, valueID := range filter.ValueIDs {
			arguments = append(arguments, valueID)
		}
	}
	return strings.Join(predicates, " AND "), arguments
}

func canonicalLanguage(value string) (string, error) {
	value = strings.ReplaceAll(strings.TrimSpace(value), "_", "-")
	if value == "" || len(value) > 35 {
		return "", ErrInvalidStoryLibraryQuery
	}
	tag, err := language.Parse(value)
	if err != nil || tag == language.Und {
		return "", ErrInvalidStoryLibraryQuery
	}
	return tag.String(), nil
}

func compatibilityValidationState(value Compatibility) string {
	switch value {
	case CompatibilityCompatible:
		return "valid"
	case CompatibilityMissing:
		return "missing"
	default:
		return "invalid"
	}
}
