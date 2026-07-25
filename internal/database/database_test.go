package database

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

func TestOpenCreatesMigratesAndReopensDatabase(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}

	database, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}

	assertPragma(t, database.SQL(), "foreign_keys", 1)
	assertPragma(t, database.SQL(), "busy_timeout", busyTimeoutMilliseconds)
	var journalMode string
	if err := database.SQL().QueryRow("PRAGMA journal_mode").Scan(&journalMode); err != nil {
		t.Fatalf("journal mode query error = %v", err)
	}
	if journalMode != "wal" {
		t.Fatalf("journal_mode = %q, want wal", journalMode)
	}
	if got := database.SQL().Stats().MaxOpenConnections; got != maxOpenConnections {
		t.Fatalf("MaxOpenConnections = %d, want %d", got, maxOpenConnections)
	}

	var migrations int
	if err := database.SQL().QueryRow("SELECT COUNT(*) FROM schema_migrations").Scan(&migrations); err != nil {
		t.Fatalf("migration count query error = %v", err)
	}
	if migrations != 11 {
		t.Fatalf("migration count = %d, want 11", migrations)
	}
	if err := database.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	reopened, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("Open(reopen) error = %v", err)
	}
	defer reopened.Close()

	identity, err := (SchemaProbe{}).Inspect(ctx, path)
	if err != nil {
		t.Fatalf("Inspect() error = %v", err)
	}
	if identity != IdentityCompatible {
		t.Fatalf("Inspect() = %q, want %q", identity, IdentityCompatible)
	}
}

func TestOpenRefusesForeignDatabase(t *testing.T) {
	t.Parallel()

	path := filepath.Join(t.TempDir(), "legacy.sqlite3")
	connection, err := sql.Open("sqlite", writableDSN(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := connection.Exec("CREATE TABLE legacy_records (id INTEGER PRIMARY KEY)"); err != nil {
		t.Fatal(err)
	}
	if err := connection.Close(); err != nil {
		t.Fatal(err)
	}

	if _, err := Open(context.Background(), path); !errors.Is(err, ErrForeignSchema) {
		t.Fatalf("Open() error = %v, want ErrForeignSchema", err)
	}
}

func TestMigrationVersion(t *testing.T) {
	t.Parallel()

	version, err := migrationVersion("012_create_stories.sql")
	if err != nil || version != 12 {
		t.Fatalf("migrationVersion() = %d, %v", version, err)
	}
	if _, err := migrationVersion("invalid.sql"); !errors.Is(err, ErrInvalidMigration) {
		t.Fatalf("migrationVersion(invalid) error = %v", err)
	}
}

func assertPragma(t *testing.T, connection *sql.DB, name string, expected int) {
	t.Helper()

	var got int
	if err := connection.QueryRow("PRAGMA " + name).Scan(&got); err != nil {
		t.Fatalf("%s query error = %v", name, err)
	}
	if got != expected {
		t.Fatalf("%s = %d, want %d", name, got, expected)
	}
}
