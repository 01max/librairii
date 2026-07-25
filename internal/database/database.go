package database

import (
	"context"
	"database/sql"
	"embed"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"net/url"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	_ "modernc.org/sqlite"
)

const (
	SchemaProduct = "librairii"
	SchemaFamily  = "wails-go-sqlite"

	busyTimeoutMilliseconds = 5000
	maxOpenConnections      = 4
)

var (
	ErrForeignSchema     = errors.New("database does not contain the Librairii Wails schema identity")
	ErrInvalidMigration  = errors.New("invalid embedded migration")
	ErrMigrationConflict = errors.New(
		"database migration history is not a supported prefix",
	)
	ErrNoSchemaConflict = errors.New("database does not have a foreign schema conflict")

	//go:embed migrations/*.sql
	migrationFiles embed.FS
)

type Database struct {
	sql    *sql.DB
	writer *sql.DB
	path   string
}

func Open(ctx context.Context, path string) (*Database, error) {
	return openDatabase(ctx, path, migrationFiles)
}

func openDatabase(
	ctx context.Context,
	path string,
	migrations fs.FS,
) (*Database, error) {
	identity, err := (SchemaProbe{}).Inspect(ctx, path)
	if err != nil {
		return nil, err
	}
	if identity == IdentityForeign {
		return nil, ErrForeignSchema
	}

	connection, err := sql.Open("sqlite", writableDSN(path))
	if err != nil {
		return nil, fmt.Errorf("open SQLite database: %w", err)
	}
	connection.SetMaxOpenConns(maxOpenConnections)
	connection.SetMaxIdleConns(maxOpenConnections)

	closeOnError := func(openErr error) (*Database, error) {
		closeErr := connection.Close()
		return nil, errors.Join(openErr, closeErr)
	}

	if err := connection.PingContext(ctx); err != nil {
		return closeOnError(fmt.Errorf("ping SQLite database: %w", err))
	}
	var backupPath string
	if identity == IdentityCompatible {
		pending, planErr := pendingMigrations(ctx, connection, migrations)
		if planErr != nil {
			return closeOnError(planErr)
		}
		if len(pending) > 0 {
			backupPath, err = createPreMigrationBackup(
				ctx,
				connection,
				path,
				pending[0],
			)
			if err != nil {
				return closeOnError(err)
			}
		}
	}
	if err := migrate(ctx, connection, migrations); err != nil {
		closeErr := connection.Close()
		var rollbackErr error
		if backupPath != "" {
			rollbackErr = restoreDatabaseBackup(backupPath, path)
		} else if identity == IdentityAbsent {
			rollbackErr = removeDatabaseFiles(path)
		}
		return nil, errors.Join(err, closeErr, rollbackErr)
	}

	writer, err := sql.Open("sqlite", writableDSN(path))
	if err != nil {
		return closeOnError(fmt.Errorf("open SQLite writer lane: %w", err))
	}
	writer.SetMaxOpenConns(1)
	writer.SetMaxIdleConns(1)
	if err := writer.PingContext(ctx); err != nil {
		_ = writer.Close()
		return closeOnError(fmt.Errorf("ping SQLite writer lane: %w", err))
	}

	return &Database{
		sql:    connection,
		writer: writer,
		path:   filepath.Clean(path),
	}, nil
}

// RelocateSchemaConflict explicitly moves a foreign or legacy SQLite database
// and its sidecars into a unique recovery directory. Nothing is overwritten,
// and the selected database path is freed only after every existing file has
// been moved successfully.
func RelocateSchemaConflict(ctx context.Context, path string) (string, error) {
	identity, err := (SchemaProbe{}).Inspect(ctx, path)
	if err != nil {
		return "", err
	}
	if identity != IdentityForeign {
		return "", ErrNoSchemaConflict
	}

	directory, err := os.MkdirTemp(
		filepath.Dir(path),
		"schema-conflict-recovery-*",
	)
	if err != nil {
		return "", fmt.Errorf("create schema conflict recovery directory: %w", err)
	}
	if err := os.Chmod(directory, 0o700); err != nil {
		_ = os.Remove(directory)
		return "", fmt.Errorf("protect schema conflict recovery directory: %w", err)
	}

	sources := databaseFiles(path)
	moved := make([][2]string, 0, len(sources))
	for _, source := range sources {
		if _, err := os.Stat(source); errors.Is(err, os.ErrNotExist) {
			continue
		} else if err != nil {
			rollbackRelocation(moved)
			_ = os.Remove(directory)
			return "", fmt.Errorf("inspect schema conflict file: %w", err)
		}
		destination := filepath.Join(directory, filepath.Base(source))
		if err := os.Rename(source, destination); err != nil {
			rollbackRelocation(moved)
			_ = os.Remove(directory)
			return "", fmt.Errorf("relocate schema conflict file: %w", err)
		}
		moved = append(moved, [2]string{source, destination})
	}
	if len(moved) == 0 {
		_ = os.Remove(directory)
		return "", ErrNoSchemaConflict
	}
	return directory, nil
}

// SQL returns the WAL-backed read pool used by application queries. Product
// mutations are composed with Writer so they share one serialized connection.
func (d *Database) SQL() *sql.DB {
	return d.sql
}

// Writer returns the single-connection application writer lane.
func (d *Database) Writer() *sql.DB {
	return d.writer
}

func (d *Database) Path() string {
	return d.path
}

func (d *Database) Close() error {
	if d == nil {
		return nil
	}
	var closeErrors []error
	if d.writer != nil {
		closeErrors = append(closeErrors, d.writer.Close())
	}
	if d.sql != nil {
		closeErrors = append(closeErrors, d.sql.Close())
	}
	return errors.Join(closeErrors...)
}

func writableDSN(path string) string {
	query := url.Values{}
	query.Add("_pragma", "foreign_keys(1)")
	query.Add("_pragma", "journal_mode(WAL)")
	query.Add("_pragma", fmt.Sprintf("busy_timeout(%d)", busyTimeoutMilliseconds))
	return (&url.URL{Scheme: "file", Path: filepath.Clean(path), RawQuery: query.Encode()}).String()
}

func readOnlyDSN(path string) string {
	query := url.Values{}
	query.Set("mode", "ro")
	query.Add("_pragma", "query_only(1)")
	return (&url.URL{Scheme: "file", Path: filepath.Clean(path), RawQuery: query.Encode()}).String()
}

func pendingMigrations(
	ctx context.Context,
	connection *sql.DB,
	migrations fs.FS,
) ([]int, error) {
	versions, err := migrationVersions(migrations)
	if err != nil {
		return nil, err
	}
	rows, err := connection.QueryContext(
		ctx,
		"SELECT version FROM schema_migrations ORDER BY version",
	)
	if err != nil {
		return nil, fmt.Errorf("inspect migration state: %w", err)
	}
	defer rows.Close()

	applied := 0
	for rows.Next() {
		var version int
		if err := rows.Scan(&version); err != nil {
			return nil, fmt.Errorf("read migration state: %w", err)
		}
		if applied >= len(versions) || versions[applied] != version {
			return nil, fmt.Errorf(
				"%w: found version %d at position %d",
				ErrMigrationConflict,
				version,
				applied+1,
			)
		}
		applied++
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate migration state: %w", err)
	}
	return append([]int(nil), versions[applied:]...), nil
}

func migrationVersions(migrations fs.FS) ([]int, error) {
	entries, err := fs.ReadDir(migrations, "migrations")
	if err != nil {
		return nil, fmt.Errorf("read embedded migrations: %w", err)
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Name() < entries[j].Name() })

	versions := make([]int, 0, len(entries))
	seen := make(map[int]struct{}, len(entries))
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".sql") {
			continue
		}
		version, err := migrationVersion(entry.Name())
		if err != nil {
			return nil, err
		}
		if _, duplicate := seen[version]; duplicate {
			return nil, fmt.Errorf("%w: duplicate version %d", ErrInvalidMigration, version)
		}
		seen[version] = struct{}{}
		versions = append(versions, version)
	}
	return versions, nil
}

func createPreMigrationBackup(
	ctx context.Context,
	connection *sql.DB,
	path string,
	nextVersion int,
) (string, error) {
	pattern := fmt.Sprintf(
		"%s.pre-migration-v%03d-*.sqlite3",
		strings.TrimSuffix(filepath.Base(path), filepath.Ext(path)),
		nextVersion,
	)
	reservation, err := os.CreateTemp(filepath.Dir(path), pattern)
	if err != nil {
		return "", fmt.Errorf("reserve pre-migration backup: %w", err)
	}
	backupPath := reservation.Name()
	if err := reservation.Close(); err != nil {
		_ = os.Remove(backupPath)
		return "", fmt.Errorf("close pre-migration backup reservation: %w", err)
	}
	if err := os.Remove(backupPath); err != nil {
		return "", fmt.Errorf("prepare pre-migration backup path: %w", err)
	}
	if _, err := connection.ExecContext(
		ctx,
		"VACUUM INTO ?",
		backupPath,
	); err != nil {
		_ = os.Remove(backupPath)
		return "", fmt.Errorf("create pre-migration backup: %w", err)
	}
	if err := os.Chmod(backupPath, 0o600); err != nil {
		return "", fmt.Errorf("protect pre-migration backup: %w", err)
	}
	file, err := os.Open(backupPath)
	if err != nil {
		return "", fmt.Errorf("open pre-migration backup: %w", err)
	}
	syncErr := file.Sync()
	closeErr := file.Close()
	if err := errors.Join(syncErr, closeErr); err != nil {
		return "", fmt.Errorf("sync pre-migration backup: %w", err)
	}
	return backupPath, nil
}

func restoreDatabaseBackup(backupPath string, path string) error {
	source, err := os.Open(backupPath)
	if err != nil {
		return fmt.Errorf("open migration rollback backup: %w", err)
	}
	defer source.Close()

	temporary, err := os.CreateTemp(
		filepath.Dir(path),
		".librairii-migration-rollback-*",
	)
	if err != nil {
		return fmt.Errorf("create migration rollback file: %w", err)
	}
	temporaryPath := temporary.Name()
	cleanupTemporary := func() {
		_ = temporary.Close()
		_ = os.Remove(temporaryPath)
	}
	if err := temporary.Chmod(0o600); err != nil {
		cleanupTemporary()
		return fmt.Errorf("protect migration rollback file: %w", err)
	}
	if _, err := io.Copy(temporary, source); err != nil {
		cleanupTemporary()
		return fmt.Errorf("copy migration rollback backup: %w", err)
	}
	if err := temporary.Sync(); err != nil {
		cleanupTemporary()
		return fmt.Errorf("sync migration rollback file: %w", err)
	}
	if err := temporary.Close(); err != nil {
		_ = os.Remove(temporaryPath)
		return fmt.Errorf("close migration rollback file: %w", err)
	}

	failedPath := temporaryPath + ".failed-database"
	if err := os.Rename(path, failedPath); err != nil {
		_ = os.Remove(temporaryPath)
		return fmt.Errorf("preserve failed migration database: %w", err)
	}
	if err := removeDatabaseSidecars(path); err != nil {
		_ = os.Rename(failedPath, path)
		_ = os.Remove(temporaryPath)
		return err
	}
	if err := os.Rename(temporaryPath, path); err != nil {
		_ = os.Rename(failedPath, path)
		_ = os.Remove(temporaryPath)
		return fmt.Errorf("restore migration backup: %w", err)
	}
	if err := os.Remove(failedPath); err != nil {
		return fmt.Errorf("remove failed migration database: %w", err)
	}
	return nil
}

func removeDatabaseFiles(path string) error {
	var errs []error
	for _, candidate := range databaseFiles(path) {
		if err := os.Remove(candidate); err != nil && !errors.Is(err, os.ErrNotExist) {
			errs = append(errs, err)
		}
	}
	return errors.Join(errs...)
}

func removeDatabaseSidecars(path string) error {
	var errs []error
	for _, candidate := range databaseFiles(path)[1:] {
		if err := os.Remove(candidate); err != nil && !errors.Is(err, os.ErrNotExist) {
			errs = append(errs, err)
		}
	}
	if err := errors.Join(errs...); err != nil {
		return fmt.Errorf("remove migration database sidecars: %w", err)
	}
	return nil
}

func databaseFiles(path string) []string {
	return []string{path, path + "-wal", path + "-shm"}
}

func rollbackRelocation(moved [][2]string) {
	for index := len(moved) - 1; index >= 0; index-- {
		_ = os.Rename(moved[index][1], moved[index][0])
	}
}

func migrate(ctx context.Context, connection *sql.DB, migrations fs.FS) error {
	if _, err := migrationVersions(migrations); err != nil {
		return err
	}
	if _, err := connection.ExecContext(ctx, `
		CREATE TABLE IF NOT EXISTS schema_migrations (
			version INTEGER PRIMARY KEY,
			applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
		)
	`); err != nil {
		return fmt.Errorf("create schema migrations table: %w", err)
	}

	entries, err := fs.ReadDir(migrations, "migrations")
	if err != nil {
		return fmt.Errorf("read embedded migrations: %w", err)
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Name() < entries[j].Name() })

	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".sql") {
			continue
		}
		version, err := migrationVersion(entry.Name())
		if err != nil {
			return err
		}

		var applied int
		err = connection.QueryRowContext(
			ctx,
			"SELECT COUNT(*) FROM schema_migrations WHERE version = ?",
			version,
		).Scan(&applied)
		if err != nil {
			return fmt.Errorf("check migration %d: %w", version, err)
		}
		if applied == 1 {
			continue
		}

		body, err := fs.ReadFile(migrations, filepath.ToSlash(filepath.Join("migrations", entry.Name())))
		if err != nil {
			return fmt.Errorf("read migration %d: %w", version, err)
		}

		transaction, err := connection.BeginTx(ctx, nil)
		if err != nil {
			return fmt.Errorf("begin migration %d: %w", version, err)
		}
		if _, err := transaction.ExecContext(ctx, string(body)); err != nil {
			_ = transaction.Rollback()
			return fmt.Errorf("apply migration %d: %w", version, err)
		}
		if _, err := transaction.ExecContext(
			ctx,
			"INSERT INTO schema_migrations (version) VALUES (?)",
			version,
		); err != nil {
			_ = transaction.Rollback()
			return fmt.Errorf("record migration %d: %w", version, err)
		}
		if err := transaction.Commit(); err != nil {
			return fmt.Errorf("commit migration %d: %w", version, err)
		}
	}
	return nil
}

func migrationVersion(name string) (int, error) {
	prefix, _, ok := strings.Cut(name, "_")
	if !ok {
		return 0, fmt.Errorf("%w: %s", ErrInvalidMigration, name)
	}
	version, err := strconv.Atoi(prefix)
	if err != nil || version <= 0 {
		return 0, fmt.Errorf("%w: %s", ErrInvalidMigration, name)
	}
	return version, nil
}
