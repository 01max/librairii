package shelves

import (
	"context"
	"testing"
)

func TestInitialSeedsDeduplicateAndProtectManualOverlaps(t *testing.T) {
	id := "123e4567-e89b-42d3-a456-426614174000"
	seeds, err := normalizeSeeds([]initialSeed{{UUID: id, CollectionIDs: []int64{2, 3}}, {UUID: id, Manual: true}, {UUID: id, CollectionIDs: []int64{3, 4}}})
	if err != nil || len(seeds) != 1 || !seeds[0].Manual || len(seeds[0].CollectionIDs) != 3 {
		t.Fatalf("overlap lost: %#v %v", seeds, err)
	}
	s, db := newShelfService(t)
	seedStory(t, db, id, "Chosen")
	shelf, err := s.CreateManual(context.Background(), "Chosen", []string{id, id})
	if err != nil || shelf.Count != 1 {
		t.Fatalf("seed: %#v %v", shelf, err)
	}
	members, err := s.Members(context.Background(), shelf.ID)
	if err != nil || len(members) != 1 || !members[0].Protected {
		t.Fatalf("seed not protected: %#v %v", members, err)
	}
	if _, err := s.CreateManual(context.Background(), "Invalid", []string{"223e4567-e89b-42d3-a456-426614174000"}); err == nil {
		t.Fatal("missing seed accepted")
	}
	listed, err := s.List(context.Background())
	if err != nil || len(listed) != 1 {
		t.Fatalf("failed creation left a shelf: %#v %v", listed, err)
	}
}
