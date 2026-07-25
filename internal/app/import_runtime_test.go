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

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/storage"
	"github.com/01max/librairii/internal/tagging"
	"github.com/google/uuid"
)

func TestImportRuntimeBoundsUnusedPreparedExports(t *testing.T) {
	t.Parallel()

	runtime := &ImportRuntime{
		preparedExports: make(map[string]exporter.PreflightReport),
	}
	preparationIDs := make([]string, 0, maxPreparedExports+1)
	for range maxPreparedExports + 1 {
		preparationID := uuid.NewString()
		preparationIDs = append(preparationIDs, preparationID)
		runtime.storePreparedExportLocked(exporter.PreflightReport{
			PreparationID: preparationID,
			CanExport:     true,
		})
	}

	if len(runtime.preparedExports) != maxPreparedExports ||
		len(runtime.preparedOrder) != maxPreparedExports {
		t.Fatalf(
			"prepared exports = %d, order = %d",
			len(runtime.preparedExports),
			len(runtime.preparedOrder),
		)
	}
	if _, found := runtime.preparedExports[preparationIDs[0]]; found {
		t.Fatal("oldest unused preparation was not evicted")
	}
	newest := preparationIDs[len(preparationIDs)-1]
	if report, found := runtime.takePreparedExportLocked(newest); !found ||
		report.PreparationID != newest ||
		len(runtime.preparedExports) != maxPreparedExports-1 ||
		len(runtime.preparedOrder) != maxPreparedExports-1 {
		t.Fatalf("takePreparedExportLocked(%q) = %#v, %t", newest, report, found)
	}
}

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
	var brokenDefinitions int
	if err := provider.SQL().QueryRow(`
		SELECT COUNT(*)
		FROM tag_definitions
		WHERE normalized_key = 'broken'
		  AND kind = 'boolean'
		  AND source = 'builtin'
		  AND presentation = 'warning'
		  AND is_protected = 1
	`).Scan(&brokenDefinitions); err != nil {
		t.Fatal(err)
	}
	if brokenDefinitions != 1 {
		t.Fatalf("broken definition count = %d", brokenDefinitions)
	}

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

func TestImportRuntimeStartReconcilesInterruptedRemoval(t *testing.T) {
	t.Parallel()

	provider := newRuntimeStorageProvider(t)
	archives := archive.NewRepository(provider.layout)
	source := filepath.Join(t.TempDir(), "interrupted.zip")
	if err := os.WriteFile(source, []byte("interrupted removal bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	staged, err := archives.Stage(context.Background(), source)
	if err != nil {
		t.Fatal(err)
	}
	managedPath, err := archives.Publish(staged)
	if err != nil {
		t.Fatal(err)
	}
	stories := catalog.NewRepository(provider.SQL())
	story, _, err := stories.Create(context.Background(), catalog.CreateStory{
		UUID:             "00112233-4455-4677-8899-aabbccddeeff",
		OriginalFilename: "interrupted.zip",
		DetectedFormat:   catalog.FormatZIP,
		SHA256:           staged.SHA256,
		ByteSize:         staged.ByteSize,
		ManagedPath:      managedPath,
	})
	if err != nil {
		t.Fatal(err)
	}
	intent := removal.Intent{
		ID:          "11112222-3333-4444-8555-666677778888",
		StoryID:     story.ID,
		ManagedPath: managedPath,
	}
	intent.TrashPath, err = archives.PlanRemovalTrash(intent.ID, managedPath)
	if err != nil {
		t.Fatal(err)
	}
	intents := removal.NewRepository(provider.SQL())
	if err := intents.Create(context.Background(), intent); err != nil {
		t.Fatal(err)
	}
	if err := archives.MoveToTrashAt(managedPath, intent.TrashPath); err != nil {
		t.Fatal(err)
	}

	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Now()},
		&runtimeEventRecorder{events: make(chan operations.Snapshot, 8)},
		1,
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(context.Background()); err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	t.Cleanup(func() {
		_ = runtime.Close()
	})
	if exists, err := archives.Exists(managedPath); err != nil || !exists {
		t.Fatalf("managed archive exists = %v, %v", exists, err)
	}
	if pending, err := intents.List(context.Background()); err != nil || len(pending) != 0 {
		t.Fatalf("pending intents = %#v, %v", pending, err)
	}
}

func TestImportRuntimeUsesActiveOfficialMetadataWithoutChangingManualTags(t *testing.T) {
	t.Parallel()

	provider := newRuntimeStorageProvider(t)
	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Now()},
		&runtimeEventRecorder{events: make(chan operations.Snapshot, 8)},
		1,
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = runtime.Close()
	})

	stories := catalog.NewRepository(provider.SQL())
	story, _, err := stories.Create(context.Background(), catalog.CreateStory{
		UUID:             "123e4567-e89b-42d3-a456-426614174000",
		EmbeddedTitle:    "Embedded title",
		OriginalFilename: "fixture.zip",
		DetectedFormat:   catalog.FormatZIP,
		SHA256:           strings.Repeat("e", 64),
		ManagedPath:      "archives/e/fixture.zip",
	})
	if err != nil {
		t.Fatal(err)
	}
	definition, err := runtime.CreateDefinition(context.Background(), tagging.CreateDefinition{
		Key:   "favorite",
		Label: "Favorite",
		Color: "#405CF5",
		Kind:  tagging.KindBoolean,
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := runtime.SetBulkBoolean(
		context.Background(),
		[]int64{story.ID},
		definition.ID,
		true,
	); err != nil {
		t.Fatal(err)
	}

	official := metadata.NewRepository(provider.SQL())
	snapshot := stageRuntimeMetadataSnapshot(t, official)
	if err := official.ActivateSnapshot(
		context.Background(),
		snapshot.ID,
		1,
		time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	page, err := runtime.List(context.Background(), library.ListRequest{})
	if err != nil {
		t.Fatal(err)
	}
	if len(page.Stories) != 1 ||
		page.Stories[0].Title != "Official title" ||
		page.Stories[0].Sources.Title != library.SourceOfficial ||
		page.Stories[0].Official == nil ||
		page.Stories[0].Official.Provenance != metadata.ProvenanceLuniiCatalog {
		t.Fatalf("List() = %#v", page)
	}
	var manualAssignments int
	if err := provider.SQL().QueryRow(
		`SELECT COUNT(*)
		 FROM story_tag_assignments
		 WHERE story_id = ? AND definition_id = ? AND source = 'manual'`,
		story.ID,
		definition.ID,
	).Scan(&manualAssignments); err != nil {
		t.Fatal(err)
	}
	if manualAssignments != 1 {
		t.Fatalf("manual assignment count = %d", manualAssignments)
	}
}

func TestImportRuntimeRefreshesMetadataThroughDurableOperation(t *testing.T) {
	t.Parallel()

	provider := newRuntimeStorageProvider(t)
	payload, err := os.ReadFile("../lunii/testdata/catalog.json")
	if err != nil {
		t.Fatal(err)
	}
	fetcher := &runtimeCatalogFetcher{payload: payload}
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}
	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(2026, time.July, 25, 17, 0, 0, 0, time.UTC)},
		events,
		1,
		WithMetadataFetcher(fetcher),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = runtime.Close()
	})
	status, err := runtime.MetadataStatus(context.Background(), metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	if status.State != metadata.CatalogNeverSynced {
		t.Fatalf("MetadataStatus(before refresh) = %#v", status)
	}
	if _, err := provider.SQL().Exec(
		"INSERT INTO stories (uuid) VALUES (?)",
		"123e4567-e89b-42d3-a456-426614174000",
	); err != nil {
		t.Fatal(err)
	}

	created, err := runtime.StartMetadataRefresh(
		context.Background(),
		metadata.DefaultLocale,
	)
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Kind != operations.KindMetadataSync ||
		terminal.Status != operations.StatusSucceeded ||
		terminal.Items[0].OutcomeCode != "metadata_refreshed" {
		t.Fatalf("metadata refresh operation = %#v", terminal)
	}
	status, err = runtime.MetadataStatus(context.Background(), metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	if status.State != metadata.CatalogFresh ||
		status.MatchedStoryCount != 1 ||
		status.ActivatedAt == "" {
		t.Fatalf("MetadataStatus(after refresh) = %#v", status)
	}

	fetcher.payload = nil
	fetcher.err = errors.New("offline")
	failed, err := runtime.StartMetadataRefresh(
		context.Background(),
		metadata.DefaultLocale,
	)
	if err != nil {
		t.Fatal(err)
	}
	terminal = events.waitTerminal(t, failed.ID)
	if terminal.Status != operations.StatusFailed ||
		terminal.ErrorCode != string(metadata.RefreshFetchFailed) {
		t.Fatalf("failed metadata refresh operation = %#v", terminal)
	}
	status, err = runtime.MetadataStatus(context.Background(), metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	if status.State != metadata.CatalogStaleCache ||
		status.MatchedStoryCount != 1 ||
		status.ErrorCode != string(metadata.RefreshFetchFailed) {
		t.Fatalf("MetadataStatus(stale cache) = %#v", status)
	}
}

type runtimeCatalogFetcher struct {
	payload []byte
	err     error
}

func (f *runtimeCatalogFetcher) FetchCatalog(context.Context) ([]byte, error) {
	return f.payload, f.err
}

func stageRuntimeMetadataSnapshot(
	t *testing.T,
	repository *metadata.Repository,
) metadata.CatalogSnapshot {
	t.Helper()

	ctx := context.Background()
	syncID := "123e4567-e89b-42d3-a456-426614174410"
	if _, err := repository.CreateSync(ctx, metadata.NewCatalogSync{
		ID:        syncID,
		Locale:    metadata.DefaultLocale,
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(ctx, metadata.NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    metadata.DefaultLocale,
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat("f", 64),
		ByteSize:  256,
		FetchedAt: time.Now(),
		Stories: []metadata.NewOfficialStoryMetadata{{
			StoryUUID:      "123e4567-e89b-42d3-a456-426614174000",
			Title:          "Official title",
			Author:         "Official Author",
			Language:       metadata.DefaultLocale,
			SourceRecordID: "fixture-record",
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
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
