package exporter

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
)

type PreflightDisposition string

const (
	DispositionReady      PreflightDisposition = "ready"
	DispositionSkipped    PreflightDisposition = "skipped"
	DispositionConflicted PreflightDisposition = "conflicted"
)

type PreflightIssueCode string

const (
	IssueShelfNeedsAttention    PreflightIssueCode = "shelf_needs_attention"
	IssueEmptyScope             PreflightIssueCode = "empty_scope"
	IssueNoExportableStories    PreflightIssueCode = "no_exportable_stories"
	IssueDestinationInvalid     PreflightIssueCode = "destination_invalid"
	IssueDestinationNotWritable PreflightIssueCode = "destination_not_writable"
	IssueArchiveMissing         PreflightIssueCode = "archive_missing"
	IssueArchiveInvalid         PreflightIssueCode = "archive_invalid"
	IssueArchiveChanged         PreflightIssueCode = "archive_changed"
	IssueUnsupportedExtension   PreflightIssueCode = "unsupported_extension"
	IssueFilenameConflict       PreflightIssueCode = "filename_conflict"
)

type PreflightIssue struct {
	Code    PreflightIssueCode `json:"code"`
	Message string             `json:"message"`
	Blocks  bool               `json:"blocks"`
}

type PreflightItem struct {
	StoryID        int64                `json:"storyId"`
	StoryUUID      string               `json:"storyUuid"`
	StoryTitle     string               `json:"storyTitle"`
	OutputName     string               `json:"outputName"`
	DetectedFormat string               `json:"detectedFormat"`
	ByteSize       int64                `json:"byteSize"`
	Disposition    PreflightDisposition `json:"disposition"`
	Issue          *PreflightIssue      `json:"issue,omitempty"`
}

type PreflightReport struct {
	Source           operations.ExportSource `json:"source"`
	Destination      string                  `json:"-"`
	DestinationLabel string                  `json:"destination"`
	ResolvedCount    int                     `json:"resolvedCount"`
	ReadyCount       int                     `json:"readyCount"`
	TotalBytes       int64                   `json:"totalBytes"`
	DetectedFormats  []string                `json:"detectedFormats"`
	CollapsedOverlap int                     `json:"collapsedOverlap"`
	Items            []PreflightItem         `json:"items"`
	Issues           []PreflightIssue        `json:"issues"`
	Partial          bool                    `json:"partial"`
	Blocked          bool                    `json:"blocked"`
	CanExport        bool                    `json:"canExport"`
	Scope            Scope                   `json:"-"`
}

type PreflightRequest struct {
	SourceType operations.ExportSourceType `json:"sourceType"`
	StoryIDs   []int64                     `json:"storyIds,omitempty"`
	Query      library.StoryLibraryQuery   `json:"query"`
	ShelfIDs   []int64                     `json:"shelfIds,omitempty"`
}

type PreflightService struct {
	resolver      *Resolver
	layout        storage.Layout
	probeWritable func(string) error
}

func NewPreflightService(
	resolver *Resolver,
	layout storage.Layout,
) (*PreflightService, error) {
	if resolver == nil ||
		layout.Root == "" ||
		layout.Archives == "" ||
		!filepath.IsAbs(layout.Root) ||
		!filepath.IsAbs(layout.Archives) {
		return nil, ErrInvalidScope
	}
	return &PreflightService{
		resolver:      resolver,
		layout:        layout,
		probeWritable: probeExportDestination,
	}, nil
}

func (s *PreflightService) Plan(
	ctx context.Context,
	request PreflightRequest,
	destination string,
) (PreflightReport, error) {
	scope, err := s.resolve(ctx, request)
	if errors.Is(err, shelves.ErrShelfNeedsAttention) ||
		errors.Is(err, shelves.ErrShelfCriteriaUnavailable) {
		issue := blockingIssue(
			IssueShelfNeedsAttention,
			"A selected shelf needs attention before it can be exported.",
		)
		return PreflightReport{
			Source:  sourceFromRequest(request),
			Issues:  []PreflightIssue{issue},
			Blocked: true,
		}, nil
	}
	if err != nil {
		return PreflightReport{}, err
	}
	return s.inspect(ctx, scope, destination)
}

func (s *PreflightService) resolve(
	ctx context.Context,
	request PreflightRequest,
) (Scope, error) {
	switch request.SourceType {
	case operations.ExportSourceSelection:
		return s.resolver.ResolveSelection(ctx, request.StoryIDs)
	case operations.ExportSourceCurrentQuery:
		return s.resolver.ResolveCurrentQuery(ctx, request.Query)
	case operations.ExportSourceShelf:
		if len(request.ShelfIDs) != 1 {
			return Scope{}, ErrInvalidScope
		}
		return s.resolver.ResolveShelf(ctx, request.ShelfIDs[0])
	case operations.ExportSourceShelves:
		return s.resolver.ResolveShelves(ctx, request.ShelfIDs)
	default:
		return Scope{}, ErrInvalidScope
	}
}

func (s *PreflightService) inspect(
	ctx context.Context,
	scope Scope,
	destination string,
) (PreflightReport, error) {
	report := PreflightReport{
		Source:           scope.Source,
		ResolvedCount:    len(scope.Stories),
		CollapsedOverlap: scope.CollapsedOverlap,
		Items:            make([]PreflightItem, 0, len(scope.Stories)),
		Scope:            scope,
	}
	if len(scope.Stories) == 0 {
		report.Issues = append(report.Issues, blockingIssue(
			IssueEmptyScope,
			"No stories match this export scope.",
		))
		report.Blocked = true
		return report, nil
	}

	resolvedDestination, err := ResolveDestination(destination)
	if err != nil {
		report.Issues = append(report.Issues, blockingIssue(
			IssueDestinationInvalid,
			"The selected export destination is unavailable.",
		))
		report.Blocked = true
		return report, nil
	}
	report.Destination = resolvedDestination
	report.DestinationLabel = filepath.Base(resolvedDestination)
	if err := s.probeWritable(resolvedDestination); err != nil {
		report.Issues = append(report.Issues, blockingIssue(
			IssueDestinationNotWritable,
			"The selected export destination is not writable.",
		))
		report.Blocked = true
		return report, nil
	}

	formats := make(map[string]struct{})
	outputNames := make(map[string]struct{}, len(scope.Stories))
	for _, story := range scope.Stories {
		formats[story.DetectedFormat] = struct{}{}
		item, err := s.inspectStory(ctx, resolvedDestination, story, outputNames)
		if err != nil {
			return PreflightReport{}, err
		}
		report.Items = append(report.Items, item)
		switch item.Disposition {
		case DispositionReady:
			if story.ByteSize > math.MaxInt64-report.TotalBytes {
				return PreflightReport{}, fmt.Errorf(
					"%w: export byte total overflow",
					ErrInvalidScope,
				)
			}
			report.ReadyCount++
			report.TotalBytes += story.ByteSize
		case DispositionSkipped, DispositionConflicted:
			report.Partial = true
		}
	}
	report.DetectedFormats = sortedKeys(formats)
	if report.ReadyCount == 0 {
		report.Issues = append(report.Issues, blockingIssue(
			IssueNoExportableStories,
			"No stories in this scope are currently exportable.",
		))
		report.Blocked = true
	}
	report.CanExport = !report.Blocked && report.ReadyCount > 0
	return report, nil
}

func (s *PreflightService) inspectStory(
	ctx context.Context,
	destination string,
	story library.ExportStory,
	outputNames map[string]struct{},
) (PreflightItem, error) {
	item := PreflightItem{
		StoryID:        story.ID,
		StoryUUID:      story.UUID,
		StoryTitle:     story.Title,
		OutputName:     story.OriginalFilename,
		DetectedFormat: story.DetectedFormat,
		ByteSize:       story.ByteSize,
		Disposition:    DispositionReady,
	}
	switch story.Verification {
	case library.CompatibilityMissing:
		return skippedItem(item, IssueArchiveMissing, "Managed archive bytes are missing."), nil
	case library.CompatibilityInvalid:
		return skippedItem(item, IssueArchiveInvalid, "Managed archive verification failed."), nil
	case library.CompatibilityCompatible:
	default:
		return skippedItem(item, IssueArchiveInvalid, "Managed archive verification is unknown."), nil
	}
	if !supportedOutputName(story.OriginalFilename, story.DetectedFormat) {
		return skippedItem(
			item,
			IssueUnsupportedExtension,
			"The stored archive does not have a supported Lunii.QT extension.",
		), nil
	}

	source, err := archive.SafeJoin(s.layout.Root, story.ManagedRelativePath)
	if err != nil {
		return skippedItem(item, IssueArchiveMissing, "Managed archive bytes are unavailable."), nil
	}
	info, err := os.Lstat(source)
	if errors.Is(err, os.ErrNotExist) {
		return skippedItem(item, IssueArchiveMissing, "Managed archive bytes are missing."), nil
	}
	if err != nil {
		return PreflightItem{}, fmt.Errorf("inspect managed export archive: %w", err)
	}
	contained, containmentErr := storage.PathContained(s.layout.Archives, source)
	if containmentErr != nil ||
		!contained ||
		!info.Mode().IsRegular() ||
		info.Mode()&os.ModeSymlink != 0 {
		return skippedItem(item, IssueArchiveMissing, "Managed archive bytes are unavailable."), nil
	}
	checksum, size, err := checksumFile(ctx, source)
	if err != nil {
		return PreflightItem{}, err
	}
	if checksum != story.SHA256 || size != story.ByteSize {
		return skippedItem(
			item,
			IssueArchiveChanged,
			"Managed archive bytes no longer match the imported archive.",
		), nil
	}

	normalizedName := strings.ToLower(story.OriginalFilename)
	if _, duplicate := outputNames[normalizedName]; duplicate {
		return conflictedItem(
			item,
			"Another story in this export uses the same destination filename.",
		), nil
	}
	outputNames[normalizedName] = struct{}{}
	outputPath := filepath.Join(destination, story.OriginalFilename)
	if _, err := os.Lstat(outputPath); err == nil {
		return conflictedItem(
			item,
			"A file with this name already exists in the destination.",
		), nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return PreflightItem{}, fmt.Errorf("inspect export filename conflict: %w", err)
	}
	return item, nil
}

func sourceFromRequest(request PreflightRequest) operations.ExportSource {
	return operations.ExportSource{
		Type:     request.SourceType,
		ShelfIDs: append([]int64(nil), request.ShelfIDs...),
	}
}

func skippedItem(
	item PreflightItem,
	code PreflightIssueCode,
	message string,
) PreflightItem {
	issue := PreflightIssue{Code: code, Message: message}
	item.Disposition = DispositionSkipped
	item.Issue = &issue
	return item
}

func conflictedItem(item PreflightItem, message string) PreflightItem {
	issue := PreflightIssue{Code: IssueFilenameConflict, Message: message}
	item.Disposition = DispositionConflicted
	item.Issue = &issue
	return item
}

func blockingIssue(code PreflightIssueCode, message string) PreflightIssue {
	return PreflightIssue{Code: code, Message: message, Blocks: true}
}

func supportedOutputName(name string, format string) bool {
	if name == "" ||
		filepath.Base(name) != name ||
		name == "." ||
		name == ".." ||
		strings.ContainsRune(name, '\x00') {
		return false
	}
	lower := strings.ToLower(name)
	var extension string
	switch catalog.ArchiveFormat(format) {
	case catalog.FormatPlainPK:
		extension = ".plain.pk"
	case catalog.FormatV1PK:
		extension = ".v1.pk"
	case catalog.FormatV2PK:
		extension = ".v2.pk"
	case catalog.FormatGenericPK:
		extension = ".pk"
	case catalog.FormatZIP, catalog.FormatStudioZIP:
		extension = ".zip"
	case catalog.FormatSevenZIP:
		extension = ".7z"
	default:
		return false
	}
	return strings.HasSuffix(lower, extension)
}

func sortedKeys(values map[string]struct{}) []string {
	keys := make([]string, 0, len(values))
	for value := range values {
		keys = append(keys, value)
	}
	sort.Strings(keys)
	return keys
}

func probeExportDestination(destination string) error {
	file, err := os.CreateTemp(destination, ".librairii-export-preflight-*")
	if err != nil {
		return err
	}
	name := file.Name()
	closeErr := file.Close()
	removeErr := os.Remove(name)
	if closeErr != nil {
		return closeErr
	}
	return removeErr
}

func checksumFile(ctx context.Context, path string) (string, int64, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", 0, fmt.Errorf("open managed export archive: %w", err)
	}
	defer file.Close()
	hasher := sha256.New()
	size, err := io.Copy(hasher, exportContextReader{ctx: ctx, source: file})
	if err != nil {
		return "", 0, fmt.Errorf("verify managed export archive: %w", err)
	}
	return hex.EncodeToString(hasher.Sum(nil)), size, nil
}

type exportContextReader struct {
	ctx    context.Context
	source io.Reader
}

func (r exportContextReader) Read(buffer []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.source.Read(buffer)
}
