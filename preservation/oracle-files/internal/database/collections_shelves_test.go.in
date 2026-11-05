package database

import (
	"context"
	"fmt"
	"path/filepath"
	"testing"
	"testing/fstest"
)

func TestCollectionsUpgradePreservesDefinitionsAndHistoricalScope(t *testing.T) {
	for _, version := range []int{14, 15} {
		t.Run(fmt.Sprint(version), func(t *testing.T) {
			ctx := context.Background()
			path := filepath.Join(t.TempDir(), "library.sqlite3")
			old, err := openDatabase(ctx, path, migrationsThrough(t, version))
			if err != nil {
				t.Fatal(err)
			}
			_, err = old.Writer().Exec(`
				INSERT INTO shelves(id,name,normalized_name,position,query_version,query_payload,validity_state,created_at)
				VALUES(7,'Adventures','adventures',0,3,'{"name":"dragon"}','needs_attention','2025-01-01');
				INSERT INTO file_operations(id,kind,status,export_source_type,export_source_shelf_ids,
				 export_source_shelf_names,export_destination,total_items,total_bytes)
				VALUES('123e4567-e89b-42d3-a456-426614174111','export','succeeded','shelf','[7]','["Adventures"]','/tmp/export',0,0);`)
			if err != nil {
				t.Fatal(err)
			}
			if err := old.Close(); err != nil {
				t.Fatal(err)
			}
			for restart := 0; restart < 2; restart++ {
				db, err := Open(ctx, path)
				if err != nil {
					t.Fatal(err)
				}
				var name, payload, validity, created, source string
				var revision, curated int
				if err := db.SQL().QueryRow(`SELECT name,query_payload,validity_state,created_at,revision FROM collections WHERE id=7`).Scan(&name, &payload, &validity, &created, &revision); err != nil {
					t.Fatal(err)
				}
				if name != "Adventures" || payload != `{"name":"dragon"}` || validity != "needs_attention" || created != "2025-01-01" || revision != 1 {
					t.Fatalf("changed definition: %q %q %q %q %d", name, payload, validity, created, revision)
				}
				if err := db.SQL().QueryRow(`SELECT count(*) FROM shelves`).Scan(&curated); err != nil || curated != 0 {
					t.Fatalf("curated copies: %d %v", curated, err)
				}
				if err := db.SQL().QueryRow(`SELECT export_source_type FROM file_operations WHERE id='123e4567-e89b-42d3-a456-426614174111'`).Scan(&source); err != nil || source != "shelf" {
					t.Fatalf("history: %q %v", source, err)
				}
				if _, err := db.Writer().Exec(`UPDATE file_operations SET export_source_type='curated_shelf' WHERE id='123e4567-e89b-42d3-a456-426614174111'`); err == nil {
					t.Fatal("history identity became mutable")
				}
				if err := db.Close(); err != nil {
					t.Fatal(err)
				}
			}
		})
	}
}

func TestCuratedSchemaRetainsUUIDDecisionsAndValidatesCopyScopes(t *testing.T) {
	db, err := Open(context.Background(), filepath.Join(t.TempDir(), "library.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	_, err = db.Writer().Exec(`
		INSERT INTO stories(id,uuid) VALUES(1,'123e4567-e89b-42d3-a456-426614174000');
		INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload) VALUES(1,'Same','same',0,4,'{}');
		INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Same','same',0);
		INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(1,'123e4567-e89b-42d3-a456-426614174000',0,'Kept');
		INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(1,'223e4567-e89b-42d3-a456-426614174000');
		DELETE FROM stories WHERE id=1;
		INSERT INTO stories(id,uuid) VALUES(9,'123e4567-e89b-42d3-a456-426614174000');
		INSERT INTO file_operations(id,kind,status,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination)
		VALUES('123e4567-e89b-42d3-a456-426614174222','export','queued','curated_shelf','[1]','["Same"]','/tmp/export');
		INSERT INTO file_operation_items(operation_id,status,source_name,story_uuid,story_title,export_skip_code,total_bytes)
		VALUES('123e4567-e89b-42d3-a456-426614174222','pending','Missing','223e4567-e89b-42d3-a456-426614174000','Missing','story_unavailable',0);`)
	if err != nil {
		t.Fatal(err)
	}
	var id, exclusions int
	if err := db.SQL().QueryRow(`SELECT s.id FROM shelf_stories m JOIN stories s ON s.uuid=m.story_uuid WHERE m.shelf_id=1`).Scan(&id); err != nil || id != 9 {
		t.Fatalf("UUID membership lost: %d %v", id, err)
	}
	if err := db.SQL().QueryRow(`SELECT count(*) FROM shelf_exclusions`).Scan(&exclusions); err != nil || exclusions != 1 {
		t.Fatalf("exclusions lost: %d %v", exclusions, err)
	}
	if _, err := db.Writer().Exec(`INSERT INTO file_operation_items(operation_id,status,source_name,story_uuid,story_title) VALUES('123e4567-e89b-42d3-a456-426614174222','pending','Bad','323e4567-e89b-42d3-a456-426614174000','Bad')`); err == nil {
		t.Fatal("incomplete ready item accepted")
	}
	if _, err := db.Writer().Exec(`UPDATE file_operation_items SET export_skip_code=NULL WHERE operation_id='123e4567-e89b-42d3-a456-426614174222'`); err == nil {
		t.Fatal("skipped scope became mutable")
	}
}

func TestCollectionMigrationFailureRestoresOldShelfTable(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "library.sqlite3")
	old, err := openDatabase(ctx, path, migrationsThrough(t, 15))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = old.Writer().Exec(`INSERT INTO shelves(name,normalized_name,position,query_version,query_payload) VALUES('Keep','keep',0,4,'{}')`); err != nil {
		t.Fatal(err)
	}
	if err := old.Close(); err != nil {
		t.Fatal(err)
	}
	migrations := migrationsThrough(t, 16)
	migrations["migrations/017_failure.sql"] = &fstest.MapFile{Data: []byte("SELECT * FROM missing_table;")}
	if _, err := openDatabase(ctx, path, migrations); err == nil {
		t.Fatal("expected migration failure")
	}
	restored, err := openDatabase(ctx, path, migrationsThrough(t, 15))
	if err != nil {
		t.Fatal(err)
	}
	defer restored.Close()
	var name string
	if err := restored.SQL().QueryRow(`SELECT name FROM shelves`).Scan(&name); err != nil || name != "Keep" {
		t.Fatalf("backup did not restore definition: %q %v", name, err)
	}
}
