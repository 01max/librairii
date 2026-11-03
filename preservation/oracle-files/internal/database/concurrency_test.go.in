package database

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"
	"time"
)

func TestApplicationWriterLaneSerializesWritesWhileWALReadsRemainLive(
	t *testing.T,
) {
	t.Parallel()

	ctx := context.Background()
	opened, err := Open(ctx, filepath.Join(t.TempDir(), "librairii.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := opened.Close(); err != nil {
			t.Error(err)
		}
	})
	readPool := opened.SQL()
	writer := opened.Writer()
	if _, err := writer.ExecContext(
		ctx,
		`CREATE TABLE concurrency_probe (
			id INTEGER PRIMARY KEY,
			value TEXT NOT NULL
		)`,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := writer.ExecContext(
		ctx,
		"INSERT INTO concurrency_probe (value) VALUES ('before')",
	); err != nil {
		t.Fatal(err)
	}

	snapshot, err := readPool.BeginTx(
		ctx,
		&sql.TxOptions{ReadOnly: true},
	)
	if err != nil {
		t.Fatal(err)
	}
	defer snapshot.Rollback()
	if got := probeCount(t, snapshot); got != 1 {
		t.Fatalf("snapshot count before writer = %d, want 1", got)
	}
	if _, err := writer.ExecContext(
		ctx,
		"INSERT INTO concurrency_probe (value) VALUES ('during snapshot')",
	); err != nil {
		t.Fatalf("WAL writer while snapshot is open: %v", err)
	}
	if got := probeCount(t, snapshot); got != 1 {
		t.Fatalf("snapshot count after writer = %d, want stable 1", got)
	}
	if got := probeCount(t, readPool); got != 2 {
		t.Fatalf("current count after writer = %d, want 2", got)
	}
	if err := snapshot.Rollback(); err != nil {
		t.Fatal(err)
	}

	locker, err := writer.Conn(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer locker.Close()
	if _, err := locker.ExecContext(ctx, "BEGIN IMMEDIATE"); err != nil {
		t.Fatal(err)
	}
	lockActive := true
	defer func() {
		if lockActive {
			_, _ = locker.ExecContext(context.Background(), "ROLLBACK")
		}
	}()
	if _, err := locker.ExecContext(
		ctx,
		"INSERT INTO concurrency_probe (value) VALUES ('first writer')",
	); err != nil {
		t.Fatal(err)
	}

	waitingWriter := make(chan error, 1)
	go func() {
		_, writeErr := writer.ExecContext(
			ctx,
			"INSERT INTO concurrency_probe (value) VALUES ('waiting writer')",
		)
		waitingWriter <- writeErr
	}()
	select {
	case err := <-waitingWriter:
		t.Fatalf("contending writer did not wait for the writer lane: %v", err)
	case <-time.After(75 * time.Millisecond):
	}
	if _, err := locker.ExecContext(ctx, "COMMIT"); err != nil {
		t.Fatal(err)
	}
	lockActive = false
	if err := locker.Close(); err != nil {
		t.Fatal(err)
	}
	select {
	case err := <-waitingWriter:
		if err != nil {
			t.Fatalf("busy-timeout writer failed: %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("contending writer did not resume after the short transaction")
	}
	if got := probeCount(t, readPool); got != 4 {
		t.Fatalf("final count = %d, want 4", got)
	}
	if got := readPool.Stats().MaxOpenConnections; got != maxOpenConnections {
		t.Fatalf("MaxOpenConnections = %d, want %d", got, maxOpenConnections)
	}
	if got := writer.Stats().MaxOpenConnections; got != 1 {
		t.Fatalf("writer lane MaxOpenConnections = %d, want 1", got)
	}
}

type countQueryer interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func probeCount(t *testing.T, queryer countQueryer) int {
	t.Helper()

	var count int
	if err := queryer.QueryRowContext(
		context.Background(),
		"SELECT COUNT(*) FROM concurrency_probe",
	).Scan(&count); err != nil {
		t.Fatal(err)
	}
	return count
}
