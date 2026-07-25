package app

import (
	"context"
	"encoding/json"
	"errors"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/tagging"
)

type fixedClock struct {
	now time.Time
}

func (c fixedClock) Now() time.Time {
	return c.now
}

type fakeDialogs struct{}

func (fakeDialogs) OpenFiles(context.Context, FileDialogRequest) ([]string, error) {
	return nil, nil
}

type recordingDialogs struct {
	paths   []string
	request FileDialogRequest
	calls   int
}

func (d *recordingDialogs) OpenFiles(
	_ context.Context,
	request FileDialogRequest,
) ([]string, error) {
	d.calls++
	d.request = request
	return append([]string(nil), d.paths...), nil
}

func (d *recordingDialogs) OpenDirectory(context.Context, string) (string, error) {
	return "", nil
}

func (fakeDialogs) OpenDirectory(context.Context, string) (string, error) {
	return "", nil
}

type emittedEvent struct {
	name    string
	payload any
}

type fakeEvents struct {
	events []emittedEvent
}

func (e *fakeEvents) Emit(_ context.Context, name string, payload any) {
	e.events = append(e.events, emittedEvent{name: name, payload: payload})
}

type fakeReadiness struct {
	report ReadinessReport
	err    error
}

func (r fakeReadiness) Check(context.Context) (ReadinessReport, error) {
	return r.report, r.err
}

type fakeResource struct {
	closed bool
}

func (r *fakeResource) Close() error {
	r.closed = true
	return nil
}

type fakeOperations struct {
	started    bool
	closed     bool
	startPaths []string
	snapshot   operations.Snapshot
	active     []operations.Snapshot
	page       library.Page
	searchPage library.Page
	detail     library.StoryDetail
	removed    removal.Result
	err        error
}

func (o *fakeOperations) Start(context.Context) error {
	o.started = true
	return o.err
}

func (o *fakeOperations) StartImport(
	_ context.Context,
	paths []string,
) (operations.Snapshot, error) {
	o.startPaths = append([]string(nil), paths...)
	return o.snapshot, o.err
}

func (o *fakeOperations) Cancel(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return o.snapshot, o.err
}

func (o *fakeOperations) Snapshot(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return o.snapshot, o.err
}

func (o *fakeOperations) Active(context.Context) ([]operations.Snapshot, error) {
	return append([]operations.Snapshot(nil), o.active...), o.err
}

func (o *fakeOperations) Close() error {
	o.closed = true
	return nil
}

func (o *fakeOperations) List(
	context.Context,
	library.ListRequest,
) (library.Page, error) {
	return o.page, o.err
}

func (o *fakeOperations) Search(
	context.Context,
	library.StoryLibraryQuery,
) (library.Page, error) {
	return o.searchPage, o.err
}

func (o *fakeOperations) Detail(
	context.Context,
	int64,
) (library.StoryDetail, error) {
	return o.detail, o.err
}

func (o *fakeOperations) Remove(
	context.Context,
	int64,
) (removal.Result, error) {
	return o.removed, o.err
}

func (o *fakeOperations) Catalog(context.Context) (tagging.Catalog, error) {
	return tagging.Catalog{}, o.err
}

func (o *fakeOperations) CreateDefinition(
	context.Context,
	tagging.CreateDefinition,
) (tagging.Definition, error) {
	return tagging.Definition{}, o.err
}

func (o *fakeOperations) RenameDefinition(
	context.Context,
	int64,
	string,
) (tagging.Definition, error) {
	return tagging.Definition{}, o.err
}

func (o *fakeOperations) RecolorDefinition(
	context.Context,
	int64,
	string,
) (tagging.Definition, error) {
	return tagging.Definition{}, o.err
}

func (o *fakeOperations) ReorderDefinitions(
	context.Context,
	[]int64,
) ([]tagging.Definition, error) {
	return nil, o.err
}

func (o *fakeOperations) PlanDefinitionDeletion(
	context.Context,
	int64,
) (tagging.DefinitionDeletionPlan, error) {
	return tagging.DefinitionDeletionPlan{}, o.err
}

func (o *fakeOperations) DeleteDefinition(
	context.Context,
	tagging.DefinitionDeletionPlan,
) error {
	return o.err
}

func (o *fakeOperations) CreateValue(
	context.Context,
	tagging.CreateValue,
) (tagging.Value, error) {
	return tagging.Value{}, o.err
}

func (o *fakeOperations) RenameValue(
	context.Context,
	int64,
	string,
) (tagging.Value, error) {
	return tagging.Value{}, o.err
}

func (o *fakeOperations) ReorderValues(
	context.Context,
	int64,
	[]int64,
) ([]tagging.Value, error) {
	return nil, o.err
}

func (o *fakeOperations) PlanValueDeletion(
	context.Context,
	int64,
) (tagging.ValueDeletionPlan, error) {
	return tagging.ValueDeletionPlan{}, o.err
}

func (o *fakeOperations) DeleteValue(
	context.Context,
	tagging.ValueDeletionPlan,
) error {
	return o.err
}

func (o *fakeOperations) AssignmentWorkspace(
	context.Context,
	[]int64,
) (tagging.AssignmentWorkspace, error) {
	return tagging.AssignmentWorkspace{}, o.err
}

func (o *fakeOperations) SetBulkBoolean(
	context.Context,
	[]int64,
	int64,
	bool,
) (tagging.AssignmentResult, error) {
	return tagging.AssignmentResult{}, o.err
}

func (o *fakeOperations) SetBulkChoiceValues(
	context.Context,
	[]int64,
	int64,
	[]int64,
) (tagging.AssignmentResult, error) {
	return tagging.AssignmentResult{}, o.err
}

func (o *fakeOperations) SetBulkChoiceValue(
	context.Context,
	[]int64,
	int64,
	int64,
	bool,
) (tagging.AssignmentResult, error) {
	return tagging.AssignmentResult{}, o.err
}

func TestApplicationLifecycle(t *testing.T) {
	t.Parallel()

	now := time.Date(2026, time.July, 25, 8, 30, 0, 0, time.FixedZone("CEST", 2*60*60))
	events := &fakeEvents{}
	resource := &fakeResource{}
	operationPort := &fakeOperations{}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: now},
		Dialogs:    fakeDialogs{},
		Events:     events,
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
		Resources:  []ResourcePort{resource},
	})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	if got := application.Status().State; got != StateInitializing {
		t.Fatalf("initial state = %q, want %q", got, StateInitializing)
	}

	if err := application.Start(context.Background()); err != nil {
		t.Fatalf("Start() error = %v", err)
	}

	status := application.Status()
	if status.State != StateReady {
		t.Fatalf("ready state = %q, want %q", status.State, StateReady)
	}
	if !status.MutationsAllowed {
		t.Fatal("mutations are disabled in ready state")
	}
	if status.StartedAt != "2026-07-25T06:30:00Z" {
		t.Fatalf("startedAt = %q", status.StartedAt)
	}

	application.AnnounceReady(context.Background())
	if len(events.events) != 1 || events.events[0].name != EventApplicationReady {
		t.Fatalf("events = %#v", events.events)
	}

	application.Stop(context.Background())
	if got := application.Status().State; got != StateStopped {
		t.Fatalf("stopped state = %q, want %q", got, StateStopped)
	}
	if !resource.closed {
		t.Fatal("resource was not closed during shutdown")
	}
	if !operationPort.started || !operationPort.closed {
		t.Fatalf("operation lifecycle = %#v", operationPort)
	}
}

func TestApplicationEntersRecoveryWhenStorageIsUnsafe(t *testing.T) {
	t.Parallel()

	operationPort := &fakeOperations{}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    fakeDialogs{},
		Events:     &fakeEvents{},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
		Readiness: fakeReadiness{report: ReadinessReport{
			MutationsAllowed: false,
			Issues:           []ReadinessIssue{{Code: "schema_mismatch"}},
		}},
	})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}
	if err := application.Start(context.Background()); err != nil {
		t.Fatalf("Start() error = %v", err)
	}

	response := application.StatusResponse()
	if response.Status.State != StateRecovery || response.Status.MutationsAllowed {
		t.Fatalf("StatusResponse() status = %#v", response.Status)
	}
	if response.Error == nil || response.Error.Code != ErrorStorageUnavailable {
		t.Fatalf("StatusResponse() error = %#v", response.Error)
	}
}

func TestNewRequiresReplaceablePorts(t *testing.T) {
	t.Parallel()

	_, err := New(Dependencies{})
	if !errors.Is(err, ErrMissingDependency) {
		t.Fatalf("New() error = %v, want ErrMissingDependency", err)
	}
}

func TestAsAPIErrorPreservesStableErrors(t *testing.T) {
	t.Parallel()

	expected := NewAPIError(ErrorConflict, "Already exists.")
	if got := AsAPIError(expected); got != expected {
		t.Fatalf("AsAPIError() = %#v, want original error", got)
	}

	got := AsAPIError(errors.New("private implementation detail"))
	if got.Code != ErrorInternal || got.Message == "private implementation detail" {
		t.Fatalf("AsAPIError() = %#v", got)
	}
}

func TestTaggingAPIErrorsClassifyExpectedValidationFailures(t *testing.T) {
	t.Parallel()

	if got := taggingAPIError(tagging.ErrValuesNotAllowed); got.Code != ErrorInvalidInput {
		t.Fatalf("taggingAPIError(values not allowed) = %#v", got)
	}
	if got := taggingAPIError(tagging.ErrDuplicateDefinition); got.Code != ErrorConflict {
		t.Fatalf("taggingAPIError(duplicate definition) = %#v", got)
	}
}

func TestImportFacadeKeepsNativePathsInsideGo(t *testing.T) {
	t.Parallel()

	dialogs := &recordingDialogs{
		paths: []string{"/private/native/clockwork.zip", "/private/native/forest.7z"},
	}
	operationPort := &fakeOperations{
		snapshot: operations.Snapshot{
			ID:         "00112233-4455-4677-8899-aabbccddeeff",
			Kind:       operations.KindImport,
			Status:     operations.StatusQueued,
			TotalItems: 2,
			Items: []operations.ItemSnapshot{
				{SourceName: "clockwork.zip", Status: operations.ItemPending},
				{SourceName: "forest.7z", Status: operations.ItemPending},
			},
		},
	}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    dialogs,
		Events:     &fakeEvents{},
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := application.Start(context.Background()); err != nil {
		t.Fatal(err)
	}

	response := application.SelectAndStartImport(context.Background())
	if response.Error != nil || response.Operation == nil {
		t.Fatalf("SelectAndStartImport() = %#v", response)
	}
	if !reflect.DeepEqual(operationPort.startPaths, dialogs.paths) {
		t.Fatalf("operation paths = %#v", operationPort.startPaths)
	}
	if !dialogs.request.Multiple ||
		!reflect.DeepEqual(dialogs.request.Extensions, []string{
			".plain.pk",
			".v1.pk",
			".v2.pk",
			".pk",
			".zip",
			".7z",
		}) {
		t.Fatalf("dialog request = %#v", dialogs.request)
	}
	encoded, err := json.Marshal(response)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(encoded), "/private/native/") {
		t.Fatalf("frontend response exposed native paths: %s", encoded)
	}
}

func TestImportFacadeTreatsEmptySelectionAsCancellation(t *testing.T) {
	t.Parallel()

	dialogs := &recordingDialogs{}
	operationPort := &fakeOperations{}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    dialogs,
		Events:     &fakeEvents{},
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := application.Start(context.Background()); err != nil {
		t.Fatal(err)
	}

	response := application.SelectAndStartImport(context.Background())
	if !response.Cancelled || response.Error != nil || response.Operation != nil {
		t.Fatalf("SelectAndStartImport() = %#v", response)
	}
	if operationPort.startPaths != nil {
		t.Fatalf("operation unexpectedly started with %#v", operationPort.startPaths)
	}
}

func TestOperationFacadeReturnsActiveSnapshots(t *testing.T) {
	t.Parallel()

	operationPort := &fakeOperations{
		active: []operations.Snapshot{{
			ID:     "00112233-4455-4677-8899-aabbccddeeff",
			Kind:   operations.KindImport,
			Status: operations.StatusRunning,
		}},
	}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    fakeDialogs{},
		Events:     &fakeEvents{},
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
	})
	if err != nil {
		t.Fatal(err)
	}

	response := application.ActiveOperations(context.Background())
	if response.Error != nil ||
		len(response.Operations) != 1 ||
		response.Operations[0].Status != operations.StatusRunning {
		t.Fatalf("ActiveOperations() = %#v", response)
	}
}

func TestLibraryFacadeReturnsTypedCollectionAndDetail(t *testing.T) {
	t.Parallel()

	operationPort := &fakeOperations{
		page: library.Page{
			Stories: []library.StorySummary{{
				ID:    7,
				UUID:  "00112233-4455-4677-8899-aabbccddeeff",
				Title: "Clockwork Forest",
			}},
			Page:       1,
			PageSize:   24,
			TotalItems: 1,
			TotalPages: 1,
			Sort:       library.SortNameAscending,
		},
		searchPage: library.Page{
			Stories: []library.StorySummary{{ID: 8, Title: "Filtered Forest"}},
			Page:    1, PageSize: 24, TotalItems: 1, TotalPages: 1,
			Sort: library.SortNameAscending,
		},
		detail: library.StoryDetail{
			Story: library.StorySummary{ID: 7, Title: "Clockwork Forest"},
			Archive: library.ArchiveDetails{
				OriginalFilename: "clockwork.zip",
				SHA256:           strings.Repeat("a", 64),
			},
		},
	}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    fakeDialogs{},
		Events:     &fakeEvents{},
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
	})
	if err != nil {
		t.Fatal(err)
	}

	page := application.ListStories(context.Background(), library.ListRequest{})
	if page.Error != nil || page.Page == nil || page.Page.TotalItems != 1 {
		t.Fatalf("ListStories() = %#v", page)
	}
	filtered := application.QueryStories(
		context.Background(),
		library.StoryLibraryQuery{Name: "forest"},
	)
	if filtered.Error != nil ||
		filtered.Page == nil ||
		filtered.Page.Stories[0].Title != "Filtered Forest" {
		t.Fatalf("QueryStories() = %#v", filtered)
	}
	detail := application.StoryDetail(context.Background(), 7)
	if detail.Error != nil ||
		detail.Detail == nil ||
		detail.Detail.Archive.OriginalFilename != "clockwork.zip" {
		t.Fatalf("StoryDetail() = %#v", detail)
	}
}

func TestRemovalFacadeRequiresReadyStorageAndReturnsStableResult(t *testing.T) {
	t.Parallel()

	operationPort := &fakeOperations{
		removed: removal.Result{
			StoryID: 7,
			UUID:    "00112233-4455-4677-8899-aabbccddeeff",
		},
	}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    fakeDialogs{},
		Events:     &fakeEvents{},
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: operationPort,
		Library:    operationPort,
		Removal:    operationPort,
		Tags:       operationPort,
	})
	if err != nil {
		t.Fatal(err)
	}
	if response := application.RemoveStory(context.Background(), 7); response.Error == nil ||
		response.Error.Code != ErrorNotReady {
		t.Fatalf("RemoveStory(before ready) = %#v", response)
	}
	if err := application.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	response := application.RemoveStory(context.Background(), 7)
	if response.Error != nil || response.Result == nil || response.Result.StoryID != 7 {
		t.Fatalf("RemoveStory() = %#v", response)
	}
}
