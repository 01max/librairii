package app

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
)

func TestFirstStoryVerticalSliceThroughPickerAndApplication(t *testing.T) {
	t.Parallel()

	provider := newRuntimeStorageProvider(t)
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}
	importRuntime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)},
		events,
		2,
	)
	if err != nil {
		t.Fatal(err)
	}
	source, err := testfixture.WriteZIP(t.TempDir(), testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	sourceBefore, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	dialogs := &recordingDialogs{paths: []string{source}}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    dialogs,
		Events:     events,
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: importRuntime,
		Library:    importRuntime,
		Removal:    importRuntime,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := application.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		application.Stop(context.Background())
	})

	started := application.SelectAndStartImport(context.Background())
	if started.Error != nil || started.Operation == nil || started.Cancelled {
		t.Fatalf("SelectAndStartImport() = %#v", started)
	}
	if len(dialogs.paths) != 1 || dialogs.paths[0] != source || !dialogs.request.Multiple {
		t.Fatalf("picker = %#v, %#v", dialogs.paths, dialogs.request)
	}
	terminal := events.waitTerminal(t, started.Operation.ID)
	if terminal.Status != operations.StatusSucceeded ||
		len(terminal.Items) != 1 ||
		terminal.Items[0].OutcomeCode != "imported" ||
		strings.Contains(terminal.Items[0].SourceName, filepath.Dir(source)) {
		t.Fatalf("terminal import = %#v", terminal)
	}

	pageResponse := application.ListStories(context.Background(), library.ListRequest{})
	if pageResponse.Error != nil ||
		pageResponse.Page == nil ||
		pageResponse.Page.TotalItems != 1 {
		t.Fatalf("ListStories() = %#v", pageResponse)
	}
	story := pageResponse.Page.Stories[0]
	detailResponse := application.StoryDetail(context.Background(), story.ID)
	if detailResponse.Error != nil ||
		detailResponse.Detail == nil ||
		detailResponse.Detail.Archive.OriginalFilename != filepath.Base(source) ||
		detailResponse.Detail.Archive.SHA256 == "" {
		t.Fatalf("StoryDetail() = %#v", detailResponse)
	}

	removed := application.RemoveStory(context.Background(), story.ID)
	if removed.Error != nil ||
		removed.Result == nil ||
		removed.Result.StoryID != story.ID {
		t.Fatalf("RemoveStory() = %#v", removed)
	}
	empty := application.ListStories(context.Background(), library.ListRequest{})
	if empty.Error != nil || empty.Page == nil || empty.Page.TotalItems != 0 {
		t.Fatalf("ListStories(after removal) = %#v", empty)
	}
	if trashFiles := regularFiles(t, provider.layout.Trash); len(trashFiles) != 1 {
		t.Fatalf("trash files = %#v", trashFiles)
	}
	sourceAfter, err := os.ReadFile(source)
	if err != nil || string(sourceAfter) != string(sourceBefore) {
		t.Fatalf("source bytes changed: %v", err)
	}
}
