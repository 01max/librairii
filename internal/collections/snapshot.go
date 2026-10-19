package collections

import (
	"context"
	"database/sql"

	"github.com/01max/librairii/internal/library"
)

// InspectInTransaction validates a source without migrating or mutating it.
// Shelf review can therefore evaluate and apply against one writer snapshot.
func InspectInTransaction(ctx context.Context, tx *sql.Tx, id int64) (OpenedCollection, error) {
	collection, err := readCollection(ctx, tx, id)
	if err != nil {
		return OpenedCollection{}, err
	}
	result := OpenedCollection{Collection: collection}
	if collection.Validity != ValidityValid {
		return result, ErrCollectionNeedsAttention
	}
	legacyID, err := library.ReadLegacyBrokenTagID(ctx, tx)
	if err != nil {
		return result, err
	}
	query, err := DecodeSavedLibraryQuery(collection.QueryVersion, collection.QueryPayload, legacyID)
	if err != nil {
		return result, err
	}
	if err := validateSavedCriteria(ctx, tx, query); err != nil {
		return result, err
	}
	result.Query = query
	return result, nil
}
