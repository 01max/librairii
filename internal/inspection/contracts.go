package inspection

import (
	"context"
	"errors"
	"fmt"
	"io"

	"github.com/01max/librairii/internal/catalog"
)

type Candidate struct {
	Path             string
	OriginalFilename string
}

type Metadata struct {
	Title       string
	Description string
	Artwork     *Artwork
}

type Artwork struct {
	EntryName string
	MediaType string
	Bytes     []byte
}

type Result struct {
	UUID     string
	Format   catalog.ArchiveFormat
	Metadata Metadata
}

// Inspector validates one archive family without extracting it to the filesystem.
type Inspector interface {
	Inspect(context.Context, Candidate, Limits) (Result, error)
}

type ErrorCode string

const (
	CodeInvalidLimits         ErrorCode = "invalid_limits"
	CodeInvalidContainer      ErrorCode = "invalid_container"
	CodeUnsupportedFormat     ErrorCode = "unsupported_format"
	CodeAmbiguousStructure    ErrorCode = "ambiguous_structure"
	CodeUnsafePath            ErrorCode = "unsafe_path"
	CodeLinkEntry             ErrorCode = "link_entry"
	CodeEntryLimit            ErrorCode = "entry_limit"
	CodeExpandedSizeLimit     ErrorCode = "expanded_size_limit"
	CodeCompressionRatioLimit ErrorCode = "compression_ratio_limit"
	CodeMetadataLimit         ErrorCode = "metadata_limit"
	CodeArtworkLimit          ErrorCode = "artwork_limit"
	CodeMissingEntry          ErrorCode = "missing_entry"
	CodeMissingAsset          ErrorCode = "missing_asset"
	CodeInvalidUUID           ErrorCode = "invalid_uuid"
	CodeMalformedMetadata     ErrorCode = "malformed_metadata"
)

type ValidationError struct {
	Code  ErrorCode
	Entry string
	Cause error
}

func (e *ValidationError) Error() string {
	switch {
	case e.Entry != "" && e.Cause != nil:
		return fmt.Sprintf("%s validation failed for %q: %v", e.Code, e.Entry, e.Cause)
	case e.Entry != "":
		return fmt.Sprintf("%s validation failed for %q", e.Code, e.Entry)
	case e.Cause != nil:
		return fmt.Sprintf("%s validation failed: %v", e.Code, e.Cause)
	default:
		return fmt.Sprintf("%s validation failed", e.Code)
	}
}

func (e *ValidationError) Unwrap() error {
	return e.Cause
}

func ErrorHasCode(err error, code ErrorCode) bool {
	var validationError *ValidationError
	return errors.As(err, &validationError) && validationError.Code == code
}

func ReadLimited(reader io.Reader, limit int64, code ErrorCode, entry string) ([]byte, error) {
	if limit <= 0 {
		return nil, &ValidationError{Code: CodeInvalidLimits}
	}

	bytes, err := io.ReadAll(io.LimitReader(reader, limit+1))
	if err != nil {
		return nil, &ValidationError{Code: code, Entry: entry, Cause: err}
	}
	if int64(len(bytes)) > limit {
		return nil, &ValidationError{Code: code, Entry: entry}
	}
	return bytes, nil
}
