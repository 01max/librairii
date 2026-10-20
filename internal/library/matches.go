package library

import "context"

func (q *Query) MatchingUUIDs(ctx context.Context, request StoryLibraryQuery) ([]string, error) {
	request, err := normalizeStoryLibraryQuery(request)
	if err != nil {
		return nil, err
	}
	if err := q.validateFilterDefinitions(ctx, request); err != nil {
		return nil, err
	}
	where, args := storyLibraryPredicate(request, q.officialLocale)
	statement := `SELECT s.uuid FROM stories s JOIN story_archives a ON a.story_id=s.id`
	if where != "" {
		statement += " WHERE " + where
	}
	statement += " ORDER BY s.uuid"
	rows, err := q.database.QueryContext(ctx, statement, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make([]string, 0)
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		result = append(result, id)
	}
	return result, rows.Err()
}
