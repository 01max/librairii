package metadata

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"
)

func TestLibraryProviderMatchesOnlyActiveLocaleByCompleteUUID(t *testing.T) {
	t.Parallel()

	repository, _ := openMetadataRepository(t)
	ctx := context.Background()
	english := stageProviderSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174400",
		"en-GB",
		"English title",
		"a",
	)
	if err := repository.ActivateSnapshot(ctx, english.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	french := stageProviderSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174401",
		"fr-FR",
		"Titre français",
		"b",
	)
	if err := repository.ActivateSnapshot(ctx, french.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	staged := stageProviderSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174402",
		"en-GB",
		"Not active",
		"c",
	)

	provider, err := NewLibraryProvider(repository, "en_GB")
	if err != nil {
		t.Fatal(err)
	}
	uppercase := "123E4567-E89B-42D3-A456-426614174000"
	found, err := provider.FindByUUIDs(ctx, []string{
		uppercase,
		"123e4567-e89b-42d3-a456-426614174099",
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(found) != 1 ||
		found[uppercase].Title != "English title" ||
		found[uppercase].Locale != "en-GB" ||
		found[uppercase].UUID != "123e4567-e89b-42d3-a456-426614174000" ||
		found[uppercase].Provenance != ProvenanceLuniiCatalog ||
		found[uppercase].ActivatedAt == "" {
		t.Fatalf("FindByUUIDs() = %#v", found)
	}
	if found[uppercase].Title == "Titre français" ||
		found[uppercase].Title == "Not active" {
		t.Fatalf("FindByUUIDs() crossed locale or activation boundary: %#v", found)
	}
	if staged.Status != SnapshotStaged {
		t.Fatalf("staged snapshot = %#v", staged)
	}
}

func TestLibraryProviderRejectsNonCompleteUUIDLookup(t *testing.T) {
	t.Parallel()

	repository, _ := openMetadataRepository(t)
	provider, err := NewLibraryProvider(repository, DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	for _, invalid := range []string{
		"123e4567e89b42d3a456426614174000",
		"not-a-uuid",
		"",
	} {
		if _, err := provider.FindByUUIDs(
			context.Background(),
			[]string{invalid},
		); !errors.Is(err, ErrInvalidMetadataLookup) {
			t.Fatalf("FindByUUIDs(%q) error = %v", invalid, err)
		}
	}
}

func stageProviderSnapshot(
	t *testing.T,
	repository *Repository,
	syncID string,
	locale string,
	title string,
	hashCharacter string,
) CatalogSnapshot {
	t.Helper()

	ctx := context.Background()
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    locale,
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    locale,
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat(hashCharacter, 64),
		ByteSize:  128,
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID:      "123e4567-e89b-42d3-a456-426614174000",
			Title:          title,
			Author:         "Official Author",
			Publisher:      "Fixture Press",
			Language:       locale,
			SourceRecordID: "fixture-record",
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
}
