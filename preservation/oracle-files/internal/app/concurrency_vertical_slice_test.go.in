package app

import (
	"archive/zip"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
)

func TestRuntimeKeepsBrowsingResponsiveDuringImportRefreshAndExport(
	t *testing.T,
) {
	t.Parallel()

	ctx := context.Background()
	provider := newRuntimeStorageProvider(t)
	payload, err := os.ReadFile("../lunii/testdata/catalog.json")
	if err != nil {
		t.Fatal(err)
	}
	events := &runtimeEventRecorder{
		events: make(chan operations.Snapshot, 256),
	}
	gate := newRuntimeActivityGate()
	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(
			2026,
			time.July,
			25,
			17,
			0,
			0,
			0,
			time.UTC,
		)},
		events,
		2,
		WithMetadataFetcher(&gatedCatalogFetcher{
			delegate: &runtimeCatalogFetcher{payload: payload},
			gate:     gate,
		}),
		func(runtime *ImportRuntime) {
			runtime.decorateImport = func(
				delegate operations.ImportService,
			) operations.ImportService {
				return &gatedImportService{delegate: delegate, gate: gate}
			}
			runtime.decorateExport = func(
				delegate operations.ExportService,
			) operations.ExportService {
				return &gatedExportService{delegate: delegate, gate: gate}
			}
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(ctx); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := runtime.Close(); err != nil {
			t.Error(err)
		}
	})

	initialPath, err := testfixture.WriteZIP(
		t.TempDir(),
		testfixture.GenericZIP(),
	)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := runtime.StartImport(ctx, []string{initialPath})
	if err != nil {
		t.Fatal(err)
	}
	initial = waitRuntimeTerminal(t, runtime, initial.ID)
	if initial.Status != operations.StatusSucceeded {
		t.Fatalf("initial import = %#v", initial)
	}
	destination := t.TempDir()
	preflight, err := runtime.PrepareExport(
		ctx,
		exporter.PreflightRequest{
			SourceType: operations.ExportSourceSelection,
			StoryIDs:   []int64{initial.Items[0].StoryID},
		},
		destination,
	)
	if err != nil || !preflight.CanExport {
		t.Fatalf("PrepareExport() = %#v, %v", preflight, err)
	}
	concurrentPath, err := testfixture.WriteZIP(
		t.TempDir(),
		concurrentPlainPKFixture(t),
	)
	if err != nil {
		t.Fatal(err)
	}

	gate.Enable()
	concurrentImport, err := runtime.StartImport(ctx, []string{concurrentPath})
	if err != nil {
		t.Fatal(err)
	}
	concurrentRefresh, err := runtime.StartMetadataRefresh(
		ctx,
		metadata.DefaultLocale,
	)
	if err != nil {
		t.Fatal(err)
	}
	concurrentExport, err := runtime.StartPreparedExport(
		ctx,
		preflight.PreparationID,
	)
	if err != nil {
		t.Fatal(err)
	}
	results := []operations.Snapshot{
		concurrentImport,
		concurrentRefresh,
		concurrentExport,
	}

	startedKinds := map[operations.Kind]bool{}
	for range 2 {
		select {
		case kind := <-gate.entered:
			startedKinds[kind] = true
		case <-time.After(time.Second):
			t.Fatal("timed out waiting for bounded workers to enter adapters")
		}
	}
	select {
	case kind := <-gate.entered:
		t.Fatalf("third operation %q exceeded the two-worker ceiling", kind)
	case <-time.After(75 * time.Millisecond):
	}
	if got := gate.Maximum(); got != 2 {
		t.Fatalf("maximum active operation workers = %d, want 2", got)
	}
	if got := provider.db.Writer().Stats().InUse; got != 0 {
		t.Fatalf("writer connections held during external work = %d, want 0", got)
	}
	running := 0
	pending := 0
	for _, result := range results {
		snapshot, snapshotErr := runtime.Snapshot(ctx, result.ID)
		if snapshotErr != nil {
			t.Fatal(snapshotErr)
		}
		switch snapshot.Status {
		case operations.StatusRunning:
			running++
		case operations.StatusQueued:
			pending++
		default:
			t.Fatalf("operation escaped activity barrier = %#v", snapshot)
		}
	}
	if running != 2 || pending != 1 {
		t.Fatalf("bounded operation states = %d running, %d pending", running, pending)
	}

	browseContext, cancelBrowse := context.WithTimeout(ctx, 750*time.Millisecond)
	defer cancelBrowse()
	for range 100 {
		page, searchErr := runtime.Search(
			browseContext,
			library.StoryLibraryQuery{Page: 1, PageSize: 20},
		)
		if searchErr != nil {
			t.Fatalf("browsing while operations are in flight: %v", searchErr)
		}
		if page.TotalItems < 1 {
			t.Fatal(errors.New("browse snapshot lost committed stories"))
		}
	}

	gate.Release()
	for _, result := range results {
		terminal := waitRuntimeTerminal(t, runtime, result.ID)
		if terminal.Status != operations.StatusSucceeded {
			t.Fatalf("concurrent operation = %#v", terminal)
		}
	}
	if len(startedKinds) != 2 {
		t.Fatalf("initial active operation kinds = %#v", startedKinds)
	}
	select {
	case <-gate.entered:
	case <-time.After(time.Second):
		t.Fatal("queued third operation never entered a worker")
	}

	active, err := runtime.Active(ctx)
	if err != nil {
		t.Fatal(err)
	}
	for _, snapshot := range active {
		if !snapshot.Terminal() {
			t.Fatalf("nonterminal operation after completion = %#v", snapshot)
		}
	}
	page, err := runtime.Search(ctx, library.StoryLibraryQuery{
		Page:     1,
		PageSize: 20,
	})
	if err != nil {
		t.Fatal(err)
	}
	if page.TotalItems != 2 {
		t.Fatalf("final story count = %d, want 2", page.TotalItems)
	}
	exportedPath := filepath.Join(destination, initial.Items[0].SourceName)
	if _, err := os.Stat(exportedPath); err != nil {
		t.Fatalf("exported archive error = %v", err)
	}
}

type runtimeActivityGate struct {
	enabled atomic.Bool
	active  atomic.Int64
	maximum atomic.Int64
	entered chan operations.Kind
	release chan struct{}
	once    sync.Once
}

func newRuntimeActivityGate() *runtimeActivityGate {
	return &runtimeActivityGate{
		entered: make(chan operations.Kind, 3),
		release: make(chan struct{}),
	}
}

func (g *runtimeActivityGate) Enable() {
	g.enabled.Store(true)
}

func (g *runtimeActivityGate) Enter(
	ctx context.Context,
	kind operations.Kind,
) (bool, error) {
	if !g.enabled.Load() {
		return false, nil
	}
	active := g.active.Add(1)
	for {
		maximum := g.maximum.Load()
		if active <= maximum || g.maximum.CompareAndSwap(maximum, active) {
			break
		}
	}
	g.entered <- kind
	select {
	case <-g.release:
		return true, nil
	case <-ctx.Done():
		g.active.Add(-1)
		return false, ctx.Err()
	}
}

func (g *runtimeActivityGate) Leave(entered bool) {
	if entered {
		g.active.Add(-1)
	}
}

func (g *runtimeActivityGate) Maximum() int64 {
	return g.maximum.Load()
}

func (g *runtimeActivityGate) Release() {
	g.once.Do(func() { close(g.release) })
}

type gatedImportService struct {
	delegate operations.ImportService
	gate     *runtimeActivityGate
}

func (s *gatedImportService) Import(
	ctx context.Context,
	sourcePath string,
) (importer.Outcome, error) {
	entered, err := s.gate.Enter(ctx, operations.KindImport)
	if err != nil {
		return importer.Outcome{}, err
	}
	defer s.gate.Leave(entered)
	return s.delegate.Import(ctx, sourcePath)
}

type gatedExportService struct {
	delegate operations.ExportService
	gate     *runtimeActivityGate
}

func (s *gatedExportService) Copy(
	ctx context.Context,
	item operations.NewItem,
	destination string,
	progress func(int64),
) (operations.ExportCopyResult, error) {
	entered, err := s.gate.Enter(ctx, operations.KindExport)
	if err != nil {
		return operations.ExportCopyResult{}, err
	}
	defer s.gate.Leave(entered)
	return s.delegate.Copy(ctx, item, destination, progress)
}

type gatedCatalogFetcher struct {
	delegate metadata.CatalogFetcher
	gate     *runtimeActivityGate
}

func (f *gatedCatalogFetcher) FetchCatalog(
	ctx context.Context,
) ([]byte, error) {
	entered, err := f.gate.Enter(ctx, operations.KindMetadataSync)
	if err != nil {
		return nil, err
	}
	defer f.gate.Leave(entered)
	return f.delegate.FetchCatalog(ctx)
}

func concurrentPlainPKFixture(t *testing.T) testfixture.Archive {
	t.Helper()

	const storyUUID = "11112222-3333-4444-8555-666677778888"
	metadataBytes, err := json.Marshal(map[string]string{
		"uuid":        storyUUID,
		"title":       "The Concurrent Observatory",
		"description": "A copyright-free concurrency fixture.",
	})
	if err != nil {
		t.Fatal(err)
	}
	fixture := testfixture.PlainPK()
	fixture.Filename = "concurrent-observatory.plain.pk"
	fixture.ExpectedUUID = storyUUID
	fixture = testfixture.ReplaceEntry(fixture, testfixture.Entry{
		Name:   "_metadata.json",
		Bytes:  metadataBytes,
		Method: zip.Deflate,
	})
	return testfixture.ReplaceEntry(fixture, testfixture.Entry{
		Name:   "uuid.bin",
		Bytes:  testfixture.UUIDBytes(storyUUID),
		Method: zip.Store,
	})
}

func waitRuntimeTerminal(
	t *testing.T,
	runtime *ImportRuntime,
	operationID string,
) operations.Snapshot {
	t.Helper()

	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		snapshot, err := runtime.Snapshot(context.Background(), operationID)
		if err != nil {
			t.Fatal(err)
		}
		if snapshot.Terminal() {
			return snapshot
		}
		time.Sleep(5 * time.Millisecond)
	}
	t.Fatalf("timed out waiting for terminal operation %s", operationID)
	return operations.Snapshot{}
}
