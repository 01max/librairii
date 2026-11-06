package app

import (
	"context"
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/operations"
)

func TestRuntimeSkipsPendingMemberFlaggedWhileEarlierCopyRuns(t *testing.T) {
	ctx := context.Background()
	provider := newRuntimeStorageProvider(t)
	gate := newRuntimeActivityGate()
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 64)}
	runtime, err := NewImportRuntime(provider, fixedClock{now: time.Now()}, events, 1, func(r *ImportRuntime) {
		r.decorateExport = func(delegate operations.ExportService) operations.ExportService {
			return &gatedExportService{delegate: delegate, gate: gate}
		}
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(ctx); err != nil {
		t.Fatal(err)
	}
	defer runtime.Close()
	defer gate.Release()
	ids := []int64{}
	for i := 1; i <= 2; i++ {
		name := fmt.Sprintf("queued-%d.zip", i)
		data := []byte(name)
		sum := sha256.Sum256(data)
		if err := os.WriteFile(filepath.Join(provider.Layout().Archives, name), data, 0600); err != nil {
			t.Fatal(err)
		}
		story, _, err := catalog.NewRepository(provider.Writer()).Create(ctx, catalog.CreateStory{UUID: fmt.Sprintf("123e4567-e89b-42d3-a456-%012d", i), EmbeddedTitle: name, OriginalFilename: name, DetectedFormat: catalog.FormatZIP, SHA256: fmt.Sprintf("%x", sum), ByteSize: int64(len(data)), ManagedPath: "archives/" + name})
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, story.ID)
	}
	destination := t.TempDir()
	prepared, err := runtime.PrepareExport(ctx, exporter.PreflightRequest{SourceType: operations.ExportSourceSelection, StoryIDs: ids}, destination)
	if err != nil || prepared.ReadyCount != 2 {
		t.Fatalf("preparation: %#v %v", prepared, err)
	}
	gate.Enable()
	started, err := runtime.StartPreparedExport(ctx, prepared.PreparationID)
	if err != nil {
		t.Fatal(err)
	}
	select {
	case <-gate.entered:
	case <-time.After(5 * time.Second):
		t.Fatal("first copy did not start")
	}
	if _, err := runtime.SetStoriesBroken(ctx, ids[1:], true); err != nil {
		t.Fatal(err)
	}
	gate.Release()
	terminal := events.waitTerminal(t, started.ID)
	if terminal.Status != operations.StatusPartiallySucceeded || terminal.Items[1].Status != operations.ItemSkipped || terminal.Items[1].OutcomeCode != "story_marked_broken" {
		t.Fatalf("queued eligibility: %#v", terminal)
	}
	if _, err := os.Stat(filepath.Join(destination, "queued-1.zip")); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(destination, "queued-2.zip")); !os.IsNotExist(err) {
		t.Fatalf("newly flagged file copied: %v", err)
	}
}
