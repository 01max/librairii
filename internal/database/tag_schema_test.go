package database

import (
	"context"
	"database/sql"
	"os"
	"path/filepath"
	"testing"
)

func TestTagSchemaConstrainsDefinitionIdentityEnumsAndOrdering(t *testing.T) {
	t.Parallel()

	connection := openTagSchemaDatabase(t)
	insertTagDefinition(t, connection, "Mood", "mood", "Mood", "#405CF5", "choice", "user", "default", 0, 0)

	assertTagStatementFails(t, connection, `
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES ('MOOD', 'mood', 'Other mood', '#506CF5', 'choice', 'user', 'default', 1, 0)
	`)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES ('invalid-kind', 'invalid-kind', 'Invalid', '#405CF5', 'text', 'user', 'default', 1, 0)
	`)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES ('invalid-source', 'invalid-source', 'Invalid', '#405CF5', 'boolean', 'remote', 'default', 1, 0)
	`)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES ('unprotected-system', 'unprotected-system', 'Invalid', '#405CF5', 'boolean', 'derived', 'system', 0, 0)
	`)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES ('bad-color', 'bad-color', 'Invalid', '#NOTHEX', 'boolean', 'user', 'default', 1, 0)
	`)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES ('duplicate-position', 'duplicate-position', 'Invalid', '#405CF5', 'boolean', 'user', 'default', 0, 0)
	`)
}

func TestTagSchemaConstrainsChoiceValuesAndNullableAssignments(t *testing.T) {
	t.Parallel()

	connection := openTagSchemaDatabase(t)
	storyID := insertTagStory(t, connection)
	booleanID := insertTagDefinition(
		t, connection, "Broken", "broken", "Broken", "#C63C3C",
		"boolean", "builtin", "warning", 0, 1,
	)
	choiceID := insertTagDefinition(
		t, connection, "Mood", "mood", "Mood", "#405CF5",
		"choice", "user", "default", 0, 0,
	)
	derivedID := insertTagDefinition(
		t, connection, "Age", "age", "Age", "#7354B8",
		"choice", "derived", "system", 0, 1,
	)
	calmID := insertTagValue(t, connection, choiceID, "Calm", "calm", "Calm", 0)
	adventureID := insertTagValue(t, connection, choiceID, "Adventure", "adventure", "Adventure", 1)
	ageID := insertTagValue(t, connection, derivedID, "3-5", "3-5", "3–5 years", 0)

	assertTagStatementFails(t, connection, `
		INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		) VALUES (?, 'invalid', 'invalid', 'Invalid', 0)
	`, booleanID)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		) VALUES (?, 'CALM', 'calm', 'Other calm', 2)
	`, choiceID)
	assertTagStatementFails(t, connection, `
		INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		) VALUES (?, 'other', 'other', 'Other', 1)
	`, choiceID)
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, 'manual')
	`, storyID, booleanID, calmID)
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, NULL, 'manual')
	`, storyID, choiceID)
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, 'manual')
	`, storyID, choiceID, ageID)
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, 'manual')
	`, storyID, derivedID, ageID)
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, 'derived')
	`, storyID, choiceID, calmID)

	insertTagAssignment(t, connection, storyID, booleanID, nil, "manual")
	insertTagAssignment(t, connection, storyID, choiceID, calmID, "manual")
	insertTagAssignment(t, connection, storyID, choiceID, adventureID, "manual")
	insertTagAssignment(t, connection, storyID, derivedID, ageID, "derived")
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, NULL, 'manual')
	`, storyID, booleanID)
	assertTagStatementFails(t, connection, `
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, 'manual')
	`, storyID, choiceID, calmID)
	assertTagStatementFails(
		t,
		connection,
		"UPDATE tag_definitions SET kind = 'boolean' WHERE id = ?",
		choiceID,
	)
	assertTagStatementFails(
		t,
		connection,
		"UPDATE tag_definitions SET source = 'derived', is_protected = 1 WHERE id = ?",
		choiceID,
	)
}

func TestTagSchemaCascadesValuesDefinitionsAndStoryAssignments(t *testing.T) {
	t.Parallel()

	connection := openTagSchemaDatabase(t)
	storyID := insertTagStory(t, connection)
	definitionID := insertTagDefinition(
		t, connection, "Mood", "mood", "Mood", "#405CF5",
		"choice", "user", "default", 0, 0,
	)
	valueID := insertTagValue(t, connection, definitionID, "Calm", "calm", "Calm", 0)
	insertTagAssignment(t, connection, storyID, definitionID, valueID, "manual")

	if _, err := connection.Exec("DELETE FROM tag_values WHERE id = ?", valueID); err != nil {
		t.Fatal(err)
	}
	assertTagAssignmentCount(t, connection, 0)

	valueID = insertTagValue(t, connection, definitionID, "Calm", "calm", "Calm", 0)
	insertTagAssignment(t, connection, storyID, definitionID, valueID, "manual")
	if _, err := connection.Exec("DELETE FROM tag_definitions WHERE id = ?", definitionID); err != nil {
		t.Fatal(err)
	}
	assertTagAssignmentCount(t, connection, 0)
}

func openTagSchemaDatabase(t *testing.T) *sql.DB {
	t.Helper()

	path := filepath.Join(t.TempDir(), "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := database.Close(); err != nil {
			t.Error(err)
		}
	})
	return database.SQL()
}

func insertTagStory(t *testing.T, connection *sql.DB) int64 {
	t.Helper()

	result, err := connection.Exec(
		"INSERT INTO stories (uuid) VALUES ('123e4567-e89b-42d3-a456-426614174000')",
	)
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}

func insertTagDefinition(
	t *testing.T,
	connection *sql.DB,
	key string,
	normalizedKey string,
	label string,
	color string,
	kind string,
	source string,
	presentation string,
	position int,
	protected int,
) int64 {
	t.Helper()

	result, err := connection.Exec(`
		INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source, presentation, position, is_protected
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
	`, key, normalizedKey, label, color, kind, source, presentation, position, protected)
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}

func insertTagValue(
	t *testing.T,
	connection *sql.DB,
	definitionID int64,
	key string,
	normalizedKey string,
	label string,
	position int,
) int64 {
	t.Helper()

	result, err := connection.Exec(`
		INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		) VALUES (?, ?, ?, ?, ?)
	`, definitionID, key, normalizedKey, label, position)
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}

func insertTagAssignment(
	t *testing.T,
	connection *sql.DB,
	storyID int64,
	definitionID int64,
	valueID any,
	source string,
) {
	t.Helper()

	if _, err := connection.Exec(`
		INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		) VALUES (?, ?, ?, ?)
	`, storyID, definitionID, valueID, source); err != nil {
		t.Fatal(err)
	}
}

func assertTagStatementFails(
	t *testing.T,
	connection *sql.DB,
	statement string,
	arguments ...any,
) {
	t.Helper()

	if _, err := connection.Exec(statement, arguments...); err == nil {
		t.Fatalf("statement unexpectedly succeeded: %s", statement)
	}
}

func assertTagAssignmentCount(t *testing.T, connection *sql.DB, expected int) {
	t.Helper()

	var count int
	if err := connection.QueryRow("SELECT COUNT(*) FROM story_tag_assignments").Scan(&count); err != nil {
		t.Fatal(err)
	}
	if count != expected {
		t.Fatalf("assignment count = %d, want %d", count, expected)
	}
}
