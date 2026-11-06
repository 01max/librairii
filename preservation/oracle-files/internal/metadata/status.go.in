package metadata

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

type CatalogStatusState string

const (
	CatalogNeverSynced CatalogStatusState = "never_synced"
	CatalogFresh       CatalogStatusState = "fresh"
	CatalogStaleCache  CatalogStatusState = "stale_cache"
)

type CatalogStatus struct {
	State             CatalogStatusState `json:"state"`
	Locale            string             `json:"locale"`
	MatchedStoryCount int                `json:"matchedStoryCount"`
	FetchedAt         string             `json:"fetchedAt,omitempty"`
	ActivatedAt       string             `json:"activatedAt,omitempty"`
	LastAttemptStatus SyncStatus         `json:"lastAttemptStatus,omitempty"`
	LastAttemptAt     string             `json:"lastAttemptAt,omitempty"`
	ErrorCode         string             `json:"errorCode,omitempty"`
	ErrorMessage      string             `json:"errorMessage,omitempty"`
}

func (r *Repository) CatalogStatus(
	ctx context.Context,
	locale string,
) (CatalogStatus, error) {
	canonical, err := canonicalLocale(locale)
	if err != nil {
		return CatalogStatus{}, err
	}
	status := CatalogStatus{
		State:  CatalogNeverSynced,
		Locale: canonical,
	}
	err = r.database.QueryRowContext(
		ctx,
		`SELECT
			snapshot.fetched_at,
			COALESCE(snapshot.activated_at, ''),
			sync.matched_story_count
		 FROM catalog_snapshots AS snapshot
		 JOIN catalog_syncs AS sync ON sync.id = snapshot.sync_id
		 WHERE snapshot.locale = ? AND snapshot.status = 'active'`,
		canonical,
	).Scan(
		&status.FetchedAt,
		&status.ActivatedAt,
		&status.MatchedStoryCount,
	)
	switch {
	case err == nil:
		status.State = CatalogFresh
	case errors.Is(err, sql.ErrNoRows):
	default:
		return CatalogStatus{}, fmt.Errorf("read active catalog status: %w", err)
	}

	err = r.database.QueryRowContext(
		ctx,
		`SELECT
			status,
			started_at,
			COALESCE(error_code, ''),
			COALESCE(error_message, '')
		 FROM catalog_syncs
		 WHERE locale = ?
		 ORDER BY started_at DESC, id DESC
		 LIMIT 1`,
		canonical,
	).Scan(
		&status.LastAttemptStatus,
		&status.LastAttemptAt,
		&status.ErrorCode,
		&status.ErrorMessage,
	)
	switch {
	case err == nil:
		if status.State == CatalogFresh &&
			(status.LastAttemptStatus == SyncFailed ||
				status.LastAttemptStatus == SyncCancelled) {
			status.State = CatalogStaleCache
		}
	case errors.Is(err, sql.ErrNoRows):
	default:
		return CatalogStatus{}, fmt.Errorf("read latest catalog sync status: %w", err)
	}
	return status, nil
}

func (r *Repository) CountMatchingStories(
	ctx context.Context,
	snapshotID int64,
) (int, error) {
	if snapshotID <= 0 {
		return 0, ErrInvalidTransition
	}
	var count int
	if err := r.database.QueryRowContext(
		ctx,
		`SELECT COUNT(*)
		 FROM official_story_metadata AS official
		 JOIN stories ON stories.uuid = official.story_uuid
		 WHERE official.snapshot_id = ?`,
		snapshotID,
	).Scan(&count); err != nil {
		return 0, fmt.Errorf("count matching catalog stories: %w", err)
	}
	return count, nil
}
