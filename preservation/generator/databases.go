package main

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/preferences"
	"github.com/01max/librairii/internal/shelves"
)

const sqlTime = "2000-01-01 00:00:00"

func execSQL(db *sql.DB, query string, args ...any) {
	_, err := db.ExecContext(ctx, query, args...)
	must(err)
}
func rowsJSON(db *sql.DB, query string) []map[string]any {
	rows := take(db.QueryContext(ctx, query))
	defer rows.Close()
	columns := take(rows.Columns())
	result := []map[string]any{}
	for rows.Next() {
		values := make([]any, len(columns))
		refs := make([]any, len(columns))
		for i := range values {
			refs[i] = &values[i]
		}
		must(rows.Scan(refs...))
		row := map[string]any{}
		for i, c := range columns {
			v := values[i]
			if b, ok := v.([]byte); ok {
				v = string(b)
			}
			row[c] = v
		}
		result = append(result, row)
	}
	must(rows.Err())
	return result
}
func fixTimes(db *sql.DB) {
	tables := rowsJSON(db, `SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name`)
	for _, r := range tables {
		table := r["name"].(string)
		for _, column := range rowsJSON(db, `PRAGMA table_info("`+table+`")`) {
			name := column["name"].(string)
			switch name {
			case "created_at", "updated_at", "applied_at", "added_at", "accepted_at", "started_at", "finished_at":
				execSQL(db, `UPDATE "`+table+`" SET "`+name+`"=? WHERE "`+name+`" IS NOT NULL AND "`+name+`"!=?`, sqlTime, sqlTime)
			}
		}
	}
}
func snapshot(db *sql.DB) any {
	tables := map[string]any{}
	for _, r := range rowsJSON(db, `SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name`) {
		name := r["name"].(string)
		tables[name] = rowsJSON(db, `SELECT * FROM "`+name+`" ORDER BY rowid`)
	}
	integrity := ""
	must(db.QueryRow(`PRAGMA integrity_check`).Scan(&integrity))
	fk := rowsJSON(db, `PRAGMA foreign_key_check`)
	if integrity != "ok" || len(fk) != 0 {
		panic("invalid synthetic database")
	}
	return map[string]any{"schema": rowsJSON(db, `SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name`), "tables": tables, "integrity": integrity, "foreignKeyViolations": fk}
}
func stableDatabase(db *sql.DB, target string) {
	fixTimes(db)
	must(os.MkdirAll(filepath.Dir(target), 0700))
	if _, err := os.Stat(target); !os.IsNotExist(err) {
		panic("database target exists")
	}
	execSQL(db, `VACUUM INTO '`+strings.ReplaceAll(target, "'", "''")+`'`)
}
func seedPrefixAt(db *sql.DB, version int) {
	switch version {
	case 2:
		execSQL(db, `INSERT INTO stories(id,uuid,embedded_title,created_at,updated_at) VALUES(1,?,'Moon legacy',?,?)`, ids[0], sqlTime, sqlTime)
		body := syntheticStory(ids[0])
		write("databases/legacy-archive.zip", body)
		execSQL(db, `INSERT INTO story_archives(id,story_id,original_filename,detected_format,sha256,byte_size,managed_path,created_at) VALUES(1,1,'legacy.zip','zip',?,?,?,?)`, hash(body), len(body), "archives/"+hash(body)+".zip", sqlTime)
	case 3:
		execSQL(db, `INSERT INTO removal_intents(id,story_id,managed_path,trash_path,created_at) SELECT '70000000-0000-4000-8000-000000000001',1,managed_path,'trash/removals/70000000-0000-4000-8000-000000000001/legacy.zip',? FROM story_archives WHERE story_id=1`, sqlTime)
	case 4:
		execSQL(db, `INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position,created_at,updated_at) VALUES(8,'favorite','favorite','Favorite','#405CF5','boolean','user',0,?,?)`, sqlTime, sqlTime)
		execSQL(db, `INSERT INTO story_tag_assignments(story_id,definition_id,source,created_at) VALUES(1,8,'manual',?)`, sqlTime)
	case 5:
		execSQL(db, `INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,presentation,position,is_protected,created_at,updated_at) VALUES(7,'broken','broken','Broken','#FF705C','boolean','builtin','warning',0,1,?,?)`, sqlTime, sqlTime)
		execSQL(db, `INSERT INTO story_tag_assignments(story_id,definition_id,source,created_at) VALUES(1,7,'manual',?)`, sqlTime)
	case 11:
		execSQL(db, `INSERT INTO shelves(id,name,normalized_name,position,query_version,query_payload,created_at,updated_at) VALUES(1,'Legacy moon','legacy moon',0,1,?, ?,?)`, `{"name":"moon","booleanFilters":[{"definitionId":7,"state":"true"},{"definitionId":8,"state":"true"}]}`, sqlTime, sqlTime)
	case 12:
		execSQL(db, `INSERT INTO file_operations(id,kind,status,total_items,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination,created_at) VALUES('80000000-0000-4000-8000-000000000001','export','queued',0,'shelf','[1]','["Legacy moon"]','/fixture/export',?)`, sqlTime)
	case 14:
		execSQL(db, `UPDATE tag_definitions SET color='#405CF5' WHERE id=7`)
	case 17:
		execSQL(db, `INSERT INTO shelves(id,name,normalized_name,position,created_at,updated_at) VALUES(1,'Approved','approved',0,?,?)`, sqlTime, sqlTime)
		execSQL(db, `INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title,added_at) VALUES(1,?,0,0,'Moon legacy',?),(1,?,1,1,'Unavailable',?)`, ids[0], sqlTime, ids[5], sqlTime)
		execSQL(db, `INSERT INTO shelf_sources(id,shelf_id,collection_id,collection_name,created_at) VALUES(1,1,1,'Legacy moon',?)`, sqlTime)
		execSQL(db, `INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id,accepted_at) VALUES(1,?,1,?)`, ids[0], sqlTime)
		execSQL(db, `INSERT INTO shelf_exclusions(shelf_id,story_uuid,created_at) VALUES(1,?,?)`, ids[4], sqlTime)
	}
}
func databaseFixtures() {
	migrations := take(filepath.Glob("internal/database/migrations/*.sql"))
	if len(migrations) != 18 {
		panic("wrong migration set")
	}
	provenance := []any{}
	for _, p := range migrations {
		b := bytesAt(p)
		name := filepath.Base(p)
		write("migrations/"+name, b)
		provenance = append(provenance, map[string]any{"path": "migrations/" + name, "sha256": hash(b)})
	}
	versions := []any{}
	for prefix := 1; prefix <= 18; prefix++ {
		dir := take(os.MkdirTemp("", "librairii-prefix-"))
		source := filepath.Join(dir, "source.sqlite3")
		raw := take(sql.Open("sqlite", source))
		raw.SetMaxOpenConns(1)
		execSQL(raw, `PRAGMA foreign_keys=ON`)
		execSQL(raw, `CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)`)
		for i := 0; i < prefix; i++ {
			tx := take(raw.BeginTx(ctx, nil))
			_, err := tx.ExecContext(ctx, string(bytesAt(migrations[i])))
			must(err)
			_, err = tx.ExecContext(ctx, `INSERT INTO schema_migrations(version,applied_at) VALUES(?,?)`, i+1, sqlTime)
			must(err)
			must(tx.Commit())
			seedPrefixAt(raw, i+1)
		}
		fixTimes(raw)
		name := fmt.Sprintf("databases/prefix-%03d.sqlite3", prefix)
		stableDatabase(raw, filepath.Join(output, name))
		before := snapshot(raw)
		must(raw.Close())
		// Every supported prefix is actually reopened/upgraded by the unchanged Go API.
		copy := filepath.Join(dir, "upgrade.sqlite3")
		must(os.WriteFile(copy, bytesAt(filepath.Join(output, name)), 0600))
		upgraded := take(database.Open(ctx, copy))
		service := take(collections.NewService(upgraded.Writer()))
		opened := []any{}
		for _, c := range take(service.List(ctx)) {
			o, err := service.Open(ctx, c.ID)
			opened = append(opened, map[string]any{"id": c.ID, "result": o, "error": errorCode(err)})
		}
		fixTimes(upgraded.Writer())
		after := snapshot(upgraded.SQL())
		must(upgraded.Close())
		jsonName := fmt.Sprintf("databases/prefix-%03d.json", prefix)
		jsonFile(jsonName, map[string]any{"prefix": prefix, "before": before, "afterGoOpen": after, "openedCollections": canonical(opened)})
		versions = append(versions, map[string]any{"prefix": prefix, "file": name, "sha256": hash(bytesAt(filepath.Join(output, name))), "expected": jsonName, "goOpen": "passed to schema018", "syntheticSeedSteps": "database-contract.md"})
		must(os.RemoveAll(dir))
	}
	// Zero means absence, not a foreign SQLite file with an empty migration table.
	jsonFile("databases/absent.json", map[string]any{"prefix": 0, "input": "database file absent", "expected": "fresh initialization to schema018; do not invent a prefix000 foreign file"})
	representativeDatabase()
	jsonFile("databases/provenance.json", map[string]any{"version": 1, "oracleCommit": "73a93303a9a44cf0a92eaf525331561ed8c04460", "go": runtime.Version(), "driver": "modernc.org/sqlite v1.60.1", "migrations": provenance, "prefixes": versions, "timestamps": sqlTime, "representation": "SQLite VACUUM INTO creates complete consistent standalone snapshots, no omitted WAL/SHM; never raw-copy a live DB", "synthetic": true, "productionData": false})
}
func representativeDatabase() {
	w := newWorld()
	defer w.close()
	seedWorld(w)
	execSQL(w.db.Writer(), `INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position,created_at,updated_at) VALUES(8,'favorite','favorite','Favorite','#405CF5','boolean','user',0,?,?),(9,'mood','mood','Mood','#267D5D','choice','user',1,?,?)`, sqlTime, sqlTime, sqlTime, sqlTime)
	execSQL(w.db.Writer(), `INSERT INTO tag_values(id,definition_id,key,normalized_key,label,position,created_at,updated_at) VALUES(11,9,'calm','calm','Calm',0,?,?)`, sqlTime, sqlTime)
	execSQL(w.db.Writer(), `INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source,created_at) VALUES(1,8,NULL,'manual',?),(2,9,11,'manual',?)`, sqlTime, sqlTime)
	c := take(w.collections.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"}))
	creation := take(w.shelves.PreviewCreation(ctx, shelves.CreationRequest{Name: "Approved", ManualUUIDs: []string{ids[2]}, CollectionIDs: []int64{c.ID}}))
	sh := take(w.shelves.CreateFromReview(ctx, creation.Token, []string{ids[1], ids[2]}))
	execSQL(w.db.Writer(), `INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title,added_at) VALUES(?,?,2,0,'Unavailable',?)`, sh.ID, ids[5], sqlTime)
	review := take(w.shelves.Preview(ctx, sh.ID))
	take(w.shelves.Apply(ctx, sh.ID, review.Token, []shelves.Decision{{UUID: ids[4], Action: "exclude"}}))
	now := take(time.Parse(time.RFC3339, frozenTime))
	repo := operations.NewRepository(w.db.Writer())
	completed := take(repo.CreateImport(ctx, "90000000-0000-4000-8000-000000000001", []operations.NewItem{{SourceName: "story-1.zip", TotalBytes: 1}}, now))
	must(repo.MarkRunning(ctx, completed.ID, now))
	must(repo.MarkItemRunning(ctx, completed.Items[0].ID))
	item := completed.Items[0]
	item.Status = operations.ItemSucceeded
	item.CompletedBytes = 1
	item.OutcomeCode = "imported"
	must(repo.CompleteItem(ctx, completed.ID, item))
	must(repo.Finish(ctx, completed.ID, operations.StatusSucceeded, "", "", now))
	pending := take(repo.CreateImport(ctx, "90000000-0000-4000-8000-000000000002", []operations.NewItem{{SourceName: "pending.zip", TotalBytes: 100}}, now))
	must(repo.MarkRunning(ctx, pending.ID, now))
	must(repo.MarkItemRunning(ctx, pending.Items[0].ID))
	must(repo.UpdateItemProgress(ctx, pending.ID, pending.Items[0].ID, 25))
	take(repo.CreateExport(ctx, "90000000-0000-4000-8000-000000000003", operations.ExportSource{Type: operations.ExportSourceCuratedShelf, ShelfIDs: []int64{sh.ID}, ShelfNames: []string{"Approved"}}, "/fixture/export", []operations.NewItem{{StoryUUID: ids[5], StoryTitle: "Unavailable", SourceName: "Unavailable", ExportSkipCode: "story_unavailable"}}, now))
	execSQL(w.db.Writer(), `INSERT INTO removal_intents(id,story_id,managed_path,trash_path,created_at) SELECT '70000000-0000-4000-8000-000000000002',2,managed_path,'trash/removals/70000000-0000-4000-8000-000000000002/story-2.zip',? FROM story_archives WHERE story_id=2`, sqlTime)
	fixTimes(w.db.Writer())
	jsonFile("libraries/schema018/state.json", snapshot(w.db.SQL()))
	jsonFile("libraries/schema018/history.json", canonical(take(repo.History(ctx, ""))))
	stableDatabase(w.db.Writer(), filepath.Join(output, "libraries/schema018/db/librairii.sqlite3"))
	libraryID := "a0000000-0000-4000-8000-000000000001"
	write("libraries/schema018/.librairii-library", []byte(libraryID+"\n"))
	for _, r := range rowsJSON(w.db.SQL(), `SELECT managed_path FROM story_archives ORDER BY id`) {
		rel := r["managed_path"].(string)
		write("libraries/schema018/"+rel, bytesAt(filepath.Join(w.dir, rel)))
	}
	profile := preferences.Profile{Version: 1, Language: "fr", Root: "/fixture/source", LibraryID: libraryID, Complete: true}
	must(profile.Validate())
	jsonFile("profiles/settings.json", profile)
	for _, phase := range []string{"reserved", "published"} {
		j := preferences.Journal{ID: "b0000000-0000-4000-8000-000000000001", LibraryID: libraryID, Source: "/fixture/source", Destination: "/fixture/destination", Phase: phase, Entries: []preferences.Entry{{Path: ".librairii-library", Size: int64(len(libraryID) + 1), SHA256: hash([]byte(libraryID + "\n"))}}}
		must(j.Validate())
		jsonFile("profiles/relocation-"+phase+".json", j)
	}
	jsonFile("profiles/recovery-contract.json", map[string]any{"libraryId": libraryID, "reserved": "source authoritative before durable profile commit", "published": "profile root decides authority; phase alone is insufficient", "absolutePaths": "deliberate non-live /fixture placeholders: do not replay; clone and identity-verify/rebind for target OS first", "pendingRemoval": "pre-move removal intent with managed bytes still present", "pendingOperations": "one running partial import and one queued skipped-unavailable Shelf export; normal restart must reconcile", "immutableFiles": "all managed files hashed in corpus manifest; no user data"})
}
