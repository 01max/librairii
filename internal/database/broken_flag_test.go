package database

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"testing/fstest"
)

func TestBrokenFlagMigrationPreservesAssignmentsAndRestarts(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "library.sqlite3")
	old, err := openDatabase(ctx, path, migrationsThrough(t, 14))
	if err != nil {
		t.Fatal(err)
	}
	_, err = old.Writer().Exec(`
        INSERT INTO stories (id, uuid) VALUES
        (1, '123e4567-e89b-42d3-a456-426614174000'),
        (2, '223e4567-e89b-42d3-a456-426614174000');
        INSERT INTO tag_definitions
          (id,key,normalized_key,label,color,kind,source,presentation,position,is_protected)
        VALUES (9,'broken','broken','Broken','#405CF5','boolean','builtin','warning',0,1),
          (10,'favorite','favorite','Favorite','#405CF5','boolean','user','default',0,0),
          (11,'age','age','Age','#267D5D','choice','derived','system',0,1);
        INSERT INTO story_tag_assignments (story_id,definition_id,source)
        VALUES (1,9,'manual'), (1,10,'manual');`)
	if err != nil {
		t.Fatal(err)
	}
	old.Close()
	for i := 0; i < 2; i++ {
		upgraded, err := Open(ctx, path)
		if err != nil {
			t.Fatal(err)
		}
		var flagged, unflagged, legacyID, definitions, assignments int
		db := upgraded.SQL()
		for _, query := range []struct {
			sql  string
			dest *int
		}{
			{"SELECT is_broken FROM stories WHERE id=1", &flagged},
			{"SELECT is_broken FROM stories WHERE id=2", &unflagged},
			{"SELECT definition_id FROM legacy_broken_tag WHERE singleton=1", &legacyID},
			{"SELECT count(*) FROM tag_definitions", &definitions},
			{"SELECT count(*) FROM story_tag_assignments", &assignments},
		} {
			if err := db.QueryRow(query.sql).Scan(query.dest); err != nil {
				t.Fatal(err)
			}
		}
		if flagged != 1 || unflagged != 0 || legacyID != 9 || definitions != 2 || assignments != 1 {
			t.Fatalf("migration/restart %d: flags %d/%d, legacy %d, definitions %d, assignments %d", i, flagged, unflagged, legacyID, definitions, assignments)
		}
		if _, err := upgraded.Writer().Exec("UPDATE stories SET is_broken=2 WHERE id=1"); err == nil {
			t.Fatal("invalid boolean accepted")
		}
		upgraded.Close()
	}
	backups, err := filepath.Glob(filepath.Join(filepath.Dir(path), "library.pre-migration-v015-*.sqlite3"))
	if err != nil || len(backups) != 1 {
		t.Fatalf("backups: %v, %v", backups, err)
	}
}

func TestBrokenFlagMigrationFailureRestoresLegacyDatabase(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "library.sqlite3")
	old, err := openDatabase(ctx, path, migrationsThrough(t, 14))
	if err != nil {
		t.Fatal(err)
	}
	old.Close()
	migrations := migrationsThrough(t, 14)
	body, err := os.ReadFile("migrations/015_story_broken_flag.sql")
	if err != nil {
		t.Fatal(err)
	}
	migrations["migrations/015_story_broken_flag.sql"] = &fstest.MapFile{Data: append(body, []byte("SELECT * FROM missing_table;")...)}
	if _, err := openDatabase(ctx, path, migrations); err == nil {
		t.Fatal("expected migration failure")
	}
	restored, err := openDatabase(ctx, path, migrationsThrough(t, 14))
	if err != nil {
		t.Fatal(err)
	}
	defer restored.Close()
	var count int
	if err := restored.SQL().QueryRow("SELECT count(*) FROM sqlite_master WHERE name='legacy_broken_tag'").Scan(&count); err != nil {
		t.Fatal(err)
	}
	if count != 0 {
		t.Fatal("failed migration was not rolled back")
	}
}
