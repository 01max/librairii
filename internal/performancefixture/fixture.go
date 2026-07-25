// Package performancefixture generates deterministic, copyright-free library
// data for collection-scale measurements.
package performancefixture

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/searchtext"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
)

const (
	MinimumLargeLibraryStories = 5_000
	BrokenDefinitionID         = int64(1)
	MoodDefinitionID           = int64(2)
	CalmValueID                = int64(1)
	SyntheticArtworkVariants   = 24
	SyntheticArtworkWidth      = 320
	SyntheticArtworkHeight     = 400
)

type Fixture struct {
	StoryCount           int
	ShelfIDs             []int64
	EmbeddedArtworkPaths []string
}

func Generate(
	ctx context.Context,
	database *sql.DB,
	layout storage.Layout,
	storyCount int,
) (Fixture, error) {
	if database == nil ||
		layout.Root == "" ||
		!filepath.IsAbs(layout.Root) ||
		storyCount < 1 {
		return Fixture{}, errors.New("performance fixture configuration is invalid")
	}
	artworkPaths, err := writeSyntheticArtwork(layout)
	if err != nil {
		return Fixture{}, err
	}
	transaction, err := database.BeginTx(ctx, nil)
	if err != nil {
		return Fixture{}, fmt.Errorf("begin performance fixture: %w", err)
	}
	defer transaction.Rollback()
	if err := seedTags(ctx, transaction); err != nil {
		return Fixture{}, err
	}
	if err := seedCatalog(ctx, transaction, storyCount); err != nil {
		return Fixture{}, err
	}

	storyStatement, err := transaction.PrepareContext(
		ctx,
		`INSERT INTO stories (
			id,
			uuid,
			embedded_title,
			embedded_description,
			embedded_artwork_path,
			display_name_normalized,
			created_at,
			updated_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
	)
	if err != nil {
		return Fixture{}, err
	}
	defer storyStatement.Close()
	archiveStatement, err := transaction.PrepareContext(
		ctx,
		`INSERT INTO story_archives (
			id,
			story_id,
			original_filename,
			detected_format,
			sha256,
			byte_size,
			managed_path,
			validation_state,
			created_at
		) VALUES (?, ?, ?, 'zip', ?, ?, ?, ?, ?)`,
	)
	if err != nil {
		return Fixture{}, err
	}
	defer archiveStatement.Close()
	metadataStatement, err := transaction.PrepareContext(
		ctx,
		`INSERT INTO official_story_metadata (
			snapshot_id,
			story_uuid,
			locale,
			title,
			title_normalized,
			description,
			author,
			publisher,
			language,
			duration_seconds,
			minimum_age,
			maximum_age,
			provenance,
			source_record_id
		) VALUES (
			1, ?, 'en-GB', ?, ?, ?, 'Librairii Fixture',
			'Synthetic Press', ?, ?, ?, ?, 'lunii_catalog', ?
		)`,
	)
	if err != nil {
		return Fixture{}, err
	}
	defer metadataStatement.Close()
	assignmentStatement, err := transaction.PrepareContext(
		ctx,
		`INSERT INTO story_tag_assignments (
			story_id,
			definition_id,
			value_id,
			source
		) VALUES (?, ?, ?, 'manual')`,
	)
	if err != nil {
		return Fixture{}, err
	}
	defer assignmentStatement.Close()

	for index := 1; index <= storyCount; index++ {
		title := syntheticTitle(index)
		description := fmt.Sprintf(
			"Copyright-free synthetic performance story %05d.",
			index,
		)
		storyUUID := fmt.Sprintf(
			"00000000-0000-4000-8000-%012x",
			index,
		)
		createdAt := time.Date(
			2026,
			time.January,
			1,
			0,
			0,
			index%60,
			index,
			time.UTC,
		).Format(time.RFC3339Nano)
		if _, err := storyStatement.ExecContext(
			ctx,
			index,
			storyUUID,
			title,
			description,
			artworkPaths[(index-1)%len(artworkPaths)],
			searchtext.Normalize(title),
			createdAt,
			createdAt,
		); err != nil {
			return Fixture{}, fmt.Errorf("insert performance story %d: %w", index, err)
		}
		checksum := fmt.Sprintf("%064x", index)
		validation := "valid"
		if index%10 == 0 {
			validation = "missing"
		} else if index%10 == 1 {
			validation = "invalid"
		}
		filename := fmt.Sprintf("synthetic-story-%05d.zip", index)
		if _, err := archiveStatement.ExecContext(
			ctx,
			index,
			index,
			filename,
			checksum,
			1024+index%4096,
			"archives/"+checksum+"/"+filename,
			validation,
			createdAt,
		); err != nil {
			return Fixture{}, fmt.Errorf(
				"insert performance archive %d: %w",
				index,
				err,
			)
		}
		language := "en-GB"
		if index%2 == 0 {
			language = "fr-FR"
		}
		minimumAge := 3 + index%8
		if _, err := metadataStatement.ExecContext(
			ctx,
			storyUUID,
			title,
			searchtext.Normalize(title),
			description,
			language,
			300+index%1800,
			minimumAge,
			minimumAge+3,
			fmt.Sprintf("synthetic-%05d", index),
		); err != nil {
			return Fixture{}, fmt.Errorf(
				"insert performance metadata %d: %w",
				index,
				err,
			)
		}
		if index%7 == 0 {
			if _, err := assignmentStatement.ExecContext(
				ctx,
				index,
				BrokenDefinitionID,
				nil,
			); err != nil {
				return Fixture{}, err
			}
		}
		if index%3 == 0 {
			if _, err := assignmentStatement.ExecContext(
				ctx,
				index,
				MoodDefinitionID,
				CalmValueID,
			); err != nil {
				return Fixture{}, err
			}
		}
	}
	if _, err := transaction.ExecContext(
		ctx,
		`UPDATE catalog_snapshots
		 SET status = 'active', activated_at = ?
		 WHERE id = 1`,
		"2026-01-01T00:00:00Z",
	); err != nil {
		return Fixture{}, err
	}
	if _, err := transaction.ExecContext(
		ctx,
		`UPDATE catalog_syncs
		 SET status = 'succeeded',
		     matched_story_count = ?,
		     finished_at = ?
		 WHERE id = '10000000-0000-4000-8000-000000000000'`,
		storyCount,
		"2026-01-01T00:00:00Z",
	); err != nil {
		return Fixture{}, err
	}
	shelfIDs, err := seedShelves(ctx, transaction)
	if err != nil {
		return Fixture{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Fixture{}, fmt.Errorf("commit performance fixture: %w", err)
	}
	if _, err := database.ExecContext(ctx, "ANALYZE"); err != nil {
		return Fixture{}, fmt.Errorf("analyze performance fixture: %w", err)
	}
	return Fixture{
		StoryCount:           storyCount,
		ShelfIDs:             shelfIDs,
		EmbeddedArtworkPaths: artworkPaths,
	}, nil
}

func seedTags(ctx context.Context, transaction *sql.Tx) error {
	if _, err := transaction.ExecContext(
		ctx,
		`INSERT INTO tag_definitions (
			id, key, normalized_key, label, color, kind, source,
			presentation, position, is_protected
		) VALUES
			(1, 'broken', 'broken', 'Broken', '#FF705C', 'boolean',
			 'builtin', 'warning', 0, 1),
			(2, 'mood', 'mood', 'Mood', '#405CF5', 'choice',
			 'user', 'default', 0, 0);
		 INSERT INTO tag_values (
			id, definition_id, key, normalized_key, label, position
		 ) VALUES (1, 2, 'calm', 'calm', 'Calm', 0)`,
	); err != nil {
		return fmt.Errorf("seed performance tags: %w", err)
	}
	return nil
}

func seedCatalog(
	ctx context.Context,
	transaction *sql.Tx,
	storyCount int,
) error {
	if _, err := transaction.ExecContext(
		ctx,
		`INSERT INTO catalog_syncs (
			id, locale, status, started_at
		) VALUES (
			'10000000-0000-4000-8000-000000000000',
			'en-GB',
			'running',
			'2026-01-01T00:00:00Z'
		);
		INSERT INTO catalog_snapshots (
			id,
			sync_id,
			locale,
			raw_path,
			raw_sha256,
			byte_size,
			record_count,
			status,
			fetched_at
		) VALUES (
			1,
			'10000000-0000-4000-8000-000000000000',
			'en-GB',
			'catalog/performance/catalog.json',
			?,
			?,
			?,
			'staged',
			'2026-01-01T00:00:00Z'
		)`,
		strings.Repeat("f", 64),
		storyCount*256,
		storyCount,
	); err != nil {
		return fmt.Errorf("seed performance catalog: %w", err)
	}
	return nil
}

func seedShelves(
	ctx context.Context,
	transaction *sql.Tx,
) ([]int64, error) {
	queries := []struct {
		name  string
		query library.StoryLibraryQuery
	}{
		{name: "Moon stories", query: library.StoryLibraryQuery{Name: "moon"}},
		{name: "English stories", query: library.StoryLibraryQuery{
			Languages: []string{"en-GB"},
		}},
		{name: "Compatible stories", query: library.StoryLibraryQuery{
			Compatibilities: []library.Compatibility{
				library.CompatibilityCompatible,
			},
		}},
		{name: "Broken stories", query: library.StoryLibraryQuery{
			BooleanFilters: []library.BooleanFilter{{
				DefinitionID: BrokenDefinitionID,
				State:        library.BooleanTrue,
			}},
		}},
		{name: "Calm stories", query: library.StoryLibraryQuery{
			ChoiceFilters: []library.ChoiceFilter{{
				DefinitionID: MoodDefinitionID,
				ValueIDs:     []int64{CalmValueID},
			}},
		}},
		{name: "Combined stories", query: CombinedQuery()},
	}
	statement, err := transaction.PrepareContext(
		ctx,
		`INSERT INTO shelves (
			id,
			name,
			normalized_name,
			position,
			query_version,
			query_payload,
			validity_state
		) VALUES (?, ?, ?, ?, ?, ?, 'valid')`,
	)
	if err != nil {
		return nil, err
	}
	defer statement.Close()
	ids := make([]int64, 0, len(queries))
	for index, candidate := range queries {
		encoded, err := shelves.EncodeSavedLibraryQuery(candidate.query)
		if err != nil {
			return nil, err
		}
		id := int64(index + 1)
		if _, err := statement.ExecContext(
			ctx,
			id,
			candidate.name,
			searchtext.Normalize(candidate.name),
			index,
			encoded.Version,
			encoded.Payload,
		); err != nil {
			return nil, err
		}
		ids = append(ids, id)
	}
	return ids, nil
}

func CombinedQuery() library.StoryLibraryQuery {
	return library.StoryLibraryQuery{
		Name:      "forest",
		Languages: []string{"en-GB"},
		Compatibilities: []library.Compatibility{
			library.CompatibilityCompatible,
		},
		BooleanFilters: []library.BooleanFilter{{
			DefinitionID: BrokenDefinitionID,
			State:        library.BooleanFalse,
		}},
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: MoodDefinitionID,
			ValueIDs:     []int64{CalmValueID},
		}},
	}
}

func syntheticTitle(index int) string {
	adjectives := []string{
		"Amber",
		"Clockwork",
		"Curious",
		"Kind",
		"Luminous",
		"Quiet",
		"Silver",
		"Starlit",
	}
	places := []string{
		"Forest",
		"Garden",
		"Harbor",
		"Moon",
		"Observatory",
		"River",
		"Train",
		"Workshop",
	}
	return fmt.Sprintf(
		"Synthetic %s %s %05d",
		adjectives[index%len(adjectives)],
		places[(index/len(adjectives))%len(places)],
		index,
	)
}

func writeSyntheticArtwork(layout storage.Layout) ([]string, error) {
	directory := filepath.Join(layout.Catalog, "embedded")
	if err := os.MkdirAll(directory, storage.DirectoryMode); err != nil {
		return nil, err
	}
	paths := make([]string, 0, SyntheticArtworkVariants)
	for variant := range SyntheticArtworkVariants {
		var output bytes.Buffer
		picture := image.NewNRGBA(image.Rect(
			0,
			0,
			SyntheticArtworkWidth,
			SyntheticArtworkHeight,
		))
		for y := range SyntheticArtworkHeight {
			for x := range SyntheticArtworkWidth {
				band := (x/12 + y/15 + variant*3) % 17
				picture.SetNRGBA(x, y, color.NRGBA{
					R: uint8((band*31 + x/3 + variant*19) % 256),
					G: uint8((band*17 + y/2 + variant*29) % 256),
					B: uint8((x/5 + y/7 + variant*43) % 256),
					A: 255,
				})
			}
		}
		if err := png.Encode(&output, picture); err != nil {
			return nil, err
		}
		path := filepath.Join(
			directory,
			fmt.Sprintf("performance-cover-%02d.png", variant+1),
		)
		if err := os.WriteFile(path, output.Bytes(), 0o600); err != nil {
			return nil, err
		}
		relative, err := filepath.Rel(layout.Root, path)
		if err != nil {
			return nil, err
		}
		paths = append(paths, filepath.ToSlash(relative))
	}
	return paths, nil
}
