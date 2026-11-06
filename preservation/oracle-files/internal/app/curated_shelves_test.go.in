package app

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelves"
)

func TestCuratedShelfBindingsRetainUUIDMembershipAcrossRestartAndReimport(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "library.sqlite3")
	db, err := database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	seed := func(id int64) {
		t.Helper()
		if _, err := db.Writer().Exec(`INSERT INTO stories(id,uuid,embedded_title,is_broken,display_name_normalized) VALUES(?,'123e4567-e89b-42d3-a456-426614174000','Kept',1,'kept')`, id); err != nil {
			t.Fatal(err)
		}
		if _, err := db.Writer().Exec(`INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path) VALUES(?,'kept.zip','zip',?,1,'archives/kept.zip')`, id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); err != nil {
			t.Fatal(err)
		}
	}
	seed(1)
	service, err := shelves.NewService(db.Writer(), library.NewQuery(db.SQL(), nil))
	if err != nil {
		t.Fatal(err)
	}
	runtime := &ImportRuntime{shelves: service}
	application := &Application{shelves: runtime}
	if response := application.CreateShelf(ctx, "Kept", nil); response.Error == nil {
		t.Fatal("unready mutation accepted")
	}
	application.ready = true
	created := application.CreateShelf(ctx, "Kept", []string{"123e4567-e89b-42d3-a456-426614174000"})
	if created.Error != nil || created.Shelf == nil || created.Shelf.Count != 1 || created.Shelf.BrokenCount != 1 {
		t.Fatalf("create: %#v", created)
	}
	id := created.Shelf.ID
	opened := application.OpenShelf(ctx, id, library.StoryLibraryQuery{})
	if opened.Error != nil || opened.Evaluation == nil || len(opened.Evaluation.Members) != 1 || opened.Evaluation.Members[0].Story == nil || !opened.Evaluation.Members[0].Story.Broken {
		t.Fatalf("flagged member hidden: %#v", opened)
	}
	if _, err := db.Writer().Exec(`DELETE FROM stories WHERE id=1`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	db, err = database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	service, err = shelves.NewService(db.Writer(), library.NewQuery(db.SQL(), nil))
	if err != nil {
		t.Fatal(err)
	}
	runtime.shelves = service
	opened = application.OpenShelf(ctx, id, library.StoryLibraryQuery{})
	if opened.Error != nil || opened.Evaluation == nil || opened.Evaluation.Shelf.UnavailableCount != 1 || opened.Evaluation.Members[0].Story != nil {
		t.Fatalf("missing UUID lost: %#v", opened)
	}
	seed(9)
	opened = application.OpenShelf(ctx, id, library.StoryLibraryQuery{})
	if opened.Error != nil || opened.Evaluation == nil || opened.Evaluation.Members[0].Story == nil || opened.Evaluation.Members[0].Story.ID != 9 || !opened.Evaluation.Members[0].Protected {
		t.Fatalf("reimport not resolved: %#v", opened)
	}
	if response := application.OpenShelf(ctx, id, library.StoryLibraryQuery{Name: "other"}); response.Error != nil || response.Evaluation == nil || response.Evaluation.TotalItems != 0 || response.Evaluation.Shelf.Count != 1 {
		t.Fatalf("view filter changed membership: %#v", response)
	}
}
