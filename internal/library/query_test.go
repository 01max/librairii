package library

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/database"
)

func TestListResolvesOfficialEmbeddedAndFallbackDisplay(t *testing.T) {
	t.Parallel()

	query, repository, _ := newLibraryQuery(t, fixedOfficialProvider{
		"00112233-4455-4677-8899-aabbccddeeff": {
			UUID:        "00112233-4455-4677-8899-aabbccddeeff",
			Locale:      "en-GB",
			Title:       "Official Clockwork Forest",
			Description: "Official description",
			Author:      "Lunii",
			Publisher:   "Fixture Press",
			Language:    "en-GB",
			ArtworkID:   "official:clockwork",
			Provenance:  "lunii_catalog",
			FetchedAt:   "2026-07-25T10:00:00Z",
			ActivatedAt: "2026-07-25T10:00:01Z",
		},
	})
	createStory(t, repository, catalog.CreateStory{
		UUID:                "00112233-4455-4677-8899-aabbccddeeff",
		EmbeddedTitle:       "Embedded title",
		EmbeddedDescription: "Embedded description",
		EmbeddedArtworkPath: "catalog/embedded/clockwork.png",
		OriginalFilename:    "clockwork.zip",
		DetectedFormat:      catalog.FormatZIP,
		SHA256:              strings.Repeat("a", 64),
		ByteSize:            10,
		ManagedPath:         "archives/a/clockwork.zip",
	})
	createStory(t, repository, catalog.CreateStory{
		UUID:                "11112222-3333-4444-8555-666677778888",
		EmbeddedTitle:       "Embedded Adventure",
		EmbeddedDescription: "Embedded only",
		EmbeddedArtworkPath: "catalog/embedded/adventure.png",
		OriginalFilename:    "adventure.7z",
		DetectedFormat:      catalog.FormatSevenZIP,
		SHA256:              strings.Repeat("b", 64),
		ByteSize:            20,
		ManagedPath:         "archives/b/adventure.7z",
	})
	createStory(t, repository, catalog.CreateStory{
		UUID:             "22223333-4444-4555-8666-777788889999",
		OriginalFilename: "fallback.pk",
		DetectedFormat:   catalog.FormatGenericPK,
		SHA256:           strings.Repeat("c", 64),
		ByteSize:         30,
		ManagedPath:      "archives/c/fallback.pk",
	})

	page, err := query.List(context.Background(), ListRequest{})
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if page.Page != 1 ||
		page.PageSize != DefaultPageSize ||
		page.Sort != SortNameAscending ||
		page.TotalItems != 3 ||
		len(page.Stories) != 3 {
		t.Fatalf("List() = %#v", page)
	}
	byUUID := map[string]StorySummary{}
	for _, story := range page.Stories {
		byUUID[story.UUID] = story
	}
	official := byUUID["00112233-4455-4677-8899-aabbccddeeff"]
	if official.Title != "Official Clockwork Forest" ||
		official.Description != "Official description" ||
		official.Author != "Lunii" ||
		official.ArtworkID != "official:clockwork" ||
		official.Sources != (DisplaySources{
			Title:       SourceOfficial,
			Description: SourceOfficial,
			Author:      SourceOfficial,
			Artwork:     SourceOfficial,
		}) ||
		official.Official == nil ||
		official.Official.Locale != "en-GB" ||
		official.Official.Publisher != "Fixture Press" ||
		official.Official.Provenance != "lunii_catalog" ||
		official.Official.FetchedAt != "2026-07-25T10:00:00Z" {
		t.Fatalf("official summary = %#v", official)
	}
	embedded := byUUID["11112222-3333-4444-8555-666677778888"]
	if embedded.Title != "Embedded Adventure" ||
		embedded.ArtworkID == "catalog/embedded/adventure.png" ||
		embedded.ArtworkID != "embedded:2" ||
		embedded.Sources.Title != SourceEmbedded {
		t.Fatalf("embedded summary = %#v", embedded)
	}
	fallback := byUUID["22223333-4444-4555-8666-777788889999"]
	if fallback.Title != "Story "+fallback.UUID ||
		fallback.Description != "" ||
		fallback.ArtworkID != "" ||
		fallback.Sources.Title != SourceFallback {
		t.Fatalf("fallback summary = %#v", fallback)
	}
}

func TestListResolvesPrecedenceIndependentlyForEveryDisplayField(t *testing.T) {
	t.Parallel()

	query, repository, _ := newLibraryQuery(t, fixedOfficialProvider{
		"00112233-4455-4677-8899-aabbccddeeff": {
			UUID:        "00112233-4455-4677-8899-aabbccddeeff",
			Locale:      "en-GB",
			Author:      "Official Author",
			Provenance:  "lunii_catalog",
			FetchedAt:   "2026-07-25T10:00:00Z",
			ActivatedAt: "2026-07-25T10:00:01Z",
		},
	})
	createStory(t, repository, catalog.CreateStory{
		UUID:                "00112233-4455-4677-8899-aabbccddeeff",
		EmbeddedTitle:       "Embedded title",
		EmbeddedDescription: "Embedded description",
		EmbeddedArtworkPath: "catalog/embedded/clockwork.png",
		OriginalFilename:    "clockwork.zip",
		DetectedFormat:      catalog.FormatZIP,
		SHA256:              strings.Repeat("f", 64),
		ManagedPath:         "archives/f/clockwork.zip",
	})

	page, err := query.List(context.Background(), ListRequest{})
	if err != nil {
		t.Fatal(err)
	}
	story := page.Stories[0]
	if story.Title != "Embedded title" ||
		story.Description != "Embedded description" ||
		story.Author != "Official Author" ||
		story.ArtworkID != "embedded:1" ||
		story.Sources.Title != SourceEmbedded ||
		story.Sources.Description != SourceEmbedded ||
		story.Sources.Author != SourceOfficial ||
		story.Sources.Artwork != SourceEmbedded ||
		story.Official == nil {
		t.Fatalf("resolved story = %#v", story)
	}
}

func TestListPaginatesAndSortsDeterministically(t *testing.T) {
	t.Parallel()

	query, repository, _ := newLibraryQuery(t, nil)
	for index, input := range []struct {
		uuid  string
		title string
		hash  string
	}{
		{uuid: "aaaaaaaa-1111-4111-8111-111111111111", title: "beta", hash: strings.Repeat("a", 64)},
		{uuid: "bbbbbbbb-2222-4222-8222-222222222222", title: "Alpha", hash: strings.Repeat("b", 64)},
		{uuid: "cccccccc-3333-4333-8333-333333333333", title: "alpha", hash: strings.Repeat("c", 64)},
	} {
		createStory(t, repository, catalog.CreateStory{
			UUID:             input.uuid,
			EmbeddedTitle:    input.title,
			OriginalFilename: input.title + ".zip",
			DetectedFormat:   catalog.FormatZIP,
			SHA256:           input.hash,
			ByteSize:         int64(index),
			ManagedPath:      "archives/" + input.hash[:1] + "/" + input.title + ".zip",
		})
	}

	first, err := query.List(context.Background(), ListRequest{Page: 1, PageSize: 2})
	if err != nil {
		t.Fatal(err)
	}
	second, err := query.List(context.Background(), ListRequest{Page: 2, PageSize: 2})
	if err != nil {
		t.Fatal(err)
	}
	if first.TotalPages != 2 ||
		len(first.Stories) != 2 ||
		first.Stories[0].UUID != "bbbbbbbb-2222-4222-8222-222222222222" ||
		first.Stories[1].UUID != "cccccccc-3333-4333-8333-333333333333" ||
		len(second.Stories) != 1 ||
		second.Stories[0].Title != "beta" {
		t.Fatalf("pages = %#v, %#v", first, second)
	}

	newest, err := query.List(context.Background(), ListRequest{
		Page: 1, PageSize: 3, Sort: SortImportedNewest,
	})
	if err != nil {
		t.Fatal(err)
	}
	if newest.Stories[0].ID <= newest.Stories[1].ID {
		t.Fatalf("newest sort = %#v", newest.Stories)
	}
}

func TestListRejectsUnboundedRequests(t *testing.T) {
	t.Parallel()

	query, _, _ := newLibraryQuery(t, nil)
	for _, request := range []ListRequest{
		{Page: -1},
		{PageSize: MaxPageSize + 1},
		{Sort: "unknown"},
	} {
		if _, err := query.List(context.Background(), request); !errors.Is(err, ErrInvalidListRequest) {
			t.Fatalf("List(%#v) error = %v", request, err)
		}
	}
}

func TestDetailIncludesArchiveVerificationWithoutManagedPath(t *testing.T) {
	t.Parallel()

	query, repository, sqlDatabase := newLibraryQuery(t, nil)
	story, _, err := repository.Create(context.Background(), catalog.CreateStory{
		UUID:             "00112233-4455-4677-8899-aabbccddeeff",
		EmbeddedTitle:    "Clockwork",
		OriginalFilename: "clockwork.v2.pk",
		DetectedFormat:   catalog.FormatV2PK,
		SHA256:           strings.Repeat("d", 64),
		ByteSize:         42,
		ManagedPath:      "archives/private/clockwork.v2.pk",
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := sqlDatabase.Exec(
		"UPDATE story_archives SET validation_state = 'missing' WHERE story_id = ?",
		story.ID,
	); err != nil {
		t.Fatal(err)
	}

	detail, err := query.Detail(context.Background(), story.ID)
	if err != nil {
		t.Fatalf("Detail() error = %v", err)
	}
	if detail.Archive.OriginalFilename != "clockwork.v2.pk" ||
		detail.Archive.DetectedFormat != string(catalog.FormatV2PK) ||
		detail.Archive.SHA256 != strings.Repeat("d", 64) ||
		detail.Archive.Verification != CompatibilityMissing ||
		detail.Story.Compatibility != CompatibilityMissing {
		t.Fatalf("Detail() = %#v", detail)
	}
	if strings.Contains(fmt.Sprintf("%#v", detail), "archives/private") {
		t.Fatalf("Detail() exposed managed path: %#v", detail)
	}
	if _, err := query.Detail(context.Background(), 0); !errors.Is(err, ErrInvalidListRequest) {
		t.Fatalf("Detail(0) error = %v", err)
	}
	if _, err := query.Detail(context.Background(), story.ID+100); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("Detail(missing) error = %v", err)
	}
}

type fixedOfficialProvider map[string]OfficialMetadata

func (p fixedOfficialProvider) FindByUUIDs(
	_ context.Context,
	uuids []string,
) (map[string]OfficialMetadata, error) {
	result := map[string]OfficialMetadata{}
	for _, uuid := range uuids {
		if metadata, found := p[uuid]; found {
			result[uuid] = metadata
		}
	}
	return result, nil
}

func newLibraryQuery(
	t *testing.T,
	official OfficialProvider,
) (*Query, *catalog.Repository, *sql.DB) {
	t.Helper()

	sqlDatabase, err := database.Open(
		context.Background(),
		filepath.Join(t.TempDir(), "library.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = sqlDatabase.Close()
	})
	return NewQuery(sqlDatabase.SQL(), official),
		catalog.NewRepository(sqlDatabase.SQL()),
		sqlDatabase.SQL()
}

func createStory(t *testing.T, repository *catalog.Repository, input catalog.CreateStory) {
	t.Helper()
	if _, _, err := repository.Create(context.Background(), input); err != nil {
		t.Fatal(err)
	}
}
