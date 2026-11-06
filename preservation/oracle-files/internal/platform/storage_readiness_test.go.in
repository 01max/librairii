package platform

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/preferences"
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
	for _, mode := range []string{"isolated", "configured", "configured-restart"} {
		t.Run(mode, func(t *testing.T) {
			testStorageReadinessRecoversForeignDatabase(t, mode)
		})
	}
}

func testStorageReadinessRecoversForeignDatabase(t *testing.T, mode string) {
	t.Helper()

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

	directory := t.TempDir()
	readiness := NewStorageReadiness(root)
	if mode != "isolated" {
		readiness = NewConfiguredStorageReadiness(directory, root, "fr")
	}
	report, err := readiness.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}
	if report.MutationsAllowed || readiness.SQL() != nil {
		t.Fatalf("Check() report = %#v, SQL = %p", report, readiness.SQL())
	}

	recoveryDirectory, err := readiness.RecoverSchemaConflict(context.Background())
	if err != nil {
		t.Fatalf("RecoverSchemaConflict() error = %v", err)
	}
	recoveredPath := filepath.Join(recoveryDirectory, "librairii.sqlite3")
	recovered, err := os.ReadFile(recoveredPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(recovered) != "legacy" {
		t.Fatalf("recovered database = %q, want legacy", recovered)
	}

	if mode == "configured-restart" {
		readiness = NewConfiguredStorageReadiness(directory, root, "en")
	}
	report, err = readiness.Check(context.Background())
	if err != nil {
		t.Fatalf("Check(after recovery) error = %v", err)
	}
	if !report.MutationsAllowed || readiness.SQL() == nil {
		t.Fatalf(
			"Check(after recovery) report = %#v, SQL = %p",
			report,
			readiness.SQL(),
		)
	}
	if err := readiness.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestConfiguredRecoveryPersistsReplacementProfile(t *testing.T) {
	ctx := context.Background()
	root := filepath.Join(t.TempDir(), "legacy")
	directory := t.TempDir()
	if err := os.MkdirAll(filepath.Dir(preferences.DatabasePath(root)), 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(preferences.DatabasePath(root), []byte("foreign"), 0600); err != nil {
		t.Fatal(err)
	}
	r := NewConfiguredStorageReadiness(directory, root, "fr")
	if _, err := r.Check(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := r.RecoverSchemaConflict(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := r.Check(ctx); err != nil {
		t.Fatal(err)
	}
	if err := r.Close(); err != nil {
		t.Fatal(err)
	}
	next := NewConfiguredStorageReadiness(directory, root, "en")
	report, err := next.Check(ctx)
	if err != nil || !report.MutationsAllowed {
		t.Fatal(report, err)
	}
	defer next.Close()
	p := next.Profile()
	if !p.Complete || p.Recovering || p.Language != "fr" || p.Root != root {
		t.Fatal(p)
	}
	if err := preferences.ValidateLibrary(ctx, p.Root, p.LibraryID); err != nil {
		t.Fatal(err)
	}
}
