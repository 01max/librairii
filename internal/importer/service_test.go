package importer

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"sync"
	"testing"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/storage"
)

func TestImportPublishesBytesArtworkAndCatalogRecord(t *testing.T) {
	t.Parallel()

	fixture := testfixture.PlainPK()
	service, repository, archiveRepository, layout, _ := newTestService(t)
	source := writeFixture(t, fixture)
	sourceBytes, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}

	outcome, err := service.Import(context.Background(), source)
	if err != nil {
		t.Fatalf("Import() error = %v", err)
	}
	if outcome.Code != OutcomeImported ||
		outcome.UUID != fixture.ExpectedUUID ||
		outcome.StoryID == 0 ||
		outcome.ArchiveID == 0 {
		t.Fatalf("Import() = %#v", outcome)
	}

	story, storyArchive, err := repository.FindByUUID(context.Background(), fixture.ExpectedUUID)
	if err != nil {
		t.Fatalf("FindByUUID() error = %v", err)
	}
	if story.EmbeddedTitle != testfixture.StoryTitle ||
		story.EmbeddedArtworkPath == "" ||
		storyArchive.DetectedFormat != catalog.FormatPlainPK {
		t.Fatalf("stored story = %#v, archive = %#v", story, storyArchive)
	}
	managed, err := archiveRepository.Resolve(storyArchive.ManagedPath)
	if err != nil {
		t.Fatal(err)
	}
	managedBytes, err := os.ReadFile(managed)
	if err != nil {
		t.Fatal(err)
	}
	if string(managedBytes) != string(sourceBytes) {
		t.Fatal("managed archive bytes differ from source")
	}
	currentSourceBytes, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	if string(currentSourceBytes) != string(sourceBytes) {
		t.Fatal("source archive was modified")
	}
	if _, err := os.Stat(filepath.Join(layout.Root, filepath.FromSlash(story.EmbeddedArtworkPath))); err != nil {
		t.Fatalf("embedded artwork missing: %v", err)
	}
	assertDirectoryEmpty(t, layout.Staging)
}

func TestImportIsIdempotentByExactChecksum(t *testing.T) {
	t.Parallel()

	service, _, _, layout, sqlDatabase := newTestService(t)
	source := writeFixture(t, testfixture.GenericZIP())
	first, err := service.Import(context.Background(), source)
	if err != nil {
		t.Fatal(err)
	}
	second, err := service.Import(context.Background(), source)
	if err != nil {
		t.Fatalf("Import(second) error = %v", err)
	}
	if second.Code != OutcomeDuplicateChecksum ||
		second.ExistingStoryID != first.StoryID ||
		second.Checksum != first.Checksum {
		t.Fatalf("Import(second) = %#v", second)
	}
	if count := countRows(t, sqlDatabase, "stories"); count != 1 {
		t.Fatalf("story count = %d", count)
	}
	if count := countManagedArchives(t, layout.Archives); count != 1 {
		t.Fatalf("managed archive count = %d", count)
	}
	assertDirectoryEmpty(t, layout.Staging)
}

func TestConcurrentExactImportsShareOneWriterLane(t *testing.T) {
	t.Parallel()

	service, _, _, layout, sqlDatabase := newTestService(t)
	fixtureBytes, err := testfixture.ZIPBytes(testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	paths := []string{
		filepath.Join(directory, "first.zip"),
		filepath.Join(directory, "second.zip"),
	}
	for _, path := range paths {
		if err := os.WriteFile(path, fixtureBytes, 0o600); err != nil {
			t.Fatal(err)
		}
	}

	outcomes := make(chan Outcome, len(paths))
	failures := make(chan error, len(paths))
	start := make(chan struct{})
	var workers sync.WaitGroup
	for _, path := range paths {
		path := path
		workers.Add(1)
		go func() {
			defer workers.Done()
			<-start
			outcome, err := service.Import(context.Background(), path)
			if err != nil {
				failures <- err
				return
			}
			outcomes <- outcome
		}()
	}
	close(start)
	workers.Wait()
	close(outcomes)
	close(failures)
	for err := range failures {
		t.Fatalf("Import() error = %v", err)
	}

	counts := map[OutcomeCode]int{}
	for outcome := range outcomes {
		counts[outcome.Code]++
	}
	if counts[OutcomeImported] != 1 || counts[OutcomeDuplicateChecksum] != 1 {
		t.Fatalf("concurrent outcomes = %#v", counts)
	}
	if count := countRows(t, sqlDatabase, "stories"); count != 1 {
		t.Fatalf("story count = %d", count)
	}
	if count := countManagedArchives(t, layout.Archives); count != 1 {
		t.Fatalf("managed archive count = %d", count)
	}
	assertDirectoryEmpty(t, layout.Staging)
}

func TestImportReportsUUIDConflictWithoutChangingExistingStory(t *testing.T) {
	t.Parallel()

	service, _, _, layout, sqlDatabase := newTestService(t)
	firstFixture := testfixture.GenericZIP()
	firstSource := writeFixture(t, firstFixture)
	first, err := service.Import(context.Background(), firstSource)
	if err != nil {
		t.Fatal(err)
	}

	conflictingFixture := testfixture.WithEntries(
		testfixture.GenericZIP(),
		testfixture.Entry{Name: "notes.txt", Bytes: []byte("different bytes")},
	)
	conflictingFixture.Filename = "conflict.zip"
	conflictingSource := writeFixture(t, conflictingFixture)
	conflict, err := service.Import(context.Background(), conflictingSource)
	if err != nil {
		t.Fatalf("Import(conflict) error = %v", err)
	}
	if conflict.Code != OutcomeUUIDConflict ||
		conflict.ExistingStoryID != first.StoryID ||
		conflict.Checksum == first.Checksum {
		t.Fatalf("Import(conflict) = %#v", conflict)
	}
	if count := countRows(t, sqlDatabase, "stories"); count != 1 {
		t.Fatalf("story count = %d", count)
	}
	if count := countManagedArchives(t, layout.Archives); count != 1 {
		t.Fatalf("managed archive count = %d", count)
	}
	assertDirectoryEmpty(t, layout.Staging)
}

func TestImportFailureAndCancellationClearStaging(t *testing.T) {
	t.Parallel()

	service, _, _, layout, sqlDatabase := newTestService(t)
	invalid := filepath.Join(t.TempDir(), "invalid.zip")
	if err := os.WriteFile(invalid, []byte("not an archive"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Import(context.Background(), invalid); !ErrorHasCode(err, ErrorInspect) {
		t.Fatalf("Import(invalid) error = %v", err)
	}
	if count := countRows(t, sqlDatabase, "stories"); count != 0 {
		t.Fatalf("story count after invalid import = %d", count)
	}
	assertDirectoryEmpty(t, layout.Staging)
	assertDirectoryEmpty(t, layout.Archives)

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := service.Import(ctx, writeFixture(t, testfixture.GenericZIP())); !errors.Is(err, context.Canceled) {
		t.Fatalf("Import(cancelled) error = %v", err)
	}
	assertDirectoryEmpty(t, layout.Staging)
	assertDirectoryEmpty(t, layout.Archives)
}

func TestDatabaseFailureCompensatesPublishedFiles(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	archiveRepository := archive.NewRepository(layout)
	artworkRepository := artwork.NewRepository(layout)
	service, err := NewService(
		archiveRepository,
		artworkRepository,
		failingCatalog{createErr: errors.New("database unavailable")},
		inspection.NewStoryInspector(),
		inspection.DefaultLimits(),
	)
	if err != nil {
		t.Fatal(err)
	}

	if _, err := service.Import(
		context.Background(),
		writeFixture(t, testfixture.PlainPK()),
	); !ErrorHasCode(err, ErrorCatalog) {
		t.Fatalf("Import() error = %v", err)
	}
	assertDirectoryEmpty(t, layout.Staging)
	assertDirectoryEmpty(t, layout.Archives)
	assertDirectoryEmpty(t, filepath.Join(layout.Catalog, "embedded"))
	if count := countManagedArchives(t, layout.Trash); count != 1 {
		t.Fatalf("trash archive count = %d", count)
	}
}

func TestCancellationAfterPublicationCompensatesManagedFiles(t *testing.T) {
	t.Parallel()

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
	ctx, cancel := context.WithCancel(context.Background())
	artworkRepository := artwork.NewRepository(layout)
	service, err := NewService(
		archive.NewRepository(layout),
		cancellingArtworkStore{
			artworkStore: artworkRepository,
			cancel:       cancel,
		},
		catalog.NewRepository(sqlDatabase.SQL()),
		inspection.NewStoryInspector(),
		inspection.DefaultLimits(),
	)
	if err != nil {
		t.Fatal(err)
	}

	if _, err := service.Import(
		ctx,
		writeFixture(t, testfixture.PlainPK()),
	); !errors.Is(err, context.Canceled) || !ErrorHasCode(err, ErrorCancelled) {
		t.Fatalf("Import() error = %v", err)
	}
	if count := countRows(t, sqlDatabase.SQL(), "stories"); count != 0 {
		t.Fatalf("story count = %d", count)
	}
	assertDirectoryEmpty(t, layout.Staging)
	assertDirectoryEmpty(t, layout.Archives)
	assertDirectoryEmpty(t, filepath.Join(layout.Catalog, "embedded"))
	if count := countManagedArchives(t, layout.Trash); count != 1 {
		t.Fatalf("trash archive count = %d", count)
	}
}

type failingCatalog struct {
	createErr error
}

func (f failingCatalog) Create(
	context.Context,
	catalog.CreateStory,
) (catalog.Story, catalog.StoryArchive, error) {
	return catalog.Story{}, catalog.StoryArchive{}, f.createErr
}

func (f failingCatalog) FindByUUID(
	context.Context,
	string,
) (catalog.Story, catalog.StoryArchive, error) {
	return catalog.Story{}, catalog.StoryArchive{}, sql.ErrNoRows
}

func (f failingCatalog) FindByChecksum(
	context.Context,
	string,
) (catalog.Story, catalog.StoryArchive, error) {
	return catalog.Story{}, catalog.StoryArchive{}, sql.ErrNoRows
}

type cancellingArtworkStore struct {
	artworkStore
	cancel context.CancelFunc
}

func (s cancellingArtworkStore) Publish(
	storyUUID string,
	mediaType string,
	content []byte,
) (string, error) {
	path, err := s.artworkStore.Publish(storyUUID, mediaType, content)
	s.cancel()
	return path, err
}

func newTestService(
	t *testing.T,
) (*Service, *catalog.Repository, *archive.Repository, storage.Layout, *sql.DB) {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	sqlDatabase, err := database.Open(context.Background(), filepath.Join(layout.Database, "librairii.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = sqlDatabase.Close()
	})
	archiveRepository := archive.NewRepository(layout)
	catalogRepository := catalog.NewRepository(sqlDatabase.SQL())
	service, err := NewService(
		archiveRepository,
		artwork.NewRepository(layout),
		catalogRepository,
		inspection.NewStoryInspector(),
		inspection.DefaultLimits(),
	)
	if err != nil {
		t.Fatal(err)
	}
	return service, catalogRepository, archiveRepository, layout, sqlDatabase.SQL()
}

func writeFixture(t *testing.T, fixture testfixture.Archive) string {
	t.Helper()

	path, err := testfixture.WriteZIP(t.TempDir(), fixture)
	if err != nil {
		t.Fatal(err)
	}
	return path
}

func countRows(t *testing.T, sqlDatabase *sql.DB, table string) int {
	t.Helper()

	var count int
	if err := sqlDatabase.QueryRow("SELECT count(*) FROM " + table).Scan(&count); err != nil {
		t.Fatal(err)
	}
	return count
}

func countManagedArchives(t *testing.T, root string) int {
	t.Helper()

	count := 0
	if err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !entry.IsDir() {
			count++
		}
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	return count
}

func assertDirectoryEmpty(t *testing.T, directory string) {
	t.Helper()

	entries, err := os.ReadDir(directory)
	if errors.Is(err, os.ErrNotExist) {
		return
	}
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("%s contains %d entries", directory, len(entries))
	}
}
