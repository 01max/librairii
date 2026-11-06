package library

import (
	"context"
	"encoding/json"
)

type ExportState struct {
	ID     int64
	Broken bool
}

func (q *Query) ExportStates(ctx context.Context, uuids []string) (map[string]ExportState, error) {
	encoded, err := json.Marshal(uuids)
	if err != nil {
		return nil, err
	}
	rows, err := q.database.QueryContext(ctx, `SELECT s.uuid,s.id,s.is_broken FROM stories s
	 JOIN story_archives a ON a.story_id=s.id WHERE s.uuid IN (SELECT value FROM json_each(?))
	 AND NOT EXISTS(SELECT 1 FROM removal_intents r WHERE r.story_id=s.id)`, string(encoded))
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := map[string]ExportState{}
	for rows.Next() {
		var id string
		var state ExportState
		if err := rows.Scan(&id, &state.ID, &state.Broken); err != nil {
			return nil, err
		}
		result[id] = state
	}
	return result, rows.Err()
}
