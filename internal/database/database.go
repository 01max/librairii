package database

import (
	"context"
	"database/sql"
	"embed"
	"errors"
	"fmt"
	"io/fs"
	"net/url"
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
	ErrForeignSchema    = errors.New("database does not contain the Librairii Wails schema identity")
	ErrInvalidMigration = errors.New("invalid embedded migration")

	//go:embed migrations/*.sql
	migrationFiles embed.FS
)

type Database struct {
	sql  *sql.DB
	path string
}

func Open(ctx context.Context, path string) (*Database, error) {
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
		_ = connection.Close()
		return nil, openErr
	}

	if err := connection.PingContext(ctx); err != nil {
		return closeOnError(fmt.Errorf("ping SQLite database: %w", err))
	}
	if err := migrate(ctx, connection, migrationFiles); err != nil {
		return closeOnError(err)
	}

	return &Database{sql: connection, path: filepath.Clean(path)}, nil
}

func (d *Database) SQL() *sql.DB {
	return d.sql
}

func (d *Database) Path() string {
	return d.path
}

func (d *Database) Close() error {
	if d == nil || d.sql == nil {
		return nil
	}
	return d.sql.Close()
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

func migrate(ctx context.Context, connection *sql.DB, migrations fs.FS) error {
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
