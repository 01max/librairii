package operations

import (
	"context"
	"errors"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/google/uuid"
)

func TestRepositoryPersistsImmutableExportScopeAndOutcomeReport(t *testing.T) {
	t.Parallel()

	repository, sqlDatabase := newOperationRepository(t)
	ctx := context.Background()
	const storyUUID = "00112233-4455-4677-8899-aabbccddeeff"
	result, err := sqlDatabase.SQL().ExecContext(
		ctx,
		`INSERT INTO stories (uuid, embedded_title, display_name_normalized)
		 VALUES (?, 'Moonlit Workshop', 'moonlit workshop')`,
		storyUUID,
	)
	if err != nil {
		t.Fatal(err)
	}
	storyID, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	checksum := strings.Repeat("a", 64)
	createdAt := time.Date(2026, time.July, 25, 12, 0, 0, 0, time.UTC)
	snapshot, err := repository.CreateExport(
		ctx,
		"00000000-0000-4000-8000-000000000020",
		ExportSource{
			Type:       ExportSourceShelves,
			ShelfIDs:   []int64{7, 8},
			ShelfNames: []string{"Bedtime", "Adventures"},
		},
		filepath.Join(t.TempDir(), "Lunii export"),
		[]NewItem{{
			StoryID:             storyID,
			StoryUUID:           storyUUID,
			StoryTitle:          "Moonlit Workshop",
			SourceName:          "moon.v2.pk",
			OutputName:          "Moonlit Workshop.v2.pk",
			ArchiveRelativePath: "archives/aa/moon.v2.pk",
			ArchiveSHA256:       checksum,
			TotalBytes:          42,
		}},
		createdAt,
	)
	if err != nil {
		t.Fatalf("CreateExport() error = %v", err)
	}
	if snapshot.Kind != KindExport ||
		snapshot.ExportSourceType != ExportSourceShelves ||
		!reflect.DeepEqual(snapshot.SourceShelfIDs, []int64{7, 8}) ||
		!reflect.DeepEqual(
			snapshot.SourceShelfNames,
			[]string{"Bedtime", "Adventures"},
		) ||
		snapshot.DestinationLabel != "Lunii export" ||
		snapshot.TotalItems != 1 ||
		snapshot.TotalBytes != 42 ||
		len(snapshot.Items) != 1 ||
		snapshot.Items[0].StoryID != storyID ||
		snapshot.Items[0].StoryUUID != storyUUID ||
		snapshot.Items[0].StoryTitle != "Moonlit Workshop" ||
		snapshot.Items[0].OutputName != "Moonlit Workshop.v2.pk" ||
		snapshot.Items[0].ArchiveRelativePath != "archives/aa/moon.v2.pk" ||
		snapshot.Items[0].ArchiveSHA256 != checksum {
		t.Fatalf("CreateExport() = %#v", snapshot)
	}

	if _, err := sqlDatabase.SQL().ExecContext(
		ctx,
		"UPDATE file_operations SET export_destination = '/changed' WHERE id = ?",
		snapshot.ID,
	); err == nil {
		t.Fatal("mutable export destination update error = nil")
	}
	if _, err := sqlDatabase.SQL().ExecContext(
		ctx,
		"UPDATE file_operation_items SET output_name = 'changed.pk' WHERE id = ?",
		snapshot.Items[0].ID,
	); err == nil {
		t.Fatal("mutable export item update error = nil")
	}

	if err := repository.MarkRunning(ctx, snapshot.ID, createdAt.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkItemRunning(ctx, snapshot.Items[0].ID); err != nil {
		t.Fatal(err)
	}
	if err := repository.UpdateItemProgress(
		ctx,
		snapshot.ID,
		snapshot.Items[0].ID,
		21,
	); err != nil {
		t.Fatal(err)
	}
	progress, err := repository.Snapshot(ctx, snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if progress.Items[0].CompletedBytes != 21 {
		t.Fatalf("Snapshot(export progress) = %#v", progress.Items[0])
	}
	if err := repository.UpdateItemProgress(
		ctx,
		snapshot.ID,
		snapshot.Items[0].ID,
		43,
	); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("UpdateItemProgress(overflow) error = %v", err)
	}
	if err := repository.RequestCancel(ctx, snapshot.ID); err != nil {
		t.Fatal(err)
	}
	if err := repository.CompleteItem(ctx, snapshot.ID, ItemSnapshot{
		ID:             snapshot.Items[0].ID,
		StoryID:        storyID,
		Status:         ItemConflicted,
		OutcomeCode:    "destination_exists",
		OutcomeMessage: "The destination file already exists.",
		TotalBytes:     42,
	}); err != nil {
		t.Fatal(err)
	}
	if err := repository.Finish(
		ctx,
		snapshot.ID,
		StatusPartiallySucceeded,
		"export_conflicts",
		"One destination file already exists.",
		createdAt.Add(2*time.Second),
	); err != nil {
		t.Fatal(err)
	}

	report, err := repository.Snapshot(ctx, snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if !report.CancelRequested ||
		report.Status != StatusPartiallySucceeded ||
		report.ErrorCode != "export_conflicts" ||
		report.Items[0].Status != ItemConflicted ||
		report.Items[0].OutcomeCode != "destination_exists" ||
		report.Items[0].OutcomeMessage != "The destination file already exists." {
		t.Fatalf("Snapshot(export report) = %#v", report)
	}
	if _, err := sqlDatabase.SQL().ExecContext(
		ctx,
		"DELETE FROM stories WHERE id = ?",
		storyID,
	); err != nil {
		t.Fatalf("Delete story after export report error = %v", err)
	}
	report, err = repository.Snapshot(ctx, snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if report.Items[0].StoryID != storyID ||
		report.Items[0].StoryUUID != storyUUID {
		t.Fatalf("immutable resolved story after deletion = %#v", report.Items[0])
	}
}

func TestRepositoryRejectsInvalidExportScope(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	_, err := repository.CreateExport(
		context.Background(),
		uuid.NewString(),
		ExportSource{
			Type:       ExportSourceCurrentQuery,
			ShelfIDs:   []int64{7},
			ShelfNames: []string{"Unexpected"},
		},
		"/tmp/export",
		[]NewItem{{
			StoryID:             1,
			StoryUUID:           "00112233-4455-4677-8899-aabbccddeeff",
			StoryTitle:          "Story",
			SourceName:          "story.zip",
			OutputName:          "Story.zip",
			ArchiveRelativePath: "archives/aa/story.zip",
			ArchiveSHA256:       strings.Repeat("a", 64),
		}},
		time.Now(),
	)
	if !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("CreateExport(invalid) error = %v", err)
	}
}

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

func TestRepositoryAllowsOnlyOneActiveMetadataSync(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	ctx := context.Background()
	first, err := repository.CreateMetadataSync(
		ctx,
		"00000000-0000-4000-8000-000000000010",
		"en-GB",
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if first.Kind != KindMetadataSync ||
		first.TotalItems != 1 ||
		len(first.Items) != 1 ||
		first.Items[0].SourceName != "en-GB" {
		t.Fatalf("CreateMetadataSync() = %#v", first)
	}
	if _, err := repository.CreateMetadataSync(
		ctx,
		"00000000-0000-4000-8000-000000000011",
		"en-GB",
		time.Now(),
	); err == nil {
		t.Fatal("CreateMetadataSync(concurrent) error = nil")
	}
	if err := repository.MarkItemRunning(ctx, first.Items[0].ID); err != nil {
		t.Fatal(err)
	}
	if err := repository.CompleteItem(ctx, first.ID, ItemSnapshot{
		ID:             first.Items[0].ID,
		SourceName:     "en-GB",
		Status:         ItemSucceeded,
		OutcomeCode:    "metadata_refreshed",
		OutcomeMessage: "Official metadata refreshed.",
		CompletedBytes: 1,
		TotalBytes:     1,
	}); err != nil {
		t.Fatal(err)
	}
	if err := repository.Finish(
		ctx,
		first.ID,
		StatusSucceeded,
		"",
		"",
		time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	if _, err := repository.CreateMetadataSync(
		ctx,
		"00000000-0000-4000-8000-000000000012",
		"en-GB",
		time.Now(),
	); err != nil {
		t.Fatalf("CreateMetadataSync(after completion) error = %v", err)
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
