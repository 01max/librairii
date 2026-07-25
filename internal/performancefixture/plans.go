package performancefixture

import (
	"context"
	"database/sql"
	"fmt"

	"github.com/01max/librairii/internal/library"
)

const resolvedFixtureTitle = `COALESCE(
	NULLIF((
		SELECT official_name.title_normalized
		FROM official_story_metadata AS official_name
		JOIN catalog_snapshots AS official_snapshot
		  ON official_snapshot.id = official_name.snapshot_id
		WHERE official_name.story_uuid = s.uuid
		  AND official_snapshot.status = 'active'
		  AND official_snapshot.locale = ?
		ORDER BY official_snapshot.activated_at DESC,
		         official_snapshot.id DESC
		LIMIT 1
	), ''),
	s.display_name_normalized
)`

type Plan map[string][]string

func QueryPlans(ctx context.Context, database *sql.DB) (Plan, error) {
	if database == nil {
		return nil, fmt.Errorf("performance query-plan database is nil")
	}
	queries := []struct {
		name      string
		statement string
		arguments []any
	}{
		{
			name: "collectionQuery",
			statement: `SELECT s.id
				FROM stories s
				JOIN story_archives a ON a.story_id = s.id
				ORDER BY ` + resolvedFixtureTitle + `, s.uuid, s.id
				LIMIT ?`,
			arguments: []any{"en-GB", library.DefaultPageSize},
		},
		{
			name: "substringSearch",
			statement: `SELECT s.id
				FROM stories s
				JOIN story_archives a ON a.story_id = s.id
				WHERE instr(` + resolvedFixtureTitle + `, ?) > 0
				ORDER BY ` + resolvedFixtureTitle + `, s.uuid, s.id
				LIMIT ?`,
			arguments: []any{
				"en-GB",
				"moon",
				"en-GB",
				library.DefaultPageSize,
			},
		},
		{
			name: "combinedFilters",
			statement: `SELECT s.id
				FROM stories s
				JOIN story_archives a ON a.story_id = s.id
				WHERE instr(` + resolvedFixtureTitle + `, ?) > 0
				  AND EXISTS (
					SELECT 1
					FROM official_story_metadata AS official_language
					JOIN catalog_snapshots AS language_snapshot
					  ON language_snapshot.id = official_language.snapshot_id
					WHERE official_language.story_uuid = s.uuid
					  AND language_snapshot.status = 'active'
					  AND official_language.language = ?
				  )
				  AND a.validation_state = ?
				  AND NOT EXISTS (
					SELECT 1
					FROM story_tag_assignments boolean_assignment
					WHERE boolean_assignment.story_id = s.id
					  AND boolean_assignment.definition_id = ?
					  AND boolean_assignment.value_id IS NULL
				  )
				  AND EXISTS (
					SELECT 1
					FROM story_tag_assignments choice_assignment
					WHERE choice_assignment.story_id = s.id
					  AND choice_assignment.definition_id = ?
					  AND choice_assignment.value_id = ?
				  )
				ORDER BY ` + resolvedFixtureTitle + `, s.uuid, s.id
				LIMIT ?`,
			arguments: []any{
				"en-GB",
				"forest",
				"en-GB",
				"valid",
				BrokenDefinitionID,
				MoodDefinitionID,
				CalmValueID,
				"en-GB",
				library.DefaultPageSize,
			},
		},
		{
			name: "shelfCounts",
			statement: `SELECT COUNT(*)
				FROM stories s
				JOIN story_archives a ON a.story_id = s.id
				WHERE a.validation_state = ?
				  AND EXISTS (
					SELECT 1
					FROM story_tag_assignments choice_assignment
					WHERE choice_assignment.story_id = s.id
					  AND choice_assignment.definition_id = ?
					  AND choice_assignment.value_id = ?
				  )`,
			arguments: []any{
				"valid",
				MoodDefinitionID,
				CalmValueID,
			},
		},
		{
			name: "deepPagination",
			statement: `SELECT s.id
				FROM stories s
				JOIN story_archives a ON a.story_id = s.id
				ORDER BY ` + resolvedFixtureTitle + `, s.uuid, s.id
				LIMIT ? OFFSET ?`,
			arguments: []any{"en-GB", library.DefaultPageSize, 4_776},
		},
		{
			name: "artworkLoad",
			statement: `SELECT embedded_artwork_path
				FROM stories
				WHERE id = ? AND embedded_artwork_path IS NOT NULL`,
			arguments: []any{1},
		},
	}
	plans := make(Plan, len(queries))
	for _, query := range queries {
		rows, err := database.QueryContext(
			ctx,
			"EXPLAIN QUERY PLAN "+query.statement,
			query.arguments...,
		)
		if err != nil {
			return nil, fmt.Errorf("explain %s: %w", query.name, err)
		}
		for rows.Next() {
			var id int
			var parent int
			var unused int
			var detail string
			if err := rows.Scan(&id, &parent, &unused, &detail); err != nil {
				_ = rows.Close()
				return nil, err
			}
			plans[query.name] = append(plans[query.name], detail)
		}
		if err := rows.Close(); err != nil {
			return nil, err
		}
	}
	return plans, nil
}
