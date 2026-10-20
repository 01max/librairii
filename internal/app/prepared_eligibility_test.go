package app

import (
	"context"
	"crypto/sha256"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/operations"
)

func TestPreparedExportRejectsChangedFlagsBeforeCopying(t *testing.T) {
	ctx := context.Background()
	provider := newRuntimeStorageProvider(t)
	runtime, err := NewImportRuntime(provider, fixedClock{now: time.Now()}, &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}, 1)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(ctx); err != nil {
		t.Fatal(err)
	}
	defer runtime.Close()
	data := []byte("managed archive")
	if err := os.WriteFile(filepath.Join(provider.Layout().Archives, "prepared.zip"), data, 0600); err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256(data)
	story, _, err := catalog.NewRepository(provider.Writer()).Create(ctx, catalog.CreateStory{UUID: "123e4567-e89b-42d3-a456-426614174000", EmbeddedTitle: "Prepared", OriginalFilename: "prepared.zip", DetectedFormat: catalog.FormatZIP, SHA256: fmt.Sprintf("%x", sum), ByteSize: int64(len(data)), ManagedPath: "archives/prepared.zip"})
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	prepared, err := runtime.PrepareExport(ctx, exporter.PreflightRequest{SourceType: operations.ExportSourceSelection, StoryIDs: []int64{story.ID}}, destination)
	if err != nil || !prepared.CanExport {
		t.Fatalf("prepare: %#v %v", prepared, err)
	}
	if _, err := runtime.SetStoriesBroken(ctx, []int64{story.ID}, true); err != nil {
		t.Fatal(err)
	}
	if _, err := runtime.StartPreparedExport(ctx, prepared.PreparationID); !errors.Is(err, exporter.ErrPreparedExportStale) {
		t.Fatalf("stale preview started: %v", err)
	}
	if _, err := os.Stat(filepath.Join(destination, "prepared.zip")); !os.IsNotExist(err) {
		t.Fatalf("flagged archive copied: %v", err)
	}
	fresh, err := runtime.PrepareExport(ctx, exporter.PreflightRequest{SourceType: operations.ExportSourceSelection, StoryIDs: []int64{story.ID}}, destination)
	if err != nil || fresh.CanExport || fresh.ReadyCount != 0 || len(fresh.Items) != 1 || fresh.Items[0].Issue == nil || fresh.Items[0].Issue.Code != exporter.IssueStoryMarkedBroken {
		t.Fatalf("fresh eligibility: %#v %v", fresh, err)
	}
}
