package operations

import (
	"context"
	"errors"
	"fmt"
	"path/filepath"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
)

func TestHistoryPersistsAndPaginatesWithoutRepeatingNewerOperations(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	filename := filepath.Join(t.TempDir(), "history.sqlite3")
	db, err := database.Open(ctx, filename)
	if err != nil {
		t.Fatal(err)
	}
	repository := NewRepository(db.SQL())
	now := time.Date(2026, time.October, 19, 9, 0, 0, 0, time.UTC)
	for index := 1; index <= 22; index++ {
		id := fmt.Sprintf("00000000-0000-4000-8000-%012d", index)
		created, err := repository.CreateImport(ctx, id, []NewItem{{SourceName: "story.zip"}}, now)
		if err != nil {
			t.Fatal(err)
		}
		if index == 22 {
			if err := repository.CompleteItem(ctx, id, ItemSnapshot{
				ID: created.Items[0].ID, Status: ItemFailed,
				OutcomeMessage: "Unsupported archive.",
			}); err != nil {
				t.Fatal(err)
			}
			if err := repository.Finish(ctx, id, StatusFailed, "invalid_archive", "Unsupported archive.", now); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	reopened, err := database.Open(ctx, filename)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	repository = NewRepository(reopened.SQL())
	first, err := repository.History(ctx, "")
	if err != nil {
		t.Fatal(err)
	}
	if len(first.Operations) != 20 || first.Operations[0].Status != StatusFailed ||
		first.Operations[0].Items[0].OutcomeMessage != "Unsupported archive." ||
		first.NextBefore != "00000000-0000-4000-8000-000000000003" {
		t.Fatalf("first history page = %#v", first)
	}
	// An insert between pages must not shift or repeat the remaining history.
	if _, err := repository.CreateMetadataSync(ctx, "00000000-0000-4000-8000-000000000023", "en-GB", now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	second, err := repository.History(ctx, first.NextBefore)
	if err != nil {
		t.Fatal(err)
	}
	if len(second.Operations) != 2 || second.NextBefore != "" ||
		second.Operations[0].ID != "00000000-0000-4000-8000-000000000002" ||
		second.Operations[1].ID != "00000000-0000-4000-8000-000000000001" {
		t.Fatalf("second history page = %#v", second)
	}
	if _, err := repository.History(ctx, "missing"); !errors.Is(err, ErrInvalidRequest) {
		t.Fatalf("History(invalid cursor) error = %v", err)
	}
}
