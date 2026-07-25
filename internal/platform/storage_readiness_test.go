package platform

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/database"
)

func TestStorageReadinessCreatesMigratedDatabase(t *testing.T) {
	t.Parallel()

	root := filepath.Join(t.TempDir(), "librairii")
	readiness := NewStorageReadiness(root)
	report, err := readiness.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}
	if !report.MutationsAllowed {
		t.Fatalf("Check() report = %#v", report)
	}
	if readiness.SQL() == nil {
		t.Fatal("SQL() = nil")
	}

	path := filepath.Join(root, "db", "librairii.sqlite3")
	identity, err := (database.SchemaProbe{}).Inspect(context.Background(), path)
	if err != nil {
		t.Fatalf("Inspect() error = %v", err)
	}
	if identity != database.IdentityCompatible {
		t.Fatalf("Inspect() = %q", identity)
	}
	if err := readiness.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
}

func TestStorageReadinessRefusesForeignDatabase(t *testing.T) {
	t.Parallel()

	root := filepath.Join(t.TempDir(), "librairii")
	databaseDirectory := filepath.Join(root, "db")
	if err := os.MkdirAll(databaseDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(databaseDirectory, "librairii.sqlite3"),
		[]byte("legacy"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}

	readiness := NewStorageReadiness(root)
	report, err := readiness.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}
	if report.MutationsAllowed || readiness.SQL() != nil {
		t.Fatalf("Check() report = %#v, SQL = %p", report, readiness.SQL())
	}
}
