package collections

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/searchtext"
)

const (
	maxCollectionNameRunes   = 80
	reorderCollectionPadding = 1
)

var (
	ErrInvalidCollectionName         = errors.New("collection name is invalid")
	ErrDuplicateCollectionName       = errors.New("collection name already exists")
	ErrCollectionNotFound            = errors.New("collection does not exist")
	ErrCollectionInUse               = errors.New("collection supplies shelves; review its detachment before deletion")
	ErrInvalidCollectionOrder        = errors.New("collection order is invalid")
	ErrCollectionNeedsAttention      = errors.New("collection needs attention")
	ErrCollectionCriteriaUnavailable = errors.New("collection criteria are unavailable")
)

type AttentionReason string

const (
	AttentionMissingCriteria   AttentionReason = "missing_criteria"
	AttentionUnmigratableQuery AttentionReason = "unmigratable_query"
)

type OpenedCollection struct {
	Collection Collection        `json:"collection"`
	Query      SavedLibraryQuery `json:"query"`
}

type Inspection struct {
	Collection      Collection
	Query           SavedLibraryQuery
	AttentionReason AttentionReason
}

type Service struct {
	database *sql.DB
}

func NewService(database *sql.DB) (*Service, error) {
	if database == nil {
		return nil, ErrMissingDatabase
	}
	return &Service{database: database}, nil
}

func (s *Service) Create(
	ctx context.Context,
	name string,
	query library.StoryLibraryQuery,
) (Collection, error) {
	name, normalizedName, err := normalizeCollectionName(name)
	if err != nil {
		return Collection{}, err
	}
	serialized, err := EncodeSavedLibraryQuery(query)
	if err != nil {
		return Collection{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Collection{}, fmt.Errorf("begin collection creation: %w", err)
	}
	defer transaction.Rollback()
	savedQuery, err := DecodeSavedLibraryQuery(serialized.Version, serialized.Payload)
	if err != nil {
		return Collection{}, err
	}
	if err := validateSavedCriteria(ctx, transaction, savedQuery); err != nil {
		return Collection{}, err
	}

	position, err := nextCollectionPosition(ctx, transaction)
	if err != nil {
		return Collection{}, err
	}
	collection, err := insertCollection(ctx, transaction, NewCollection{
		Name:           name,
		NormalizedName: normalizedName,
		Position:       position,
		QueryVersion:   serialized.Version,
		QueryPayload:   serialized.Payload,
	})
	if isDuplicateCollectionName(err) {
		return Collection{}, ErrDuplicateCollectionName
	}
	if err != nil {
		return Collection{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Collection{}, fmt.Errorf("commit collection creation: %w", err)
	}
	return collection, nil
}

func (s *Service) Open(ctx context.Context, collectionID int64) (OpenedCollection, error) {
	inspection, err := s.Inspect(ctx, collectionID)
	if err != nil {
		return OpenedCollection{}, err
	}
	if inspection.Collection.Validity != ValidityValid {
		return OpenedCollection{}, ErrCollectionNeedsAttention
	}
	return OpenedCollection{
		Collection: inspection.Collection,
		Query:      inspection.Query,
	}, nil
}

func (s *Service) Inspect(ctx context.Context, collectionID int64) (Inspection, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Inspection{}, fmt.Errorf("begin collection inspection: %w", err)
	}
	defer transaction.Rollback()

	collection, err := readCollection(ctx, transaction, collectionID)
	if errors.Is(err, sql.ErrNoRows) {
		return Inspection{}, ErrCollectionNotFound
	}
	if err != nil {
		return Inspection{}, fmt.Errorf("read collection for inspection: %w", err)
	}
	legacyID, err := library.ReadLegacyBrokenTagID(ctx, transaction)
	if err != nil {
		return Inspection{}, err
	}
	migrated, err := MigrateSavedLibraryQuery(
		collection.QueryVersion,
		collection.QueryPayload, legacyID,
	)
	if err != nil {
		return finishAttentionInspection(
			ctx,
			transaction,
			collection,
			AttentionUnmigratableQuery,
		)
	}
	query, err := DecodeSavedLibraryQuery(migrated.Version, migrated.Payload)
	if err != nil {
		return finishAttentionInspection(
			ctx,
			transaction,
			collection,
			AttentionUnmigratableQuery,
		)
	}
	if err := validateSavedCriteria(ctx, transaction, query); err != nil {
		if errors.Is(err, ErrCollectionCriteriaUnavailable) {
			return finishAttentionInspection(
				ctx,
				transaction,
				collection,
				AttentionMissingCriteria,
			)
		}
		return Inspection{}, err
	}
	if collection.QueryVersion != migrated.Version ||
		collection.QueryPayload != migrated.Payload {
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE collections
			 SET query_version = ?,
			     query_payload = ?,
			     updated_at = CURRENT_TIMESTAMP
			 WHERE id = ?`,
			migrated.Version,
			migrated.Payload,
			collection.ID,
		); err != nil {
			return Inspection{}, fmt.Errorf("migrate saved collection query: %w", err)
		}
		collection.Revision++
		collection.QueryVersion = migrated.Version
		collection.QueryPayload = migrated.Payload
	}
	if err := transaction.Commit(); err != nil {
		return Inspection{}, fmt.Errorf("commit collection inspection: %w", err)
	}
	if collection.Validity != ValidityValid {
		return Inspection{
			Collection:      collection,
			Query:           query,
			AttentionReason: AttentionMissingCriteria,
		}, nil
	}
	return Inspection{Collection: collection, Query: query}, nil
}

func (s *Service) Rename(
	ctx context.Context,
	collectionID int64,
	name string,
) (Collection, error) {
	name, normalizedName, err := normalizeCollectionName(name)
	if err != nil {
		return Collection{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Collection{}, fmt.Errorf("begin collection rename: %w", err)
	}
	defer transaction.Rollback()

	result, err := transaction.ExecContext(
		ctx,
		`UPDATE collections
		 SET name = ?,
		     normalized_name = ?,
		     updated_at = CURRENT_TIMESTAMP
		 WHERE id = ?`,
		name,
		normalizedName,
		collectionID,
	)
	if isDuplicateCollectionName(err) {
		return Collection{}, ErrDuplicateCollectionName
	}
	if err := requireCollectionMutation(result, err); err != nil {
		return Collection{}, err
	}
	collection, err := readCollection(ctx, transaction, collectionID)
	if err != nil {
		return Collection{}, fmt.Errorf("read renamed collection: %w", err)
	}
	if err := transaction.Commit(); err != nil {
		return Collection{}, fmt.Errorf("commit collection rename: %w", err)
	}
	return collection, nil
}

func (s *Service) Duplicate(
	ctx context.Context,
	collectionID int64,
	name string,
) (Collection, error) {
	name, normalizedName, err := normalizeCollectionName(name)
	if err != nil {
		return Collection{}, err
	}
	inspection, err := s.Inspect(ctx, collectionID)
	if err != nil {
		return Collection{}, err
	}
	if inspection.Collection.Validity != ValidityValid {
		return Collection{}, ErrCollectionNeedsAttention
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Collection{}, fmt.Errorf("begin collection duplication: %w", err)
	}
	defer transaction.Rollback()

	source, err := readCollection(ctx, transaction, collectionID)
	if errors.Is(err, sql.ErrNoRows) {
		return Collection{}, ErrCollectionNotFound
	}
	if err != nil {
		return Collection{}, fmt.Errorf("read collection to duplicate: %w", err)
	}
	if source.Validity != ValidityValid {
		return Collection{}, ErrCollectionNeedsAttention
	}
	legacyID, err := library.ReadLegacyBrokenTagID(ctx, transaction)
	if err != nil {
		return Collection{}, err
	}
	migrated, err := MigrateSavedLibraryQuery(
		source.QueryVersion,
		source.QueryPayload, legacyID,
	)
	if err != nil {
		return Collection{}, err
	}
	position, err := nextCollectionPosition(ctx, transaction)
	if err != nil {
		return Collection{}, err
	}
	duplicate, err := insertCollection(ctx, transaction, NewCollection{
		Name:           name,
		NormalizedName: normalizedName,
		Position:       position,
		QueryVersion:   migrated.Version,
		QueryPayload:   migrated.Payload,
		Validity:       source.Validity,
	})
	if isDuplicateCollectionName(err) {
		return Collection{}, ErrDuplicateCollectionName
	}
	if err != nil {
		return Collection{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Collection{}, fmt.Errorf("commit collection duplication: %w", err)
	}
	return duplicate, nil
}

func (s *Service) ReplaceQuery(
	ctx context.Context,
	collectionID int64,
	query library.StoryLibraryQuery,
) (Collection, error) {
	serialized, err := EncodeSavedLibraryQuery(query)
	if err != nil {
		return Collection{}, err
	}
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Collection{}, fmt.Errorf("begin collection query replacement: %w", err)
	}
	defer transaction.Rollback()
	savedQuery, err := DecodeSavedLibraryQuery(serialized.Version, serialized.Payload)
	if err != nil {
		return Collection{}, err
	}
	if err := validateSavedCriteria(ctx, transaction, savedQuery); err != nil {
		return Collection{}, err
	}

	result, err := transaction.ExecContext(
		ctx,
		`UPDATE collections
		 SET query_version = ?,
		     query_payload = ?,
		     validity_state = 'valid',
		     updated_at = CURRENT_TIMESTAMP
		 WHERE id = ?`,
		serialized.Version,
		serialized.Payload,
		collectionID,
	)
	if err := requireCollectionMutation(result, err); err != nil {
		return Collection{}, err
	}
	collection, err := readCollection(ctx, transaction, collectionID)
	if err != nil {
		return Collection{}, fmt.Errorf("read updated collection: %w", err)
	}
	if err := transaction.Commit(); err != nil {
		return Collection{}, fmt.Errorf("commit collection query replacement: %w", err)
	}
	return collection, nil
}

func (s *Service) Reorder(
	ctx context.Context,
	orderedIDs []int64,
) ([]Collection, error) {
	transaction, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin collection reorder: %w", err)
	}
	defer transaction.Rollback()

	current, err := listCollections(ctx, transaction)
	if err != nil {
		return nil, err
	}
	if !sameCollectionIDs(current, orderedIDs) {
		return nil, ErrInvalidCollectionOrder
	}
	if len(orderedIDs) > 0 {
		maxPosition := current[len(current)-1].Position
		offset := maxPosition + len(orderedIDs) + reorderCollectionPadding
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE collections
			 SET position = position + ?,
			     updated_at = CURRENT_TIMESTAMP`,
			offset,
		); err != nil {
			return nil, fmt.Errorf("stage collection reorder: %w", err)
		}
		for position, collectionID := range orderedIDs {
			result, err := transaction.ExecContext(
				ctx,
				`UPDATE collections
				 SET position = ?,
				     updated_at = CURRENT_TIMESTAMP
				 WHERE id = ?`,
				position,
				collectionID,
			)
			if err := requireCollectionMutation(result, err); err != nil {
				return nil, fmt.Errorf("position collection: %w", err)
			}
		}
	}
	reordered, err := listCollections(ctx, transaction)
	if err != nil {
		return nil, err
	}
	if err := transaction.Commit(); err != nil {
		return nil, fmt.Errorf("commit collection reorder: %w", err)
	}
	return reordered, nil
}

func (s *Service) Delete(ctx context.Context, collectionID int64) error {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	var active int
	if err := tx.QueryRowContext(ctx, `SELECT count(*) FROM shelf_sources WHERE collection_id=? AND active=1`, collectionID).Scan(&active); err != nil {
		return err
	}
	if active > 0 {
		return ErrCollectionInUse
	}
	if _, err := tx.ExecContext(ctx, `UPDATE shelf_sources SET collection_id=NULL WHERE collection_id=? AND active=0`, collectionID); err != nil {
		return err
	}
	result, err := tx.ExecContext(
		ctx,
		"DELETE FROM collections WHERE id = ?",
		collectionID,
	)
	if err := requireCollectionMutation(result, err); err != nil {
		return err
	}
	return tx.Commit()
}

func (s *Service) List(ctx context.Context) ([]Collection, error) {
	return listCollections(ctx, s.database)
}

func finishAttentionInspection(
	ctx context.Context,
	transaction *sql.Tx,
	collection Collection,
	reason AttentionReason,
) (Inspection, error) {
	if collection.Validity == ValidityValid {
		result, err := transaction.ExecContext(
			ctx,
			`UPDATE collections
			 SET validity_state = 'needs_attention',
			     updated_at = CURRENT_TIMESTAMP
			 WHERE id = ? AND validity_state = 'valid'`,
			collection.ID,
		)
		if err := requireCollectionMutation(result, err); err != nil {
			return Inspection{}, fmt.Errorf("mark collection as needing attention: %w", err)
		}
		collection.Revision++
		collection.Validity = ValidityNeedsAttention
	}
	if err := transaction.Commit(); err != nil {
		return Inspection{}, fmt.Errorf("commit collection attention state: %w", err)
	}
	return Inspection{Collection: collection, AttentionReason: reason}, nil
}

func normalizeCollectionName(value string) (string, string, error) {
	if !utf8.ValidString(value) {
		return "", "", ErrInvalidCollectionName
	}
	for _, character := range value {
		if unicode.IsControl(character) && !unicode.IsSpace(character) {
			return "", "", ErrInvalidCollectionName
		}
	}
	name := strings.Join(strings.Fields(value), " ")
	normalized := searchtext.Normalize(name)
	if name == "" ||
		normalized == "" ||
		utf8.RuneCountInString(name) > maxCollectionNameRunes ||
		utf8.RuneCountInString(normalized) > maxCollectionNameRunes {
		return "", "", ErrInvalidCollectionName
	}
	return name, normalized, nil
}

func nextCollectionPosition(ctx context.Context, queryer collectionQueryer) (int, error) {
	var position int
	if err := queryer.QueryRowContext(
		ctx,
		"SELECT COALESCE(MAX(position), -1) + 1 FROM collections",
	).Scan(&position); err != nil {
		return 0, fmt.Errorf("read next collection position: %w", err)
	}
	return position, nil
}

func requireCollectionMutation(result sql.Result, err error) error {
	if err != nil {
		return err
	}
	affected, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if affected != 1 {
		return ErrCollectionNotFound
	}
	return nil
}

func sameCollectionIDs(collections []Collection, orderedIDs []int64) bool {
	if len(collections) != len(orderedIDs) {
		return false
	}
	seen := make(map[int64]struct{}, len(orderedIDs))
	for _, collection := range collections {
		seen[collection.ID] = struct{}{}
	}
	for _, collectionID := range orderedIDs {
		if _, exists := seen[collectionID]; !exists {
			return false
		}
		delete(seen, collectionID)
	}
	return len(seen) == 0
}
