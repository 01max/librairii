package catalog

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/database"
)

type storyCreateHookFunc func(context.Context, *sql.Tx, int64) error

func (f storyCreateHookFunc) AfterStoryCreate(
	ctx context.Context,
	transaction *sql.Tx,
	storyID int64,
) error {
	return f(ctx, transaction, storyID)
}

func TestRepositoryCreatesAndFindsStoryArchive(t *testing.T) {
	t.Parallel()

	connection := openTestDatabase(t)
	repository := NewRepository(connection)
	input := validCreateStory()

	story, archive, err := repository.Create(context.Background(), input)
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	if story.ID == 0 || archive.ID == 0 || archive.StoryID != story.ID {
		t.Fatalf("Create() story = %#v, archive = %#v", story, archive)
	}

	foundStory, foundArchive, err := repository.FindByChecksum(context.Background(), input.SHA256)
	if err != nil {
		t.Fatalf("FindByChecksum() error = %v", err)
	}
	if foundStory.UUID != input.UUID || foundArchive.ManagedPath != input.ManagedPath {
		t.Fatalf("FindByChecksum() story = %#v, archive = %#v", foundStory, foundArchive)
	}
}

func TestRepositoryRollsBackStoryAndArchiveWhenCreateHookFails(t *testing.T) {
	t.Parallel()

	connection := openTestDatabase(t)
	hookError := errors.New("projection failed")
	repository := NewRepository(
		connection,
		storyCreateHookFunc(func(
			_ context.Context,
			_ *sql.Tx,
			storyID int64,
		) error {
			if storyID <= 0 {
				t.Fatalf("story hook id = %d", storyID)
			}
			return hookError
		}),
	)
	if _, _, err := repository.Create(
		context.Background(),
		validCreateStory(),
	); !errors.Is(err, hookError) {
		t.Fatalf("Create() error = %v", err)
	}
	for _, table := range []string{"stories", "story_archives"} {
		var count int
		if err := connection.QueryRow("SELECT COUNT(*) FROM " + table).Scan(&count); err != nil {
			t.Fatal(err)
		}
		if count != 0 {
			t.Fatalf("%s count = %d", table, count)
		}
	}
}

func TestRepositoryEnforcesUUIDAndChecksumIdentity(t *testing.T) {
	t.Parallel()

	connection := openTestDatabase(t)
	repository := NewRepository(connection)
	input := validCreateStory()
	if _, _, err := repository.Create(context.Background(), input); err != nil {
		t.Fatal(err)
	}

	duplicateUUID := input
	duplicateUUID.SHA256 = strings.Repeat("b", 64)
	duplicateUUID.ManagedPath = "archives/bb/story.pk"
	if _, _, err := repository.Create(context.Background(), duplicateUUID); err == nil {
		t.Fatal("Create(duplicate UUID) error = nil")
	}

	duplicateChecksum := input
	duplicateChecksum.UUID = "123e4567-e89b-42d3-a456-426614174001"
	duplicateChecksum.ManagedPath = "archives/aa/other.pk"
	if _, _, err := repository.Create(context.Background(), duplicateChecksum); err == nil {
		t.Fatal("Create(duplicate checksum) error = nil")
	}
}

func TestMigrationRejectsInvalidRelativePathAndOperationState(t *testing.T) {
	t.Parallel()

	connection := openTestDatabase(t)
	repository := NewRepository(connection)
	input := validCreateStory()
	input.ManagedPath = "../outside.pk"
	if _, _, err := repository.Create(context.Background(), input); err == nil {
		t.Fatal("Create(path escape) error = nil")
	}

	_, err := connection.Exec(`
		INSERT INTO file_operations (id, kind, status, completed_items, total_items)
		VALUES ('123e4567-e89b-42d3-a456-426614174099', 'import', 'unknown', 0, 1)
	`)
	if err == nil {
		t.Fatal("invalid operation state was accepted")
	}
}

func TestRepositoryFindMissingReturnsSQLNoRows(t *testing.T) {
	t.Parallel()

	repository := NewRepository(openTestDatabase(t))
	if _, _, err := repository.FindByUUID(context.Background(), "123e4567-e89b-42d3-a456-426614174000"); err != sql.ErrNoRows {
		t.Fatalf("FindByUUID() error = %v, want sql.ErrNoRows", err)
	}
}

func TestRepositoryDeletesStoryAndArchiveTogether(t *testing.T) {
	t.Parallel()

	connection := openTestDatabase(t)
	repository := NewRepository(connection)
	story, _, err := repository.Create(context.Background(), validCreateStory())
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.Delete(context.Background(), story.ID); err != nil {
		t.Fatalf("Delete() error = %v", err)
	}
	if _, _, err := repository.FindByID(context.Background(), story.ID); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("FindByID(deleted) error = %v", err)
	}
	var archives int
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM story_archives WHERE story_id = ?",
		story.ID,
	).Scan(&archives); err != nil {
		t.Fatal(err)
	}
	if archives != 0 {
		t.Fatalf("archive count = %d", archives)
	}
	if err := repository.Delete(context.Background(), story.ID); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("Delete(missing) error = %v", err)
	}
}

func validCreateStory() CreateStory {
	return CreateStory{
		UUID:                "123e4567-e89b-42d3-a456-426614174000",
		EmbeddedTitle:       "The Mountain Dragon",
		EmbeddedDescription: "A synthetic story.",
		OriginalFilename:    "mountain-dragon.plain.pk",
		DetectedFormat:      FormatPlainPK,
		SHA256:              strings.Repeat("a", 64),
		ByteSize:            128,
		ManagedPath:         "archives/aa/mountain-dragon.plain.pk",
	}
}

func openTestDatabase(t *testing.T) *sql.DB {
	t.Helper()

	path := filepath.Join(t.TempDir(), "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}
	connection, err := database.Open(context.Background(), path)
	if err != nil {
		t.Fatalf("database.Open() error = %v", err)
	}
	t.Cleanup(func() {
		if err := connection.Close(); err != nil {
			t.Errorf("Close() error = %v", err)
		}
	})
	return connection.SQL()
}
