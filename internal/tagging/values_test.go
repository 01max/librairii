package tagging

import (
	"context"
	"errors"
	"testing"
)

func TestServiceCreatesRenamesAndReordersChoiceValues(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, service, "mood", KindChoice)
	calm, err := service.CreateValue(context.Background(), CreateValue{
		DefinitionID: definition.ID,
		Key:          "  Très Calme ",
		Label:        "  Very calm ",
	})
	if err != nil {
		t.Fatal(err)
	}
	if calm.Key != "tres-calme" ||
		calm.NormalizedKey != "tres-calme" ||
		calm.Label != "Very calm" {
		t.Fatalf("CreateValue() = %#v", calm)
	}
	if _, err := service.CreateValue(context.Background(), CreateValue{
		DefinitionID: definition.ID,
		Key:          "TRES CALME",
		Label:        "Duplicate",
	}); !errors.Is(err, ErrDuplicateValue) {
		t.Fatalf("CreateValue(duplicate) error = %v", err)
	}
	adventure := createChoiceValue(t, service, definition.ID, "adventure")
	bedtime := createChoiceValue(t, service, definition.ID, "bedtime")

	storyID := insertServiceStory(t, connection)
	insertServiceAssignment(t, connection, storyID, definition.ID, calm.ID)
	renamed, err := service.RenameValue(context.Background(), calm.ID, "  Quiet time ")
	if err != nil {
		t.Fatal(err)
	}
	if renamed.ID != calm.ID ||
		renamed.Key != calm.Key ||
		renamed.Label != "Quiet time" {
		t.Fatalf("RenameValue() = %#v", renamed)
	}
	var assignedValueID int64
	if err := connection.QueryRow(
		"SELECT value_id FROM story_tag_assignments WHERE story_id = ?",
		storyID,
	).Scan(&assignedValueID); err != nil {
		t.Fatal(err)
	}
	if assignedValueID != calm.ID {
		t.Fatalf("assigned value id = %d, want %d", assignedValueID, calm.ID)
	}

	values, err := service.ReorderValues(
		context.Background(),
		definition.ID,
		[]int64{bedtime.ID, calm.ID, adventure.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(values) != 3 ||
		values[0].ID != bedtime.ID ||
		values[1].ID != calm.ID ||
		values[2].ID != adventure.ID {
		t.Fatalf("ReorderValues() = %#v", values)
	}
	if _, err := service.ReorderValues(
		context.Background(),
		definition.ID,
		[]int64{calm.ID, adventure.ID},
	); !errors.Is(err, ErrInvalidValueOrder) {
		t.Fatalf("ReorderValues(incomplete) error = %v", err)
	}
}

func TestServiceRejectsValuesForBooleanAndDerivedDefinitions(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	broken, err := SeedBuiltIns(context.Background(), connection)
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	userBoolean := createUserDefinition(t, service, "favorite", KindBoolean)
	derivedChoiceID := insertDerivedDefinition(t, connection)

	for _, definitionID := range []int64{broken.ID, userBoolean.ID} {
		if _, err := service.CreateValue(context.Background(), CreateValue{
			DefinitionID: definitionID,
			Key:          "invalid",
			Label:        "Invalid",
		}); !errors.Is(err, ErrValuesNotAllowed) {
			t.Fatalf("CreateValue(boolean %d) error = %v", definitionID, err)
		}
	}
	if _, err := service.CreateValue(context.Background(), CreateValue{
		DefinitionID: derivedChoiceID,
		Key:          "3-5",
		Label:        "3–5 years",
	}); !errors.Is(err, ErrProtectedDefinition) {
		t.Fatalf("CreateValue(derived) error = %v", err)
	}
}

func TestServicePlansAndConfirmsChoiceValueDeletion(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, service, "mood", KindChoice)
	value := createChoiceValue(t, service, definition.ID, "calm")
	firstStoryID := insertServiceStory(t, connection)
	insertServiceAssignment(t, connection, firstStoryID, definition.ID, value.ID)

	plan, err := service.PlanValueDeletion(context.Background(), value.ID)
	if err != nil {
		t.Fatal(err)
	}
	if plan.AssignmentCount != 1 || plan.AffectedShelfCount != 0 {
		t.Fatalf("PlanValueDeletion() = %#v", plan)
	}
	secondStoryID := insertServiceStoryWithUUID(
		t,
		connection,
		"223e4567-e89b-42d3-a456-426614174001",
	)
	insertServiceAssignment(t, connection, secondStoryID, definition.ID, value.ID)
	if err := service.DeleteValue(context.Background(), plan); !errors.Is(
		err,
		ErrValueDeletePlanStale,
	) {
		t.Fatalf("DeleteValue(stale) error = %v", err)
	}

	plan, err = service.PlanValueDeletion(context.Background(), value.ID)
	if err != nil {
		t.Fatal(err)
	}
	if plan.AssignmentCount != 2 {
		t.Fatalf("PlanValueDeletion(second) = %#v", plan)
	}
	if err := service.DeleteValue(context.Background(), plan); err != nil {
		t.Fatalf("DeleteValue() error = %v", err)
	}
	if _, err := service.PlanValueDeletion(
		context.Background(),
		value.ID,
	); !errors.Is(err, ErrValueNotFound) {
		t.Fatalf("PlanValueDeletion(deleted) error = %v", err)
	}
	var assignments int
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM story_tag_assignments WHERE definition_id = ?",
		definition.ID,
	).Scan(&assignments); err != nil {
		t.Fatal(err)
	}
	if assignments != 0 {
		t.Fatalf("assignment count = %d", assignments)
	}
}

func createChoiceValue(
	t *testing.T,
	service *Service,
	definitionID int64,
	key string,
) Value {
	t.Helper()

	value, err := service.CreateValue(context.Background(), CreateValue{
		DefinitionID: definitionID,
		Key:          key,
		Label:        key,
	})
	if err != nil {
		t.Fatal(err)
	}
	return value
}
