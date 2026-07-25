package metadata

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/01max/librairii/internal/tagging"
)

const (
	AgeDefinitionKey      = "age"
	AgeDefinitionLabel    = "Age"
	AgeDefinitionColor    = "#FF705C"
	AgeDefinitionPosition = 0
)

var (
	ErrInvalidProjectionConfig = errors.New("catalog projection configuration is invalid")
	ErrDerivedFacetDrift       = errors.New("derived facet identity drifted")
)

type AgeBand struct {
	Key     string
	Label   string
	Minimum int
	Maximum int
}

type CatalogProjectionConfig struct {
	AgeBands []AgeBand
}

func DefaultCatalogProjectionConfig() CatalogProjectionConfig {
	return CatalogProjectionConfig{
		AgeBands: []AgeBand{
			{
				Key:     "3-5",
				Label:   "3–5 years",
				Minimum: 3,
				Maximum: 5,
			},
			{
				Key:     "6-8",
				Label:   "6–8 years",
				Minimum: 6,
				Maximum: 8,
			},
		},
	}
}

type ProjectionResult struct {
	DefinitionID    int64
	MatchedStories  int
	AssignedStories int
	Unassigned      int
}

type CatalogProjector struct {
	database *sql.DB
	config   CatalogProjectionConfig
}

func NewCatalogProjector(
	database *sql.DB,
	config CatalogProjectionConfig,
) (*CatalogProjector, error) {
	if database == nil {
		return nil, ErrInvalidProjectionConfig
	}
	config, err := validateProjectionConfig(config)
	if err != nil {
		return nil, err
	}
	return &CatalogProjector{
		database: database,
		config:   config,
	}, nil
}

func (p *CatalogProjector) Rebuild(
	ctx context.Context,
	snapshotID int64,
) (ProjectionResult, error) {
	if snapshotID <= 0 {
		return ProjectionResult{}, ErrInvalidProjectionConfig
	}
	transaction, err := p.database.BeginTx(ctx, nil)
	if err != nil {
		return ProjectionResult{}, fmt.Errorf("begin catalog projection: %w", err)
	}
	defer transaction.Rollback()

	result, err := projectCatalogSnapshot(ctx, transaction, snapshotID, p.config)
	if err != nil {
		return ProjectionResult{}, err
	}
	if err := transaction.Commit(); err != nil {
		return ProjectionResult{}, fmt.Errorf("commit catalog projection: %w", err)
	}
	return result, nil
}

func (p *CatalogProjector) EnsureFacets(ctx context.Context) (int64, error) {
	transaction, err := p.database.BeginTx(ctx, nil)
	if err != nil {
		return 0, fmt.Errorf("begin derived facet seed: %w", err)
	}
	defer transaction.Rollback()

	definitionID, _, err := ensureAgeFacet(ctx, transaction, p.config)
	if err != nil {
		return 0, err
	}
	if err := transaction.Commit(); err != nil {
		return 0, fmt.Errorf("commit derived facet seed: %w", err)
	}
	return definitionID, nil
}

func SeedDefaultDerivedFacets(ctx context.Context, database *sql.DB) error {
	projector, err := NewCatalogProjector(
		database,
		DefaultCatalogProjectionConfig(),
	)
	if err != nil {
		return err
	}
	_, err = projector.EnsureFacets(ctx)
	return err
}

func projectCatalogSnapshot(
	ctx context.Context,
	transaction *sql.Tx,
	snapshotID int64,
	config CatalogProjectionConfig,
) (ProjectionResult, error) {
	var snapshotStatus SnapshotStatus
	if err := transaction.QueryRowContext(
		ctx,
		"SELECT status FROM catalog_snapshots WHERE id = ?",
		snapshotID,
	).Scan(&snapshotStatus); err != nil {
		return ProjectionResult{}, fmt.Errorf("read projection snapshot: %w", err)
	}
	definitionID, values, err := ensureAgeFacet(ctx, transaction, config)
	if err != nil {
		return ProjectionResult{}, err
	}
	if _, err := transaction.ExecContext(
		ctx,
		`DELETE FROM story_tag_assignments
		 WHERE definition_id = ? AND source = 'derived'`,
		definitionID,
	); err != nil {
		return ProjectionResult{}, fmt.Errorf("clear derived age assignments: %w", err)
	}

	rows, err := transaction.QueryContext(
		ctx,
		`SELECT
			stories.id,
			official_story_metadata.minimum_age,
			official_story_metadata.maximum_age
		 FROM official_story_metadata
		 JOIN stories
		   ON stories.uuid = official_story_metadata.story_uuid
		 WHERE official_story_metadata.snapshot_id = ?
		 ORDER BY stories.id`,
		snapshotID,
	)
	if err != nil {
		return ProjectionResult{}, fmt.Errorf("load age projection inputs: %w", err)
	}
	defer rows.Close()

	result := ProjectionResult{DefinitionID: definitionID}
	for rows.Next() {
		var storyID int64
		var minimum sql.NullInt64
		var maximum sql.NullInt64
		if err := rows.Scan(&storyID, &minimum, &maximum); err != nil {
			return ProjectionResult{}, fmt.Errorf("scan age projection input: %w", err)
		}
		result.MatchedStories++
		bandIndex, found := matchingAgeBand(minimum, maximum, config.AgeBands)
		if !found {
			result.Unassigned++
			continue
		}
		if _, err := transaction.ExecContext(
			ctx,
			`INSERT INTO story_tag_assignments (
				story_id,
				definition_id,
				value_id,
				source
			 ) VALUES (?, ?, ?, 'derived')`,
			storyID,
			definitionID,
			values[bandIndex],
		); err != nil {
			return ProjectionResult{}, fmt.Errorf("assign derived age value: %w", err)
		}
		result.AssignedStories++
	}
	if err := rows.Err(); err != nil {
		return ProjectionResult{}, fmt.Errorf("iterate age projection inputs: %w", err)
	}
	return result, nil
}

func ensureAgeFacet(
	ctx context.Context,
	transaction *sql.Tx,
	config CatalogProjectionConfig,
) (int64, []int64, error) {
	if _, err := transaction.ExecContext(
		ctx,
		`INSERT INTO tag_definitions (
			key,
			normalized_key,
			label,
			color,
			kind,
			source,
			presentation,
			position,
			is_protected
		 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)
		 ON CONFLICT(normalized_key) DO NOTHING`,
		AgeDefinitionKey,
		AgeDefinitionKey,
		AgeDefinitionLabel,
		AgeDefinitionColor,
		tagging.KindChoice,
		tagging.SourceDerived,
		tagging.PresentationSystem,
		AgeDefinitionPosition,
	); err != nil {
		return 0, nil, fmt.Errorf("seed derived age definition: %w", err)
	}

	var definitionID int64
	var key string
	var normalizedKey string
	var label string
	var color string
	var kind tagging.Kind
	var source tagging.Source
	var presentation tagging.Presentation
	var position int
	var protected int
	if err := transaction.QueryRowContext(
		ctx,
		`SELECT
			id,
			key,
			normalized_key,
			label,
			color,
			kind,
			source,
			presentation,
			position,
			is_protected
		 FROM tag_definitions
		 WHERE normalized_key = ? COLLATE NOCASE`,
		AgeDefinitionKey,
	).Scan(
		&definitionID,
		&key,
		&normalizedKey,
		&label,
		&color,
		&kind,
		&source,
		&presentation,
		&position,
		&protected,
	); err != nil {
		return 0, nil, fmt.Errorf("read derived age definition: %w", err)
	}
	if key != AgeDefinitionKey ||
		normalizedKey != AgeDefinitionKey ||
		label != AgeDefinitionLabel ||
		color != AgeDefinitionColor ||
		kind != tagging.KindChoice ||
		source != tagging.SourceDerived ||
		presentation != tagging.PresentationSystem ||
		position != AgeDefinitionPosition ||
		protected != 1 {
		return 0, nil, ErrDerivedFacetDrift
	}

	valueIDs := make([]int64, len(config.AgeBands))
	for index, band := range config.AgeBands {
		if _, err := transaction.ExecContext(
			ctx,
			`INSERT INTO tag_values (
				definition_id,
				key,
				normalized_key,
				label,
				position
			 ) VALUES (?, ?, ?, ?, ?)
			 ON CONFLICT(definition_id, normalized_key) DO NOTHING`,
			definitionID,
			band.Key,
			band.Key,
			band.Label,
			index,
		); err != nil {
			return 0, nil, fmt.Errorf("seed derived age value %s: %w", band.Key, err)
		}
		var key string
		var normalizedKey string
		var label string
		var position int
		if err := transaction.QueryRowContext(
			ctx,
			`SELECT id, key, normalized_key, label, position
			 FROM tag_values
			 WHERE definition_id = ? AND normalized_key = ? COLLATE NOCASE`,
			definitionID,
			band.Key,
		).Scan(
			&valueIDs[index],
			&key,
			&normalizedKey,
			&label,
			&position,
		); err != nil {
			return 0, nil, fmt.Errorf("read derived age value %s: %w", band.Key, err)
		}
		if key != band.Key ||
			normalizedKey != band.Key ||
			label != band.Label ||
			position != index {
			return 0, nil, ErrDerivedFacetDrift
		}
	}
	var valueCount int
	if err := transaction.QueryRowContext(
		ctx,
		"SELECT COUNT(*) FROM tag_values WHERE definition_id = ?",
		definitionID,
	).Scan(&valueCount); err != nil {
		return 0, nil, fmt.Errorf("count derived age values: %w", err)
	}
	if valueCount != len(config.AgeBands) {
		return 0, nil, ErrDerivedFacetDrift
	}
	return definitionID, valueIDs, nil
}

func matchingAgeBand(
	minimum sql.NullInt64,
	maximum sql.NullInt64,
	bands []AgeBand,
) (int, bool) {
	if !minimum.Valid || !maximum.Valid || minimum.Int64 > maximum.Int64 {
		return 0, false
	}
	match := -1
	for index, band := range bands {
		if minimum.Int64 >= int64(band.Minimum) &&
			maximum.Int64 <= int64(band.Maximum) {
			if match != -1 {
				return 0, false
			}
			match = index
		}
	}
	return match, match != -1
}

func validateProjectionConfig(
	config CatalogProjectionConfig,
) (CatalogProjectionConfig, error) {
	if len(config.AgeBands) == 0 || len(config.AgeBands) > 32 {
		return CatalogProjectionConfig{}, ErrInvalidProjectionConfig
	}
	normalized := CatalogProjectionConfig{
		AgeBands: append([]AgeBand(nil), config.AgeBands...),
	}
	keys := make(map[string]struct{}, len(normalized.AgeBands))
	for index, band := range normalized.AgeBands {
		if band.Key == "" ||
			band.Key != strings.TrimSpace(band.Key) ||
			band.Label == "" ||
			band.Label != strings.TrimSpace(band.Label) ||
			utf8.RuneCountInString(band.Key) > 64 ||
			utf8.RuneCountInString(band.Label) > 80 ||
			band.Minimum < 0 ||
			band.Maximum < band.Minimum {
			return CatalogProjectionConfig{}, ErrInvalidProjectionConfig
		}
		for _, character := range band.Key {
			if (character < 'a' || character > 'z') &&
				(character < '0' || character > '9') &&
				character != '-' {
				return CatalogProjectionConfig{}, ErrInvalidProjectionConfig
			}
		}
		if _, exists := keys[band.Key]; exists {
			return CatalogProjectionConfig{}, ErrInvalidProjectionConfig
		}
		keys[band.Key] = struct{}{}
		for otherIndex := 0; otherIndex < index; otherIndex++ {
			other := normalized.AgeBands[otherIndex]
			if band.Minimum <= other.Maximum && other.Minimum <= band.Maximum {
				return CatalogProjectionConfig{}, ErrInvalidProjectionConfig
			}
		}
	}
	return normalized, nil
}
