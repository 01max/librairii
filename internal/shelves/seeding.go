package shelves

import (
	"context"
	"database/sql"
)

// initialSeed is constructed by the server from reviewed Collection results or
// explicit manual selections. Clients cannot manufacture acceptance origins.
type initialSeed struct {
	UUID          string
	Manual        bool
	CollectionIDs []int64
}

func normalizeSeeds(seeds []initialSeed) ([]initialSeed, error) {
	result := make([]initialSeed, 0, len(seeds))
	positions := map[string]int{}
	for _, seed := range seeds {
		if _, err := uniqueUUIDs([]string{seed.UUID}); err != nil {
			return nil, err
		}
		if !seed.Manual && len(seed.CollectionIDs) == 0 {
			return nil, ErrInvalidInput
		}
		position, exists := positions[seed.UUID]
		if !exists {
			position = len(result)
			positions[seed.UUID] = position
			result = append(result, initialSeed{UUID: seed.UUID})
		}
		item := &result[position]
		item.Manual = item.Manual || seed.Manual
		seen := map[int64]bool{}
		for _, id := range item.CollectionIDs {
			seen[id] = true
		}
		for _, id := range seed.CollectionIDs {
			if id <= 0 {
				return nil, ErrInvalidInput
			}
			if !seen[id] {
				item.CollectionIDs = append(item.CollectionIDs, id)
				seen[id] = true
			}
		}
	}
	return result, nil
}

func (s *Service) CreateManual(ctx context.Context, name string, manualUUIDs []string) (Shelf, error) {
	seeds := make([]initialSeed, 0, len(manualUUIDs))
	for _, id := range manualUUIDs {
		seeds = append(seeds, initialSeed{UUID: id, Manual: true})
	}
	seeds, err := normalizeSeeds(seeds)
	if err != nil {
		return Shelf{}, err
	}
	uuids := make([]string, 0, len(seeds))
	for _, seed := range seeds {
		uuids = append(uuids, seed.UUID)
	}
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Shelf{}, err
	}
	defer tx.Rollback()
	titles, err := activeTitles(ctx, tx, uuids)
	if err != nil {
		return Shelf{}, err
	}
	if len(titles) != len(uuids) {
		return Shelf{}, ErrNotFound
	}
	id, err := createShelf(ctx, tx, name)
	if err != nil {
		return Shelf{}, err
	}
	if err := insertManualSeeds(ctx, tx, id, seeds, titles); err != nil {
		return Shelf{}, err
	}
	result, err := readShelf(ctx, tx, id)
	if err != nil {
		return Shelf{}, err
	}
	return result, tx.Commit()
}

func insertManualSeeds(ctx context.Context, tx *sql.Tx, id int64, seeds []initialSeed, titles map[string]string) error {
	for _, seed := range seeds {
		if !seed.Manual {
			return ErrInvalidInput
		}
		if _, err := addMember(ctx, tx, id, seed.UUID, titles[seed.UUID], true); err != nil {
			return err
		}
	}
	return nil
}
