package library

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

var (
	ErrInvalidFlagRequest   = errors.New("invalid story flag request")
	ErrFlagStoryUnavailable = errors.New("story is no longer available")
	ErrFlagRestoreStale     = errors.New("story flags changed since this action")
)

const maxFlagStories = 1000

type FlagChange struct {
	StoryID  int64 `json:"storyId"`
	Previous bool  `json:"previous"`
	Current  bool  `json:"current"`
}

type FlagResult struct {
	RequestedStories int          `json:"requestedStories"`
	ChangedStories   int          `json:"changedStories"`
	Changes          []FlagChange `json:"changes"`
}

type BrokenFlags struct{ database *sql.DB }

func NewBrokenFlags(database *sql.DB) *BrokenFlags { return &BrokenFlags{database: database} }

func (s *BrokenFlags) Set(ctx context.Context, ids []int64, broken bool) (FlagResult, error) {
	if len(ids) == 0 || len(ids) > maxFlagStories {
		return FlagResult{}, ErrInvalidFlagRequest
	}
	seen := make(map[int64]bool, len(ids))
	changes := make([]FlagChange, 0, len(ids))
	for _, id := range ids {
		if id <= 0 {
			return FlagResult{}, ErrInvalidFlagRequest
		}
		if !seen[id] {
			changes = append(changes, FlagChange{StoryID: id, Current: broken})
			seen[id] = true
		}
	}
	return s.apply(ctx, changes, false)
}

func (s *BrokenFlags) Restore(ctx context.Context, changes []FlagChange) (FlagResult, error) {
	if len(changes) == 0 || len(changes) > maxFlagStories {
		return FlagResult{}, ErrInvalidFlagRequest
	}
	seen := make(map[int64]bool, len(changes))
	for _, change := range changes {
		if change.StoryID <= 0 || seen[change.StoryID] || change.Previous == change.Current {
			return FlagResult{}, ErrInvalidFlagRequest
		}
		seen[change.StoryID] = true
	}
	return s.apply(ctx, changes, true)
}

func (s *BrokenFlags) apply(ctx context.Context, requested []FlagChange, restore bool) (FlagResult, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return FlagResult{}, fmt.Errorf("begin story flag update: %w", err)
	}
	defer tx.Rollback()
	result := FlagResult{RequestedStories: len(requested), Changes: []FlagChange{}}
	for _, change := range requested {
		var current bool
		err := tx.QueryRowContext(ctx, `SELECT s.is_broken FROM stories s
            JOIN story_archives a ON a.story_id=s.id
            WHERE s.id=? AND NOT EXISTS (SELECT 1 FROM removal_intents r WHERE r.story_id=s.id)`, change.StoryID).Scan(&current)
		if errors.Is(err, sql.ErrNoRows) {
			return FlagResult{}, ErrFlagStoryUnavailable
		}
		if err != nil {
			return FlagResult{}, fmt.Errorf("read story flag: %w", err)
		}
		target := change.Current
		if restore {
			if current != change.Current {
				return FlagResult{}, ErrFlagRestoreStale
			}
			target = change.Previous
		}
		if target != current {
			result.Changes = append(result.Changes, FlagChange{StoryID: change.StoryID, Previous: current, Current: target})
		}
	}
	for _, change := range result.Changes {
		if _, err := tx.ExecContext(ctx, "UPDATE stories SET is_broken=?, updated_at=CURRENT_TIMESTAMP WHERE id=?", change.Current, change.StoryID); err != nil {
			return FlagResult{}, fmt.Errorf("write story flag: %w", err)
		}
	}
	if err := tx.Commit(); err != nil {
		return FlagResult{}, fmt.Errorf("commit story flags: %w", err)
	}
	result.ChangedStories = len(result.Changes)
	return result, nil
}
