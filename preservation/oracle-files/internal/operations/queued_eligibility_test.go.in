package operations

import (
	"context"
	"sync/atomic"
	"testing"
)

func TestManagerSkipsStoryFlaggedBeforeItsQueuedCopy(t *testing.T) {
	repository, db := newOperationRepository(t)
	items := makeExportWorkItems(t, db, 2)
	events := newEventRecorder()
	var copied atomic.Int32
	var secondFlagged atomic.Bool
	exports := exportServiceFunc(func(_ context.Context, item NewItem, _ string, _ func(int64)) (ExportCopyResult, error) {
		copied.Add(1)
		secondFlagged.Store(true)
		return ExportCopyResult{OutputName: item.OutputName, ByteSize: item.TotalBytes, SHA256: item.ArchiveSHA256}, nil
	})
	manager := newTestManagerWithExports(t, repository, exports, events, 1)
	manager.deps.CheckExportEligibility = func(_ context.Context, item NewItem) (string, string, error) {
		if item.OutputName == "story-2.zip" && secondFlagged.Load() {
			return "story_marked_broken", "This story is marked as broken.", nil
		}
		return "", "", nil
	}
	created, err := manager.StartExport(context.Background(), ExportSource{Type: ExportSourceSelection}, t.TempDir(), items)
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusPartiallySucceeded || copied.Load() != 1 || terminal.Items[1].Status != ItemSkipped || terminal.Items[1].OutcomeCode != "story_marked_broken" {
		t.Fatalf("queued flag was ignored: %#v copies=%d", terminal, copied.Load())
	}
}
