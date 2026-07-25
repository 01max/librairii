package performancefixture

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/storage"
)

func TestGenerateBuildsCopyrightFreeFiveThousandStoryFixture(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = opened.Close()
	})
	fixture, err := Generate(
		context.Background(),
		opened.SQL(),
		layout,
		MinimumLargeLibraryStories,
	)
	if err != nil {
		t.Fatal(err)
	}
	if fixture.StoryCount != MinimumLargeLibraryStories ||
		len(fixture.ShelfIDs) != 6 ||
		fixture.EmbeddedArtworkPath == "" {
		t.Fatalf("Generate() = %#v", fixture)
	}
	var storyCount int
	var officialCount int
	var shelfCount int
	var nonSyntheticCount int
	if err := opened.SQL().QueryRow(
		"SELECT COUNT(*) FROM stories",
	).Scan(&storyCount); err != nil {
		t.Fatal(err)
	}
	if err := opened.SQL().QueryRow(
		"SELECT COUNT(*) FROM official_story_metadata",
	).Scan(&officialCount); err != nil {
		t.Fatal(err)
	}
	if err := opened.SQL().QueryRow(
		"SELECT COUNT(*) FROM shelves",
	).Scan(&shelfCount); err != nil {
		t.Fatal(err)
	}
	if err := opened.SQL().QueryRow(
		`SELECT COUNT(*)
		 FROM stories
		 WHERE embedded_title NOT LIKE 'Synthetic %'
		    OR embedded_description NOT LIKE 'Copyright-free synthetic %'`,
	).Scan(&nonSyntheticCount); err != nil {
		t.Fatal(err)
	}
	if storyCount != MinimumLargeLibraryStories ||
		officialCount != MinimumLargeLibraryStories ||
		shelfCount != len(fixture.ShelfIDs) ||
		nonSyntheticCount != 0 {
		t.Fatalf(
			"counts = stories:%d official:%d shelves:%d nonSynthetic:%d",
			storyCount,
			officialCount,
			shelfCount,
			nonSyntheticCount,
		)
	}
	if !strings.HasPrefix(
		fixture.EmbeddedArtworkPath,
		"catalog/embedded/",
	) {
		t.Fatalf("embedded artwork path = %q", fixture.EmbeddedArtworkPath)
	}
	plans, err := QueryPlans(context.Background(), opened.SQL())
	if err != nil {
		t.Fatal(err)
	}
	combinedPlan := strings.Join(plans["combinedFilters"], "\n")
	for _, index := range []string{
		"idx_story_archives_validation_story",
		"idx_official_metadata_language_story",
		"idx_story_tag_assignments_filter",
	} {
		if !strings.Contains(combinedPlan, index) {
			t.Fatalf("combined plan omitted %s:\n%s", index, combinedPlan)
		}
	}
	if got := strings.Join(plans["substringSearch"], "\n"); !strings.Contains(
		got,
		"idx_stories_display_name_normalized",
	) {
		t.Fatalf("substring plan omitted normalized-name index:\n%s", got)
	}
	if got := strings.Join(plans["artworkLoad"], "\n"); !strings.Contains(
		got,
		"INTEGER PRIMARY KEY",
	) {
		t.Fatalf("artwork plan omitted primary-key lookup:\n%s", got)
	}
}
