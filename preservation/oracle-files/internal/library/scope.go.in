package library

import (
	"context"
	"encoding/json"
)

// StoriesInScope evaluates ordinary query criteria within explicit UUID
// membership. It resolves the complete set in one statement, before the caller
// applies its own manual ordering and pagination.
func (q *Query) StoriesInScope(ctx context.Context, uuids []string, request StoryLibraryQuery) ([]StorySummary, error) {
	request, err := normalizeStoryLibraryQuery(request)
	if err != nil {
		return nil, err
	}
	if err := q.validateFilterDefinitions(ctx, request); err != nil {
		return nil, err
	}
	encoded, err := json.Marshal(uuids)
	if err != nil {
		return nil, err
	}
	where, args := storyLibraryPredicate(request, q.officialLocale)
	if where != "" {
		where = "(" + where + ") AND "
	}
	where += `s.uuid IN (SELECT value FROM json_each(?))`
	args = append(args, string(encoded))
	records, err := q.localRecordsOrdered(ctx, where, args, request.Sort)
	if err != nil {
		return nil, err
	}
	return q.summariesFromRecords(ctx, records)
}
