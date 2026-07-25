package app

import (
	"archive/zip"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/01max/librairii/internal/exporter"
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
		WithMetadataFetcher(&runtimeCatalogFetcher{payload: payload}),
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

	type startResult struct {
		snapshot operations.Snapshot
		err      error
	}
	start := make(chan struct{})
	importResult := make(chan startResult, 1)
	refreshResult := make(chan startResult, 1)
	exportResult := make(chan startResult, 1)
	browseErrors := make(chan error, 1)
	var activity sync.WaitGroup
	activity.Add(4)
	go func() {
		defer activity.Done()
		<-start
		snapshot, startErr := runtime.StartImport(ctx, []string{concurrentPath})
		importResult <- startResult{snapshot: snapshot, err: startErr}
	}()
	go func() {
		defer activity.Done()
		<-start
		snapshot, startErr := runtime.StartMetadataRefresh(
			ctx,
			metadata.DefaultLocale,
		)
		refreshResult <- startResult{snapshot: snapshot, err: startErr}
	}()
	go func() {
		defer activity.Done()
		<-start
		snapshot, startErr := runtime.StartPreparedExport(
			ctx,
			preflight.PreparationID,
		)
		exportResult <- startResult{snapshot: snapshot, err: startErr}
	}()
	go func() {
		defer activity.Done()
		<-start
		for range 100 {
			page, searchErr := runtime.Search(ctx, library.StoryLibraryQuery{
				Page:     1,
				PageSize: 20,
			})
			if searchErr != nil {
				browseErrors <- searchErr
				return
			}
			if page.TotalItems < 1 {
				browseErrors <- errors.New("browse snapshot lost committed stories")
				return
			}
		}
		browseErrors <- nil
	}()
	close(start)
	activity.Wait()

	results := []startResult{
		<-importResult,
		<-refreshResult,
		<-exportResult,
	}
	if err := <-browseErrors; err != nil {
		t.Fatalf("concurrent browsing error = %v", err)
	}
	for _, result := range results {
		if result.err != nil {
			t.Fatalf("start concurrent operation error = %v", result.err)
		}
		terminal := waitRuntimeTerminal(t, runtime, result.snapshot.ID)
		if terminal.Status != operations.StatusSucceeded {
			t.Fatalf("concurrent operation = %#v", terminal)
		}
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
