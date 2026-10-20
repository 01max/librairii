package app

import (
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"testing"
	"time"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
	"github.com/01max/librairii/internal/tagging"
)

func TestMigratedCollectionThroughShelfReviewRestartAndExport(t *testing.T) {
	ctx := context.Background()
	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(layout.Database, "librairii.sqlite3")
	legacy, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	legacy.SetMaxOpenConns(1)
	defer legacy.Close()
	if _, err := legacy.Exec("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"); err != nil {
		t.Fatal(err)
	}
	files, err := filepath.Glob("../database/migrations/*.sql")
	if err != nil {
		t.Fatal(err)
	}
	for _, file := range files {
		version, err := strconv.Atoi(filepath.Base(file)[:3])
		if err != nil {
			t.Fatal(err)
		}
		if version > 14 {
			continue
		}
		body, err := os.ReadFile(file)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := legacy.Exec(string(body)); err != nil {
			t.Fatal(err)
		}
		if _, err := legacy.Exec("INSERT INTO schema_migrations(version) VALUES (?)", version); err != nil {
			t.Fatal(err)
		}
	}
	ids := []int64{}
	var healthyBytes []byte
	for index, fixture := range []testfixture.Archive{
		testfixture.GenericZIP(),
		storyFixtureWithUUID(testfixture.GenericZIP(), "11112222-3333-4444-8555-666677778888", "second.zip"),
	} {
		source, err := testfixture.WriteZIP(t.TempDir(), fixture)
		if err != nil {
			t.Fatal(err)
		}
		content, err := os.ReadFile(source)
		if err != nil {
			t.Fatal(err)
		}
		sum := sha256.Sum256(content)
		hash := hex.EncodeToString(sum[:])
		name := fmt.Sprintf("legacy-%d.zip", index)
		relative := filepath.ToSlash(filepath.Join("archives", hash, name))
		target := filepath.Join(layout.Root, filepath.FromSlash(relative))
		if err := os.MkdirAll(filepath.Dir(target), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(target, content, 0600); err != nil {
			t.Fatal(err)
		}
		story, _, err := catalog.NewRepository(legacy).Create(ctx, catalog.CreateStory{UUID: fixture.ExpectedUUID, EmbeddedTitle: name, OriginalFilename: name, DetectedFormat: catalog.FormatZIP, SHA256: hash, ByteSize: int64(len(content)), ManagedPath: relative})
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, story.ID)
		if index == 1 {
			healthyBytes = content
		}
	}
	if _, err := legacy.Exec(`INSERT INTO tag_definitions
        (id,key,normalized_key,label,color,kind,source,presentation,position,is_protected)
        VALUES (9,'broken','broken','Broken','#405CF5','boolean','builtin','warning',0,1);
        INSERT INTO shelves (id,name,normalized_name,position,query_version,query_payload)
        VALUES (7,'Broken shelf','broken shelf',0,3,'{"includeBroken":true,"anyOf":[{"booleanFilters":[{"definitionId":9,"state":"true"}]}]}');`); err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec("INSERT INTO story_tag_assignments (story_id,definition_id,source) VALUES (?,9,'manual')", ids[0]); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	provider := &runtimeStorageProvider{layout: layout, db: opened}
	payload, err := os.ReadFile("../lunii/testdata/catalog.json")
	if err != nil {
		t.Fatal(err)
	}
	var document map[string]any
	if err := json.Unmarshal(payload, &document); err != nil {
		t.Fatal(err)
	}
	entry := document["response"].(map[string]any)["fixture-pack-001"].(map[string]any)
	entry["uuid"] = testfixture.StoryUUID
	delete(entry["localized_infos"].(map[string]any)["en_GB"].(map[string]any), "image")
	payload, err = json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	start := func() (*Application, *ImportRuntime, *runtimeEventRecorder) {
		t.Helper()
		events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 64)}
		runtime, err := NewImportRuntime(provider, fixedClock{now: time.Now()}, events, 1, WithMetadataFetcher(&runtimeCatalogFetcher{payload: payload}))
		if err != nil {
			t.Fatal(err)
		}
		application, err := New(Dependencies{Clock: fixedClock{now: time.Now()}, Dialogs: fakeDialogs{}, Events: events, Readiness: fakeReadiness{report: ReadinessReport{MutationsAllowed: true}}, Operations: runtime, Library: runtime, Removal: runtime, Tags: runtime, Collections: runtime, Shelves: runtime})
		if err != nil {
			t.Fatal(err)
		}
		if err := application.Start(ctx); err != nil {
			t.Fatal(err)
		}
		if application.Status().State != StateReady {
			t.Fatalf("startup=%#v", application.Status())
		}
		return application, runtime, events
	}
	application, runtime, events := start()
	defer func() { application.Stop(ctx); provider.db.Close() }()
	assertQuery := func(mode library.BrokenVisibility, want int) {
		t.Helper()
		response := application.QueryStories(ctx, library.StoryLibraryQuery{})
		if response.Error != nil || response.Page == nil || response.Page.TotalItems != want {
			t.Fatalf("query %s=%#v", mode, response)
		}
	}
	assertQuery("", 2)
	compatibility := application.LibraryCompatibility(ctx)
	if compatibility.Error != nil || compatibility.Compatibility.LegacyBrokenTagID != 9 {
		t.Fatalf("compatibility=%#v", compatibility)
	}
	shelf := application.OpenCollection(ctx, 7, library.ListRequest{})
	if shelf.Error != nil || shelf.Evaluation == nil || shelf.Evaluation.Page.TotalItems != 2 || shelf.Evaluation.Page.Stories[0].ID != ids[0] {
		t.Fatalf("migrated shelf=%#v", shelf)
	}
	set := application.SetStoriesBroken(ctx, ids[1:], true)
	if set.Error != nil || set.Result == nil {
		t.Fatalf("set=%#v", set)
	}
	assertQuery("", 2)
	if undo := application.RestoreStoryFlags(ctx, set.Result.Changes); undo.Error != nil {
		t.Fatalf("undo=%#v", undo)
	}
	assertQuery("", 2)
	cleared := application.SetStoriesBroken(ctx, ids[:1], false)
	if cleared.Error != nil || cleared.Result == nil {
		t.Fatalf("clear=%#v", cleared)
	}
	assertQuery("", 2)
	if undo := application.RestoreStoryFlags(ctx, cleared.Result.Changes); undo.Error != nil {
		t.Fatalf("undo clear=%#v", undo)
	}
	application.Stop(ctx)
	if err := provider.db.Close(); err != nil {
		t.Fatal(err)
	}
	provider.db, err = database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	application, runtime, events = start()
	assertQuery("", 2)
	refresh := application.RefreshOfficialMetadata(ctx)
	if refresh.Error != nil || refresh.Operation == nil {
		t.Fatalf("refresh=%#v", refresh)
	}
	if terminal := events.waitTerminal(t, refresh.Operation.ID); terminal.Status != operations.StatusSucceeded {
		t.Fatalf("refresh terminal=%#v", terminal)
	}
	detail := application.StoryDetail(ctx, ids[0])
	if detail.Error != nil || detail.Detail == nil || !detail.Detail.Story.Broken || detail.Detail.Story.Sources.Title != library.SourceOfficial {
		t.Fatalf("refreshed flagged detail=%#v", detail)
	}
	destination := t.TempDir()
	preflight, err := runtime.PrepareExport(ctx, exporter.PreflightRequest{SourceType: operations.ExportSourceSelection, StoryIDs: ids}, destination)
	if err != nil || !preflight.CanExport || preflight.ReadyCount != 1 {
		t.Fatalf("preflight=%#v, %v", preflight, err)
	}
	exported := application.StartPreparedExport(ctx, preflight.PreparationID)
	if exported.Error != nil || exported.Operation == nil {
		t.Fatalf("export=%#v", exported)
	}
	if terminal := events.waitTerminal(t, exported.Operation.ID); terminal.Status != operations.StatusPartiallySucceeded {
		t.Fatalf("export terminal=%#v", terminal)
	}
	if _, err := os.Stat(filepath.Join(destination, "legacy-0.zip")); !os.IsNotExist(err) {
		t.Fatalf("flagged archive was exported: %v", err)
	}
	copied, err := os.ReadFile(filepath.Join(destination, "legacy-1.zip"))
	if err != nil || !bytes.Equal(copied, healthyBytes) {
		t.Fatalf("flagged archive bytes changed: %v", err)
	}
	var reserved int
	if err := provider.SQL().QueryRow("SELECT count(*) FROM tag_definitions WHERE normalized_key='broken' AND source='builtin'").Scan(&reserved); err != nil {
		t.Fatal(err)
	}
	if reserved != 0 {
		t.Fatal("restart or refresh reseeded the retired tag")
	}
	// The migrated saved query now supplies a curated Shelf. Preview alone never creates it.
	seed := application.PreviewShelfCreation(ctx, shelves.CreationRequest{Name: "Approved", CollectionIDs: []int64{7}, ManualUUIDs: []string{testfixture.StoryUUID}})
	if seed.Error != nil || seed.Review == nil || len(seed.Review.Additions) != 2 {
		t.Fatalf("seed=%#v", seed)
	}
	initialUUIDs := []string{}
	for _, item := range seed.Review.Additions {
		initialUUIDs = append(initialUUIDs, item.UUID)
	}
	created := application.CreateShelfFromReview(ctx, seed.Review.Token, initialUUIDs)
	if created.Error != nil || created.Shelf == nil {
		t.Fatalf("create Shelf=%#v", created)
	}
	shelfID := created.Shelf.ID
	thirdUUID := "99992222-3333-4444-8555-666677778888"
	thirdPath, err := testfixture.WriteZIP(t.TempDir(), storyFixtureWithUUID(testfixture.GenericZIP(), thirdUUID, "third.zip"))
	if err != nil {
		t.Fatal(err)
	}
	imported, err := runtime.StartImport(ctx, []string{thirdPath})
	if err != nil {
		t.Fatal(err)
	}
	if result := events.waitTerminal(t, imported.ID); result.Status != operations.StatusSucceeded {
		t.Fatalf("third import=%#v", result)
	}
	proposal := application.PreviewShelfUpdate(ctx, shelfID)
	if proposal.Error != nil || proposal.Review == nil || len(proposal.Review.Additions) != 1 {
		t.Fatalf("import suggestion=%#v", proposal)
	}
	if result := application.ApplyShelfUpdate(ctx, shelfID, proposal.Review.Token, []shelves.Decision{{UUID: thirdUUID, Action: "exclude"}}); result.Error != nil {
		t.Fatal(result.Error)
	}
	proposal = application.PreviewShelfUpdate(ctx, shelfID)
	if proposal.Error != nil || len(proposal.Review.Additions) != 0 {
		t.Fatalf("exclusion lost=%#v", proposal)
	}
	if result := application.ClearShelfExclusion(ctx, shelfID, thirdUUID); result.Error != nil {
		t.Fatal(result.Error)
	}

	tag := application.CreateTagDefinition(ctx, tagging.CreateDefinition{Key: "favorite", Label: "Favorite", Color: "#405CF5", Kind: tagging.KindBoolean})
	if tag.Error != nil || tag.Definition == nil {
		t.Fatalf("favorite=%#v", tag)
	}
	current := application.QueryStories(ctx, library.StoryLibraryQuery{})
	var thirdID int64
	for _, story := range current.Page.Stories {
		if story.UUID == thirdUUID {
			thirdID = story.ID
		}
	}
	if result := application.SetBooleanTag(ctx, []int64{ids[1], thirdID}, tag.Definition.ID, true); result.Error != nil {
		t.Fatal(result.Error)
	}
	replacement := application.ReplaceCollectionQuery(ctx, 7, library.StoryLibraryQuery{BooleanFilters: []library.BooleanFilter{{DefinitionID: tag.Definition.ID, State: library.BooleanTrue}}})
	if replacement.Error != nil {
		t.Fatal(replacement.Error)
	}
	proposal = application.PreviewShelfUpdate(ctx, shelfID)
	if proposal.Error != nil || len(proposal.Review.Additions) != 1 || len(proposal.Review.Removals) != 0 {
		t.Fatalf("tag-driven review lost protected member=%#v", proposal)
	}
	if result := application.ApplyShelfUpdate(ctx, shelfID, proposal.Review.Token, []shelves.Decision{{UUID: thirdUUID, Action: "add"}}); result.Error != nil {
		t.Fatal(result.Error)
	}
	if result := application.RemoveStoriesFromShelf(ctx, shelfID, []string{initialUUIDs[1]}); result.Error != nil {
		t.Fatal(result.Error)
	}
	removed := application.RemoveStory(ctx, ids[0])
	if removed.Error != nil || removed.Result == nil {
		t.Fatalf("remove approved UUID=%#v", removed)
	}

	unavailable := application.OpenShelf(ctx, shelfID, library.StoryLibraryQuery{})
	if unavailable.Error != nil || unavailable.Evaluation.Shelf.UnavailableCount != 1 {
		t.Fatalf("unavailable placeholder=%#v", unavailable)
	}
	missingPlan, err := runtime.PrepareExport(ctx, exporter.PreflightRequest{SourceType: operations.ExportSourceCuratedShelf, ShelfIDs: []int64{shelfID}}, t.TempDir())
	if err != nil || missingPlan.ReadyCount != 1 || missingPlan.ResolvedCount != 2 {
		t.Fatalf("unavailable export=%#v %v", missingPlan, err)
	}
	sourcePath, err := testfixture.WriteZIP(t.TempDir(), testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	reimported, err := runtime.StartImport(ctx, []string{sourcePath})
	if err != nil {
		t.Fatal(err)
	}
	if result := events.waitTerminal(t, reimported.ID); result.Status != operations.StatusSucceeded {
		t.Fatalf("reimport=%#v", result)
	}
	reconnected := application.OpenShelf(ctx, shelfID, library.StoryLibraryQuery{})
	var newID int64
	for _, member := range reconnected.Evaluation.Members {
		if member.UUID == testfixture.StoryUUID {
			if member.Story == nil || !member.Protected {
				t.Fatalf("reconnected member=%#v", member)
			}
			newID = member.Story.ID
		}
	}
	if newID == 0 || newID == ids[0] {
		t.Fatalf("UUID did not reconnect to new row: %d", newID)
	}
	if result := application.SetStoriesBroken(ctx, []int64{newID}, true); result.Error != nil {
		t.Fatal(result.Error)
	}
	application.Stop(ctx)
	if err := provider.db.Close(); err != nil {
		t.Fatal(err)
	}
	provider.db, err = database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	application, runtime, events = start()
	persisted := application.OpenShelf(ctx, shelfID, library.StoryLibraryQuery{})
	if persisted.Error != nil || len(persisted.Evaluation.Members) != 2 || len(persisted.Evaluation.Exclusions) != 0 || persisted.Evaluation.Shelf.BrokenCount != 1 {
		t.Fatalf("restart decisions=%#v", persisted)
	}
	final, err := runtime.PrepareExport(ctx, exporter.PreflightRequest{SourceType: operations.ExportSourceCuratedShelf, ShelfIDs: []int64{shelfID}}, t.TempDir())
	if err != nil || final.ReadyCount != 1 || final.ResolvedCount != 2 {
		t.Fatalf("Shelf export=%#v %v", final, err)
	}
	exported = application.StartPreparedExport(ctx, final.PreparationID)
	if exported.Error != nil || exported.Operation == nil {
		t.Fatalf("curated export=%#v", exported)
	}
	terminal := events.waitTerminal(t, exported.Operation.ID)
	if terminal.Status != operations.StatusPartiallySucceeded || terminal.ExportSourceType != operations.ExportSourceCuratedShelf {
		t.Fatalf("curated terminal=%#v", terminal)
	}

}
