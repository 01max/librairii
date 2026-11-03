package inspection

import (
	"context"
	"encoding/json"
	"image/png"
	"path"
	"strings"

	"github.com/01max/librairii/internal/catalog"
	"github.com/google/uuid"
)

const (
	studioStoryEntry     = "story.json"
	studioThumbnailEntry = "thumbnail.png"
)

type studioStory struct {
	UUID        string      `json:"uuid"`
	Title       string      `json:"title"`
	Description string      `json:"description"`
	StageNodes  []stageNode `json:"stageNodes"`
}

type stageNode struct {
	Image string `json:"image"`
	Audio string `json:"audio"`
}

func inspectStudio(
	ctx context.Context,
	archive archiveView,
	limits Limits,
	format catalog.ArchiveFormat,
) (Result, error) {
	storyBytes, err := archive.read(
		ctx,
		studioStoryEntry,
		limits.MaxMetadataBytes,
		CodeMetadataLimit,
	)
	if err != nil {
		return Result{}, err
	}

	var story studioStory
	decoder := json.NewDecoder(bytesReader(storyBytes))
	if err := decoder.Decode(&story); err != nil {
		return Result{}, &ValidationError{
			Code:  CodeMalformedMetadata,
			Entry: studioStoryEntry,
			Cause: err,
		}
	}
	if err := ensureJSONEnd(decoder); err != nil {
		return Result{}, &ValidationError{
			Code:  CodeMalformedMetadata,
			Entry: studioStoryEntry,
			Cause: err,
		}
	}

	storyUUID, err := uuid.Parse(story.UUID)
	if err != nil || storyUUID == uuid.Nil {
		return Result{}, &ValidationError{
			Code:  CodeInvalidUUID,
			Entry: studioStoryEntry,
			Cause: err,
		}
	}
	if err := validateStudioAssetReferences(archive, story.StageNodes, limits); err != nil {
		return Result{}, err
	}

	thumbnailBytes, err := archive.read(
		ctx,
		studioThumbnailEntry,
		limits.MaxArtworkBytes,
		CodeArtworkLimit,
	)
	if err != nil {
		return Result{}, err
	}
	if _, err := png.DecodeConfig(bytesReader(thumbnailBytes)); err != nil {
		return Result{}, &ValidationError{
			Code:  CodeArtworkLimit,
			Entry: studioThumbnailEntry,
			Cause: err,
		}
	}

	return Result{
		UUID:   storyUUID.String(),
		Format: format,
		Metadata: Metadata{
			Title:       strings.TrimSpace(story.Title),
			Description: strings.TrimSpace(story.Description),
			Artwork: &Artwork{
				EntryName: studioThumbnailEntry,
				MediaType: "image/png",
				Bytes:     thumbnailBytes,
			},
		},
	}, nil
}

func validateStudioAssetReferences(
	archive archiveView,
	nodes []stageNode,
	limits Limits,
) error {
	if len(nodes) == 0 {
		return &ValidationError{Code: CodeMissingAsset, Entry: "stageNodes"}
	}

	referenceCount := 0
	for _, node := range nodes {
		for _, reference := range []string{node.Image, node.Audio} {
			if reference == "" {
				continue
			}
			referenceCount++
			if err := validateStudioAssetReference(reference, limits); err != nil {
				return err
			}
			if !archive.has(reference) {
				return &ValidationError{Code: CodeMissingAsset, Entry: reference}
			}
		}
	}
	if referenceCount == 0 {
		return &ValidationError{Code: CodeMissingAsset, Entry: "stageNodes"}
	}
	return nil
}

func validateStudioAssetReference(reference string, limits Limits) error {
	if err := validateEntryPath(reference, limits); err != nil {
		return &ValidationError{Code: CodeUnsafePath, Entry: reference, Cause: err}
	}
	clean := path.Clean(reference)
	if !strings.HasPrefix(clean, "assets/") || clean == "assets" {
		return &ValidationError{Code: CodeUnsafePath, Entry: reference}
	}
	return nil
}
