package operations

import (
	"context"
	"errors"
	"path/filepath"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/google/uuid"
)

func TestRepositoryPersistsOperationProgress(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	ctx := context.Background()
	createdAt := time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)
	operationID := uuid.NewString()
	snapshot, err := repository.CreateImport(ctx, operationID, []NewItem{
		{SourceName: "first.zip", TotalBytes: 10},
		{SourceName: "second.7z", TotalBytes: 20},
	}, createdAt)
	if err != nil {
		t.Fatalf("CreateImport() error = %v", err)
	}
	if snapshot.Status != StatusQueued ||
		snapshot.TotalItems != 2 ||
		len(snapshot.Items) != 2 ||
		snapshot.Items[0].Status != ItemPending {
		t.Fatalf("CreateImport() = %#v", snapshot)
	}

	startedAt := createdAt.Add(time.Second)
	if err := repository.MarkRunning(ctx, operationID, startedAt); err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkItemRunning(ctx, snapshot.Items[0].ID); err != nil {
		t.Fatal(err)
	}
	if err := repository.CompleteItem(ctx, operationID, ItemSnapshot{
		ID:             snapshot.Items[0].ID,
		SourceName:     "first.zip",
		Status:         ItemSucceeded,
		OutcomeCode:    "imported",
		OutcomeMessage: "Story imported.",
		CompletedBytes: 10,
		TotalBytes:     10,
	}); err != nil {
		t.Fatal(err)
	}
	if err := repository.RequestCancel(ctx, operationID); err != nil {
		t.Fatal(err)
	}
	if err := repository.CompleteItem(ctx, operationID, ItemSnapshot{
		ID:             snapshot.Items[1].ID,
		SourceName:     "second.7z",
		Status:         ItemCancelled,
		OutcomeCode:    "cancelled",
		OutcomeMessage: "Import cancelled.",
		TotalBytes:     20,
	}); err != nil {
		t.Fatal(err)
	}
	finishedAt := startedAt.Add(time.Second)
	if err := repository.Finish(
		ctx,
		operationID,
		StatusPartiallySucceeded,
		"partial_failure",
		"Some files could not be imported.",
		finishedAt,
	); err != nil {
		t.Fatal(err)
	}

	snapshot, err = repository.Snapshot(ctx, operationID)
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.Status != StatusPartiallySucceeded ||
		snapshot.CompletedItems != 2 ||
		!snapshot.CancelRequested ||
		snapshot.StartedAt != formatTime(startedAt) ||
		snapshot.FinishedAt != formatTime(finishedAt) ||
		snapshot.Items[0].OutcomeCode != "imported" ||
		snapshot.Items[1].Status != ItemCancelled {
		t.Fatalf("Snapshot() = %#v", snapshot)
	}
	if err := repository.RequestCancel(ctx, operationID); !errors.Is(err, ErrOperationNotActive) {
		t.Fatalf("RequestCancel(terminal) error = %v", err)
	}
}

func TestRepositoryRejectsInvalidTransitions(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	ctx := context.Background()
	snapshot, err := repository.CreateImport(
		ctx,
		uuid.NewString(),
		[]NewItem{{SourceName: "story.zip"}},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkItemRunning(ctx, snapshot.Items[0].ID); err != nil {
		t.Fatal(err)
	}
	item := ItemSnapshot{
		ID:         snapshot.Items[0].ID,
		SourceName: "story.zip",
		Status:     ItemFailed,
		TotalBytes: 0,
	}
	if err := repository.CompleteItem(ctx, snapshot.ID, item); err != nil {
		t.Fatal(err)
	}
	if err := repository.CompleteItem(ctx, snapshot.ID, item); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("CompleteItem(second) error = %v", err)
	}
	if err := repository.Finish(
		ctx,
		snapshot.ID,
		StatusRunning,
		"",
		"",
		time.Now(),
	); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("Finish(nonterminal) error = %v", err)
	}
}

func TestRepositoryInterruptsNonresumableOperations(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	ctx := context.Background()
	operationID := uuid.NewString()
	snapshot, err := repository.CreateImport(
		ctx,
		operationID,
		[]NewItem{
			{SourceName: "running.zip", TotalBytes: 10},
			{SourceName: "pending.zip", TotalBytes: 20},
		},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkRunning(ctx, operationID, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkItemRunning(ctx, snapshot.Items[0].ID); err != nil {
		t.Fatal(err)
	}

	interrupted, err := repository.InterruptActive(ctx, time.Now())
	if err != nil {
		t.Fatalf("InterruptActive() error = %v", err)
	}
	if len(interrupted) != 1 ||
		interrupted[0].Status != StatusInterrupted ||
		interrupted[0].CompletedItems != 2 ||
		interrupted[0].ErrorCode != "interrupted" {
		t.Fatalf("InterruptActive() = %#v", interrupted)
	}
	for _, item := range interrupted[0].Items {
		if item.Status != ItemCancelled || item.OutcomeCode != "interrupted" {
			t.Fatalf("interrupted item = %#v", item)
		}
	}
	if again, err := repository.InterruptActive(ctx, time.Now()); err != nil || len(again) != 0 {
		t.Fatalf("InterruptActive(second) = %#v, %v", again, err)
	}
}

func TestRepositoryInterruptsOneOperationWithStableFailure(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	ctx := context.Background()
	snapshot, err := repository.CreateImport(
		ctx,
		uuid.NewString(),
		[]NewItem{{SourceName: "story.zip"}},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}

	interrupted, err := repository.Interrupt(
		ctx,
		snapshot.ID,
		"persistence_failed",
		"Operation progress could not be saved.",
		time.Now(),
	)
	if err != nil {
		t.Fatalf("Interrupt() error = %v", err)
	}
	if interrupted.Status != StatusInterrupted ||
		interrupted.CompletedItems != interrupted.TotalItems ||
		interrupted.ErrorCode != "persistence_failed" ||
		interrupted.Items[0].Status != ItemCancelled ||
		interrupted.Items[0].OutcomeCode != "persistence_failed" {
		t.Fatalf("Interrupt() = %#v", interrupted)
	}
}

func TestRepositoryListsOnlyActiveOperationsNewestFirst(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	ctx := context.Background()
	finished, err := repository.CreateImport(
		ctx,
		"00000000-0000-4000-8000-000000000001",
		[]NewItem{{SourceName: "finished.zip"}},
		time.Date(2026, time.July, 25, 8, 0, 0, 0, time.UTC),
	)
	if err != nil {
		t.Fatal(err)
	}
	older, err := repository.CreateImport(
		ctx,
		"00000000-0000-4000-8000-000000000002",
		[]NewItem{{SourceName: "older.zip"}},
		time.Date(2026, time.July, 25, 9, 0, 0, 0, time.UTC),
	)
	if err != nil {
		t.Fatal(err)
	}
	newer, err := repository.CreateImport(
		ctx,
		"00000000-0000-4000-8000-000000000003",
		[]NewItem{{SourceName: "newer.zip"}},
		time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkItemRunning(ctx, finished.Items[0].ID); err != nil {
		t.Fatal(err)
	}
	if err := repository.CompleteItem(ctx, finished.ID, ItemSnapshot{
		ID:         finished.Items[0].ID,
		SourceName: finished.Items[0].SourceName,
		Status:     ItemSucceeded,
	}); err != nil {
		t.Fatal(err)
	}
	if err := repository.Finish(ctx, finished.ID, StatusSucceeded, "", "", time.Now()); err != nil {
		t.Fatal(err)
	}

	active, err := repository.ActiveSnapshots(ctx)
	if err != nil {
		t.Fatalf("ActiveSnapshots() error = %v", err)
	}
	if len(active) != 2 ||
		active[0].ID != newer.ID ||
		active[1].ID != older.ID {
		t.Fatalf("ActiveSnapshots() = %#v", active)
	}
}

func newOperationRepository(t *testing.T) (*Repository, *database.Database) {
	t.Helper()

	sqlDatabase, err := database.Open(
		context.Background(),
		filepath.Join(t.TempDir(), "operations.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = sqlDatabase.Close()
	})
	return NewRepository(sqlDatabase.SQL()), sqlDatabase
}
