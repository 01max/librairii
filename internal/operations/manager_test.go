package operations

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/storage"
	"github.com/google/uuid"
)

func TestManagerBoundsConcurrentImportsAndPersistsCompletion(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	started := make(chan struct{}, 2)
	release := make(chan struct{})
	var startedCount atomic.Int64
	imports := &trackingImportService{
		handler: func(ctx context.Context, _ string) (importer.Outcome, error) {
			if startedCount.Add(1) <= 2 {
				started <- struct{}{}
			}
			select {
			case <-release:
				return importer.Outcome{Code: importer.OutcomeImported}, nil
			case <-ctx.Done():
				return importer.Outcome{}, ctx.Err()
			}
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
	for range 2 {
		select {
		case <-started:
		case <-time.After(5 * time.Second):
			t.Fatal("two import workers did not run concurrently")
		}
	}
	close(release)
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

func TestManagerWorkerEventsRetainLifecycleContext(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	imports := &trackingImportService{}
	manager, err := NewManager(Dependencies{
		Repository: repository,
		Imports:    imports,
		Events:     events,
		Clock: &testClock{
			now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC),
		},
		StagingCleaner: fakeCleaner{},
		Workers:        1,
		RecoveryDelay:  5 * time.Millisecond,
	})
	if err != nil {
		t.Fatal(err)
	}
	type lifecycleContextKey struct{}
	lifecycleContext := context.WithValue(
		context.Background(),
		lifecycleContextKey{},
		"wails-runtime",
	)
	if err := manager.Start(lifecycleContext); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := manager.Close(); err != nil {
			t.Error(err)
		}
	})

	created, err := manager.StartImport(
		context.Background(),
		makeSourcePaths(t, 1),
	)
	if err != nil {
		t.Fatal(err)
	}
	events.waitFor(t, created.ID, StatusRunning)
	eventContext := events.contextFor(created.ID, StatusRunning)
	if eventContext == nil ||
		eventContext.Value(lifecycleContextKey{}) != "wails-runtime" {
		t.Fatal("worker event lost the application lifecycle context")
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

func TestManagerRunsMetadataRefreshWithProgressAndMatchedCount(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	refresh := metadataRefreshFunc(func(
		context.Context,
		string,
	) (metadata.RefreshResult, error) {
		return metadata.RefreshResult{
			Sync: metadata.CatalogSync{MatchedStoryCount: 4},
		}, nil
	})
	manager := newTestManagerWithMetadata(t, repository, refresh, events)

	created, err := manager.StartMetadataRefresh(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	if created.Kind != KindMetadataSync ||
		created.Status != StatusQueued ||
		created.CompletedItems != 0 ||
		created.TotalItems != 1 {
		t.Fatalf("StartMetadataRefresh() = %#v", created)
	}
	running := events.waitFor(t, created.ID, StatusRunning)
	if running.CompletedItems != 0 || running.Items[0].Status == ItemSucceeded {
		t.Fatalf("running metadata snapshot = %#v", running)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusSucceeded ||
		terminal.CompletedItems != 1 ||
		terminal.Items[0].Status != ItemSucceeded ||
		terminal.Items[0].OutcomeCode != "metadata_refreshed" ||
		terminal.Items[0].OutcomeMessage !=
			"Official metadata refreshed; 4 local stories matched." {
		t.Fatalf("terminal metadata snapshot = %#v", terminal)
	}
}

func TestManagerRunsExportWithPersistedProgressAndMixedReport(t *testing.T) {
	t.Parallel()

	repository, sqlDatabase := newOperationRepository(t)
	workItems := makeExportWorkItems(t, sqlDatabase, 3)
	workItems[2].PlannedStatus = ItemSkipped
	workItems[2].OutcomeCode = "archive_missing"
	workItems[2].OutcomeMessage = "Managed archive bytes are missing."
	events := newEventRecorder()
	exports := exportServiceFunc(func(
		_ context.Context,
		item NewItem,
		_ string,
		progress func(int64),
	) (ExportCopyResult, error) {
		progress(item.TotalBytes)
		if item.OutputName == "story-2.zip" {
			return ExportCopyResult{
				OutcomeCode: "filename_conflict",
			}, errors.New("destination appeared")
		}
		return ExportCopyResult{
			OutputName:  item.OutputName,
			ByteSize:    item.TotalBytes,
			SHA256:      item.ArchiveSHA256,
			OutcomeCode: "exported",
		}, nil
	})
	manager := newTestManagerWithExports(t, repository, exports, events, 2)

	created, err := manager.StartExport(
		context.Background(),
		ExportSource{
			Type:       ExportSourceShelves,
			ShelfIDs:   []int64{7, 8},
			ShelfNames: []string{"Bedtime", "Adventures"},
		},
		t.TempDir(),
		workItems,
	)
	if err != nil {
		t.Fatal(err)
	}
	if created.TotalBytes != workItems[0].Item.TotalBytes+workItems[1].Item.TotalBytes ||
		created.Items[2].TotalBytes != 0 {
		t.Fatalf("queued export totals = %#v", created)
	}
	progress := events.waitProgress(t, created.ID)
	if progress.Items[0].CompletedBytes == 0 &&
		progress.Items[1].CompletedBytes == 0 {
		t.Fatalf("running export progress = %#v", progress)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusPartiallySucceeded ||
		terminal.CompletedItems != 3 ||
		terminal.ExportSourceType != ExportSourceShelves ||
		!reflect.DeepEqual(terminal.SourceShelfNames, []string{"Bedtime", "Adventures"}) {
		t.Fatalf("terminal export = %#v", terminal)
	}
	statuses := map[string]ItemSnapshot{}
	for _, item := range terminal.Items {
		statuses[item.OutputName] = item
	}
	if statuses["story-1.zip"].Status != ItemSucceeded ||
		statuses["story-1.zip"].OutcomeCode != "exported" ||
		statuses["story-2.zip"].Status != ItemConflicted ||
		statuses["story-2.zip"].OutcomeCode != "filename_conflict" ||
		statuses["story-3.zip"].Status != ItemSkipped ||
		statuses["story-3.zip"].OutcomeCode != "archive_missing" {
		t.Fatalf("terminal export items = %#v", statuses)
	}
}

func TestManagerDoesNotApplyTheImportPickerLimitToExportScopes(t *testing.T) {
	t.Parallel()

	workItems := make([]ExportWorkItem, maxImportItems+1)
	for index := range workItems {
		workItems[index].PlannedStatus = ItemPending
	}
	manager := &Manager{}

	_, err := manager.StartExport(
		context.Background(),
		ExportSource{Type: ExportSourceSelection},
		t.TempDir(),
		workItems,
	)
	if !errors.Is(err, ErrManagerNotStarted) {
		t.Fatalf("StartExport(large scope) error = %v", err)
	}
}

func TestManagerCancelsMetadataRefreshAndRejectsConcurrentRequest(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	started := make(chan struct{})
	refresh := metadataRefreshFunc(func(
		ctx context.Context,
		_ string,
	) (metadata.RefreshResult, error) {
		close(started)
		<-ctx.Done()
		return metadata.RefreshResult{}, ctx.Err()
	})
	manager := newTestManagerWithMetadata(t, repository, refresh, events)
	created, err := manager.StartMetadataRefresh(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	<-started
	if _, err := manager.StartMetadataRefresh(
		context.Background(),
		"en-GB",
	); !errors.Is(err, ErrOperationActive) {
		t.Fatalf("StartMetadataRefresh(concurrent) error = %v", err)
	}
	if _, err := manager.Cancel(context.Background(), created.ID); err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusCancelled ||
		terminal.Items[0].Status != ItemCancelled ||
		terminal.Items[0].OutcomeCode != string(metadata.RefreshCancelled) {
		t.Fatalf("cancelled metadata snapshot = %#v", terminal)
	}
}

func TestManagerPersistsMetadataRefreshFailureCode(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	events := newEventRecorder()
	refresh := metadataRefreshFunc(func(
		context.Context,
		string,
	) (metadata.RefreshResult, error) {
		return metadata.RefreshResult{}, &metadata.RefreshError{
			Code:  metadata.RefreshInvalidCatalog,
			Cause: errors.New("fixture is corrupt"),
		}
	})
	manager := newTestManagerWithMetadata(t, repository, refresh, events)
	created, err := manager.StartMetadataRefresh(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusFailed ||
		terminal.ErrorCode != string(metadata.RefreshInvalidCatalog) ||
		terminal.Items[0].OutcomeCode != string(metadata.RefreshInvalidCatalog) ||
		terminal.ErrorMessage != "The downloaded official catalog was invalid." {
		t.Fatalf("failed metadata snapshot = %#v", terminal)
	}
}

func TestManagerStartupReconcilesOperationsAndStaging(t *testing.T) {
	t.Parallel()

	repository, sqlDatabase := newOperationRepository(t)
	ctx := context.Background()
	createdImport, err := repository.CreateImport(
		ctx,
		uuid.NewString(),
		[]NewItem{{SourceName: "abandoned.zip"}},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	createdMetadata, err := repository.CreateMetadataSync(
		ctx,
		uuid.NewString(),
		"en-GB",
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	exportWork := makeExportWorkItems(t, sqlDatabase, 1)
	exportDestination := t.TempDir()
	createdExport, err := repository.CreateExport(
		ctx,
		uuid.NewString(),
		ExportSource{Type: ExportSourceSelection},
		exportDestination,
		[]NewItem{exportWork[0].Item},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	created := []Snapshot{createdImport, createdMetadata, createdExport}
	for _, snapshot := range created {
		if err := repository.MarkRunning(ctx, snapshot.ID, time.Now()); err != nil {
			t.Fatal(err)
		}
		if err := repository.MarkItemRunning(ctx, snapshot.Items[0].ID); err != nil {
			t.Fatal(err)
		}
	}

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	abandoned := filepath.Join(layout.Staging, "import-abandoned")
	if err := os.MkdirAll(abandoned, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(abandoned, "archive"),
		[]byte("partial"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
	var recoveredDestinations []string
	recovery := exportRecoveryFunc(func(destination string) error {
		recoveredDestinations = append(recoveredDestinations, destination)
		return nil
	})
	events := newEventRecorder()
	manager, err := NewManager(Dependencies{
		Repository:     repository,
		Imports:        &trackingImportService{},
		ExportRecovery: recovery,
		Events:         events,
		Clock:          &testClock{now: time.Now()},
		StagingCleaner: archive.NewRepository(layout),
		Workers:        1,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := manager.Start(ctx); err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	t.Cleanup(func() {
		_ = manager.Close()
	})

	wantMessage := map[Kind]string{
		KindImport:       "Select the archive again to retry.",
		KindMetadataSync: "Start a new refresh to retry.",
		KindExport:       "Completed files were preserved; start a new export to retry.",
	}
	emitted := make(map[string]Snapshot, len(created))
	for range created {
		snapshot := <-events.events
		emitted[snapshot.ID] = snapshot
	}
	for _, active := range created {
		reconciled, err := manager.Snapshot(ctx, active.ID)
		if err != nil {
			t.Fatal(err)
		}
		if reconciled.Status != StatusInterrupted ||
			reconciled.ErrorCode != "interrupted" ||
			!strings.Contains(reconciled.ErrorMessage, wantMessage[active.Kind]) ||
			reconciled.Items[0].OutcomeCode != "interrupted" {
			t.Fatalf("reconciled %s snapshot = %#v", active.Kind, reconciled)
		}
		if got := emitted[active.ID]; got.Status != StatusInterrupted {
			t.Fatalf("reconciliation event = %#v", got)
		}
	}
	if !reflect.DeepEqual(recoveredDestinations, []string{exportDestination}) {
		t.Fatalf("recovered destinations = %#v", recoveredDestinations)
	}
	if entries, err := os.ReadDir(layout.Staging); err != nil || len(entries) != 0 {
		t.Fatalf("staging entries = %#v, %v", entries, err)
	}
}

func TestManagerStartupReportsExportCleanupFailureAndContinues(t *testing.T) {
	t.Parallel()

	repository, sqlDatabase := newOperationRepository(t)
	ctx := context.Background()
	exportWork := makeExportWorkItems(t, sqlDatabase, 1)
	created, err := repository.CreateExport(
		ctx,
		uuid.NewString(),
		ExportSource{Type: ExportSourceSelection},
		t.TempDir(),
		[]NewItem{exportWork[0].Item},
		time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	events := newEventRecorder()
	manager, err := NewManager(Dependencies{
		Repository: repository,
		Imports:    &trackingImportService{},
		ExportRecovery: exportRecoveryFunc(func(string) error {
			return errors.New("destination is unavailable")
		}),
		Events:         events,
		Clock:          &testClock{now: time.Now()},
		StagingCleaner: fakeCleaner{},
		Workers:        1,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := manager.Start(ctx); err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	t.Cleanup(func() {
		_ = manager.Close()
	})

	reconciled, err := manager.Snapshot(ctx, created.ID)
	if err != nil {
		t.Fatal(err)
	}
	if reconciled.Status != StatusInterrupted ||
		reconciled.ErrorCode != "interrupted_cleanup_failed" ||
		!strings.Contains(reconciled.ErrorMessage, "Check the destination folder") ||
		reconciled.Items[0].OutcomeCode != "interrupted_cleanup_failed" {
		t.Fatalf("reconciled snapshot = %#v", reconciled)
	}
	if emitted := events.waitFor(t, created.ID, StatusInterrupted); emitted.Status != StatusInterrupted {
		t.Fatalf("reconciliation event = %#v", emitted)
	}
}

func TestManagerRetriesTransientItemCompletionFailure(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	flaky := &flakyOperationRepository{
		managerRepository: repository,
		completeFailures:  1,
	}
	events := newEventRecorder()
	manager := newTestManagerWithRepository(
		t,
		flaky,
		&trackingImportService{},
		events,
		fakeCleaner{},
		1,
	)

	created, err := manager.StartImport(context.Background(), makeSourcePaths(t, 1))
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusSucceeded {
		t.Fatalf("terminal snapshot = %#v", terminal)
	}
	if flaky.completeCalls != 2 {
		t.Fatalf("CompleteItem() calls = %d, want 2", flaky.completeCalls)
	}
}

func TestManagerInterruptsOperationWhenFinishCannotPersist(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	flaky := &flakyOperationRepository{
		managerRepository: repository,
		finishFailures:    persistenceAttempts,
	}
	events := newEventRecorder()
	manager := newTestManagerWithRepository(
		t,
		flaky,
		&trackingImportService{},
		events,
		fakeCleaner{},
		1,
	)

	created, err := manager.StartImport(context.Background(), makeSourcePaths(t, 1))
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusInterrupted ||
		terminal.ErrorCode != "persistence_failed" {
		t.Fatalf("terminal snapshot = %#v", terminal)
	}
	if flaky.finishCalls != persistenceAttempts || flaky.interruptCalls != 1 {
		t.Fatalf(
			"Finish() calls = %d, Interrupt() calls = %d",
			flaky.finishCalls,
			flaky.interruptCalls,
		)
	}

	persisted, err := repository.Snapshot(context.Background(), created.ID)
	if err != nil {
		t.Fatal(err)
	}
	if persisted.Status != StatusInterrupted ||
		persisted.CompletedItems != persisted.TotalItems {
		t.Fatalf("persisted snapshot = %#v", persisted)
	}
}

func TestManagerInterruptsOperationWhenItemCompletionCannotPersist(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	flaky := &flakyOperationRepository{
		managerRepository: repository,
		completeFailures:  persistenceAttempts,
	}
	events := newEventRecorder()
	manager := newTestManagerWithRepository(
		t,
		flaky,
		&trackingImportService{},
		events,
		fakeCleaner{},
		1,
	)

	created, err := manager.StartImport(context.Background(), makeSourcePaths(t, 1))
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != StatusInterrupted ||
		terminal.ErrorCode != "persistence_failed" {
		t.Fatalf("terminal snapshot = %#v", terminal)
	}
	if flaky.completeCalls != persistenceAttempts || flaky.interruptCalls != 1 {
		t.Fatalf(
			"CompleteItem() calls = %d, Interrupt() calls = %d",
			flaky.completeCalls,
			flaky.interruptCalls,
		)
	}
}

func TestManagerKeepsReconcilingUntilInterruptionIsDurable(t *testing.T) {
	t.Parallel()

	repository, _ := newOperationRepository(t)
	flaky := &flakyOperationRepository{
		managerRepository: repository,
		completeFailures:  persistenceAttempts,
		interruptFailures: persistenceAttempts,
	}
	events := newEventRecorder()
	manager := newTestManagerWithRepository(
		t,
		flaky,
		&trackingImportService{},
		events,
		fakeCleaner{},
		1,
	)

	created, err := manager.StartImport(context.Background(), makeSourcePaths(t, 1))
	if err != nil {
		t.Fatal(err)
	}
	synthetic := events.waitTerminal(t, created.ID)
	if synthetic.Status != StatusInterrupted ||
		synthetic.ErrorCode != "persistence_failed" {
		t.Fatalf("synthetic snapshot = %#v", synthetic)
	}
	durable := events.waitFor(t, created.ID, StatusInterrupted)
	if durable.CompletedItems != durable.TotalItems {
		t.Fatalf("durable snapshot = %#v", durable)
	}
	if flaky.interruptCalls != persistenceAttempts+1 {
		t.Fatalf(
			"Interrupt() calls = %d, want %d",
			flaky.interruptCalls,
			persistenceAttempts+1,
		)
	}
	persisted, err := repository.Snapshot(context.Background(), created.ID)
	if err != nil {
		t.Fatal(err)
	}
	if persisted.Status != StatusInterrupted {
		t.Fatalf("persisted snapshot = %#v", persisted)
	}
}

type flakyOperationRepository struct {
	managerRepository

	mu                sync.Mutex
	completeFailures  int
	finishFailures    int
	interruptFailures int
	completeCalls     int
	finishCalls       int
	interruptCalls    int
}

func (r *flakyOperationRepository) CompleteItem(
	ctx context.Context,
	operationID string,
	item ItemSnapshot,
) error {
	r.mu.Lock()
	r.completeCalls++
	if r.completeFailures > 0 {
		r.completeFailures--
		r.mu.Unlock()
		return errors.New("injected item completion failure")
	}
	r.mu.Unlock()
	return r.managerRepository.CompleteItem(ctx, operationID, item)
}

func (r *flakyOperationRepository) Finish(
	ctx context.Context,
	id string,
	status Status,
	errorCode string,
	errorMessage string,
	now time.Time,
) error {
	r.mu.Lock()
	r.finishCalls++
	if r.finishFailures > 0 {
		r.finishFailures--
		r.mu.Unlock()
		return errors.New("injected operation finish failure")
	}
	r.mu.Unlock()
	return r.managerRepository.Finish(ctx, id, status, errorCode, errorMessage, now)
}

func (r *flakyOperationRepository) Interrupt(
	ctx context.Context,
	id string,
	errorCode string,
	errorMessage string,
	now time.Time,
) (Snapshot, error) {
	r.mu.Lock()
	r.interruptCalls++
	if r.interruptFailures > 0 {
		r.interruptFailures--
		r.mu.Unlock()
		return Snapshot{}, errors.New("injected operation interruption failure")
	}
	r.mu.Unlock()
	return r.managerRepository.Interrupt(ctx, id, errorCode, errorMessage, now)
}

type trackingImportService struct {
	mu            sync.Mutex
	active        int
	maximumActive int
	handler       func(context.Context, string) (importer.Outcome, error)
}

type metadataRefreshFunc func(
	context.Context,
	string,
) (metadata.RefreshResult, error)

func (f metadataRefreshFunc) Refresh(
	ctx context.Context,
	locale string,
) (metadata.RefreshResult, error) {
	return f(ctx, locale)
}

type exportServiceFunc func(
	context.Context,
	NewItem,
	string,
	func(int64),
) (ExportCopyResult, error)

func (f exportServiceFunc) Copy(
	ctx context.Context,
	item NewItem,
	destination string,
	progress func(int64),
) (ExportCopyResult, error) {
	return f(ctx, item, destination, progress)
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
	events   chan Snapshot
	mu       sync.Mutex
	contexts map[string]context.Context
}

func newEventRecorder() *eventRecorder {
	return &eventRecorder{
		events:   make(chan Snapshot, 256),
		contexts: make(map[string]context.Context),
	}
}

func (r *eventRecorder) Emit(ctx context.Context, name string, payload any) {
	if name != EventChanged {
		return
	}
	snapshot, ok := payload.(Snapshot)
	if ok {
		r.mu.Lock()
		r.contexts[eventContextKey(snapshot.ID, snapshot.Status)] = ctx
		r.mu.Unlock()
		r.events <- snapshot
	}
}

func (r *eventRecorder) contextFor(
	operationID string,
	status Status,
) context.Context {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.contexts[eventContextKey(operationID, status)]
}

func eventContextKey(operationID string, status Status) string {
	return operationID + ":" + string(status)
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

func (r *eventRecorder) waitProgress(
	t *testing.T,
	operationID string,
) Snapshot {
	t.Helper()
	timer := time.NewTimer(5 * time.Second)
	defer timer.Stop()
	for {
		select {
		case snapshot := <-r.events:
			if snapshot.ID != operationID || snapshot.Status != StatusRunning {
				continue
			}
			for _, item := range snapshot.Items {
				if item.CompletedBytes > 0 {
					return snapshot
				}
			}
		case <-timer.C:
			t.Fatalf("timed out waiting for export progress %s", operationID)
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

type exportRecoveryFunc func(string) error

func (f exportRecoveryFunc) CleanupAbandoned(destination string) error {
	return f(destination)
}

func newTestManager(
	t *testing.T,
	repository *Repository,
	imports ImportService,
	events *eventRecorder,
	cleaner StagingCleaner,
	workers int,
) *Manager {
	return newTestManagerWithRepository(t, repository, imports, events, cleaner, workers)
}

func newTestManagerWithRepository(
	t *testing.T,
	repository managerRepository,
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
		RecoveryDelay:  5 * time.Millisecond,
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

func newTestManagerWithMetadata(
	t *testing.T,
	repository managerRepository,
	refresh MetadataRefreshService,
	events *eventRecorder,
) *Manager {
	t.Helper()

	manager, err := NewManager(Dependencies{
		Repository:     repository,
		Imports:        &trackingImportService{},
		Metadata:       refresh,
		Events:         events,
		Clock:          &testClock{now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)},
		StagingCleaner: fakeCleaner{},
		Workers:        1,
		RecoveryDelay:  5 * time.Millisecond,
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

func newTestManagerWithExports(
	t *testing.T,
	repository managerRepository,
	exports ExportService,
	events *eventRecorder,
	workers int,
) *Manager {
	t.Helper()

	manager, err := NewManager(Dependencies{
		Repository:     repository,
		Imports:        &trackingImportService{},
		Exports:        exports,
		Events:         events,
		Clock:          &testClock{now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)},
		StagingCleaner: fakeCleaner{},
		Workers:        workers,
		RecoveryDelay:  5 * time.Millisecond,
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

func makeExportWorkItems(
	t *testing.T,
	sqlDatabase *database.Database,
	count int,
) []ExportWorkItem {
	t.Helper()

	items := make([]ExportWorkItem, 0, count)
	for index := range count {
		storyUUID := uuid.NewString()
		result, err := sqlDatabase.SQL().Exec(
			`INSERT INTO stories (uuid, embedded_title, display_name_normalized)
			 VALUES (?, ?, ?)`,
			storyUUID,
			"Story",
			"story",
		)
		if err != nil {
			t.Fatal(err)
		}
		storyID, err := result.LastInsertId()
		if err != nil {
			t.Fatal(err)
		}
		name := "story-" + string(rune('1'+index)) + ".zip"
		items = append(items, ExportWorkItem{
			Item: NewItem{
				StoryID:             storyID,
				StoryUUID:           storyUUID,
				StoryTitle:          "Story",
				SourceName:          name,
				OutputName:          name,
				ArchiveRelativePath: "archives/aa/" + name,
				ArchiveSHA256:       strings.Repeat("a", 64),
				TotalBytes:          42,
			},
			PlannedStatus: ItemPending,
		})
	}
	return items
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
