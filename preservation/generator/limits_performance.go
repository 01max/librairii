package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/performancefixture"
	"github.com/01max/librairii/internal/shelves"
)

func limitFixtures() {
	defaults := inspection.DefaultLimits()
	rows := []any{}
	account := func(id string, entries []inspection.EntryInfo, repeat int) {
		budget := take(inspection.NewBudget(defaults))
		var err error
		for n := 0; n < repeat && err == nil; n++ {
			for _, e := range entries {
				err = budget.Account(e)
				if err != nil {
					break
				}
			}
		}
		rows = append(rows, map[string]any{"id": id, "limits": defaults, "entries": entries, "repeat": repeat, "error": errorCode(err)})
	}
	for _, n := range []int{511, 512, 513} {
		account(fmt.Sprintf("path-bytes-%d", n), []inspection.EntryInfo{{Name: strings.Repeat("a", n), CompressedBytes: 1, UncompressedBytes: 1}}, 1)
	}
	for _, n := range []int{15, 16, 17} {
		account(fmt.Sprintf("path-depth-%d", n), []inspection.EntryInfo{{Name: strings.Repeat("a/", n-1) + "z", CompressedBytes: 1, UncompressedBytes: 1}}, 1)
	}
	for _, n := range []int{9999, 10000, 10001} {
		account(fmt.Sprintf("entry-count-%d", n), []inspection.EntryInfo{{Name: "entry", CompressedBytes: 1, UncompressedBytes: 1}}, n)
	}
	for _, delta := range []int64{-1, 0, 1} {
		n := defaults.MaxExpandedBytes + delta
		account(fmt.Sprintf("expanded-bytes-%d", n), []inspection.EntryInfo{{Name: "payload", CompressedBytes: n, UncompressedBytes: n}}, 1)
	}
	for _, ratio := range []int64{199, 200, 201} {
		account(fmt.Sprintf("aggregate-ratio-%d", ratio), []inspection.EntryInfo{{Name: "payload", CompressedBytes: 100, UncompressedBytes: 100 * ratio}}, 1)
	}
	for _, p := range []string{"../escape", "/absolute", "C:/drive", "a\\b", "a\x00b", "a/../safe"} {
		account("path-"+fmt.Sprintf("%x", []byte(p)), []inspection.EntryInfo{{Name: p, CompressedBytes: 1, UncompressedBytes: 1}}, 1)
	}
	jsonFile("limits/archive-headers.json", rows)
	reads := []any{}
	for _, kind := range []struct {
		name  string
		limit int64
		code  inspection.ErrorCode
	}{{"metadata", defaults.MaxMetadataBytes, inspection.CodeMetadataLimit}, {"artwork", defaults.MaxArtworkBytes, inspection.CodeArtworkLimit}} {
		for _, delta := range []int64{-1, 0, 1} {
			n := kind.limit + delta
			result, err := inspection.ReadLimited(strings.NewReader(strings.Repeat("x", int(n))), kind.limit, kind.code, "fixture")
			r := map[string]any{"kind": kind.name, "byteRecipe": "ASCII x repeated inputBytes times", "inputBytes": n, "limit": kind.limit, "error": errorCode(err)}
			if err == nil {
				r["readBytes"] = len(result)
				r["sha256"] = hash(result)
			}
			reads = append(reads, r)
		}
	}
	jsonFile("limits/bounded-reads.json", reads)
	queries := []any{}
	for _, n := range []int{199, 200, 201} {
		q := library.StoryLibraryQuery{Name: strings.Repeat("é", n)}
		encoded, err := library.EncodeStoryLibraryQuery(q)
		queries = append(queries, map[string]any{"id": fmt.Sprintf("search-runes-%d", n), "query": q, "canonicalHash": encoded, "error": queryError(err)})
	}
	for _, n := range []int{99, 100, 101} {
		q := library.StoryLibraryQuery{PageSize: n}
		encoded, err := library.EncodeStoryLibraryQuery(q)
		queries = append(queries, map[string]any{"id": fmt.Sprintf("page-size-%d", n), "query": q, "canonicalHash": encoded, "error": queryError(err)})
	}
	for _, n := range []int{49, 50, 51} {
		q := library.StoryLibraryQuery{}
		for i := 1; i <= n; i++ {
			q.BooleanFilters = append(q.BooleanFilters, library.BooleanFilter{DefinitionID: int64(i), State: library.BooleanTrue})
		}
		encoded, err := library.EncodeStoryLibraryQuery(q)
		queries = append(queries, map[string]any{"id": fmt.Sprintf("filter-groups-%d", n), "query": q, "canonicalHash": encoded, "error": queryError(err)})
	}
	jsonFile("limits/queries.json", queries)
}
func queryError(err error) string {
	if err != nil {
		if !errors.Is(err, library.ErrInvalidStoryLibraryQuery) {
			panic(err)
		}
		return "invalid_story_query"
	}
	return ""
}
func performanceFixtures() {
	w := newWorld()
	defer w.close()
	fixture := take(performancefixture.Generate(ctx, w.db.Writer(), w.layout, 5000))
	stories := rowsJSON(w.db.SQL(), `SELECT s.id,s.uuid,s.embedded_title,s.display_name_normalized,s.created_at,s.is_broken,a.validation_state,a.byte_size,s.embedded_artwork_path,m.title AS official_title,m.language,m.minimum_age,m.maximum_age FROM stories s JOIN story_archives a ON a.story_id=s.id LEFT JOIN official_story_metadata m ON m.story_uuid=s.uuid ORDER BY s.id`)
	var jsonl strings.Builder
	for _, r := range stories {
		jsonl.Write(take(json.Marshal(r)))
		jsonl.WriteByte('\n')
	}
	write("performance/stories.jsonl", []byte(jsonl.String()))
	for _, rel := range fixture.EmbeddedArtworkPaths {
		write("performance/"+rel, bytesAt(w.dir+"/"+rel))
	}
	summary := map[string]any{"storyCount": fixture.StoryCount, "collectionIds": fixture.CollectionIDs, "titleCycle": 64, "artworkVariants": 24, "artworkWidth": 320, "artworkHeight": 400, "archivePayloads": "record-only synthetic performance data, not usable export archives", "validationDistribution": rowsJSON(w.db.SQL(), `SELECT validation_state,count(*) AS count FROM story_archives GROUP BY validation_state ORDER BY validation_state`), "languageDistribution": rowsJSON(w.db.SQL(), `SELECT language,count(*) AS count FROM official_story_metadata GROUP BY language ORDER BY language`), "tagDistribution": rowsJSON(w.db.SQL(), `SELECT definition_id,value_id,count(*) AS count FROM story_tag_assignments GROUP BY definition_id,value_id ORDER BY definition_id,value_id`), "queryPlans": take(performancefixture.QueryPlans(ctx, w.db.SQL()))}
	jsonFile("performance/distribution.json", summary)
	reviewPerformanceFixture()
}
func reviewPerformanceFixture() {
	w := newWorld()
	defer w.close()
	tx := take(w.db.Writer().BeginTx(ctx, nil))
	for index := 0; index < 5000; index++ {
		id := fmt.Sprintf("123e4567-e89b-42d3-a456-%012d", index)
		title := fmt.Sprintf("Story %05d shared %s", index, []string{"moon", "forest"}[index%2])
		_, err := tx.Exec(`INSERT INTO stories(id,uuid,embedded_title,display_name_normalized,is_broken) VALUES(?,?,?,?,?)`, index+1, id, title, title, index%5 == 0)
		must(err)
		_, err = tx.Exec(`INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path) VALUES(?,?,'zip',?,1,?)`, index+1, id+".zip", fmt.Sprintf("%064d", index), "archives/"+id+".zip")
		must(err)
	}
	must(tx.Commit())
	all := take(w.collections.Create(ctx, "All shared", library.StoryLibraryQuery{Name: "shared"}))
	moon := take(w.collections.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"}))
	shelf := take(w.shelves.Create(ctx, "Review"))
	take(w.shelves.AttachSources(ctx, shelf.ID, []int64{all.ID, moon.ID}))
	review := take(w.shelves.Preview(ctx, shelf.ID))
	candidates := []any{}
	for _, c := range review.Additions {
		sources := []int64{}
		for _, src := range c.Sources {
			sources = append(sources, src.CollectionID)
		}
		candidates = append(candidates, map[string]any{"uuid": c.UUID, "title": c.Title, "broken": c.Story.Broken, "sourceCollectionIds": sources})
	}
	decisions := make([]shelves.Decision, 50)
	for i := range decisions {
		decisions[i] = shelves.Decision{UUID: review.Additions[i].UUID, Action: "add"}
	}
	take(w.shelves.Apply(ctx, shelf.ID, review.Token, decisions))
	after := take(w.shelves.Preview(ctx, shelf.ID))
	jsonFile("performance/review5000.json", map[string]any{"storyCount": 5000, "brokenCount": 1000, "sourceCount": 2, "initialAdditions": candidates, "decisions": decisions, "remainingAdditions": len(after.Additions), "approved": take(w.shelves.Get(ctx, shelf.ID)), "recipe": "index0..4999; UUID123e4567-e89b-42d3-a456-<12 decimal digits>; Story %05d shared moon/forest by index parity; Broken when index%5==0; record-only archive SHA is index formatted as64 decimal digits"})
}
