package operations

import (
	"context"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/storage"
	"github.com/google/uuid"
)

func TestManagerBoundsConcurrentImportsAndPersistsCompletion(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	imports := &trackingImportService{
		handler: func(context.Context, string) (importer.Outcome, error) {
			time.Sleep(10 * time.Millisecond)
			return importer.Outcome{Code: importer.OutcomeImported}, nil
		},
	}
	manager := newTestManager(t, repository, imports, events, fakeCleaner{}, 2)
	paths := makeSourcePaths(t, 6)

	created, err := manager.StartImport(context.Background(), paths)
	if err != nil {
		t.Fatalf("StartImport() error = %v", err)
	}
	if created.Status != StatusQueued || created.TotalItems != len(paths) {
		t.Fatalf("StartImport() = %#v", created)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusSucceeded ||
		terminal.CompletedItems != len(paths) {
		t.Fatalf("terminal snapshot = %#v", terminal)
	}
	if imports.maximum() != 2 {
		t.Fatalf("maximum active imports = %d, want 2", imports.maximum())
	}
	for _, item := range terminal.Items {
		if item.Status != ItemSucceeded || item.OutcomeCode != "imported" {
			t.Fatalf("terminal item = %#v", item)
		}
	}
}

func TestManagerPersistsStableMixedOutcomeCodes(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	imports := &trackingImportService{
		handler: func(_ context.Context, source string) (importer.Outcome, error) {
			switch filepath.Base(source) {
			case "imported.zip":
				return importer.Outcome{Code: importer.OutcomeImported}, nil
			case "duplicate.zip":
				return importer.Outcome{Code: importer.OutcomeDuplicateChecksum}, nil
			case "conflict.zip":
				return importer.Outcome{Code: importer.OutcomeUUIDConflict}, nil
			default:
				return importer.Outcome{}, &importer.Error{
					Code: importer.ErrorInspect,
					Cause: &inspection.ValidationError{
						Code: inspection.CodeInvalidContainer,
					},
				}
			}
		},
	}
	manager := newTestManager(t, repository, imports, events, fakeCleaner{}, 2)
	paths := []string{
		filepath.Join(t.TempDir(), "imported.zip"),
		filepath.Join(t.TempDir(), "duplicate.zip"),
		filepath.Join(t.TempDir(), "conflict.zip"),
		filepath.Join(t.TempDir(), "invalid.zip"),
	}

	created, err := manager.StartImport(context.Background(), paths)
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusPartiallySucceeded {
		t.Fatalf("terminal status = %q", terminal.Status)
	}
	got := map[string]ItemSnapshot{}
	for _, item := range terminal.Items {
		got[item.SourceName] = item
	}
	if got["imported.zip"].Status != ItemSucceeded ||
		got["imported.zip"].OutcomeCode != "imported" ||
		got["duplicate.zip"].Status != ItemSkipped ||
		got["duplicate.zip"].OutcomeCode != "duplicate_checksum" ||
		got["conflict.zip"].Status != ItemConflicted ||
		got["conflict.zip"].OutcomeCode != "uuid_conflict" ||
		got["invalid.zip"].Status != ItemFailed ||
		got["invalid.zip"].OutcomeCode != "invalid_container" {
		t.Fatalf("terminal items = %#v", got)
	}
}

func TestManagerCancellationReachesRunningAndQueuedItems(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	imports := &trackingImportService{
		handler: func(ctx context.Context, _ string) (importer.Outcome, error) {
			<-ctx.Done()
			return importer.Outcome{}, ctx.Err()
		},
	}
	manager := newTestManager(t, repository, imports, events, fakeCleaner{}, 2)

	created, err := manager.StartImport(context.Background(), makeSourcePaths(t, 5))
	if err != nil {
		t.Fatal(err)
	}
	cancelled, err := manager.Cancel(context.Background(), created.ID)
	if err != nil {
		t.Fatalf("Cancel() error = %v", err)
	}
	if !cancelled.CancelRequested {
		t.Fatalf("Cancel() = %#v", cancelled)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusCancelled || terminal.CompletedItems != terminal.TotalItems {
		t.Fatalf("terminal snapshot = %#v", terminal)
	}
	for _, item := range terminal.Items {
		if item.Status != ItemCancelled || item.OutcomeCode != "cancelled" {
			t.Fatalf("cancelled item = %#v", item)
		}
	}
}

func TestManagerStartupReconcilesOperationsAndStaging(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	created, err := repository.CreateImport(
		context.Background(),
		uuid.NewString(),
		[]NewItem{{SourceName: "abandoned.zip"}},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkRunning(context.Background(), created.ID, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := repository.MarkItemRunning(context.Background(), created.Items[0].ID); err != nil {
		t.Fatal(err)
	}

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	abandoned := filepath.Join(layout.Staging, "import-abandoned")
	if err := os.MkdirAll(abandoned, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(abandoned, "archive"), []byte("partial"), 0o600); err != nil {
		t.Fatal(err)
	}
	events := newEventRecorder()
	manager, err := NewManager(Dependencies{
		Repository:     repository,
		Imports:        &trackingImportService{},
		Events:         events,
		Clock:          &testClock{now: time.Now()},
		StagingCleaner: archive.NewRepository(layout),
		Workers:        1,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := manager.Start(context.Background()); err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	t.Cleanup(func() {
		_ = manager.Close()
	})

	reconciled, err := manager.Snapshot(context.Background(), created.ID)
	if err != nil {
		t.Fatal(err)
	}
	if reconciled.Status != StatusInterrupted ||
		reconciled.Items[0].OutcomeCode != "interrupted" {
		t.Fatalf("reconciled snapshot = %#v", reconciled)
	}
	if entries, err := os.ReadDir(layout.Staging); err != nil || len(entries) != 0 {
		t.Fatalf("staging entries = %#v, %v", entries, err)
	}
	if emitted := events.waitFor(t, created.ID, StatusInterrupted); emitted.Status != StatusInterrupted {
		t.Fatalf("reconciliation event = %#v", emitted)
	}
}

type trackingImportService struct {
	mu            sync.Mutex
	active        int
	maximumActive int
	handler       func(context.Context, string) (importer.Outcome, error)
}

func (s *trackingImportService) Import(
	ctx context.Context,
	source string,
) (importer.Outcome, error) {
	s.mu.Lock()
	s.active++
	if s.active > s.maximumActive {
		s.maximumActive = s.active
	}
	s.mu.Unlock()
	defer func() {
		s.mu.Lock()
		s.active--
		s.mu.Unlock()
	}()
	if s.handler == nil {
		return importer.Outcome{Code: importer.OutcomeImported}, nil
	}
	return s.handler(ctx, source)
}

func (s *trackingImportService) maximum() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.maximumActive
}

type eventRecorder struct {
	events chan Snapshot
}

func newEventRecorder() *eventRecorder {
	return &eventRecorder{events: make(chan Snapshot, 256)}
}

func (r *eventRecorder) Emit(_ context.Context, name string, payload any) {
	if name != EventChanged {
		return
	}
	snapshot, ok := payload.(Snapshot)
	if ok {
		r.events <- snapshot
	}
}

func (r *eventRecorder) waitTerminal(t *testing.T, operationID string) Snapshot {
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
			t.Fatalf("timed out waiting for terminal operation %s", operationID)
		}
	}
}

func (r *eventRecorder) waitFor(
	t *testing.T,
	operationID string,
	status Status,
) Snapshot {
	t.Helper()
	timer := time.NewTimer(5 * time.Second)
	defer timer.Stop()
	for {
		select {
		case snapshot := <-r.events:
			if snapshot.ID == operationID && snapshot.Status == status {
				return snapshot
			}
		case <-timer.C:
			t.Fatalf("timed out waiting for %s operation %s", status, operationID)
		}
	}
}

type testClock struct {
	mu  sync.Mutex
	now time.Time
}

func (c *testClock) Now() time.Time {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.now = c.now.Add(time.Millisecond)
	return c.now
}

type fakeCleaner struct {
	err error
}

func (c fakeCleaner) CleanupAbandoned() error {
	return c.err
}

func newTestManager(
	t *testing.T,
	repository *Repository,
	imports ImportService,
	events *eventRecorder,
	cleaner StagingCleaner,
	workers int,
) *Manager {
	t.Helper()

	manager, err := NewManager(Dependencies{
		Repository:     repository,
		Imports:        imports,
		Events:         events,
		Clock:          &testClock{now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)},
		StagingCleaner: cleaner,
		Workers:        workers,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := manager.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := manager.Close(); err != nil {
			t.Error(err)
		}
	})
	return manager
}

func makeSourcePaths(t *testing.T, count int) []string {
	t.Helper()

	directory := t.TempDir()
	paths := make([]string, 0, count)
	for index := range count {
		path := filepath.Join(directory, uuid.NewString()+".zip")
		if err := os.WriteFile(path, []byte{byte(index)}, 0o600); err != nil {
			t.Fatal(err)
		}
		paths = append(paths, path)
	}
	return paths
}
