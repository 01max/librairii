// The exporter is compiled only inside an exact disposable Go oracle snapshot.
package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"golang.org/x/text/unicode/norm"
	"os"
	"path/filepath"
	"strings"
	"unicode"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/searchtext"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
)

var ctx = context.Background()
var output string

const frozenTime = "2000-01-01T00:00:00Z"

func must(err error) {
	if err != nil {
		panic(err)
	}
}
func take[T any](v T, err error) T { must(err); return v }
func hash(b []byte) string         { sum := sha256.Sum256(b); return hex.EncodeToString(sum[:]) }
func bytesAt(path string) []byte   { return take(os.ReadFile(path)) }
func write(name string, body []byte) {
	p := filepath.Join(output, name)
	must(os.MkdirAll(filepath.Dir(p), 0700))
	must(os.WriteFile(p, body, 0600))
}
func jsonFile(name string, v any) {
	body := take(json.MarshalIndent(v, "", "  "))
	write(name, append(body, '\n'))
}

// Only opaque review authorization tokens and known observation timestamps vary.
// IDs, warning codes, order, query structure, checksums and archive bytes survive.
func canonical(v any) any {
	var tree any
	must(json.Unmarshal(take(json.Marshal(v)), &tree))
	var visit func(any) any
	visit = func(value any) any {
		switch node := value.(type) {
		case map[string]any:
			for k, v := range node {
				switch k {
				case "token":
					if v != "" {
						node[k] = "<review-ticket>"
					}
				case "importedAt", "createdAt", "updatedAt", "acceptedAt", "addedAt":
					if v != nil && v != "" {
						node[k] = frozenTime
					}
				default:
					node[k] = visit(v)
				}
			}
		case []any:
			for i, v := range node {
				node[i] = visit(v)
			}
		}
		return value
	}
	return visit(tree)
}
func errorCode(err error) string {
	if err == nil {
		return ""
	}
	var validation *inspection.ValidationError
	if errors.As(err, &validation) {
		return string(validation.Code)
	}
	switch {
	case errors.Is(err, collections.ErrUnsupportedSavedQueryVersion):
		return "unsupported_saved_query_version"
	case errors.Is(err, collections.ErrInvalidSavedLibraryQuery):
		return "invalid_saved_query"
	case errors.Is(err, library.ErrInvalidStoryLibraryHash):
		return "invalid_url_query"
	case errors.Is(err, shelves.ErrStaleReview):
		return "stale_review"
	case errors.Is(err, shelves.ErrInvalidInput):
		return "invalid_shelf_input"
	case errors.Is(err, library.ErrFlagRestoreStale):
		return "stale_flag_restore"
	case errors.Is(err, library.ErrFlagStoryUnavailable):
		return "flag_story_unavailable"
	}
	panic(fmt.Sprintf("unclassified fixture error: %v", err))
}
func unicodeGolden() {
	type pair struct {
		Input      string `json:"input"`
		Normalized string `json:"normalized"`
	}
	inputs := []string{"  ÉTÉ\t100%_Magique  ", "ΟΣ", "ος", "Straße", "STRASSE", "İstanbul", "e\u0301te\u0301", "Æsir ŒUVRE", "ﬃ ① ＡＢＣ", "a\u00a0\u2003 b", "\u0301", "  ", "💫 Étoile", "中文故事", "ひらがな カタカナ", "한글", "العَرَبِيَّة", "100%_\\literal", "Kelvin Ångström", "I ı İ i", "Σσς", "Cafe\u034f", "a\u200db", "Å A\u030a", "\tMoon\r\n Forest\t"}
	result := []pair{}
	for _, v := range inputs {
		result = append(result, pair{v, searchtext.Normalize(v)})
	}
	jsonFile("unicode.json", result)
	jsonFile("unicode-metadata.json", map[string]any{"goUnicodeVersion": unicode.Version, "normalizationUnicodeVersion": norm.Version, "textModule": "golang.org/x/text v0.43.0", "policy": "TrimSpace then cases.Fold then NFKD; strip Mn; collapse Unicode whitespace; no ASCII approximation"})
}
func queryGolden() {
	type savedInput struct {
		ID       string `json:"id"`
		Version  int    `json:"version"`
		Payload  string `json:"payload"`
		LegacyID int64  `json:"legacyBrokenId"`
	}
	saved := []savedInput{}
	for version := 1; version <= 5; version++ {
		saved = append(saved, savedInput{fmt.Sprintf("saved-v%d-default", version), version, `{}`, 7})
		saved = append(saved, savedInput{fmt.Sprintf("saved-v%d-nested", version), version, `{"anyOf":[{"name":"  ÉTÉ  %_ ","booleanFilters":[{"definitionId":7,"state":"true"}]},{"name":"Moon"}]}`, 7})
	}
	saved = append(saved, savedInput{"v4-retired-visibility", 4, `{"brokenVisibility":"only","anyOf":[{"brokenState":"false"},{"name":"moon"}]}`, 7}, savedInput{"v5-reused-user-tag", 5, `{"booleanFilters":[{"definitionId":7,"state":"true"}]}`, 7}, savedInput{"v5-retired-filter-rejected", 5, `{"brokenState":"true"}`, 7}, savedInput{"unknown-version", 6, `{}`, 7}, savedInput{"duplicate-property", 5, `{"name":"moon","name":"forest"}`, 7}, savedInput{"unknown-property", 5, `{"unexpected":true}`, 7}, savedInput{"invalid-json", 5, `{`, 7}, savedInput{"duplicate-filter", 5, `{"booleanFilters":[{"definitionId":4,"state":"true"},{"definitionId":4,"state":"false"}]}`, 7})
	rows := []any{}
	for _, in := range saved {
		migrated, err := collections.MigrateSavedLibraryQuery(in.Version, in.Payload, in.LegacyID)
		row := map[string]any{"input": in, "error": errorCode(err)}
		if err == nil {
			row["canonicalVersion"] = migrated.Version
			row["canonicalPayload"] = migrated.Payload
		}
		rows = append(rows, row)
	}
	jsonFile("saved-queries.json", rows)
	hashes := []string{"", "#/library?v=1", "#/library?v=2", "#/library?v=3", "#/library?v=1&includeBroken=true&bool=7%3Atrue&bool=8%3Afalse", "#/library?v=2&brokenVisibility=only&name=Moon", "#/library?v=3&bool=7%3Atrue", "#/library?v=3&brokenState=true", "#/library?v=3&name=%C3%89T%C3%89+%25_&page=2&size=12&sort=imported_desc", "#/library?v=3&v=3", "#/library?v=99", "#/library?v=3&unknown=1", "#/library?v=3&page=0", "#/library?v=3&choice=9%3A12%2C11"}
	// Include all existing shared codec cases, preserving their exact canonical hashes.
	var baseline []struct {
		Hash string `json:"hash"`
	}
	must(json.Unmarshal(bytesAt("internal/library/testdata/story_library_query_codec.json"), &baseline))
	for _, r := range baseline {
		hashes = append(hashes, r.Hash)
	}
	urls := []any{}
	for i, h := range hashes {
		q, err := library.DecodeStoryLibraryQuery(h, 7)
		row := map[string]any{"id": fmt.Sprintf("url-%02d", i), "input": h, "legacyBrokenId": 7, "error": errorCode(err)}
		if err == nil {
			row["query"] = q
			row["canonicalHash"] = take(library.EncodeStoryLibraryQuery(q))
		}
		urls = append(urls, row)
	}
	jsonFile("url-queries.json", urls)
}
func inspectionGolden() {
	fixtures := []testfixture.Archive{testfixture.PlainPK(), testfixture.V1PK(), testfixture.V2PK(), testfixture.GenericPK(), testfixture.GenericZIP(), testfixture.StudioZIP()}
	missing := testfixture.WithoutEntry(testfixture.PlainPK(), "ni")
	missing.Filename = "missing-ni.plain.pk"
	fixtures = append(fixtures, missing)
	disguised := testfixture.StudioZIP()
	disguised.Filename = "studio-disguised.pk"
	fixtures = append(fixtures, disguised)
	unsafe := testfixture.WithEntries(testfixture.GenericZIP(), testfixture.Entry{Name: "../escape", Bytes: []byte("unsafe")})
	unsafe.Filename = "traversal.zip"
	fixtures = append(fixtures, unsafe)
	duplicate := testfixture.WithEntries(testfixture.PlainPK(), testfixture.Entry{Name: "ni", Bytes: []byte("duplicate")})
	duplicate.Filename = "duplicate-path.plain.pk"
	fixtures = append(fixtures, duplicate)
	nested := testfixture.WithEntries(testfixture.PlainPK(), testfixture.Entry{Name: "nested.zip", Bytes: []byte("nested")})
	nested.Filename = "nested.plain.pk"
	fixtures = append(fixtures, nested)
	inspector := inspection.NewStoryInspector()
	rows := []any{}
	inspect := func(name string, body []byte) {
		write("archives/"+name, body)
		p := filepath.Join(output, "archives", name)
		r, err := inspector.Inspect(ctx, inspection.Candidate{Path: p, OriginalFilename: name}, inspection.DefaultLimits())
		row := map[string]any{"file": "archives/" + name, "sha256": hash(body), "bytes": len(body), "error": errorCode(err)}
		if err == nil {
			row["uuid"] = r.UUID
			row["format"] = r.Format
			row["title"] = r.Metadata.Title
			row["description"] = r.Metadata.Description
			if r.Metadata.Artwork != nil {
				row["artwork"] = map[string]any{"entry": r.Metadata.Artwork.EntryName, "mediaType": r.Metadata.Artwork.MediaType, "sha256": hash(r.Metadata.Artwork.Bytes)}
			}
		}
		if hash(bytesAt(p)) != hash(body) {
			panic("inspection mutated source")
		}
		rows = append(rows, row)
	}
	for _, f := range fixtures {
		inspect(f.Filename, take(testfixture.ZIPBytes(f)))
	}
	for _, name := range []string{"generic.7z", "studio.7z", "bomb.7z", "deep-path.7z", "missing.7z", "nested.7z", "symlink.7z"} {
		inspect(name, bytesAt(filepath.Join("internal/inspection/testfixture/testdata", name)))
	}
	inspect("invalid.zip", []byte("not an archive"))
	inspect("unsupported.txt", take(testfixture.ZIPBytes(testfixture.GenericZIP())))
	jsonFile("inspection.json", rows)
}

type world struct {
	dir         string
	layout      storage.Layout
	db          *database.Database
	query       *library.Query
	collections *collections.Service
	shelves     *shelves.Service
}

func newWorld() *world {
	dir := take(os.MkdirTemp("", "librairii-golden-world-"))
	layout := take(storage.Initialize(dir))
	db := take(database.Open(ctx, filepath.Join(layout.Database, "librairii.sqlite3")))
	q := library.NewQuery(db.SQL(), nil)
	return &world{dir, layout, db, q, take(collections.NewService(db.Writer())), take(shelves.NewService(db.Writer(), q))}
}
func (w *world) close() { must(w.db.Close()); must(os.RemoveAll(w.dir)) }
func importerFor(w *world) *importer.Service {
	return take(importer.NewService(archive.NewRepository(w.layout), artwork.NewRepository(w.layout), catalog.NewRepository(w.db.Writer()), inspection.NewStoryInspector(), inspection.DefaultLimits()))
}
func importGolden() {
	w := newWorld()
	defer w.close()
	service := importerFor(w)
	rows := []any{}
	for _, name := range []string{"clockwork-forest.plain.pk", "clockwork-forest.plain.pk", "clockwork-forest.zip", "invalid.zip"} {
		path := filepath.Join(output, "archives", name)
		before := hash(bytesAt(path))
		r, err := service.Import(ctx, path)
		code := ""
		var ie *importer.Error
		if errors.As(err, &ie) {
			code = string(ie.Code)
		} else {
			must(err)
		}
		rows = append(rows, map[string]any{"input": "archives/" + name, "result": r, "error": code})
		if before != hash(bytesAt(path)) {
			panic("import changed source")
		}
	}
	jsonFile("imports.json", rows)
}

var ids = []string{"123e4567-e89b-42d3-a456-426614174000", "223e4567-e89b-42d3-a456-426614174000", "323e4567-e89b-42d3-a456-426614174000", "423e4567-e89b-42d3-a456-426614174000", "523e4567-e89b-42d3-a456-426614174000", "623e4567-e89b-42d3-a456-426614174000"}

func syntheticStory(id string) []byte {
	f := testfixture.GenericZIP()
	old := strings.ReplaceAll(testfixture.StoryUUID, "-", "")
	newID := strings.ReplaceAll(id, "-", "")
	for j := range f.Entries {
		f.Entries[j].Name = strings.ReplaceAll(strings.ReplaceAll(f.Entries[j].Name, testfixture.StoryUUID, id), old, newID)
		if f.Entries[j].Name == "uuid.bin" {
			f.Entries[j].Bytes = testfixture.UUIDBytes(id)
		}
	}
	return take(testfixture.ZIPBytes(f))
}
func seedWorld(w *world) {
	titles := []string{"Old ocean", "Moon still", "Manual ocean", "Moon forest", "Moon excluded"}
	repo := catalog.NewRepository(w.db.Writer())
	for i, title := range titles {
		body := syntheticStory(ids[i])
		sum := hash(body)
		rel := "archives/" + sum + ".zip"
		must(os.WriteFile(filepath.Join(w.dir, rel), body, 0600))
		inspected := take(inspection.NewStoryInspector().Inspect(ctx, inspection.Candidate{Path: filepath.Join(w.dir, rel), OriginalFilename: fmt.Sprintf("story-%d.zip", i+1)}, inspection.DefaultLimits()))
		if inspected.UUID != ids[i] {
			panic("synthetic archive UUID mismatch")
		}
		_, _, err := repo.Create(ctx, catalog.CreateStory{UUID: ids[i], EmbeddedTitle: title, OriginalFilename: fmt.Sprintf("story-%d.zip", i+1), DetectedFormat: catalog.FormatZIP, SHA256: sum, ByteSize: int64(len(body)), ManagedPath: rel})
		must(err)
	}
	_, err := library.NewBrokenFlags(w.db.Writer()).Set(ctx, []int64{4}, true)
	must(err)
}
func membershipGolden() {
	w := newWorld()
	defer w.close()
	seedWorld(w)
	jsonFile("membership-scenario.json", map[string]any{"storyUUIDs": ids, "titles": []string{"Old ocean", "Moon still", "Manual ocean", "Moon forest", "Moon excluded", "Unavailable"}, "brokenStoryIds": []int{4}, "collections": []any{map[string]any{"id": 1, "name": "Moon", "query": map[string]any{"name": "moon"}}, map[string]any{"id": 2, "name": "Forest", "query": map[string]any{"name": "forest"}}}, "initialMembers": []string{ids[0], ids[1], ids[2], ids[5]}, "sourceDerived": []string{ids[0], ids[1]}, "manualRetention": []string{ids[2]}, "excluded": []string{ids[4]}, "steps": []string{"review-complete (read only)", "flag story1 without changing review eligibility", "explicit Keep story1 + Add story4", "replay consumed ticket must fail", "ordinary Remove story4 without exclusion", "explicit Exclude story4", "unexclude story4 without membership insertion", "mark Forest source needs_attention; additions allowed, removals suspended", "restore healthy source; export with existing story-2.zip destination conflict"}})
	moon := take(w.collections.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"}))
	forest := take(w.collections.Create(ctx, "Forest", library.StoryLibraryQuery{Name: "forest"}))
	resolver := take(exporter.NewResolver(w.query, w.collections, w.shelves))
	union := take(resolver.ResolveCollections(ctx, []int64{moon.ID, forest.ID}))
	jsonFile("collection-union.json", canonical(map[string]any{"sources": []int64{moon.ID, forest.ID}, "scope": union}))
	shelf := take(w.shelves.CreateManual(ctx, "Approved", []string{ids[0], ids[1], ids[2]}))
	take(w.shelves.AttachSources(ctx, shelf.ID, []int64{moon.ID, forest.ID}))
	take(w.shelves.SetProtected(ctx, shelf.ID, ids[0], false))
	take(w.shelves.SetProtected(ctx, shelf.ID, ids[1], false))
	_, err := w.db.Writer().Exec(`INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,3,0,'Unavailable')`, shelf.ID, ids[5])
	must(err)
	_, err = w.db.Writer().Exec(`INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)`, shelf.ID, ids[4])
	must(err)
	jsonFile("shelf-membership.json", canonical(take(w.shelves.Open(ctx, shelf.ID, library.StoryLibraryQuery{}))))
	review := take(w.shelves.Preview(ctx, shelf.ID))
	jsonFile("review-complete.json", canonical(review))
	// Flag-only changes do not invalidate the preview; all approved decisions remain explicit.
	take(library.NewBrokenFlags(w.db.Writer()).Set(ctx, []int64{1}, true))
	after := take(w.shelves.Apply(ctx, shelf.ID, review.Token, []shelves.Decision{{UUID: ids[0], Action: "keep"}, {UUID: ids[3], Action: "add"}}))
	jsonFile("review-apply.json", canonical(map[string]any{"decisions": []shelves.Decision{{UUID: ids[0], Action: "keep"}, {UUID: ids[3], Action: "add"}}, "shelf": after, "members": take(w.shelves.Members(ctx, shelf.ID))}))
	_, err = w.shelves.Apply(ctx, shelf.ID, review.Token, []shelves.Decision{{UUID: ids[3], Action: "add"}})
	jsonFile("review-replay.json", map[string]any{"error": errorCode(err)})
	take(w.shelves.RemoveStories(ctx, shelf.ID, []string{ids[3]}))
	jsonFile("ordinary-removal.json", canonical(map[string]any{"members": take(w.shelves.Members(ctx, shelf.ID)), "exclusions": take(w.shelves.Exclusions(ctx, shelf.ID)), "review": take(w.shelves.Preview(ctx, shelf.ID))}))
	fresh := take(w.shelves.Preview(ctx, shelf.ID))
	take(w.shelves.Apply(ctx, shelf.ID, fresh.Token, []shelves.Decision{{UUID: ids[3], Action: "exclude"}}))
	jsonFile("explicit-exclusion.json", canonical(map[string]any{"members": take(w.shelves.Members(ctx, shelf.ID)), "exclusions": take(w.shelves.Exclusions(ctx, shelf.ID)), "review": take(w.shelves.Preview(ctx, shelf.ID))}))
	take(w.shelves.ClearExclusion(ctx, shelf.ID, ids[3]))
	jsonFile("unexclude.json", canonical(map[string]any{"members": take(w.shelves.Members(ctx, shelf.ID)), "review": take(w.shelves.Preview(ctx, shelf.ID))}))
	_, err = w.db.Writer().Exec(`UPDATE collections SET validity_state='needs_attention' WHERE id=?`, forest.ID)
	must(err)
	jsonFile("review-incomplete.json", canonical(take(w.shelves.Preview(ctx, shelf.ID))))
	duplicate := take(w.shelves.Duplicate(ctx, shelf.ID, "Independent copy"))
	jsonFile("shelf-duplicate.json", canonical(take(w.shelves.Open(ctx, duplicate.ID, library.StoryLibraryQuery{}))))
	plans := take(exporter.NewPreflightService(resolver, w.layout))
	dest := filepath.Join(w.dir, "export")
	must(os.Mkdir(dest, 0700))
	must(os.WriteFile(filepath.Join(dest, "story-2.zip"), []byte("existing destination"), 0600))
	requests := []exporter.PreflightRequest{{SourceType: operations.ExportSourceSelection, StoryIDs: []int64{1, 2, 4}}, {SourceType: operations.ExportSourceCurrentQuery, Query: library.StoryLibraryQuery{Name: "moon"}}, {SourceType: operations.ExportSourceCollection, CollectionIDs: []int64{moon.ID}}, {SourceType: operations.ExportSourceCollections, CollectionIDs: []int64{moon.ID, forest.ID}}, {SourceType: operations.ExportSourceCuratedShelf, ShelfIDs: []int64{shelf.ID}}, {SourceType: operations.ExportSourceCuratedShelves, ShelfIDs: []int64{shelf.ID, duplicate.ID}}, {SourceType: operations.ExportSourceCurrentQuery, QueryShelfID: shelf.ID, Query: library.StoryLibraryQuery{Name: "ocean"}}, {SourceType: operations.ExportSourceSelection, QueryShelfID: shelf.ID, ShelfSelectionUUIDs: []string{ids[5]}}, {SourceType: operations.ExportSourceSelection, StoryIDs: []int64{1, 4}}}
	// Restore healthy source for union planning, while preserving the incomplete-review golden above.
	_, err = w.db.Writer().Exec(`UPDATE collections SET validity_state='valid' WHERE id=?`, forest.ID)
	must(err)
	exported := []any{}
	for _, request := range requests {
		report := take(plans.Plan(ctx, request, dest))
		report.DestinationLabel = "<export-destination>"
		exported = append(exported, canonical(map[string]any{"request": request, "report": report, "operationItems": report.OperationItems()}))
	}
	jsonFile("export-plans.json", exported)
}
func main() {
	flag.StringVar(&output, "out", "", "new output directory")
	flag.Parse()
	if output == "" || !filepath.IsAbs(output) {
		panic("explicit absolute output required")
	}
	if _, err := os.Stat(output); !errors.Is(err, os.ErrNotExist) {
		panic("output exists or cannot be inspected")
	}
	must(os.MkdirAll(output, 0700))
	unicodeGolden()
	queryGolden()
	inspectionGolden()
	importGolden()
	membershipGolden()
	reviewEdgesGolden()
	extraFixtures()
}

func reviewEdgesGolden() {
	w := newWorld()
	defer w.close()
	seedWorld(w)
	c := take(w.collections.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"}))
	request := shelves.CreationRequest{Name: "Seeded", ManualUUIDs: []string{ids[0]}, CollectionIDs: []int64{c.ID}}
	preview := take(w.shelves.PreviewCreation(ctx, request))
	if len(take(w.shelves.List(ctx))) != 0 {
		panic("creation preview mutated shelves")
	}
	jsonFile("creation-preview.json", canonical(map[string]any{"request": request, "review": preview, "shelvesAfterPreview": take(w.shelves.List(ctx))}))
	sh := take(w.shelves.CreateFromReview(ctx, preview.Token, []string{ids[0]}))
	jsonFile("creation-apply.json", canonical(take(w.shelves.Open(ctx, sh.ID, library.StoryLibraryQuery{}))))
	stale := take(w.shelves.Preview(ctx, sh.ID))
	take(w.shelves.AddStories(ctx, sh.ID, []string{ids[1]}))
	_, err := w.shelves.Apply(ctx, sh.ID, stale.Token, []shelves.Decision{{UUID: ids[3], Action: "add"}})
	jsonFile("review-stale.json", map[string]any{"error": errorCode(err)})
	fresh := take(w.shelves.Preview(ctx, sh.ID))
	before := take(w.shelves.Get(ctx, sh.ID))
	_, err = w.shelves.Apply(ctx, sh.ID, fresh.Token, []shelves.Decision{{UUID: ids[5], Action: "add"}})
	after := take(w.shelves.Get(ctx, sh.ID))
	if before != after {
		panic("invalid review partially wrote")
	}
	jsonFile("review-invalid-decision.json", map[string]any{"uuid": ids[5], "action": "add", "error": errorCode(err), "before": before, "after": after})
	deferred := take(w.shelves.Apply(ctx, sh.ID, fresh.Token, []shelves.Decision{{UUID: ids[3], Action: "defer"}}))
	if before != deferred {
		panic("defer changed shelf")
	}
	jsonFile("review-deferred.json", map[string]any{"shelf": deferred, "unchanged": true})
}
