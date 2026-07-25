package database

import (
	"context"
	"database/sql"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"testing/fstest"

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
	if got := database.Writer().Stats().MaxOpenConnections; got != 1 {
		t.Fatalf("Writer MaxOpenConnections = %d, want 1", got)
	}

	var migrations int
	if err := database.SQL().QueryRow("SELECT COUNT(*) FROM schema_migrations").Scan(&migrations); err != nil {
		t.Fatalf("migration count query error = %v", err)
	}
	if migrations != 13 {
		t.Fatalf("migration count = %d, want 13", migrations)
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

func TestOpenBacksUpOnlyWhenCompatibleMigrationsArePending(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	root := t.TempDir()
	path := filepath.Join(root, "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	archives := filepath.Join(root, "archives", "managed.pk")
	if err := os.MkdirAll(filepath.Dir(archives), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(archives, []byte("managed archive"), 0o600); err != nil {
		t.Fatal(err)
	}

	bootstrap, err := openDatabase(ctx, path, migrationsThrough(t, 1))
	if err != nil {
		t.Fatalf("bootstrap Open() error = %v", err)
	}
	if err := bootstrap.Close(); err != nil {
		t.Fatal(err)
	}

	upgraded, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("upgrade Open() error = %v", err)
	}
	if err := upgraded.Close(); err != nil {
		t.Fatal(err)
	}

	backups, err := filepath.Glob(filepath.Join(
		filepath.Dir(path),
		"librairii.pre-migration-v002-*.sqlite3",
	))
	if err != nil {
		t.Fatal(err)
	}
	if len(backups) != 1 {
		t.Fatalf("pre-migration backups = %v, want exactly one", backups)
	}
	backup, err := sql.Open("sqlite", readOnlyDSN(backups[0]))
	if err != nil {
		t.Fatal(err)
	}
	defer backup.Close()
	var backupMigrations int
	if err := backup.QueryRow("SELECT COUNT(*) FROM schema_migrations").
		Scan(&backupMigrations); err != nil {
		t.Fatal(err)
	}
	if backupMigrations != 1 {
		t.Fatalf("backup migration count = %d, want 1", backupMigrations)
	}

	reopened, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("reopen Open() error = %v", err)
	}
	if err := reopened.Close(); err != nil {
		t.Fatal(err)
	}
	backupsAfterReopen, err := filepath.Glob(filepath.Join(
		filepath.Dir(path),
		"librairii.pre-migration-*.sqlite3",
	))
	if err != nil {
		t.Fatal(err)
	}
	if len(backupsAfterReopen) != 1 {
		t.Fatalf("backups after no-op reopen = %v, want one", backupsAfterReopen)
	}
	assertFileBytes(t, archives, []byte("managed archive"))
}

func TestFailedMigrationRestoresBackupWithoutTouchingArchives(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	root := t.TempDir()
	path := filepath.Join(root, "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	archive := filepath.Join(root, "archives", "sha256", "story.v2.pk")
	if err := os.MkdirAll(filepath.Dir(archive), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(archive, []byte("archive survives"), 0o600); err != nil {
		t.Fatal(err)
	}

	bootstrap, err := openDatabase(ctx, path, migrationsThrough(t, 1))
	if err != nil {
		t.Fatal(err)
	}
	if err := bootstrap.Close(); err != nil {
		t.Fatal(err)
	}

	failing := migrationsThrough(t, 1)
	failing["migrations/002_committed_before_failure.sql"] = &fstest.MapFile{
		Data: []byte("CREATE TABLE migration_two (id INTEGER PRIMARY KEY);"),
	}
	failing["migrations/003_failure.sql"] = &fstest.MapFile{
		Data: []byte("CREATE TABLE migration_three (id INTEGER); SELECT * FROM missing_table;"),
	}
	if _, err := openDatabase(ctx, path, failing); err == nil {
		t.Fatal("Open(failing migrations) error = nil")
	}

	connection, err := sql.Open("sqlite", readOnlyDSN(path))
	if err != nil {
		t.Fatal(err)
	}
	defer connection.Close()
	var migrationCount int
	if err := connection.QueryRow("SELECT COUNT(*) FROM schema_migrations").
		Scan(&migrationCount); err != nil {
		t.Fatal(err)
	}
	if migrationCount != 1 {
		t.Fatalf("migration count after rollback = %d, want 1", migrationCount)
	}
	if _, err := connection.Exec("SELECT * FROM migration_two"); err == nil {
		t.Fatal("migration_two survived failed migration rollback")
	}
	assertFileBytes(t, archive, []byte("archive survives"))

	backups, err := filepath.Glob(filepath.Join(
		filepath.Dir(path),
		"librairii.pre-migration-v002-*.sqlite3",
	))
	if err != nil || len(backups) != 1 {
		t.Fatalf("rollback backups = %v, %v", backups, err)
	}
}

func TestFailedFirstMigrationRollsBackToAbsentDatabase(t *testing.T) {
	t.Parallel()

	path := filepath.Join(t.TempDir(), "librairii.sqlite3")
	failing := fstest.MapFS{
		"migrations/001_failure.sql": &fstest.MapFile{
			Data: []byte("CREATE TABLE partial (id INTEGER); SELECT * FROM missing_table;"),
		},
	}
	if _, err := openDatabase(context.Background(), path, failing); err == nil {
		t.Fatal("Open(failing first migration) error = nil")
	}
	if _, err := os.Stat(path); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("database after failed first migration stat error = %v", err)
	}
}

func TestRelocateSchemaConflictPreservesEveryLegacyFile(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	root := t.TempDir()
	path := filepath.Join(root, "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	legacyFiles := map[string][]byte{
		path:          []byte("legacy database bytes"),
		path + "-wal": []byte("legacy wal bytes"),
		path + "-shm": []byte("legacy shm bytes"),
	}
	for name, payload := range legacyFiles {
		if err := os.WriteFile(name, payload, 0o600); err != nil {
			t.Fatal(err)
		}
	}
	archive := filepath.Join(root, "archives", "managed.pk")
	if err := os.MkdirAll(filepath.Dir(archive), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(archive, []byte("managed archive"), 0o600); err != nil {
		t.Fatal(err)
	}

	recoveryDirectory, err := RelocateSchemaConflict(ctx, path)
	if err != nil {
		t.Fatalf("RelocateSchemaConflict() error = %v", err)
	}
	for name, payload := range legacyFiles {
		if _, err := os.Stat(name); !errors.Is(err, os.ErrNotExist) {
			t.Fatalf("legacy source %q still exists: %v", name, err)
		}
		assertFileBytes(
			t,
			filepath.Join(recoveryDirectory, filepath.Base(name)),
			payload,
		)
	}
	assertFileBytes(t, archive, []byte("managed archive"))

	fresh, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("Open(after explicit recovery) error = %v", err)
	}
	if err := fresh.Close(); err != nil {
		t.Fatal(err)
	}
	assertFileBytes(
		t,
		filepath.Join(recoveryDirectory, filepath.Base(path)),
		legacyFiles[path],
	)
}

func TestRelocateSchemaMarkerConflictPreservesLegacySQLite(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "librairii.sqlite3")
	legacy, err := sql.Open("sqlite", writableDSN(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(
		"CREATE TABLE legacy_stories (id INTEGER PRIMARY KEY, title TEXT)",
	); err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(
		"INSERT INTO legacy_stories (title) VALUES ('preserved')",
	); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}

	recoveryDirectory, err := RelocateSchemaConflict(ctx, path)
	if err != nil {
		t.Fatalf("RelocateSchemaConflict() error = %v", err)
	}
	recoveredPath := filepath.Join(recoveryDirectory, filepath.Base(path))
	recovered, err := sql.Open("sqlite", readOnlyDSN(recoveredPath))
	if err != nil {
		t.Fatal(err)
	}
	defer recovered.Close()
	var title string
	if err := recovered.QueryRow("SELECT title FROM legacy_stories").Scan(&title); err != nil {
		t.Fatal(err)
	}
	if title != "preserved" {
		t.Fatalf("recovered legacy title = %q, want preserved", title)
	}
	if identity, err := (SchemaProbe{}).Inspect(ctx, path); err != nil ||
		identity != IdentityAbsent {
		t.Fatalf("Inspect(relocated path) = %q, %v", identity, err)
	}
}

func TestRelocationFailureReportsIncompleteRollbackAndRecoveryDirectory(t *testing.T) {
	t.Parallel()

	root := t.TempDir()
	recoveryDirectory := filepath.Join(root, "schema-conflict-recovery-test")
	if err := os.Mkdir(recoveryDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	livePath := filepath.Join(root, "librairii.sqlite3")
	if err := os.Mkdir(livePath, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(livePath, "blocks-rollback"),
		[]byte("occupied"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
	recoveredPath := filepath.Join(recoveryDirectory, filepath.Base(livePath))
	if err := os.WriteFile(recoveredPath, []byte("legacy"), 0o600); err != nil {
		t.Fatal(err)
	}

	cause := errors.New("later sidecar move failed")
	preservedDirectory, err := relocationFailure(
		recoveryDirectory,
		[][2]string{{livePath, recoveredPath}},
		cause,
	)
	if !errors.Is(err, cause) {
		t.Fatalf("relocationFailure() error = %v, want original cause", err)
	}
	if preservedDirectory != recoveryDirectory {
		t.Fatalf(
			"relocationFailure() directory = %q, want %q",
			preservedDirectory,
			recoveryDirectory,
		)
	}
	if !strings.Contains(err.Error(), "rollback schema conflict relocation") {
		t.Fatalf("relocationFailure() error omits rollback failure: %v", err)
	}
	assertFileBytes(t, recoveredPath, []byte("legacy"))
	assertFileBytes(t, filepath.Join(livePath, "blocks-rollback"), []byte("occupied"))
}

func TestOpenRejectsUnsupportedMigrationHistoryWithoutChangingDatabase(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "librairii.sqlite3")
	connection, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := connection.SQL().Exec(
		"INSERT INTO schema_migrations (version) VALUES (999)",
	); err != nil {
		t.Fatal(err)
	}
	if err := connection.Close(); err != nil {
		t.Fatal(err)
	}

	if _, err := Open(ctx, path); !errors.Is(err, ErrMigrationConflict) {
		t.Fatalf("Open(conflicting history) error = %v", err)
	}
	reopened, err := sql.Open("sqlite", readOnlyDSN(path))
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	var futureVersion int
	if err := reopened.QueryRow(
		"SELECT COUNT(*) FROM schema_migrations WHERE version = 999",
	).Scan(&futureVersion); err != nil {
		t.Fatal(err)
	}
	if futureVersion != 1 {
		t.Fatalf("future migration marker count = %d, want 1", futureVersion)
	}
	backups, err := filepath.Glob(filepath.Join(
		filepath.Dir(path),
		"librairii.pre-migration-*.sqlite3",
	))
	if err != nil || len(backups) != 0 {
		t.Fatalf("conflicting migration backups = %v, %v", backups, err)
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

func migrationsThrough(t *testing.T, maximumVersion int) fstest.MapFS {
	t.Helper()

	result := fstest.MapFS{}
	entries, err := fs.ReadDir(migrationFiles, "migrations")
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		version, err := migrationVersion(entry.Name())
		if err != nil {
			t.Fatal(err)
		}
		if version > maximumVersion {
			continue
		}
		name := filepath.ToSlash(filepath.Join("migrations", entry.Name()))
		body, err := fs.ReadFile(migrationFiles, name)
		if err != nil {
			t.Fatal(err)
		}
		result[name] = &fstest.MapFile{Data: body}
	}
	return result
}

func assertFileBytes(t *testing.T, path string, expected []byte) {
	t.Helper()
	actual, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(actual) != string(expected) {
		t.Fatalf("%s bytes = %q, want %q", path, actual, expected)
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
