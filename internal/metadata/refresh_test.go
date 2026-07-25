package metadata

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/storage"
)

func TestRefreshServiceStagesValidatesAndActivatesCatalog(t *testing.T) {
	t.Parallel()

	harness := newRefreshHarness(t)
	harness.fetcher.payload = readCatalogFixture(t)
	harness.setIDs("123e4567-e89b-42d3-a456-426614174300")

	result, err := harness.service.Refresh(context.Background(), "en_GB")
	if err != nil {
		t.Fatal(err)
	}
	if result.Sync.Status != SyncSucceeded ||
		result.Snapshot.Status != SnapshotActive ||
		result.StoryCount != 1 ||
		result.Snapshot.RecordCount != 1 ||
		result.Snapshot.Locale != "en-GB" {
		t.Fatalf("Refresh() = %#v", result)
	}
	rawPath, err := harness.store.Resolve(result.Snapshot.RawPath)
	if err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(rawPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(raw) != string(harness.fetcher.payload) {
		t.Fatal("active raw snapshot bytes changed")
	}
	rows, err := harness.repository.MetadataForSnapshot(
		context.Background(),
		result.Snapshot.ID,
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) != 1 ||
		rows[0].Title != "The Clockwork Mountain" ||
		rows[0].Locale != "en-GB" ||
		rows[0].Language != "en-GB" {
		t.Fatalf("official metadata = %#v", rows)
	}
	assertNoRefreshStaging(t, harness.layout)
}

func TestRefreshServiceKeepsLastKnownGoodAfterCorruptCatalog(t *testing.T) {
	t.Parallel()

	harness := newRefreshHarness(t)
	harness.fetcher.payload = readCatalogFixture(t)
	harness.setIDs(
		"123e4567-e89b-42d3-a456-426614174301",
		"123e4567-e89b-42d3-a456-426614174302",
	)
	first, err := harness.service.Refresh(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}

	harness.fetcher.payload = []byte(`{"response":`)
	_, err = harness.service.Refresh(context.Background(), "en-GB")
	if !RefreshErrorHasCode(err, RefreshInvalidCatalog) {
		t.Fatalf("Refresh(corrupt) error = %v", err)
	}
	active, err := harness.repository.ActiveSnapshot(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	if active.ID != first.Snapshot.ID {
		t.Fatalf("active snapshot = %#v, want id %d", active, first.Snapshot.ID)
	}
	failed, err := harness.repository.Sync(
		context.Background(),
		"123e4567-e89b-42d3-a456-426614174302",
	)
	if err != nil {
		t.Fatal(err)
	}
	if failed.Status != SyncFailed ||
		failed.ErrorCode != string(RefreshInvalidCatalog) ||
		failed.ErrorMessage != refreshFailureMessage(RefreshInvalidCatalog) {
		t.Fatalf("failed sync = %#v", failed)
	}
	entries, err := os.ReadDir(harness.layout.Catalog)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != first.Sync.ID {
		t.Fatalf("catalog entries = %#v", entries)
	}
	assertNoRefreshStaging(t, harness.layout)
}

func TestRefreshServiceKeepsLastKnownGoodAfterTimeoutOrOfflineFailure(t *testing.T) {
	t.Parallel()

	failures := []struct {
		name string
		err  error
	}{
		{name: "timeout", err: context.DeadlineExceeded},
		{name: "offline", err: errors.New("network is offline")},
	}
	for index, failure := range failures {
		failure := failure
		t.Run(failure.name, func(t *testing.T) {
			t.Parallel()

			harness := newRefreshHarness(t)
			harness.fetcher.payload = readCatalogFixture(t)
			harness.setIDs(
				"123e4567-e89b-42d3-a456-426614174310",
				[]string{
					"123e4567-e89b-42d3-a456-426614174311",
					"123e4567-e89b-42d3-a456-426614174312",
				}[index],
			)
			first, err := harness.service.Refresh(context.Background(), "en-GB")
			if err != nil {
				t.Fatal(err)
			}

			harness.fetcher.payload = nil
			harness.fetcher.err = failure.err
			_, err = harness.service.Refresh(context.Background(), "en-GB")
			if !RefreshErrorHasCode(err, RefreshFetchFailed) {
				t.Fatalf("Refresh(%s) error = %v", failure.name, err)
			}
			active, err := harness.repository.ActiveSnapshot(context.Background(), "en-GB")
			if err != nil {
				t.Fatal(err)
			}
			if active.ID != first.Snapshot.ID {
				t.Fatalf("active snapshot = %#v", active)
			}
		})
	}
}

func TestRefreshServiceRejectsStagedSnapshotWhenActivationFails(t *testing.T) {
	t.Parallel()

	harness := newRefreshHarness(t)
	harness.fetcher.payload = readCatalogFixture(t)
	harness.setIDs("123e4567-e89b-42d3-a456-426614174320")
	first, err := harness.service.Refresh(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}

	failingRepository := &activationFailureRepository{
		Repository: harness.repository,
		err:        errors.New("injected activation failure"),
	}
	service, err := NewRefreshService(harness.fetcher, failingRepository, harness.store)
	if err != nil {
		t.Fatal(err)
	}
	service.now = harness.service.now
	service.newID = func() string {
		return "123e4567-e89b-42d3-a456-426614174321"
	}
	_, err = service.Refresh(context.Background(), "en-GB")
	if !RefreshErrorHasCode(err, RefreshPersistenceFailed) {
		t.Fatalf("Refresh(activation failure) error = %v", err)
	}
	active, err := harness.repository.ActiveSnapshot(context.Background(), "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	if active.ID != first.Snapshot.ID {
		t.Fatalf("active snapshot = %#v", active)
	}
	var rejected int64
	if err := harness.connection.QueryRow(
		`SELECT id
		 FROM catalog_snapshots
		 WHERE sync_id = ? AND status = 'rejected'`,
		"123e4567-e89b-42d3-a456-426614174321",
	).Scan(&rejected); err != nil {
		t.Fatal(err)
	}
	if rejected == 0 {
		t.Fatal("failed activation did not reject staged snapshot")
	}
}

func TestRefreshServicePersistsCancellationWithUncancelledCleanupContext(t *testing.T) {
	t.Parallel()

	harness := newRefreshHarness(t)
	harness.setIDs("123e4567-e89b-42d3-a456-426614174330")
	cancelled, cancel := context.WithCancel(context.Background())
	cancel()
	harness.fetcher.err = context.Canceled

	_, err := harness.service.Refresh(cancelled, "en-GB")
	if !RefreshErrorHasCode(err, RefreshCancelled) {
		t.Fatalf("Refresh(cancelled before sync creation) error = %v", err)
	}

	activeContext, activeCancel := context.WithCancel(context.Background())
	harness.setIDs("123e4567-e89b-42d3-a456-426614174331")
	harness.service.fetcher = &cancellingFetcher{cancel: activeCancel}
	_, err = harness.service.Refresh(activeContext, "en-GB")
	if !RefreshErrorHasCode(err, RefreshCancelled) {
		t.Fatalf("Refresh(cancelled fetch) error = %v", err)
	}
	sync, err := harness.repository.Sync(
		context.Background(),
		"123e4567-e89b-42d3-a456-426614174331",
	)
	if err != nil {
		t.Fatal(err)
	}
	if sync.Status != SyncCancelled ||
		sync.ErrorCode != string(RefreshCancelled) {
		t.Fatalf("cancelled sync = %#v", sync)
	}
}

type stubCatalogFetcher struct {
	payload []byte
	err     error
}

func (f *stubCatalogFetcher) FetchCatalog(context.Context) ([]byte, error) {
	return f.payload, f.err
}

type cancellingFetcher struct {
	cancel context.CancelFunc
}

func (f *cancellingFetcher) FetchCatalog(context.Context) ([]byte, error) {
	f.cancel()
	return nil, context.Canceled
}

type activationFailureRepository struct {
	*Repository
	err error
}

func (r *activationFailureRepository) ActivateSnapshot(
	context.Context,
	int64,
	int,
	time.Time,
) error {
	return r.err
}

type refreshHarness struct {
	service    *RefreshService
	fetcher    *stubCatalogFetcher
	repository *Repository
	store      *RawSnapshotStore
	layout     storage.Layout
	connection *sql.DB
	ids        []string
}

func newRefreshHarness(t *testing.T) *refreshHarness {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := opened.Close(); err != nil {
			t.Error(err)
		}
	})
	repository := NewRepository(opened.SQL())
	store := NewRawSnapshotStore(layout)
	fetcher := &stubCatalogFetcher{}
	service, err := NewRefreshService(fetcher, repository, store)
	if err != nil {
		t.Fatal(err)
	}
	harness := &refreshHarness{
		service:    service,
		fetcher:    fetcher,
		repository: repository,
		store:      store,
		layout:     layout,
		connection: opened.SQL(),
	}
	service.now = func() time.Time {
		return time.Date(2026, time.July, 25, 12, 0, 0, 0, time.UTC)
	}
	service.newID = func() string {
		if len(harness.ids) == 0 {
			t.Fatal("refresh ID queue is empty")
		}
		id := harness.ids[0]
		harness.ids = harness.ids[1:]
		return id
	}
	return harness
}

func (h *refreshHarness) setIDs(ids ...string) {
	h.ids = append(h.ids, ids...)
}

func assertNoRefreshStaging(t *testing.T, layout storage.Layout) {
	t.Helper()

	entries, err := os.ReadDir(layout.Staging)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("refresh staging entries = %#v", entries)
	}
}
