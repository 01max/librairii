package library

import (
	"context"
	"encoding/json"
)

// ExportUUIDs resolves available archives in a stable approved UUID scope.
// Missing UUIDs are retained by the Shelf resolver as unavailable references.
func (q *Query) ExportUUIDs(ctx context.Context, uuids []string, request StoryLibraryQuery) ([]ExportStory, error) {
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
	return q.exportStoriesFromRecords(ctx, records)
}
