package library

import (
	"context"
	"errors"
	"strconv"
	"testing"
)

func TestBrokenFlagsAreAtomicIdempotentAndExactlyReversible(t *testing.T) {
	ctx := context.Background()
	_, repo, db := newLibraryQuery(t, nil)
	first := createQueryableStory(t, repo, "00112233-4455-4677-8899-aabbccddeeff", "First", "a")
	second := createQueryableStory(t, repo, "11112222-3333-4444-8555-666677778888", "Second", "b")
	service := NewBrokenFlags(db)
	assertFlag := func(id int64, want bool) {
		t.Helper()
		var got bool
		if err := db.QueryRow("SELECT is_broken FROM stories WHERE id=?", id).Scan(&got); err != nil {
			t.Fatal(err)
		}
		if got != want {
			t.Fatalf("flag %d=%t, want %t", id, got, want)
		}
	}
	if _, err := service.Set(ctx, []int64{first.ID}, true); err != nil {
		t.Fatal(err)
	}
	result, err := service.Set(ctx, []int64{first.ID, second.ID, second.ID}, true)
	if err != nil || result.RequestedStories != 2 || result.ChangedStories != 1 || result.Changes[0].StoryID != second.ID || result.Changes[0].Previous {
		t.Fatalf("result=%#v, %v", result, err)
	}
	undone, err := service.Restore(ctx, result.Changes)
	if err != nil || undone.ChangedStories != 1 {
		t.Fatalf("restore=%#v, %v", undone, err)
	}
	assertFlag(first.ID, true)
	assertFlag(second.ID, false)
	noop, err := service.Set(ctx, []int64{first.ID}, true)
	if err != nil || noop.ChangedStories != 0 || len(noop.Changes) != 0 {
		t.Fatalf("noop=%#v, %v", noop, err)
	}
	for _, ids := range [][]int64{nil, {0}, {-1}} {
		if _, err := service.Set(ctx, ids, true); !errors.Is(err, ErrInvalidFlagRequest) {
			t.Fatalf("ids=%v: %v", ids, err)
		}
	}
	if _, err := service.Set(ctx, []int64{first.ID, 999}, false); !errors.Is(err, ErrFlagStoryUnavailable) {
		t.Fatalf("unavailable: %v", err)
	}
	assertFlag(first.ID, true)
	result, err = service.Set(ctx, []int64{first.ID, second.ID}, false)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Set(ctx, []int64{first.ID}, true); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Restore(ctx, result.Changes); !errors.Is(err, ErrFlagRestoreStale) {
		t.Fatalf("stale undo=%v", err)
	}
	assertFlag(first.ID, true)
	assertFlag(second.ID, false)
	result, err = service.Set(ctx, []int64{first.ID, second.ID}, false)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec("DELETE FROM stories WHERE id=?", first.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Restore(ctx, result.Changes); !errors.Is(err, ErrFlagStoryUnavailable) {
		t.Fatalf("removed undo=%v", err)
	}
	assertFlag(second.ID, false)
}

func TestBrokenFlagWriteFailureRollsBackWholeBatch(t *testing.T) {
	ctx := context.Background()
	_, repo, db := newLibraryQuery(t, nil)
	first := createQueryableStory(t, repo, "00112233-4455-4677-8899-aabbccddeeff", "First", "a")
	second := createQueryableStory(t, repo, "11112222-3333-4444-8555-666677778888", "Second", "b")
	if _, err := db.Exec(`CREATE TRIGGER fail_second_flag BEFORE UPDATE OF is_broken ON stories WHEN NEW.id=` + strconv.FormatInt(second.ID, 10) + ` BEGIN SELECT RAISE(ABORT,'fixture failure'); END`); err != nil {
		t.Fatal(err)
	}
	if _, err := NewBrokenFlags(db).Set(ctx, []int64{first.ID, second.ID}, true); err == nil {
		t.Fatal("expected write failure")
	}
	var count int
	if err := db.QueryRow("SELECT count(*) FROM stories WHERE is_broken=1").Scan(&count); err != nil {
		t.Fatal(err)
	}
	if count != 0 {
		t.Fatalf("partially committed %d flags", count)
	}
}
