package metadata

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/library"
)

const DefaultLocale = "en-GB"

var ErrInvalidMetadataLookup = errors.New("official metadata lookup is invalid")

type activeMetadataRepository interface {
	ActiveMetadataByUUIDs(
		context.Context,
		string,
		[]string,
	) (map[string]OfficialStoryMetadata, error)
}

type LibraryProvider struct {
	repository activeMetadataRepository
	locale     string
}

func NewLibraryProvider(
	repository activeMetadataRepository,
	locale string,
) (*LibraryProvider, error) {
	if repository == nil {
		return nil, ErrInvalidMetadataLookup
	}
	canonical, err := canonicalLocale(locale)
	if err != nil {
		return nil, ErrInvalidMetadataLookup
	}
	return &LibraryProvider{
		repository: repository,
		locale:     canonical,
	}, nil
}

func (p *LibraryProvider) FindByUUIDs(
	ctx context.Context,
	storyUUIDs []string,
) (map[string]library.OfficialMetadata, error) {
	if len(storyUUIDs) == 0 {
		return map[string]library.OfficialMetadata{}, nil
	}
	originalToCanonical := make(map[string]string, len(storyUUIDs))
	canonicalUUIDs := make([]string, 0, len(storyUUIDs))
	seen := make(map[string]struct{}, len(storyUUIDs))
	for _, original := range storyUUIDs {
		canonical, err := canonicalUUID(original)
		if err != nil {
			return nil, ErrInvalidMetadataLookup
		}
		originalToCanonical[original] = canonical
		if _, exists := seen[canonical]; exists {
			continue
		}
		seen[canonical] = struct{}{}
		canonicalUUIDs = append(canonicalUUIDs, canonical)
	}
	found, err := p.repository.ActiveMetadataByUUIDs(ctx, p.locale, canonicalUUIDs)
	if err != nil {
		return nil, err
	}
	result := make(map[string]library.OfficialMetadata, len(found))
	for original, canonical := range originalToCanonical {
		story, exists := found[canonical]
		if !exists {
			continue
		}
		result[original] = library.OfficialMetadata{
			UUID:            story.StoryUUID,
			Locale:          story.Locale,
			Title:           story.Title,
			Description:     story.Description,
			Author:          story.Author,
			Publisher:       story.Publisher,
			Language:        story.Language,
			DurationSeconds: story.DurationSeconds,
			MinimumAge:      story.MinimumAge,
			MaximumAge:      story.MaximumAge,
			ArtworkID:       story.ArtworkID,
			Provenance:      story.Provenance,
			SourceRecordID:  story.SourceRecordID,
			SourceUpdatedAt: story.SourceUpdatedAt,
			FetchedAt:       story.FetchedAt,
			ActivatedAt:     story.ActivatedAt,
		}
	}
	return result, nil
}

func (p *LibraryProvider) DisplayLocale() string {
	return p.locale
}

var _ library.OfficialProvider = (*LibraryProvider)(nil)
