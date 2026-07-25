package exporter

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
)

func TestPreflightReportsReadySkippedAndConflictedStories(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	ready := writeManagedExportStory(t, layout, 1, "ready.zip", []byte("ready"))
	missing := ready
	missing.ID = 2
	missing.Title = "Missing"
	missing.OriginalFilename = "missing.zip"
	missing.Verification = library.CompatibilityMissing
	unsupported := writeManagedExportStory(
		t,
		layout,
		3,
		"unsupported.rar",
		[]byte("unsupported"),
	)
	changed := writeManagedExportStory(
		t,
		layout,
		4,
		"changed.zip",
		[]byte("changed"),
	)
	changed.SHA256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	conflicted := writeManagedExportStory(
		t,
		layout,
		5,
		"existing.7z",
		[]byte("existing source"),
	)
	conflicted.DetectedFormat = string(catalog.FormatSevenZIP)
	existingBytes := []byte("do not replace")
	if err := os.WriteFile(
		filepath.Join(destination, conflicted.OriginalFilename),
		existingBytes,
		0o600,
	); err != nil {
		t.Fatal(err)
	}

	libraryQuery := &fakeLibraryResolver{
		stories: map[int64]library.ExportStory{
			1: ready,
			2: missing,
			3: unsupported,
			4: changed,
			5: conflicted,
		},
	}
	resolver, err := NewResolver(libraryQuery, fakeShelfResolver{})
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}
	report, err := service.Plan(context.Background(), PreflightRequest{
		SourceType: operations.ExportSourceSelection,
		StoryIDs:   []int64{1, 2, 3, 4, 5},
	}, destination)
	if err != nil {
		t.Fatal(err)
	}
	if !report.CanExport ||
		report.Blocked ||
		!report.Partial ||
		report.ResolvedCount != 5 ||
		report.ReadyCount != 1 ||
		report.TotalBytes != ready.ByteSize ||
		!reflect.DeepEqual(
			report.DetectedFormats,
			[]string{string(catalog.FormatSevenZIP), string(catalog.FormatZIP)},
		) {
		t.Fatalf("preflight report = %#v", report)
	}
	gotDispositions := make([]PreflightDisposition, 0, len(report.Items))
	gotIssues := make([]PreflightIssueCode, 0, len(report.Items)-1)
	for _, item := range report.Items {
		gotDispositions = append(gotDispositions, item.Disposition)
		if item.Issue != nil {
			gotIssues = append(gotIssues, item.Issue.Code)
		}
	}
	if !reflect.DeepEqual(gotDispositions, []PreflightDisposition{
		DispositionReady,
		DispositionSkipped,
		DispositionSkipped,
		DispositionSkipped,
		DispositionConflicted,
	}) || !reflect.DeepEqual(gotIssues, []PreflightIssueCode{
		IssueArchiveMissing,
		IssueUnsupportedExtension,
		IssueArchiveChanged,
		IssueFilenameConflict,
	}) {
		t.Fatalf("dispositions = %#v, issues = %#v", gotDispositions, gotIssues)
	}
	workItems := report.OperationItems()
	if len(workItems) != 5 ||
		workItems[0].PlannedStatus != operations.ItemPending ||
		workItems[1].PlannedStatus != operations.ItemSkipped ||
		workItems[4].PlannedStatus != operations.ItemConflicted ||
		workItems[1].OutcomeCode != string(IssueArchiveMissing) ||
		workItems[0].Item.ArchiveRelativePath != ready.ManagedRelativePath {
		t.Fatalf("operation work items = %#v", workItems)
	}
	gotExisting, err := os.ReadFile(filepath.Join(destination, conflicted.OriginalFilename))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(gotExisting, existingBytes) {
		t.Fatalf("existing destination changed to %q", gotExisting)
	}
}

func TestPreflightBlocksEmptyInvalidShelfAndUnwritableDestination(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	libraryQuery := &fakeLibraryResolver{
		pages:   map[string]map[int][]int64{},
		stories: map[int64]library.ExportStory{},
	}
	resolver, err := NewResolver(
		libraryQuery,
		failingShelfResolver{err: shelves.ErrShelfNeedsAttention},
	)
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}
	invalidShelf, err := service.Plan(context.Background(), PreflightRequest{
		SourceType: operations.ExportSourceShelf,
		ShelfIDs:   []int64{9},
	}, t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	assertBlockedPreflight(t, invalidShelf, IssueShelfNeedsAttention)

	empty, err := service.Plan(context.Background(), PreflightRequest{
		SourceType: operations.ExportSourceSelection,
	}, t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	assertBlockedPreflight(t, empty, IssueEmptyScope)

	story := writeManagedExportStory(t, layout, 1, "story.zip", []byte("story"))
	libraryQuery.stories[1] = story
	service.probeWritable = func(string) error {
		return errors.New("read only")
	}
	unwritable, err := service.Plan(context.Background(), PreflightRequest{
		SourceType: operations.ExportSourceSelection,
		StoryIDs:   []int64{1},
	}, t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	assertBlockedPreflight(t, unwritable, IssueDestinationNotWritable)
}

func TestPreflightCollapsesDuplicateOutputNames(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	first := writeManagedExportStory(t, layout, 1, "Story.ZIP", []byte("first"))
	second := writeManagedExportStory(t, layout, 2, "story.zip", []byte("second"))
	libraryQuery := &fakeLibraryResolver{
		stories: map[int64]library.ExportStory{1: first, 2: second},
	}
	resolver, err := NewResolver(libraryQuery, fakeShelfResolver{})
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}
	report, err := service.Plan(context.Background(), PreflightRequest{
		SourceType: operations.ExportSourceSelection,
		StoryIDs:   []int64{1, 2},
	}, t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if !report.CanExport ||
		!report.Partial ||
		report.ReadyCount != 1 ||
		report.Items[1].Disposition != DispositionConflicted {
		t.Fatalf("duplicate-name report = %#v", report)
	}
}

func TestPreflightSkipsFilenameRejectedByOperationPersistence(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	story := writeManagedExportStory(
		t,
		layout,
		1,
		" clockwork-forest.zip",
		[]byte("archive"),
	)
	resolver, err := NewResolver(
		&fakeLibraryResolver{
			stories: map[int64]library.ExportStory{story.ID: story},
		},
		fakeShelfResolver{},
	)
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}

	report, err := service.Plan(
		context.Background(),
		PreflightRequest{
			SourceType: operations.ExportSourceSelection,
			StoryIDs:   []int64{story.ID},
		},
		t.TempDir(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if report.CanExport ||
		!report.Blocked ||
		len(report.Items) != 1 ||
		report.Items[0].Disposition != DispositionSkipped ||
		report.Items[0].Issue == nil ||
		report.Items[0].Issue.Code != IssueUnsupportedExtension {
		t.Fatalf("preflight = %#v", report)
	}
}

type failingShelfResolver struct {
	err error
}

func (f failingShelfResolver) Open(
	context.Context,
	int64,
) (shelves.OpenedShelf, error) {
	return shelves.OpenedShelf{}, f.err
}

func writeManagedExportStory(
	t *testing.T,
	layout storage.Layout,
	id int64,
	name string,
	content []byte,
) library.ExportStory {
	t.Helper()
	sum := sha256.Sum256(content)
	checksum := hex.EncodeToString(sum[:])
	relative := filepath.ToSlash(filepath.Join("archives", checksum, name))
	path := filepath.Join(layout.Root, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, content, 0o600); err != nil {
		t.Fatal(err)
	}
	return library.ExportStory{
		ID:                  id,
		UUID:                "00112233-4455-4677-8899-aabbccddeeff",
		Title:               name,
		OriginalFilename:    name,
		DetectedFormat:      string(catalog.FormatZIP),
		SHA256:              checksum,
		ByteSize:            int64(len(content)),
		ManagedRelativePath: relative,
		Verification:        library.CompatibilityCompatible,
	}
}

func assertBlockedPreflight(
	t *testing.T,
	report PreflightReport,
	code PreflightIssueCode,
) {
	t.Helper()
	if !report.Blocked ||
		report.CanExport ||
		len(report.Issues) != 1 ||
		report.Issues[0].Code != code ||
		!report.Issues[0].Blocks {
		t.Fatalf("blocked preflight = %#v", report)
	}
}
