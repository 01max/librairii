package database

import (
	"context"
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

func TestSchemaProbeClassifiesDatabaseFiles(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	probe := SchemaProbe{}
	root := t.TempDir()

	identity, err := probe.Inspect(ctx, filepath.Join(root, "missing.sqlite3"))
	if err != nil || identity != IdentityAbsent {
		t.Fatalf("Inspect(missing) = %q, %v", identity, err)
	}

	random := filepath.Join(root, "random.sqlite3")
	if err := os.WriteFile(random, []byte("not sqlite"), 0o600); err != nil {
		t.Fatal(err)
	}
	identity, err = probe.Inspect(ctx, random)
	if err != nil || identity != IdentityForeign {
		t.Fatalf("Inspect(random) = %q, %v", identity, err)
	}

	legacy := filepath.Join(root, "legacy.sqlite3")
	connection, err := sql.Open("sqlite", writableDSN(legacy))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := connection.Exec("CREATE TABLE legacy (id INTEGER PRIMARY KEY)"); err != nil {
		t.Fatal(err)
	}
	if err := connection.Close(); err != nil {
		t.Fatal(err)
	}
	identity, err = probe.Inspect(ctx, legacy)
	if err != nil || identity != IdentityForeign {
		t.Fatalf("Inspect(legacy) = %q, %v", identity, err)
	}
}
