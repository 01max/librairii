package diagnostics

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/storage"
)

func TestLoggerRotatesWithinFileAndByteBounds(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, time.July, 25, 12, 0, 0, 0, time.UTC)
	logger, err := NewLogger(
		layout.Logs,
		Policy{MaxFiles: 3, MaxBytes: 160},
		func() time.Time {
			now = now.Add(time.Millisecond)
			return now
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = logger.Close()
	})
	for index := range 12 {
		if err := logger.Record(
			LevelInfo,
			EventOperationChanged,
			"import:running:"+string(rune('a'+index)),
		); err != nil {
			t.Fatal(err)
		}
	}

	entries, err := os.ReadDir(layout.Logs)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 3 {
		t.Fatalf("retained log files = %d, want 3", len(entries))
	}
	for _, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			t.Fatal(err)
		}
		if info.Size() > 160 {
			t.Fatalf("%s size = %d, want <= 160", entry.Name(), info.Size())
		}
		if info.Mode().Perm() != 0o600 {
			t.Fatalf("%s permissions = %o", entry.Name(), info.Mode().Perm())
		}
	}
	snapshots, err := logger.snapshots()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshots) != 3 {
		t.Fatalf("log snapshots = %#v", snapshots)
	}
}

func TestExportContainsOnlyAggregateFactsAndValidatedLogs(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = opened.Close()
	})
	const (
		privateTitle = "PRIVATE_STORY_TITLE"
		archiveToken = "ARCHIVE_BYTES_SECRET"
		artworkToken = "ARTWORK_BYTES_SECRET"
		apiToken     = "API_TOKEN_SECRET"
	)
	if _, err := opened.SQL().Exec(
		`INSERT INTO stories (uuid, embedded_title, display_name_normalized)
		 VALUES (?, ?, ?)`,
		"123e4567-e89b-42d3-a456-426614174000",
		privateTitle,
		"private story title",
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(layout.Archives, "private.zip"),
		[]byte(archiveToken),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(layout.Catalog, "private-artwork.png"),
		[]byte(artworkToken),
		0o600,
	); err != nil {
		t.Fatal(err)
	}

	now := time.Date(2026, time.July, 25, 12, 30, 0, 0, time.UTC)
	logger, err := NewLogger(layout.Logs, DefaultPolicy(), func() time.Time {
		return now
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = logger.Close()
	})
	if err := logger.Record(
		LevelInfo,
		EventRuntimeStarted,
		"ready",
	); err != nil {
		t.Fatal(err)
	}
	malicious, err := os.OpenFile(
		filepath.Join(layout.Logs, currentLogName),
		os.O_APPEND|os.O_WRONLY,
		0,
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := malicious.WriteString(
		`{"time":"2026-07-25T12:30:00Z","level":"info","event":"runtime_started","state":"ready","token":"` +
			apiToken + `","path":"` + layout.Root + "\"}\n",
	); err != nil {
		t.Fatal(err)
	}
	if err := malicious.Close(); err != nil {
		t.Fatal(err)
	}

	service, err := NewService(layout, opened.SQL(), logger)
	if err != nil {
		t.Fatal(err)
	}
	service.now = func() time.Time { return now }
	service.newID = func() string {
		return "00000000-0000-4000-8000-000000000703"
	}
	destination := t.TempDir()
	report, err := service.Export(context.Background(), destination)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(report.FileName, layout.Root) || report.ByteSize <= 0 {
		t.Fatalf("Export() = %#v", report)
	}

	reader, err := zip.OpenReader(filepath.Join(destination, report.FileName))
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()
	var exported bytes.Buffer
	var manifest Manifest
	for _, file := range reader.File {
		if file.Name != "diagnostics.json" &&
			!strings.HasPrefix(file.Name, "logs/events") {
			t.Fatalf("unexpected diagnostic entry = %q", file.Name)
		}
		source, err := file.Open()
		if err != nil {
			t.Fatal(err)
		}
		payload, err := io.ReadAll(source)
		closeErr := source.Close()
		if err != nil || closeErr != nil {
			t.Fatalf("read %s = %v, close = %v", file.Name, err, closeErr)
		}
		exported.Write(payload)
		if file.Name == "diagnostics.json" {
			if err := json.Unmarshal(payload, &manifest); err != nil {
				t.Fatal(err)
			}
		}
	}
	if manifest.FormatVersion != formatVersion ||
		manifest.SchemaVersion != 12 ||
		manifest.StoryCount != 1 ||
		manifest.ShelfCount != 0 ||
		manifest.OperationCount != 0 {
		t.Fatalf("manifest = %#v", manifest)
	}
	for _, forbidden := range []string{
		privateTitle,
		archiveToken,
		artworkToken,
		apiToken,
		layout.Root,
	} {
		if bytes.Contains(exported.Bytes(), []byte(forbidden)) {
			t.Fatalf("diagnostic export contains forbidden value %q", forbidden)
		}
	}
	if !bytes.Contains(exported.Bytes(), []byte(EventRuntimeStarted)) {
		t.Fatal("diagnostic export omitted validated lifecycle log")
	}
}
