package removal

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/storage"
)

func TestRemoveMovesArchiveToTrashBeforeDeletingAssociations(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	databaseConnection, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = databaseConnection.Close()
	})
	archives := archive.NewRepository(layout)
	source := filepath.Join(t.TempDir(), "clockwork.zip")
	bytes := []byte("synthetic owned archive")
	if err := os.WriteFile(source, bytes, 0o600); err != nil {
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
	stories := catalog.NewRepository(databaseConnection.SQL())
	story, _, err := stories.Create(context.Background(), catalog.CreateStory{
		UUID:             "00112233-4455-4677-8899-aabbccddeeff",
		EmbeddedTitle:    "Clockwork Forest",
		OriginalFilename: "clockwork.zip",
		DetectedFormat:   catalog.FormatZIP,
		SHA256:           staged.SHA256,
		ByteSize:         staged.ByteSize,
		ManagedPath:      managedPath,
	})
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewService(
		stories,
		archives,
		NewRepository(databaseConnection.SQL()),
	)
	if err != nil {
		t.Fatal(err)
	}

	result, err := service.Remove(context.Background(), story.ID)
	if err != nil {
		t.Fatalf("Remove() error = %v", err)
	}
	if result.StoryID != story.ID || result.UUID != story.UUID {
		t.Fatalf("Remove() = %#v", result)
	}
	if _, _, err := stories.FindByID(context.Background(), story.ID); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("FindByID(removed) error = %v", err)
	}
	var intents int
	if err := databaseConnection.SQL().QueryRow(
		"SELECT COUNT(*) FROM removal_intents",
	).Scan(&intents); err != nil || intents != 0 {
		t.Fatalf("removal intents = %d, %v", intents, err)
	}
	managed, err := archive.SafeJoin(layout.Root, managedPath)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(managed); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("managed archive remains active: %v", err)
	}
	trashed := findOnlyFile(t, layout.Trash)
	got, err := os.ReadFile(trashed)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(bytes) {
		t.Fatalf("trash bytes = %q", got)
	}
	sourceBytes, err := os.ReadFile(source)
	if err != nil || string(sourceBytes) != string(bytes) {
		t.Fatalf("source bytes = %q, %v", sourceBytes, err)
	}
}

func TestReconcileRestoresArchiveAfterInterruptedRemoval(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	databaseConnection, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = databaseConnection.Close()
	})
	managedPath := "archives/aa/clockwork.zip"
	managed, err := archive.SafeJoin(layout.Root, managedPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := archive.PrepareDestination(layout.Archives, managed); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(managed, []byte("managed story"), 0o600); err != nil {
		t.Fatal(err)
	}
	stories := catalog.NewRepository(databaseConnection.SQL())
	story, _, err := stories.Create(context.Background(), catalog.CreateStory{
		UUID:             "00112233-4455-4677-8899-aabbccddeeff",
		OriginalFilename: "clockwork.zip",
		DetectedFormat:   catalog.FormatZIP,
		SHA256:           strings.Repeat("a", 64),
		ByteSize:         13,
		ManagedPath:      managedPath,
	})
	if err != nil {
		t.Fatal(err)
	}
	archives := archive.NewRepository(layout)
	intent := Intent{
		ID:          "11112222-3333-4444-8555-666677778888",
		StoryID:     story.ID,
		ManagedPath: managedPath,
	}
	intent.TrashPath, err = archives.PlanRemovalTrash(intent.ID, managedPath)
	if err != nil {
		t.Fatal(err)
	}
	intents := NewRepository(databaseConnection.SQL())
	if err := intents.Create(context.Background(), intent); err != nil {
		t.Fatal(err)
	}
	if err := archives.MoveToTrashAt(managedPath, intent.TrashPath); err != nil {
		t.Fatal(err)
	}

	service, err := NewService(stories, archives, intents)
	if err != nil {
		t.Fatal(err)
	}
	if err := service.Reconcile(context.Background()); err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	if exists, err := archives.Exists(managedPath); err != nil || !exists {
		t.Fatalf("managed archive exists = %v, %v", exists, err)
	}
	if exists, err := archives.Exists(intent.TrashPath); err != nil || exists {
		t.Fatalf("trash archive exists = %v, %v", exists, err)
	}
	if pending, err := intents.List(context.Background()); err != nil || len(pending) != 0 {
		t.Fatalf("pending intents = %#v, %v", pending, err)
	}
	if _, _, err := stories.FindByID(context.Background(), story.ID); err != nil {
		t.Fatalf("story association was lost: %v", err)
	}
}

func TestRemoveRestoresArchiveWhenDatabaseDeletionFails(t *testing.T) {
	t.Parallel()

	catalogFailure := errors.New("database write failed")
	stories := &fakeCatalog{
		story: catalog.Story{ID: 7, UUID: "00112233-4455-4677-8899-aabbccddeeff"},
		archive: catalog.StoryArchive{
			StoryID:     7,
			ManagedPath: "archives/aa/clockwork.zip",
		},
		deleteErr: catalogFailure,
	}
	archives := &fakeArchives{trashPath: "trash/removals/intent/clockwork.zip"}
	intents := newFakeIntents()
	service, err := NewService(stories, archives, intents)
	if err != nil {
		t.Fatal(err)
	}

	_, err = service.Remove(context.Background(), 7)
	if !ErrorHasCode(err, ErrorDelete) || !errors.Is(err, catalogFailure) {
		t.Fatalf("Remove() error = %v", err)
	}
	if !stories.deleteCalled {
		t.Fatal("database deletion was not attempted")
	}
	if archives.moves != 1 ||
		archives.restores != 1 ||
		archives.restoredTrash != archives.trashPath ||
		archives.restoredManaged != stories.archive.ManagedPath {
		t.Fatalf("archive calls = %#v", archives)
	}
}

func TestRemoveReportsTrashAndRestoreFailuresWithoutDeleting(t *testing.T) {
	t.Parallel()

	stories := &fakeCatalog{
		story:   catalog.Story{ID: 7},
		archive: catalog.StoryArchive{ManagedPath: "archives/aa/clockwork.zip"},
	}
	trashFailure := errors.New("trash unavailable")
	service, err := NewService(
		stories,
		&fakeArchives{moveErr: trashFailure},
		newFakeIntents(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Remove(context.Background(), 7); !ErrorHasCode(err, ErrorTrash) {
		t.Fatalf("Remove(trash failure) error = %v", err)
	}
	if stories.deleteCalled {
		t.Fatal("story was deleted before archive reached trash")
	}

	stories.deleteErr = errors.New("delete failed")
	restoreFailure := errors.New("restore failed")
	service, err = NewService(
		stories,
		&fakeArchives{
			trashPath:  "trash/removals/intent/clockwork.zip",
			restoreErr: restoreFailure,
		},
		newFakeIntents(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Remove(context.Background(), 7); !ErrorHasCode(err, ErrorRestore) ||
		!errors.Is(err, restoreFailure) {
		t.Fatalf("Remove(restore failure) error = %v", err)
	}
}

func TestRemoveCancellationLeavesStoryUntouched(t *testing.T) {
	t.Parallel()

	stories := &fakeCatalog{}
	archives := &fakeArchives{}
	service, err := NewService(stories, archives, newFakeIntents())
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := service.Remove(ctx, 7); !errors.Is(err, context.Canceled) {
		t.Fatalf("Remove(cancelled) error = %v", err)
	}
	if stories.findCalled || stories.deleteCalled || archives.moves != 0 {
		t.Fatalf("cancelled removal changed state: %#v, %#v", stories, archives)
	}
}

type fakeCatalog struct {
	story        catalog.Story
	archive      catalog.StoryArchive
	findErr      error
	deleteErr    error
	findCalled   bool
	deleteCalled bool
}

func (c *fakeCatalog) FindByID(
	context.Context,
	int64,
) (catalog.Story, catalog.StoryArchive, error) {
	c.findCalled = true
	return c.story, c.archive, c.findErr
}

func (c *fakeCatalog) Delete(context.Context, int64) error {
	c.deleteCalled = true
	return c.deleteErr
}

type fakeArchives struct {
	trashPath       string
	moveErr         error
	restoreErr      error
	moves           int
	restores        int
	restoredTrash   string
	restoredManaged string
}

func (a *fakeArchives) PlanRemovalTrash(string, string) (string, error) {
	if a.trashPath == "" {
		a.trashPath = "trash/removals/intent/clockwork.zip"
	}
	return a.trashPath, nil
}

func (a *fakeArchives) MoveToTrashAt(string, string) error {
	a.moves++
	return a.moveErr
}

func (a *fakeArchives) RestoreFromTrash(trashPath string, managedPath string) error {
	a.restores++
	a.restoredTrash = trashPath
	a.restoredManaged = managedPath
	return a.restoreErr
}

func (a *fakeArchives) Exists(string) (bool, error) {
	return false, nil
}

type fakeIntents struct {
	items   map[string]Intent
	listErr error
}

func newFakeIntents() *fakeIntents {
	return &fakeIntents{items: map[string]Intent{}}
}

func (s *fakeIntents) Create(_ context.Context, intent Intent) error {
	s.items[intent.ID] = intent
	return nil
}

func (s *fakeIntents) Delete(_ context.Context, intentID string) error {
	if _, found := s.items[intentID]; !found {
		return sql.ErrNoRows
	}
	delete(s.items, intentID)
	return nil
}

func (s *fakeIntents) List(context.Context) ([]Intent, error) {
	var intents []Intent
	for _, intent := range s.items {
		intents = append(intents, intent)
	}
	return intents, s.listErr
}

func findOnlyFile(t *testing.T, root string) string {
	t.Helper()
	var files []string
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.Type().IsRegular() {
			files = append(files, path)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(files) != 1 {
		t.Fatalf("trash files = %s", strings.Join(files, ", "))
	}
	return files[0]
}
