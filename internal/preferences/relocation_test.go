package preferences

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/database"
	"github.com/google/uuid"
	"golang.org/x/sys/unix"
	"os"
	"path/filepath"
	"testing"
)

func moveFixture(t *testing.T) (Journal, Profile, string) {
	t.Helper()
	ctx := context.Background()
	base := t.TempDir()
	source := filepath.Join(base, "source")
	destination := filepath.Join(base, "destination")
	config := filepath.Join(base, "profile")
	os.MkdirAll(filepath.Join(source, "db"), 0700)
	db, err := database.Open(ctx, DatabasePath(source))
	if err != nil {
		t.Fatal(err)
	}
	for _, statement := range []string{
		`INSERT INTO stories(id,uuid,embedded_title) VALUES(73,'11111111-1111-1111-1111-111111111111','Une histoire')`,
		`INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position) VALUES(42,'favorite','favorite','Favoris','#405CF5','boolean','user',0)`,
		`INSERT INTO story_tag_assignments(story_id,definition_id,source) VALUES(73,42,'manual')`,
		`INSERT INTO shelves(id,name,normalized_name,position,query_version,query_payload) VALUES(42,'Mes histoires','mes histoires',0,1,'{}')`,
		`INSERT INTO file_operations(id,kind,status) VALUES('22222222-2222-2222-2222-222222222222','import','succeeded')`,
	} {
		if _, err := db.Writer().ExecContext(ctx, statement); err != nil {
			t.Fatal(err)
		}
	}
	if err = db.Close(); err != nil {
		t.Fatal(err)
	}
	id, err := EnsureIdentity(source)
	if err != nil {
		t.Fatal(err)
	}
	for _, dir := range []string{"archives", "catalog", "trash", "staging", "logs"} {
		os.MkdirAll(filepath.Join(source, dir), 0700)
		os.WriteFile(filepath.Join(source, dir, "example"), []byte("preserved "+dir), 0600)
	}
	entries, err := Manifest(ctx, source)
	if err != nil {
		t.Fatal(err)
	}
	j := Journal{ID: uuid.NewString(), LibraryID: id, Source: source, Destination: destination, Phase: "reserved", Entries: entries}
	p := Profile{Version: Version, Language: "fr", Root: source, LibraryID: id, Complete: true}
	return j, p, config
}
func TestRestartBeforeCommitRetainsSource(t *testing.T) {
	for _, phase := range []string{"journal", "reserved", "copied", "published"} {
		t.Run(phase, func(t *testing.T) {
			j, p, config := moveFixture(t)
			ctx := context.Background()
			if err := WriteJournal(config, j); err != nil {
				t.Fatal(err)
			}
			if phase != "journal" {
				if err := Reserve(j.Destination, j.ID, 0); err != nil {
					t.Fatal(err)
				}
			}
			if phase == "copied" || phase == "published" {
				if err := CopyLibrary(ctx, j, nil); err != nil {
					t.Fatal(err)
				}
			}
			if phase == "published" {
				if err := Publish(j.Workspace(), j.Destination, j.ID); err != nil {
					t.Fatal(err)
				}
			}
			if err := Reconcile(ctx, config, p); err != nil {
				t.Fatal(err)
			}
			if err := ValidateLibrary(ctx, p.Root, p.LibraryID); err != nil {
				t.Fatal(err)
			}
			if _, err := os.Stat(j.Destination); !errors.Is(err, os.ErrNotExist) {
				t.Fatal("destination not discarded", err)
			}
		})
	}
}
func TestPostCommitRecoveryNeverUsesOldSource(t *testing.T) {
	j, p, config := moveFixture(t)
	ctx := context.Background()
	Reserve(j.Destination, j.ID, 0)
	if err := CopyLibrary(ctx, j, nil); err != nil {
		t.Fatal(err)
	}
	if err := VerifyCopy(ctx, j, j.Workspace()); err != nil {
		t.Fatal(err)
	}
	if err := Publish(j.Workspace(), j.Destination, j.ID); err != nil {
		t.Fatal(err)
	}
	WriteJournal(config, j)
	p.Root = j.Destination
	os.WriteFile(filepath.Join(j.Source, "foreign"), []byte("keep"), 0600)
	if err := Reconcile(ctx, config, p); !errors.Is(err, ErrCleanupPending) {
		t.Fatal(err)
	}
	if data, err := os.ReadFile(filepath.Join(j.Source, "foreign")); err != nil || string(data) != "keep" {
		t.Fatal(err)
	}
	if err := ValidateLibrary(ctx, j.Destination, j.LibraryID); err != nil {
		t.Fatal(err)
	}
	os.Rename(j.Destination, j.Destination+"-offline")
	if err := Reconcile(ctx, config, p); err == nil {
		t.Fatal("accepted missing committed root")
	}
}
func TestCancellationCorruptionAndUnknownDestinationFiles(t *testing.T) {
	j, p, config := moveFixture(t)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := CopyLibrary(ctx, j, nil); !errors.Is(err, context.Canceled) {
		t.Fatal(err)
	}
	if err := DiscardOwned(context.Background(), j.Workspace(), j); err != nil {
		t.Fatal(err)
	}
	if err := CopyLibrary(context.Background(), j, nil); err != nil {
		t.Fatal(err)
	}
	os.WriteFile(filepath.Join(j.Workspace(), "archives", "example"), []byte("corrupt"), 0600)
	if err := VerifyCopy(context.Background(), j, j.Workspace()); err == nil {
		t.Fatal("accepted corruption")
	}
	WriteJournal(config, j)
	if err := Reconcile(context.Background(), config, p); !errors.Is(err, ErrCleanupPending) {
		t.Fatal(err)
	}
	if err := ValidateLibrary(context.Background(), j.Source, j.LibraryID); err != nil {
		t.Fatal(err)
	}
}

func TestCompleteLibraryMovePreservesEveryRecordedByte(t *testing.T) {
	j, p, config := moveFixture(t)
	ctx := context.Background()
	external := filepath.Join(t.TempDir(), "original.pk")
	os.WriteFile(external, []byte("original"), 0600)
	if err := WriteJournal(config, j); err != nil {
		t.Fatal(err)
	}
	if err := Reserve(j.Destination, j.ID, 0); err != nil {
		t.Fatal(err)
	}
	var copied int64
	if err := CopyLibrary(ctx, j, func(n int64) { copied += n }); err != nil {
		t.Fatal(err)
	}
	if copied == 0 {
		t.Fatal("no byte progress")
	}
	if err := VerifyCopy(ctx, j, j.Workspace()); err != nil {
		t.Fatal(err)
	}
	if err := Publish(j.Workspace(), j.Destination, j.ID); err != nil {
		t.Fatal(err)
	}
	store := NewStore(config)
	if _, err := store.Update(func(next *Profile) error { p.Root = j.Destination; *next = p; return nil }); err != nil {
		t.Fatal(err)
	}
	if err := Reconcile(ctx, config, p); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(j.Source); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("old root still exists", err)
	}
	if err := VerifyCopy(ctx, j, j.Destination); err != nil {
		t.Fatal(err)
	}
	db, err := database.Open(ctx, DatabasePath(j.Destination))
	if err != nil {
		t.Fatal(err)
	}
	for _, query := range []string{"SELECT COUNT(*) FROM stories WHERE id=73 AND embedded_title='Une histoire'", "SELECT COUNT(*) FROM tag_definitions WHERE id=42 AND label='Favoris'", "SELECT COUNT(*) FROM story_tag_assignments WHERE story_id=73 AND definition_id=42", "SELECT COUNT(*) FROM shelves WHERE id=42", "SELECT COUNT(*) FROM file_operations"} {
		var count int
		if err := db.SQL().QueryRowContext(ctx, query).Scan(&count); err != nil || count != 1 {
			t.Fatal(query, count, err)
		}
	}
	db.Close()
	reloaded, err := NewStore(config).Load()
	if err != nil || reloaded.Root != j.Destination || reloaded.Language != "fr" {
		t.Fatal(reloaded, err)
	}
	if data, err := os.ReadFile(external); err != nil || string(data) != "original" {
		t.Fatal(err)
	}
}

// Set LIBRAIRII_TEST_DESTINATION_ROOT to a writable second local volume.
func TestCompleteLibraryMoveAcrossVolumes(t *testing.T) {
	root := os.Getenv("LIBRAIRII_TEST_DESTINATION_ROOT")
	if root == "" {
		t.Skip("second volume not configured")
	}
	j, p, config := moveFixture(t)
	ctx := context.Background()
	var sourceStat, targetStat unix.Stat_t
	if err := unix.Stat(j.Source, &sourceStat); err != nil {
		t.Fatal(err)
	}
	if err := unix.Stat(root, &targetStat); err != nil {
		t.Fatal(err)
	}
	if sourceStat.Dev == targetStat.Dev {
		t.Skip("destination must be a separate filesystem")
	}
	target, err := os.MkdirTemp(root, "librairii-relocation-")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { os.RemoveAll(target) })
	j.Destination = target
	if err = WriteJournal(config, j); err != nil {
		t.Fatal(err)
	}
	if err = Reserve(target, j.ID, 0); err != nil {
		t.Fatal(err)
	}
	if err = CopyLibrary(ctx, j, nil); err != nil {
		t.Fatal(err)
	}
	if err = VerifyCopy(ctx, j, j.Workspace()); err != nil {
		t.Fatal(err)
	}
	if err = Publish(j.Workspace(), target, j.ID); err != nil {
		t.Fatal(err)
	}
	p.Root = target
	if err = Reconcile(ctx, config, p); err != nil {
		t.Fatal(err)
	}
	if err = VerifyCopy(ctx, j, target); err != nil {
		t.Fatal(err)
	}
}

func TestCancellationDuringFileCopyReleasesMoveForRetry(t *testing.T) {
	j, p, config := moveFixture(t)
	sourceFile := filepath.Join(j.Source, "000-large-archive")
	contents := make([]byte, 1024*1024)
	for i := range contents {
		contents[i] = byte(i % 251)
	}
	if err := os.WriteFile(sourceFile, contents, 0600); err != nil {
		t.Fatal(err)
	}
	var err error
	j.Entries, err = Manifest(context.Background(), j.Source)
	if err != nil {
		t.Fatal(err)
	}
	if err := WriteJournal(config, j); err != nil {
		t.Fatal(err)
	}
	if err := Reserve(j.Destination, j.ID, 0); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var copied int64
	err = CopyLibrary(ctx, j, func(n int64) {
		copied += n
		if copied >= 128*1024 {
			cancel()
		}
	})
	if !errors.Is(err, context.Canceled) || copied == 0 || copied >= int64(len(contents)) {
		t.Fatal(copied, err)
	}
	if err := Reconcile(context.Background(), config, p); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{j.Workspace(), j.Destination, filepath.Join(config, "relocation.json")} {
		if _, err := os.Stat(path); !errors.Is(err, os.ErrNotExist) {
			t.Fatalf("%s remains: %v", path, err)
		}
	}
	if err := ValidateLibrary(context.Background(), j.Source, j.LibraryID); err != nil {
		t.Fatal(err)
	}
	if err := Reserve(j.Destination, j.ID, 0); err != nil {
		t.Fatal(err)
	}
	if err := CopyLibrary(context.Background(), j, nil); err != nil {
		t.Fatal(err)
	}
	if err := VerifyCopy(context.Background(), j, j.Workspace()); err != nil {
		t.Fatal(err)
	}
}
