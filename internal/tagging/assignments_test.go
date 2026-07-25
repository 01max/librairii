package tagging

import (
	"context"
	"errors"
	"testing"
)

func TestServiceSetsBooleanAssignmentsIdempotentlyForOneOrManyStories(t *testing.T) {
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
	firstStoryID := insertServiceStory(t, connection)
	secondStoryID := insertServiceStoryWithUUID(
		t,
		connection,
		"223e4567-e89b-42d3-a456-426614174001",
	)

	result, err := service.SetStoryBoolean(
		context.Background(),
		firstStoryID,
		broken.ID,
		true,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 1 || result.AssignmentsAdded != 1 {
		t.Fatalf("SetStoryBoolean() = %#v", result)
	}
	result, err = service.SetBulkBoolean(
		context.Background(),
		[]int64{secondStoryID, firstStoryID, firstStoryID},
		broken.ID,
		true,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.RequestedStories != 2 ||
		result.ChangedStories != 1 ||
		result.AssignmentsAdded != 1 {
		t.Fatalf("SetBulkBoolean() = %#v", result)
	}
	result, err = service.SetBulkBoolean(
		context.Background(),
		[]int64{firstStoryID, secondStoryID},
		broken.ID,
		false,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 2 || result.AssignmentsRemoved != 2 {
		t.Fatalf("SetBulkBoolean(false) = %#v", result)
	}
	result, err = service.SetStoryBoolean(
		context.Background(),
		firstStoryID,
		broken.ID,
		false,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 0 {
		t.Fatalf("SetStoryBoolean(idempotent false) = %#v", result)
	}
}

func TestServiceReplacesChoiceAssignmentsIdempotently(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, service, "mood", KindChoice)
	calm := createChoiceValue(t, service, definition.ID, "calm")
	adventure := createChoiceValue(t, service, definition.ID, "adventure")
	firstStoryID := insertServiceStory(t, connection)
	secondStoryID := insertServiceStoryWithUUID(
		t,
		connection,
		"223e4567-e89b-42d3-a456-426614174001",
	)

	result, err := service.SetBulkChoiceValues(
		context.Background(),
		[]int64{firstStoryID, secondStoryID},
		definition.ID,
		[]int64{calm.ID, calm.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.RequestedStories != 2 ||
		result.ChangedStories != 2 ||
		result.AssignmentsAdded != 2 {
		t.Fatalf("SetBulkChoiceValues() = %#v", result)
	}
	result, err = service.SetStoryChoiceValues(
		context.Background(),
		firstStoryID,
		definition.ID,
		[]int64{calm.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 0 {
		t.Fatalf("SetStoryChoiceValues(idempotent) = %#v", result)
	}
	result, err = service.SetStoryChoiceValues(
		context.Background(),
		firstStoryID,
		definition.ID,
		[]int64{calm.ID, adventure.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 1 || result.AssignmentsAdded != 1 {
		t.Fatalf("SetStoryChoiceValues(add) = %#v", result)
	}
	result, err = service.SetBulkChoiceValues(
		context.Background(),
		[]int64{firstStoryID, secondStoryID},
		definition.ID,
		[]int64{adventure.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 2 ||
		result.AssignmentsAdded != 1 ||
		result.AssignmentsRemoved != 2 {
		t.Fatalf("SetBulkChoiceValues(replace) = %#v", result)
	}
	result, err = service.SetBulkChoiceValues(
		context.Background(),
		[]int64{firstStoryID, secondStoryID},
		definition.ID,
		nil,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.AssignmentsRemoved != 2 || result.ChangedStories != 2 {
		t.Fatalf("SetBulkChoiceValues(clear) = %#v", result)
	}
}

func TestServiceRejectsManualDerivedAssignmentsAndKindMismatches(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	storyID := insertServiceStory(t, connection)
	userBoolean := createUserDefinition(t, service, "favorite", KindBoolean)
	userChoice := createUserDefinition(t, service, "mood", KindChoice)
	choiceValue := createChoiceValue(t, service, userChoice.ID, "calm")
	derivedID := insertDerivedDefinition(t, connection)
	derivedValueID := insertServiceValue(t, connection, derivedID, "3-5", 0)

	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		storyID,
		derivedID,
		[]int64{derivedValueID},
	); !errors.Is(err, ErrDerivedAssignment) {
		t.Fatalf("SetStoryChoiceValues(derived) error = %v", err)
	}
	if _, err := service.SetStoryBoolean(
		context.Background(),
		storyID,
		userChoice.ID,
		true,
	); !errors.Is(err, ErrAssignmentKind) {
		t.Fatalf("SetStoryBoolean(choice) error = %v", err)
	}
	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		storyID,
		userBoolean.ID,
		[]int64{choiceValue.ID},
	); !errors.Is(err, ErrAssignmentKind) {
		t.Fatalf("SetStoryChoiceValues(boolean) error = %v", err)
	}
}

func TestServiceValidatesEntireBulkAssignmentBeforeMutation(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, service, "mood", KindChoice)
	value := createChoiceValue(t, service, definition.ID, "calm")
	storyID := insertServiceStory(t, connection)

	if _, err := service.SetBulkChoiceValues(
		context.Background(),
		[]int64{storyID, 999_999},
		definition.ID,
		[]int64{value.ID},
	); !errors.Is(err, ErrStoryNotFound) {
		t.Fatalf("SetBulkChoiceValues(missing story) error = %v", err)
	}
	var assignments int
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM story_tag_assignments",
	).Scan(&assignments); err != nil {
		t.Fatal(err)
	}
	if assignments != 0 {
		t.Fatalf("assignment count = %d", assignments)
	}

	otherDefinition := createUserDefinition(t, service, "theme", KindChoice)
	otherValue := createChoiceValue(t, service, otherDefinition.ID, "night")
	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		storyID,
		definition.ID,
		[]int64{otherValue.ID},
	); !errors.Is(err, ErrValueNotFound) {
		t.Fatalf("SetStoryChoiceValues(wrong value) error = %v", err)
	}
}
