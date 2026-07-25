package tagging

import (
	"context"
	"errors"
	"slices"
	"testing"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelves"
)

func TestDefinitionDeletionPlansAndInvalidatesEveryReferencingShelf(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	connection := openTaggingDatabase(t)
	tags, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	shelfService, err := shelves.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	repository, err := shelves.NewRepository(connection)
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
	if plan.AffectedShelfCount != 2 ||
		!slices.Equal(plan.AffectedShelfIDs, []int64{calmShelf.ID, mixedShelf.ID}) {
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
	if plan.AffectedShelfCount != 3 {
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
			shelves.ValidityNeedsAttention,
		)
	}
	assertTagReferenceShelfValidity(
		t,
		repository,
		unrelatedShelf.ID,
		shelves.ValidityValid,
	)
	preserved, err := repository.Shelf(ctx, calmShelf.ID)
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
	shelfService, err := shelves.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	repository, err := shelves.NewRepository(connection)
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
	if plan.AffectedShelfCount != 2 ||
		!slices.Equal(plan.AffectedShelfIDs, []int64{calmShelf.ID, mixedShelf.ID}) {
		t.Fatalf("PlanValueDeletion() = %#v", plan)
	}
	if err := tags.DeleteValue(ctx, plan); err != nil {
		t.Fatalf("DeleteValue() error = %v", err)
	}
	assertTagReferenceShelfValidity(
		t,
		repository,
		calmShelf.ID,
		shelves.ValidityNeedsAttention,
	)
	assertTagReferenceShelfValidity(
		t,
		repository,
		mixedShelf.ID,
		shelves.ValidityNeedsAttention,
	)
	assertTagReferenceShelfValidity(
		t,
		repository,
		dreamyShelf.ID,
		shelves.ValidityValid,
	)
	preserved, err := repository.Shelf(ctx, mixedShelf.ID)
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
	if dreamyPlan.AffectedShelfCount != 2 ||
		!slices.Equal(
			dreamyPlan.AffectedShelfIDs,
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
		shelves.ValidityNeedsAttention,
	)
	assertTagReferenceShelfValidity(
		t,
		repository,
		dreamyShelf.ID,
		shelves.ValidityNeedsAttention,
	)
}

func createTagReferenceShelf(
	t *testing.T,
	service *shelves.Service,
	name string,
	query library.StoryLibraryQuery,
) shelves.Shelf {
	t.Helper()

	shelf, err := service.Create(context.Background(), name, query)
	if err != nil {
		t.Fatal(err)
	}
	return shelf
}

func assertTagReferenceShelfValidity(
	t *testing.T,
	repository *shelves.Repository,
	shelfID int64,
	want shelves.Validity,
) {
	t.Helper()

	shelf, err := repository.Shelf(context.Background(), shelfID)
	if err != nil {
		t.Fatal(err)
	}
	if shelf.Validity != want {
		t.Fatalf("shelf %d validity = %q, want %q", shelfID, shelf.Validity, want)
	}
}
