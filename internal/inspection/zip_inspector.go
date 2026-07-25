package inspection

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"image/png"
	"io"
	"path"
	"strings"

	"github.com/01max/librairii/internal/catalog"
	"github.com/google/uuid"
)

const (
	plainUUIDEntry    = "uuid.bin"
	embeddedMetadata  = "_metadata.json"
	embeddedArtwork   = "_thumbnail.png"
	maxUUIDEntryBytes = 16
)

type ZIPInspector struct{}

func NewZIPInspector() *ZIPInspector {
	return &ZIPInspector{}
}

func (i *ZIPInspector) Inspect(
	ctx context.Context,
	candidate Candidate,
	limits Limits,
) (Result, error) {
	if !isZIPFamilyFilename(candidate.OriginalFilename) {
		return Result{}, &ValidationError{Code: CodeUnsupportedFormat}
	}
	archive, err := openValidatedZIP(ctx, candidate, limits)
	if err != nil {
		return Result{}, err
	}
	defer archive.Close()

	if isPlainFilename(candidate.OriginalFilename) || archive.has(plainUUIDEntry) {
		return inspectPlain(ctx, archive, limits)
	}
	return inspectLuniiZIP(ctx, archive, candidate.OriginalFilename, limits)
}

func inspectPlain(ctx context.Context, archive *validatedZIP, limits Limits) (Result, error) {
	for _, name := range []string{"uuid.bin", "ni", "li.plain", "ri.plain", "si.plain"} {
		if !archive.has(name) {
			return Result{}, &ValidationError{Code: CodeMissingEntry, Entry: name}
		}
	}
	if !archive.hasFileBelow("rf") {
		return Result{}, &ValidationError{Code: CodeMissingEntry, Entry: "rf/"}
	}
	if !archive.hasFileBelow("sf") {
		return Result{}, &ValidationError{Code: CodeMissingEntry, Entry: "sf/"}
	}

	uuidBytes, err := archive.read(ctx, plainUUIDEntry, maxUUIDEntryBytes, CodeInvalidUUID)
	if err != nil {
		return Result{}, err
	}
	storyUUID, err := uuid.FromBytes(uuidBytes)
	if err != nil {
		return Result{}, &ValidationError{Code: CodeInvalidUUID, Entry: plainUUIDEntry, Cause: err}
	}
	if storyUUID == uuid.Nil {
		return Result{}, &ValidationError{Code: CodeInvalidUUID, Entry: plainUUIDEntry}
	}
	result := Result{UUID: storyUUID.String(), Format: catalog.FormatPlainPK}
	if err := readEmbeddedMetadata(ctx, archive, limits, &result); err != nil {
		return Result{}, err
	}
	return result, nil
}

func inspectLuniiZIP(
	ctx context.Context,
	archive *validatedZIP,
	filename string,
	limits Limits,
) (Result, error) {
	roots := make(map[string]string)
	for name, file := range archive.entries {
		if file.FileInfo().IsDir() {
			continue
		}
		root, _, found := strings.Cut(name, "/")
		if !found {
			continue
		}
		storyUUID, err := parsePackUUID(root)
		if err == nil {
			roots[root] = storyUUID
		}
	}
	if len(roots) == 0 {
		return Result{}, &ValidationError{Code: CodeInvalidUUID}
	}
	if len(roots) != 1 {
		return Result{}, &ValidationError{Code: CodeAmbiguousStructure}
	}

	var root string
	var storyUUID string
	for root, storyUUID = range roots {
	}
	for _, name := range []string{"ni", "li", "ri", "si"} {
		entry := path.Join(root, name)
		if !archive.has(entry) {
			return Result{}, &ValidationError{Code: CodeMissingEntry, Entry: entry}
		}
	}
	if !archive.hasFileBelow(path.Join(root, "rf")) {
		return Result{}, &ValidationError{Code: CodeMissingEntry, Entry: path.Join(root, "rf") + "/"}
	}
	if !archive.hasFileBelow(path.Join(root, "sf")) {
		return Result{}, &ValidationError{Code: CodeMissingEntry, Entry: path.Join(root, "sf") + "/"}
	}

	format, err := luniiZIPFormat(filename)
	if err != nil {
		return Result{}, err
	}
	result := Result{UUID: storyUUID, Format: format}
	if err := readEmbeddedMetadata(ctx, archive, limits, &result); err != nil {
		return Result{}, err
	}
	return result, nil
}

func readEmbeddedMetadata(
	ctx context.Context,
	archive *validatedZIP,
	limits Limits,
	result *Result,
) error {
	if archive.has(embeddedMetadata) {
		bytes, err := archive.read(
			ctx,
			embeddedMetadata,
			limits.MaxMetadataBytes,
			CodeMetadataLimit,
		)
		if err != nil {
			return err
		}
		var metadata struct {
			UUID        string `json:"uuid"`
			Title       string `json:"title"`
			Description string `json:"description"`
		}
		decoder := json.NewDecoder(bytesReader(bytes))
		if err := decoder.Decode(&metadata); err != nil {
			return &ValidationError{
				Code:  CodeMalformedMetadata,
				Entry: embeddedMetadata,
				Cause: err,
			}
		}
		if err := ensureJSONEnd(decoder); err != nil {
			return &ValidationError{
				Code:  CodeMalformedMetadata,
				Entry: embeddedMetadata,
				Cause: err,
			}
		}
		if metadata.UUID != "" {
			metadataUUID, err := uuid.Parse(metadata.UUID)
			if err != nil || metadataUUID.String() != result.UUID {
				return &ValidationError{Code: CodeInvalidUUID, Entry: embeddedMetadata, Cause: err}
			}
		}
		result.Metadata.Title = strings.TrimSpace(metadata.Title)
		result.Metadata.Description = strings.TrimSpace(metadata.Description)
	}

	if archive.has(embeddedArtwork) {
		bytes, err := archive.read(
			ctx,
			embeddedArtwork,
			limits.MaxArtworkBytes,
			CodeArtworkLimit,
		)
		if err != nil {
			return err
		}
		if _, err := png.DecodeConfig(bytesReader(bytes)); err != nil {
			return &ValidationError{Code: CodeArtworkLimit, Entry: embeddedArtwork, Cause: err}
		}
		result.Metadata.Artwork = &Artwork{
			EntryName: embeddedArtwork,
			MediaType: "image/png",
			Bytes:     bytes,
		}
	}
	return nil
}

func parsePackUUID(value string) (string, error) {
	parsed, err := uuid.Parse(value)
	if err != nil {
		return "", err
	}
	if parsed == uuid.Nil {
		return "", fmt.Errorf("nil UUID is not a story identifier")
	}
	return parsed.String(), nil
}

func luniiZIPFormat(filename string) (catalog.ArchiveFormat, error) {
	lowerName := strings.ToLower(filename)
	switch {
	case strings.HasSuffix(lowerName, ".v1.pk"):
		return catalog.FormatV1PK, nil
	case strings.HasSuffix(lowerName, ".v2.pk"):
		return catalog.FormatV2PK, nil
	case strings.HasSuffix(lowerName, ".plain.pk"):
		return "", &ValidationError{Code: CodeAmbiguousStructure}
	case strings.HasSuffix(lowerName, ".pk"):
		return catalog.FormatGenericPK, nil
	case strings.HasSuffix(lowerName, ".zip"):
		return catalog.FormatZIP, nil
	default:
		return "", &ValidationError{Code: CodeUnsupportedFormat}
	}
}

func isZIPFamilyFilename(filename string) bool {
	lowerName := strings.ToLower(filename)
	return strings.HasSuffix(lowerName, ".pk") || strings.HasSuffix(lowerName, ".zip")
}

func isPlainFilename(filename string) bool {
	return strings.HasSuffix(strings.ToLower(filename), ".plain.pk")
}

func ensureJSONEnd(decoder *json.Decoder) error {
	var trailing any
	if err := decoder.Decode(&trailing); err == nil {
		return fmt.Errorf("metadata contains multiple JSON values")
	} else if !errors.Is(err, io.EOF) {
		return err
	}
	return nil
}

func bytesReader(value []byte) *bytes.Reader {
	return bytes.NewReader(value)
}
