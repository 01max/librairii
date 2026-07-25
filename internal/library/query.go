package library

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
)

const (
	DefaultPageSize = 24
	MaxPageSize     = 100
)

var ErrInvalidListRequest = errors.New("library list request is invalid")

type MetadataSource string

const (
	SourceOfficial MetadataSource = "official"
	SourceEmbedded MetadataSource = "embedded"
	SourceFallback MetadataSource = "fallback"
)

type Compatibility string

const (
	CompatibilityCompatible Compatibility = "compatible"
	CompatibilityMissing    Compatibility = "missing"
	CompatibilityInvalid    Compatibility = "invalid"
)

type Sort string

const (
	SortNameAscending  Sort = "name_asc"
	SortImportedNewest Sort = "imported_desc"
)

type ListRequest struct {
	Page     int  `json:"page"`
	PageSize int  `json:"pageSize"`
	Sort     Sort `json:"sort"`
}

type DisplaySources struct {
	Title       MetadataSource `json:"title"`
	Description MetadataSource `json:"description"`
	Author      MetadataSource `json:"author"`
	Artwork     MetadataSource `json:"artwork"`
}

type StorySummary struct {
	ID                  int64          `json:"id"`
	UUID                string         `json:"uuid"`
	Title               string         `json:"title"`
	Description         string         `json:"description,omitempty"`
	Author              string         `json:"author,omitempty"`
	ArtworkID           string         `json:"artworkId,omitempty"`
	Sources             DisplaySources `json:"sources"`
	DetectedFormat      string         `json:"detectedFormat"`
	Compatibility       Compatibility  `json:"compatibility"`
	CompatibilityReason string         `json:"compatibilityReason,omitempty"`
	ByteSize            int64          `json:"byteSize"`
	ImportedAt          string         `json:"importedAt"`
}

type ArchiveDetails struct {
	OriginalFilename string        `json:"originalFilename"`
	DetectedFormat   string        `json:"detectedFormat"`
	SHA256           string        `json:"sha256"`
	ByteSize         int64         `json:"byteSize"`
	Verification     Compatibility `json:"verification"`
}

type StoryDetail struct {
	Story   StorySummary   `json:"story"`
	Archive ArchiveDetails `json:"archive"`
}

type Page struct {
	Stories    []StorySummary `json:"stories"`
	Page       int            `json:"page"`
	PageSize   int            `json:"pageSize"`
	TotalItems int            `json:"totalItems"`
	TotalPages int            `json:"totalPages"`
	Sort       Sort           `json:"sort"`
}

type OfficialMetadata struct {
	UUID        string
	Title       string
	Description string
	Author      string
	ArtworkID   string
}

type OfficialProvider interface {
	FindByUUIDs(context.Context, []string) (map[string]OfficialMetadata, error)
}

type Query struct {
	database *sql.DB
	official OfficialProvider
}

func NewQuery(database *sql.DB, official OfficialProvider) *Query {
	if official == nil {
		official = emptyOfficialProvider{}
	}
	return &Query{database: database, official: official}
}

func (q *Query) List(ctx context.Context, request ListRequest) (Page, error) {
	request, err := normalizeListRequest(request)
	if err != nil {
		return Page{}, err
	}
	return q.searchNormalized(ctx, StoryLibraryQuery{
		Page:     request.Page,
		PageSize: request.PageSize,
		Sort:     request.Sort,
	})
}

func (q *Query) summariesFromRecords(
	ctx context.Context,
	records []localRecord,
) ([]StorySummary, error) {
	official, err := q.official.FindByUUIDs(ctx, recordUUIDs(records))
	if err != nil {
		return nil, fmt.Errorf("load official display metadata: %w", err)
	}
	stories := make([]StorySummary, 0, len(records))
	for _, record := range records {
		stories = append(stories, resolveSummary(record, official[record.uuid]))
	}
	return stories, nil
}

func (q *Query) Detail(ctx context.Context, storyID int64) (StoryDetail, error) {
	if storyID <= 0 {
		return StoryDetail{}, ErrInvalidListRequest
	}
	records, err := q.localRecords(ctx, storyID)
	if err != nil {
		return StoryDetail{}, err
	}
	if len(records) == 0 {
		return StoryDetail{}, sql.ErrNoRows
	}
	record := records[0]
	official, err := q.official.FindByUUIDs(ctx, []string{record.uuid})
	if err != nil {
		return StoryDetail{}, fmt.Errorf("load official display metadata: %w", err)
	}
	verification, _ := compatibility(record.validationState)
	return StoryDetail{
		Story: resolveSummary(record, official[record.uuid]),
		Archive: ArchiveDetails{
			OriginalFilename: record.originalFilename,
			DetectedFormat:   record.detectedFormat,
			SHA256:           record.sha256,
			ByteSize:         record.byteSize,
			Verification:     verification,
		},
	}, nil
}

type localRecord struct {
	id                  int64
	uuid                string
	embeddedTitle       string
	embeddedDescription string
	embeddedArtworkPath string
	createdAt           string
	originalFilename    string
	detectedFormat      string
	sha256              string
	byteSize            int64
	validationState     string
}

func (q *Query) localRecords(ctx context.Context, storyID int64) ([]localRecord, error) {
	if storyID == 0 {
		return q.localRecordsWhere(ctx, "", nil)
	}
	return q.localRecordsWhere(ctx, "s.id = ?", []any{storyID})
}

func (q *Query) localRecordsWhere(
	ctx context.Context,
	predicate string,
	arguments []any,
) ([]localRecord, error) {
	statement := localRecordSelect
	if predicate != "" {
		statement += " WHERE " + predicate
	}
	statement += " ORDER BY s.id"
	return q.queryLocalRecords(ctx, statement, arguments)
}

func (q *Query) localRecordPage(
	ctx context.Context,
	predicate string,
	arguments []any,
	order Sort,
	limit int,
	offset int,
) ([]localRecord, error) {
	statement := localRecordSelect
	if predicate != "" {
		statement += " WHERE " + predicate
	}
	if order == SortImportedNewest {
		statement += " ORDER BY s.created_at DESC, s.id DESC"
	} else {
		statement += " ORDER BY s.display_name_normalized, s.uuid, s.id"
	}
	statement += " LIMIT ? OFFSET ?"
	pageArguments := append([]any(nil), arguments...)
	pageArguments = append(pageArguments, limit, offset)
	return q.queryLocalRecords(ctx, statement, pageArguments)
}

func (q *Query) countLocalRecords(
	ctx context.Context,
	predicate string,
	arguments []any,
) (int, error) {
	statement := `SELECT COUNT(*)
		FROM stories s
		JOIN story_archives a ON a.story_id = s.id`
	if predicate != "" {
		statement += " WHERE " + predicate
	}
	var count int
	if err := q.database.QueryRowContext(ctx, statement, arguments...).Scan(&count); err != nil {
		return 0, err
	}
	return count, nil
}

const localRecordSelect = `SELECT
		s.id,
		s.uuid,
		COALESCE(s.embedded_title, ''),
		COALESCE(s.embedded_description, ''),
		COALESCE(s.embedded_artwork_path, ''),
		s.created_at,
		a.original_filename,
		a.detected_format,
		a.sha256,
		a.byte_size,
		a.validation_state
	FROM stories s
	JOIN story_archives a ON a.story_id = s.id`

func (q *Query) queryLocalRecords(
	ctx context.Context,
	statement string,
	arguments []any,
) ([]localRecord, error) {
	rows, err := q.database.QueryContext(ctx, statement, arguments...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var records []localRecord
	for rows.Next() {
		var record localRecord
		if err := rows.Scan(
			&record.id,
			&record.uuid,
			&record.embeddedTitle,
			&record.embeddedDescription,
			&record.embeddedArtworkPath,
			&record.createdAt,
			&record.originalFilename,
			&record.detectedFormat,
			&record.sha256,
			&record.byteSize,
			&record.validationState,
		); err != nil {
			return nil, err
		}
		records = append(records, record)
	}
	return records, rows.Err()
}

func resolveSummary(record localRecord, official OfficialMetadata) StorySummary {
	title, titleSource := resolveText(
		official.Title,
		record.embeddedTitle,
		"Story "+record.uuid,
	)
	description, descriptionSource := resolveText(
		official.Description,
		record.embeddedDescription,
		"",
	)
	author, authorSource := resolveText(official.Author, "", "")
	artworkID := strings.TrimSpace(official.ArtworkID)
	artworkSource := SourceFallback
	if artworkID != "" {
		artworkSource = SourceOfficial
	} else if record.embeddedArtworkPath != "" {
		artworkID = fmt.Sprintf("embedded:%d", record.id)
		artworkSource = SourceEmbedded
	}
	compatibility, reason := compatibility(record.validationState)
	return StorySummary{
		ID:          record.id,
		UUID:        record.uuid,
		Title:       title,
		Description: description,
		Author:      author,
		ArtworkID:   artworkID,
		Sources: DisplaySources{
			Title:       titleSource,
			Description: descriptionSource,
			Author:      authorSource,
			Artwork:     artworkSource,
		},
		DetectedFormat:      record.detectedFormat,
		Compatibility:       compatibility,
		CompatibilityReason: reason,
		ByteSize:            record.byteSize,
		ImportedAt:          record.createdAt,
	}
}

func resolveText(
	official string,
	embedded string,
	fallback string,
) (string, MetadataSource) {
	if value := strings.TrimSpace(official); value != "" {
		return value, SourceOfficial
	}
	if value := strings.TrimSpace(embedded); value != "" {
		return value, SourceEmbedded
	}
	return fallback, SourceFallback
}

func compatibility(validationState string) (Compatibility, string) {
	switch validationState {
	case "valid":
		return CompatibilityCompatible, ""
	case "missing":
		return CompatibilityMissing, "Managed archive bytes are missing."
	default:
		return CompatibilityInvalid, "Managed archive verification failed."
	}
}

func normalizeListRequest(request ListRequest) (ListRequest, error) {
	if request.Page == 0 {
		request.Page = 1
	}
	if request.PageSize == 0 {
		request.PageSize = DefaultPageSize
	}
	if request.Sort == "" {
		request.Sort = SortNameAscending
	}
	if request.Page < 1 ||
		request.PageSize < 1 ||
		request.PageSize > MaxPageSize ||
		(request.Sort != SortNameAscending && request.Sort != SortImportedNewest) {
		return ListRequest{}, ErrInvalidListRequest
	}
	return request, nil
}

func recordUUIDs(records []localRecord) []string {
	uuids := make([]string, 0, len(records))
	for _, record := range records {
		uuids = append(uuids, record.uuid)
	}
	return uuids
}

type emptyOfficialProvider struct{}

func (emptyOfficialProvider) FindByUUIDs(
	context.Context,
	[]string,
) (map[string]OfficialMetadata, error) {
	return map[string]OfficialMetadata{}, nil
}
