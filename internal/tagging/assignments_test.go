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

func TestAssignmentWorkspaceReportsSingleAndMixedSelectionStates(t *testing.T) {
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
	mood := createUserDefinition(t, service, "mood", KindChoice)
	calm := createChoiceValue(t, service, mood.ID, "calm")
	bold := createChoiceValue(t, service, mood.ID, "bold")
	firstStoryID := insertServiceStory(t, connection)
	secondStoryID := insertServiceStoryWithUUID(
		t,
		connection,
		"223e4567-e89b-42d3-a456-426614174001",
	)
	if _, err := service.SetBulkBoolean(
		context.Background(),
		[]int64{firstStoryID, secondStoryID},
		broken.ID,
		true,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		firstStoryID,
		mood.ID,
		[]int64{calm.ID, bold.ID},
	); err != nil {
		t.Fatal(err)
	}
	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		secondStoryID,
		mood.ID,
		[]int64{bold.ID},
	); err != nil {
		t.Fatal(err)
	}

	workspace, err := service.AssignmentWorkspace(
		context.Background(),
		[]int64{secondStoryID, firstStoryID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if workspace.RequestedStories != 2 || len(workspace.States) != 2 {
		t.Fatalf("AssignmentWorkspace() = %#v", workspace)
	}
	if workspace.States[0].DefinitionID != broken.ID ||
		workspace.States[0].AssignedStories != 2 {
		t.Fatalf("broken state = %#v", workspace.States[0])
	}
	moodState := workspace.States[1]
	if moodState.DefinitionID != mood.ID ||
		len(moodState.Values) != 2 ||
		moodState.Values[0].ValueID != calm.ID ||
		moodState.Values[0].AssignedStories != 1 ||
		moodState.Values[1].ValueID != bold.ID ||
		moodState.Values[1].AssignedStories != 2 {
		t.Fatalf("mood state = %#v", moodState)
	}
}

func TestServiceTogglesOneChoiceValueAcrossStoriesWithoutReplacingOthers(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	mood := createUserDefinition(t, service, "mood", KindChoice)
	calm := createChoiceValue(t, service, mood.ID, "calm")
	bold := createChoiceValue(t, service, mood.ID, "bold")
	firstStoryID := insertServiceStory(t, connection)
	secondStoryID := insertServiceStoryWithUUID(
		t,
		connection,
		"223e4567-e89b-42d3-a456-426614174001",
	)
	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		firstStoryID,
		mood.ID,
		[]int64{calm.ID},
	); err != nil {
		t.Fatal(err)
	}
	result, err := service.SetBulkChoiceValue(
		context.Background(),
		[]int64{firstStoryID, secondStoryID},
		mood.ID,
		bold.ID,
		true,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ChangedStories != 2 || result.AssignmentsAdded != 2 {
		t.Fatalf("SetBulkChoiceValue() = %#v", result)
	}
	workspace, err := service.AssignmentWorkspace(
		context.Background(),
		[]int64{firstStoryID},
	)
	if err != nil {
		t.Fatal(err)
	}
	moodState := workspace.States[0]
	if moodState.Values[0].AssignedStories != 1 ||
		moodState.Values[1].AssignedStories != 1 {
		t.Fatalf("other choice was replaced: %#v", moodState)
	}
}
