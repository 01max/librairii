package tagging

import (
	"context"
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/database"
)

func TestSeedBuiltInsCreatesExactlyOneCanonicalBrokenDefinition(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	first, err := SeedBuiltIns(context.Background(), connection)
	if err != nil {
		t.Fatalf("SeedBuiltIns() error = %v", err)
	}
	second, err := SeedBuiltIns(context.Background(), connection)
	if err != nil {
		t.Fatalf("SeedBuiltIns(second) error = %v", err)
	}
	if first != second || !first.canonicalBroken() {
		t.Fatalf("broken definitions = %#v, %#v", first, second)
	}

	var definitions int
	var values int
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM tag_definitions WHERE normalized_key = 'broken' COLLATE NOCASE",
	).Scan(&definitions); err != nil {
		t.Fatal(err)
	}
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM tag_values WHERE definition_id = ?",
		first.ID,
	).Scan(&values); err != nil {
		t.Fatal(err)
	}
	if definitions != 1 || values != 0 {
		t.Fatalf("broken definitions = %d, values = %d", definitions, values)
	}
}

func TestProtectedBrokenDefinitionCannotDriftOrGainValues(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	broken, err := SeedBuiltIns(context.Background(), connection)
	if err != nil {
		t.Fatal(err)
	}

	assertProtectedMutationFails(
		t,
		connection,
		"UPDATE tag_definitions SET key = 'damaged', normalized_key = 'damaged' WHERE id = ?",
		broken.ID,
	)
	assertProtectedMutationFails(
		t,
		connection,
		"UPDATE tag_definitions SET kind = 'choice' WHERE id = ?",
		broken.ID,
	)
	assertProtectedMutationFails(
		t,
		connection,
		"UPDATE tag_definitions SET presentation = 'default' WHERE id = ?",
		broken.ID,
	)
	assertProtectedMutationFails(
		t,
		connection,
		"UPDATE tag_definitions SET color = '#405CF5' WHERE id = ?",
		broken.ID,
	)
	assertProtectedMutationFails(
		t,
		connection,
		"DELETE FROM tag_definitions WHERE id = ?",
		broken.ID,
	)
	assertProtectedMutationFails(
		t,
		connection,
		`INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		) VALUES (?, 'true', 'true', 'True', 0)`,
		broken.ID,
	)
	assertProtectedMutationFails(
		t,
		connection,
		`INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source,
			presentation, position, is_protected
		) VALUES (
			'Broken', 'BROKEN', 'Drifted', '#405CF5', 'choice', 'user',
			'default', 1, 0
		)`,
	)
}

func openTaggingDatabase(t *testing.T) *sql.DB {
	t.Helper()

	path := filepath.Join(t.TempDir(), "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	sqlDatabase, err := database.Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := sqlDatabase.Close(); err != nil {
			t.Error(err)
		}
	})
	return sqlDatabase.SQL()
}

func assertProtectedMutationFails(
	t *testing.T,
	connection *sql.DB,
	statement string,
	arguments ...any,
) {
	t.Helper()

	if _, err := connection.Exec(statement, arguments...); err == nil {
		t.Fatalf("protected mutation unexpectedly succeeded: %s", statement)
	}
}
