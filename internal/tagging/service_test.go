package tagging

import (
	"context"
	"database/sql"
	"errors"
	"testing"
)

func TestServiceCreatesNormalizesAndValidatesUserDefinitions(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	if _, err := SeedBuiltIns(context.Background(), connection); err != nil {
		t.Fatal(err)
	}
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition, err := service.CreateDefinition(context.Background(), CreateDefinition{
		Key:   "  Humeur d'Été  ",
		Label: "  Summer mood ",
		Color: "#405cf5",
		Kind:  KindChoice,
	})
	if err != nil {
		t.Fatalf("CreateDefinition() error = %v", err)
	}
	if definition.Key != "humeur-d-ete" ||
		definition.NormalizedKey != "humeur-d-ete" ||
		definition.Label != "Summer mood" ||
		definition.Color != "#405CF5" ||
		definition.Source != SourceUser ||
		definition.Protected {
		t.Fatalf("CreateDefinition() = %#v", definition)
	}
	if _, err := service.CreateDefinition(context.Background(), CreateDefinition{
		Key:   "HUMEUR D ETE",
		Label: "Duplicate",
		Color: "#405CF5",
		Kind:  KindBoolean,
	}); !errors.Is(err, ErrDuplicateDefinition) {
		t.Fatalf("CreateDefinition(duplicate) error = %v", err)
	}
	if _, err := service.CreateDefinition(context.Background(), CreateDefinition{
		Key:   "broken",
		Label: "User broken",
		Color: "#405CF5",
		Kind:  KindBoolean,
	}); !errors.Is(err, ErrDuplicateDefinition) {
		t.Fatalf("CreateDefinition(protected key) error = %v", err)
	}
	if _, err := service.CreateDefinition(context.Background(), CreateDefinition{
		Key:   "pale",
		Label: "Pale",
		Color: "#FFFFFF",
		Kind:  KindBoolean,
	}); !errors.Is(err, ErrInvalidDefinition) {
		t.Fatalf("CreateDefinition(pale) error = %v", err)
	}
}

func TestServiceRenamesRecolorsAndReordersOnlyUserDefinitions(t *testing.T) {
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
	first := createUserDefinition(t, service, "first", KindBoolean)
	second := createUserDefinition(t, service, "second", KindChoice)
	third := createUserDefinition(t, service, "third", KindBoolean)

	renamed, err := service.RenameDefinition(context.Background(), second.ID, "  Evening mood ")
	if err != nil {
		t.Fatal(err)
	}
	if renamed.Label != "Evening mood" || renamed.Key != second.Key {
		t.Fatalf("RenameDefinition() = %#v", renamed)
	}
	recolored, err := service.RecolorDefinition(context.Background(), second.ID, "#17223d")
	if err != nil {
		t.Fatal(err)
	}
	if recolored.Color != "#17223D" {
		t.Fatalf("RecolorDefinition() = %#v", recolored)
	}

	definitions, err := service.ReorderDefinitions(
		context.Background(),
		[]int64{third.ID, first.ID, second.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(definitions) != 4 ||
		definitions[0].ID != broken.ID ||
		definitions[1].ID != third.ID ||
		definitions[2].ID != first.ID ||
		definitions[3].ID != second.ID {
		t.Fatalf("ReorderDefinitions() = %#v", definitions)
	}
	if _, err := service.ReorderDefinitions(
		context.Background(),
		[]int64{first.ID, second.ID},
	); !errors.Is(err, ErrInvalidOrder) {
		t.Fatalf("ReorderDefinitions(incomplete) error = %v", err)
	}
	if _, err := service.RenameDefinition(
		context.Background(),
		broken.ID,
		"Damaged",
	); !errors.Is(err, ErrProtectedDefinition) {
		t.Fatalf("RenameDefinition(broken) error = %v", err)
	}
}

func TestServicePlansAndConfirmsDefinitionDeletion(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	if _, err := SeedBuiltIns(context.Background(), connection); err != nil {
		t.Fatal(err)
	}
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, service, "mood", KindChoice)
	storyID := insertServiceStory(t, connection)
	valueID := insertServiceValue(t, connection, definition.ID, "calm", 0)
	insertServiceAssignment(t, connection, storyID, definition.ID, valueID)

	plan, err := service.PlanDefinitionDeletion(context.Background(), definition.ID)
	if err != nil {
		t.Fatal(err)
	}
	if plan.ValueCount != 1 || plan.AssignmentCount != 1 || plan.AffectedShelfCount != 0 {
		t.Fatalf("PlanDefinitionDeletion() = %#v", plan)
	}
	secondStoryID := insertServiceStoryWithUUID(
		t,
		connection,
		"223e4567-e89b-42d3-a456-426614174001",
	)
	insertServiceAssignment(t, connection, secondStoryID, definition.ID, valueID)
	if err := service.DeleteDefinition(context.Background(), plan); !errors.Is(
		err,
		ErrDeletePlanStale,
	) {
		t.Fatalf("DeleteDefinition(stale) error = %v", err)
	}

	plan, err = service.PlanDefinitionDeletion(context.Background(), definition.ID)
	if err != nil {
		t.Fatal(err)
	}
	if err := service.DeleteDefinition(context.Background(), plan); err != nil {
		t.Fatalf("DeleteDefinition() error = %v", err)
	}
	if _, err := service.PlanDefinitionDeletion(
		context.Background(),
		definition.ID,
	); !errors.Is(err, ErrDefinitionNotFound) {
		t.Fatalf("PlanDefinitionDeletion(deleted) error = %v", err)
	}
}

func TestServiceRejectsProtectedBuiltInAndDerivedDeletion(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	broken, err := SeedBuiltIns(context.Background(), connection)
	if err != nil {
		t.Fatal(err)
	}
	derivedID := insertDerivedDefinition(t, connection)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	for _, definitionID := range []int64{broken.ID, derivedID} {
		if _, err := service.PlanDefinitionDeletion(
			context.Background(),
			definitionID,
		); !errors.Is(err, ErrProtectedDefinition) {
			t.Fatalf("PlanDefinitionDeletion(%d) error = %v", definitionID, err)
		}
		if _, err := service.RecolorDefinition(
			context.Background(),
			definitionID,
			"#405CF5",
		); !errors.Is(err, ErrProtectedDefinition) {
			t.Fatalf("RecolorDefinition(%d) error = %v", definitionID, err)
		}
	}
}

func createUserDefinition(
	t *testing.T,
	service *Service,
	key string,
	kind Kind,
) Definition {
	t.Helper()

	definition, err := service.CreateDefinition(context.Background(), CreateDefinition{
		Key:   key,
		Label: key,
		Color: "#405CF5",
		Kind:  kind,
	})
	if err != nil {
		t.Fatal(err)
	}
	return definition
}

func insertServiceStory(t *testing.T, connection interface {
	Exec(string, ...any) (sql.Result, error)
}) int64 {
	t.Helper()
	return insertServiceStoryWithUUID(
		t,
		connection,
		"123e4567-e89b-42d3-a456-426614174000",
	)
}

func insertServiceStoryWithUUID(
	t *testing.T,
	connection interface {
		Exec(string, ...any) (sql.Result, error)
	},
	uuid string,
) int64 {
	t.Helper()

	result, err := connection.Exec("INSERT INTO stories (uuid) VALUES (?)", uuid)
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}

func insertServiceValue(
	t *testing.T,
	connection interface {
		Exec(string, ...any) (sql.Result, error)
	},
	definitionID int64,
	key string,
	position int,
) int64 {
	t.Helper()

	result, err := connection.Exec(`
		INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		) VALUES (?, ?, ?, ?, ?)
	`, definitionID, key, key, key, position)
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}

func insertServiceAssignment(
	t *testing.T,
	connection interface {
		Exec(string, ...any) (sql.Result, error)
	},
	storyID int64,
	definitionID int64,
	valueID int64,
) {
	t.Helper()

	if _, err := connection.Exec(`
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, 'manual')
	`, storyID, definitionID, valueID); err != nil {
		t.Fatal(err)
	}
}

func insertDerivedDefinition(t *testing.T, connection interface {
	Exec(string, ...any) (sql.Result, error)
}) int64 {
	t.Helper()

	result, err := connection.Exec(`
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source,
			presentation, position, is_protected
		) VALUES ('age', 'age', 'Age', '#7354B8', 'choice', 'derived', 'system', 0, 1)
	`)
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}
