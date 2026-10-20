package tagging

import (
	"context"
	"errors"
	"slices"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestDefinitionDeletionPlansAndInvalidatesEveryReferencingShelf(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	connection := openTaggingDatabase(t)
	tags, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	shelfService, err := collections.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	repository, err := collections.NewRepository(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, tags, "mood", KindChoice)
	if _, err := connection.Exec(`
		INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Keep','keep',0);
		INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected)
		VALUES(1,'123e4567-e89b-42d3-a456-426614174000',0,1);
		INSERT INTO shelf_exclusions(shelf_id,story_uuid)
		VALUES(1,'223e4567-e89b-42d3-a456-426614174000');`); err != nil {
		t.Fatal(err)
	}
	calm := createChoiceValue(t, tags, definition.ID, "calm")
	dreamy := createChoiceValue(t, tags, definition.ID, "dreamy")

	calmShelf := createTagReferenceShelf(t, shelfService, "Calm", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: definition.ID,
			ValueIDs:     []int64{calm.ID},
		}},
	})
	mixedShelf := createTagReferenceShelf(t, shelfService, "Mixed", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: definition.ID,
			ValueIDs:     []int64{calm.ID, dreamy.ID},
		}},
	})
	unrelatedShelf := createTagReferenceShelf(
		t,
		shelfService,
		"Unrelated",
		library.StoryLibraryQuery{Name: "moon"},
	)

	plan, err := tags.PlanDefinitionDeletion(ctx, definition.ID)
	if err != nil {
		t.Fatal(err)
	}
	if plan.AffectedCollectionCount != 2 ||
		!slices.Equal(plan.AffectedCollectionIDs, []int64{calmShelf.ID, mixedShelf.ID}) {
		t.Fatalf("PlanDefinitionDeletion() = %#v", plan)
	}

	lateShelf := createTagReferenceShelf(t, shelfService, "Late", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: definition.ID,
			ValueIDs:     []int64{dreamy.ID},
		}},
	})
	if err := tags.DeleteDefinition(ctx, plan); !errors.Is(err, ErrDeletePlanStale) {
		t.Fatalf("DeleteDefinition(stale shelves) error = %v", err)
	}

	plan, err = tags.PlanDefinitionDeletion(ctx, definition.ID)
	if err != nil {
		t.Fatal(err)
	}
	if plan.AffectedCollectionCount != 3 {
		t.Fatalf("PlanDefinitionDeletion(second) = %#v", plan)
	}
	if err := tags.DeleteDefinition(ctx, plan); err != nil {
		t.Fatalf("DeleteDefinition() error = %v", err)
	}
	for _, shelfID := range []int64{calmShelf.ID, mixedShelf.ID, lateShelf.ID} {
		assertTagReferenceShelfValidity(
			t,
			repository,
			shelfID,
			collections.ValidityNeedsAttention,
		)
	}
	assertTagReferenceShelfValidity(
		t,
		repository,
		unrelatedShelf.ID,
		collections.ValidityValid,
	)
	preserved, err := repository.Collection(ctx, calmShelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	if preserved.QueryPayload != calmShelf.QueryPayload {
		t.Fatalf(
			"referencing shelf payload changed from %q to %q",
			calmShelf.QueryPayload,
			preserved.QueryPayload,
		)
	}
	var members, protected, exclusions int
	if err := connection.QueryRow(`SELECT count(*),sum(protected) FROM shelf_stories WHERE shelf_id=1`).Scan(&members, &protected); err != nil {
		t.Fatal(err)
	}
	if err := connection.QueryRow(`SELECT count(*) FROM shelf_exclusions WHERE shelf_id=1`).Scan(&exclusions); err != nil {
		t.Fatal(err)
	}
	if members != 1 || protected != 1 || exclusions != 1 {
		t.Fatalf("tag deletion changed curated decisions: %d %d %d", members, protected, exclusions)
	}
}

func TestValueDeletionInvalidatesWholeQueryWithoutDroppingMissingCriterion(
	t *testing.T,
) {
	t.Parallel()

	ctx := context.Background()
	connection := openTaggingDatabase(t)
	tags, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	shelfService, err := collections.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	repository, err := collections.NewRepository(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, tags, "mood", KindChoice)
	calm := createChoiceValue(t, tags, definition.ID, "calm")
	dreamy := createChoiceValue(t, tags, definition.ID, "dreamy")

	calmShelf := createTagReferenceShelf(t, shelfService, "Calm", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: definition.ID,
			ValueIDs:     []int64{calm.ID},
		}},
	})
	mixedShelf := createTagReferenceShelf(t, shelfService, "Mixed", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: definition.ID,
			ValueIDs:     []int64{calm.ID, dreamy.ID},
		}},
	})
	dreamyShelf := createTagReferenceShelf(t, shelfService, "Dreamy", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: definition.ID,
			ValueIDs:     []int64{dreamy.ID},
		}},
	})

	plan, err := tags.PlanValueDeletion(ctx, calm.ID)
	if err != nil {
		t.Fatal(err)
	}
	if plan.AffectedCollectionCount != 2 ||
		!slices.Equal(plan.AffectedCollectionIDs, []int64{calmShelf.ID, mixedShelf.ID}) {
		t.Fatalf("PlanValueDeletion() = %#v", plan)
	}
	if err := tags.DeleteValue(ctx, plan); err != nil {
		t.Fatalf("DeleteValue() error = %v", err)
	}
	assertTagReferenceShelfValidity(
		t,
		repository,
		calmShelf.ID,
		collections.ValidityNeedsAttention,
	)
	assertTagReferenceShelfValidity(
		t,
		repository,
		mixedShelf.ID,
		collections.ValidityNeedsAttention,
	)
	assertTagReferenceShelfValidity(
		t,
		repository,
		dreamyShelf.ID,
		collections.ValidityValid,
	)
	preserved, err := repository.Collection(ctx, mixedShelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	if preserved.QueryPayload != mixedShelf.QueryPayload {
		t.Fatalf(
			"mixed shelf payload changed from %q to %q",
			mixedShelf.QueryPayload,
			preserved.QueryPayload,
		)
	}

	dreamyPlan, err := tags.PlanValueDeletion(ctx, dreamy.ID)
	if err != nil {
		t.Fatal(err)
	}
	if dreamyPlan.AffectedCollectionCount != 2 ||
		!slices.Equal(
			dreamyPlan.AffectedCollectionIDs,
			[]int64{mixedShelf.ID, dreamyShelf.ID},
		) {
		t.Fatalf("PlanValueDeletion(after prior invalidation) = %#v", dreamyPlan)
	}
	if err := tags.DeleteValue(ctx, dreamyPlan); err != nil {
		t.Fatalf("DeleteValue(after prior invalidation) error = %v", err)
	}
	assertTagReferenceShelfValidity(
		t,
		repository,
		mixedShelf.ID,
		collections.ValidityNeedsAttention,
	)
	assertTagReferenceShelfValidity(
		t,
		repository,
		dreamyShelf.ID,
		collections.ValidityNeedsAttention,
	)
}

func createTagReferenceShelf(
	t *testing.T,
	service *collections.Service,
	name string,
	query library.StoryLibraryQuery,
) collections.Collection {
	t.Helper()

	shelf, err := service.Create(context.Background(), name, query)
	if err != nil {
		t.Fatal(err)
	}
	return shelf
}

func assertTagReferenceShelfValidity(
	t *testing.T,
	repository *collections.Repository,
	shelfID int64,
	want collections.Validity,
) {
	t.Helper()

	shelf, err := repository.Collection(context.Background(), shelfID)
	if err != nil {
		t.Fatal(err)
	}
	if shelf.Validity != want {
		t.Fatalf("shelf %d validity = %q, want %q", shelfID, shelf.Validity, want)
	}
}
