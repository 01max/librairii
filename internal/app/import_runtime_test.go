package app

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/storage"
)

func TestImportRuntimeComposesNativeImportSlice(t *testing.T) {
	t.Parallel()

	provider := newRuntimeStorageProvider(t)
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}
	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)},
		events,
		2,
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := runtime.Snapshot(context.Background(), "missing"); !errors.Is(
		err,
		ErrImportRuntimeNotReady,
	) {
		t.Fatalf("Snapshot(before Start) error = %v", err)
	}
	if err := runtime.Start(context.Background()); err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	t.Cleanup(func() {
		if err := runtime.Close(); err != nil {
			t.Error(err)
		}
	})

	source, err := testfixture.WriteZIP(t.TempDir(), testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	created, err := runtime.StartImport(context.Background(), []string{source})
	if err != nil {
		t.Fatalf("StartImport() error = %v", err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != operations.StatusSucceeded ||
		terminal.Items[0].OutcomeCode != "imported" ||
		terminal.Items[0].SourceName != filepath.Base(source) {
		t.Fatalf("terminal snapshot = %#v", terminal)
	}
	if strings.Contains(terminal.Items[0].SourceName, filepath.Dir(source)) {
		t.Fatalf("snapshot exposed source path: %#v", terminal.Items[0])
	}
	after, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	if string(before) != string(after) {
		t.Fatal("import runtime modified the selected source")
	}

	page, err := runtime.List(context.Background(), library.ListRequest{})
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if page.TotalItems != 1 || len(page.Stories) != 1 {
		t.Fatalf("List() = %#v", page)
	}
	detail, err := runtime.Detail(context.Background(), page.Stories[0].ID)
	if err != nil {
		t.Fatalf("Detail() error = %v", err)
	}
	if detail.Archive.OriginalFilename != filepath.Base(source) ||
		detail.Archive.SHA256 == "" ||
		strings.Contains(detail.Archive.OriginalFilename, filepath.Dir(source)) {
		t.Fatalf("Detail() = %#v", detail)
	}
	removed, err := runtime.Remove(context.Background(), page.Stories[0].ID)
	if err != nil {
		t.Fatalf("Remove() error = %v", err)
	}
	if removed.StoryID != page.Stories[0].ID || removed.UUID != page.Stories[0].UUID {
		t.Fatalf("Remove() = %#v", removed)
	}
	afterRemoval, err := runtime.List(context.Background(), library.ListRequest{})
	if err != nil {
		t.Fatalf("List(after removal) error = %v", err)
	}
	if afterRemoval.TotalItems != 0 {
		t.Fatalf("List(after removal) = %#v", afterRemoval)
	}
	if trashFiles := regularFiles(t, provider.layout.Trash); len(trashFiles) != 1 {
		t.Fatalf("trash files = %#v", trashFiles)
	}
}

type runtimeStorageProvider struct {
	layout storage.Layout
	db     *database.Database
}

func newRuntimeStorageProvider(t *testing.T) *runtimeStorageProvider {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	sqlDatabase, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = sqlDatabase.Close()
	})
	return &runtimeStorageProvider{layout: layout, db: sqlDatabase}
}

func (p *runtimeStorageProvider) Layout() storage.Layout {
	return p.layout
}

func (p *runtimeStorageProvider) SQL() *sql.DB {
	return p.db.SQL()
}

type runtimeEventRecorder struct {
	events chan operations.Snapshot
}

func (r *runtimeEventRecorder) Emit(_ context.Context, name string, payload any) {
	if name != operations.EventChanged {
		return
	}
	if snapshot, ok := payload.(operations.Snapshot); ok {
		r.events <- snapshot
	}
}

func (r *runtimeEventRecorder) waitTerminal(
	t *testing.T,
	operationID string,
) operations.Snapshot {
	t.Helper()
	timer := time.NewTimer(5 * time.Second)
	defer timer.Stop()
	for {
		select {
		case snapshot := <-r.events:
			if snapshot.ID == operationID && snapshot.Terminal() {
				return snapshot
			}
		case <-timer.C:
			t.Fatalf("timed out waiting for operation %s", operationID)
		}
	}
}

func regularFiles(t *testing.T, root string) []string {
	t.Helper()
	var files []string
	if err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.Type().IsRegular() {
			files = append(files, path)
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	return files
}
