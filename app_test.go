package main

import (
	"context"
	"testing"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/tagging"
)

type facadeClock struct{}

func (facadeClock) Now() time.Time {
	return time.Date(2026, time.July, 25, 9, 0, 0, 0, time.UTC)
}

type facadeDialogs struct{}

func (facadeDialogs) OpenFiles(context.Context, coreapp.FileDialogRequest) ([]string, error) {
	return nil, nil
}

func (facadeDialogs) OpenDirectory(context.Context, string) (string, error) {
	return "", nil
}

func (facadeDialogs) RevealDirectory(context.Context, string) error {
	return nil
}

type facadeEvents struct{}

func (facadeEvents) Emit(context.Context, string, any) {}

type facadeReadiness struct{}

func (facadeReadiness) Check(context.Context) (coreapp.ReadinessReport, error) {
	return coreapp.ReadinessReport{MutationsAllowed: true}, nil
}

type exportFacadeDialogs struct {
	destination string
	revealed    string
}

func (d *exportFacadeDialogs) OpenFiles(
	context.Context,
	coreapp.FileDialogRequest,
) ([]string, error) {
	return nil, nil
}

func (d *exportFacadeDialogs) OpenDirectory(context.Context, string) (string, error) {
	return d.destination, nil
}

func (d *exportFacadeDialogs) RevealDirectory(
	_ context.Context,
	destination string,
) error {
	d.revealed = destination
	return nil
}

type exportFacadeOperations struct {
	facadeOperations
	request       exporter.PreflightRequest
	destination   string
	preparationID string
	snapshot      operations.Snapshot
}

func (o *exportFacadeOperations) PrepareExport(
	_ context.Context,
	request exporter.PreflightRequest,
	destination string,
) (exporter.PreflightReport, error) {
	o.request = request
	o.destination = destination
	return exporter.PreflightReport{
		PreparationID:    "00000000-0000-4000-8000-000000000120",
		Destination:      destination,
		DestinationLabel: "Lunii export",
		CanExport:        true,
	}, nil
}

func (o *exportFacadeOperations) StartPreparedExport(
	_ context.Context,
	preparationID string,
) (operations.Snapshot, error) {
	o.preparationID = preparationID
	return o.snapshot, nil
}

func (o *exportFacadeOperations) Snapshot(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return o.snapshot, nil
}

type facadeOperations struct{}

func (facadeOperations) Start(context.Context) error {
	return nil
}

func (facadeOperations) StartImport(
	context.Context,
	[]string,
) (operations.Snapshot, error) {
	return operations.Snapshot{}, nil
}

func (facadeOperations) StartMetadataRefresh(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return operations.Snapshot{}, nil
}

func (facadeOperations) PrepareExport(
	context.Context,
	exporter.PreflightRequest,
	string,
) (exporter.PreflightReport, error) {
	return exporter.PreflightReport{}, nil
}

func (facadeOperations) StartPreparedExport(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return operations.Snapshot{}, nil
}

func (facadeOperations) MetadataStatus(
	context.Context,
	string,
) (metadata.CatalogStatus, error) {
	return metadata.CatalogStatus{}, nil
}

func (facadeOperations) Cancel(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return operations.Snapshot{}, nil
}

func (facadeOperations) Snapshot(
	context.Context,
	string,
) (operations.Snapshot, error) {
	return operations.Snapshot{}, nil
}

func (facadeOperations) Active(context.Context) ([]operations.Snapshot, error) {
	return nil, nil
}

func (facadeOperations) Close() error {
	return nil
}

func (facadeOperations) List(
	context.Context,
	library.ListRequest,
) (library.Page, error) {
	return library.Page{}, nil
}

func (facadeOperations) Search(
	context.Context,
	library.StoryLibraryQuery,
) (library.Page, error) {
	return library.Page{}, nil
}

func (facadeOperations) Detail(
	context.Context,
	int64,
) (library.StoryDetail, error) {
	return library.StoryDetail{}, nil
}

func (facadeOperations) Remove(
	context.Context,
	int64,
) (removal.Result, error) {
	return removal.Result{}, nil
}

func (facadeOperations) Catalog(context.Context) (tagging.Catalog, error) {
	return tagging.Catalog{}, nil
}

func (facadeOperations) CreateDefinition(context.Context, tagging.CreateDefinition) (tagging.Definition, error) {
	return tagging.Definition{}, nil
}

func (facadeOperations) RenameDefinition(context.Context, int64, string) (tagging.Definition, error) {
	return tagging.Definition{}, nil
}

func (facadeOperations) RecolorDefinition(context.Context, int64, string) (tagging.Definition, error) {
	return tagging.Definition{}, nil
}

func (facadeOperations) ReorderDefinitions(context.Context, []int64) ([]tagging.Definition, error) {
	return nil, nil
}

func (facadeOperations) PlanDefinitionDeletion(context.Context, int64) (tagging.DefinitionDeletionPlan, error) {
	return tagging.DefinitionDeletionPlan{}, nil
}

func (facadeOperations) DeleteDefinition(context.Context, tagging.DefinitionDeletionPlan) error {
	return nil
}

func (facadeOperations) CreateValue(context.Context, tagging.CreateValue) (tagging.Value, error) {
	return tagging.Value{}, nil
}

func (facadeOperations) RenameValue(context.Context, int64, string) (tagging.Value, error) {
	return tagging.Value{}, nil
}

func (facadeOperations) ReorderValues(context.Context, int64, []int64) ([]tagging.Value, error) {
	return nil, nil
}

func (facadeOperations) PlanValueDeletion(context.Context, int64) (tagging.ValueDeletionPlan, error) {
	return tagging.ValueDeletionPlan{}, nil
}

func (facadeOperations) DeleteValue(context.Context, tagging.ValueDeletionPlan) error {
	return nil
}

func (facadeOperations) AssignmentWorkspace(
	context.Context,
	[]int64,
) (tagging.AssignmentWorkspace, error) {
	return tagging.AssignmentWorkspace{}, nil
}

func (facadeOperations) SetBulkBoolean(
	context.Context,
	[]int64,
	int64,
	bool,
) (tagging.AssignmentResult, error) {
	return tagging.AssignmentResult{}, nil
}

func (facadeOperations) SetBulkChoiceValues(
	context.Context,
	[]int64,
	int64,
	[]int64,
) (tagging.AssignmentResult, error) {
	return tagging.AssignmentResult{}, nil
}

func (facadeOperations) SetBulkChoiceValue(
	context.Context,
	[]int64,
	int64,
	int64,
	bool,
) (tagging.AssignmentResult, error) {
	return tagging.AssignmentResult{}, nil
}

func (facadeOperations) ListShelves(context.Context) ([]shelves.Summary, error) {
	return nil, nil
}

func (facadeOperations) CreateShelf(
	context.Context,
	string,
	library.StoryLibraryQuery,
) (shelves.Shelf, error) {
	return shelves.Shelf{}, nil
}

func (facadeOperations) OpenShelf(
	context.Context,
	int64,
	library.ListRequest,
) (shelves.Evaluation, error) {
	return shelves.Evaluation{}, nil
}

func (facadeOperations) RenameShelf(
	context.Context,
	int64,
	string,
) (shelves.Shelf, error) {
	return shelves.Shelf{}, nil
}

func (facadeOperations) DuplicateShelf(
	context.Context,
	int64,
	string,
) (shelves.Shelf, error) {
	return shelves.Shelf{}, nil
}

func (facadeOperations) ReplaceShelfQuery(
	context.Context,
	int64,
	library.StoryLibraryQuery,
) (shelves.Shelf, error) {
	return shelves.Shelf{}, nil
}

func (facadeOperations) ReorderShelves(
	context.Context,
	[]int64,
) ([]shelves.Shelf, error) {
	return nil, nil
}

func (facadeOperations) DeleteShelf(context.Context, int64) error {
	return nil
}

func (facadeOperations) PreviewShelves(
	context.Context,
	[]int64,
) (shelves.SelectionPreview, error) {
	return shelves.SelectionPreview{}, nil
}

func TestAppExposesTypedLifecycleStatus(t *testing.T) {
	t.Parallel()

	core, err := coreapp.New(coreapp.Dependencies{
		Clock:      facadeClock{},
		Dialogs:    facadeDialogs{},
		Events:     facadeEvents{},
		Readiness:  facadeReadiness{},
		Operations: facadeOperations{},
		Library:    facadeOperations{},
		Removal:    facadeOperations{},
		Tags:       facadeOperations{},
		Shelves:    facadeOperations{},
	})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}
	facade := NewApp(core)

	facade.startup(context.Background())
	response := facade.ApplicationStatus()

	if response.Error != nil {
		t.Fatalf("ApplicationStatus() error = %#v", response.Error)
	}
	if response.Status.State != coreapp.StateReady {
		t.Fatalf("ApplicationStatus() state = %q", response.Status.State)
	}
}

func TestAppExposesExportPreparationStartAndRevealBindings(t *testing.T) {
	t.Parallel()

	destination := t.TempDir()
	dialogs := &exportFacadeDialogs{destination: destination}
	operationID := "00000000-0000-4000-8000-000000000121"
	operationPort := &exportFacadeOperations{
		snapshot: operations.Snapshot{
			ID:     operationID,
			Kind:   operations.KindExport,
			Status: operations.StatusQueued,
		},
	}
	core, err := coreapp.New(coreapp.Dependencies{
		Clock:      facadeClock{},
		Dialogs:    dialogs,
		Events:     facadeEvents{},
		Readiness:  facadeReadiness{},
		Operations: operationPort,
		Library:    facadeOperations{},
		Removal:    facadeOperations{},
		Tags:       facadeOperations{},
		Shelves:    facadeOperations{},
	})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}
	facade := NewApp(core)
	facade.startup(context.Background())
	request := exporter.PreflightRequest{
		SourceType: operations.ExportSourceSelection,
		StoryIDs:   []int64{41, 42},
	}

	prepared := facade.SelectAndPreflightExport(request)
	if prepared.Error != nil ||
		prepared.Preflight == nil ||
		operationPort.request.SourceType != request.SourceType ||
		len(operationPort.request.StoryIDs) != 2 ||
		operationPort.destination != destination {
		t.Fatalf("SelectAndPreflightExport() = %#v", prepared)
	}
	started := facade.StartPreparedExport(prepared.Preflight.PreparationID)
	if started.Error != nil ||
		started.Operation == nil ||
		operationPort.preparationID != prepared.Preflight.PreparationID {
		t.Fatalf("StartPreparedExport() = %#v", started)
	}

	operationPort.snapshot.Status = operations.StatusSucceeded
	operationPort.snapshot.Destination = destination
	revealed := facade.RevealExportDestination(operationID)
	if revealed.Error != nil ||
		!revealed.Success ||
		dialogs.revealed != destination {
		t.Fatalf("RevealExportDestination() = %#v", revealed)
	}
}

func TestAppCanQuitAfterDOMReadyForPackagedSmoke(t *testing.T) {
	t.Parallel()

	core, err := coreapp.New(coreapp.Dependencies{
		Clock:      facadeClock{},
		Dialogs:    facadeDialogs{},
		Events:     facadeEvents{},
		Readiness:  facadeReadiness{},
		Operations: facadeOperations{},
		Library:    facadeOperations{},
		Removal:    facadeOperations{},
		Tags:       facadeOperations{},
		Shelves:    facadeOperations{},
	})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}
	quitCalled := false
	facade := NewApp(core, WithQuitAfterDOMReady(func(context.Context) {
		quitCalled = true
	}))

	facade.startup(context.Background())
	facade.domReady(context.Background())

	if !quitCalled {
		t.Fatal("quit callback was not called")
	}
}
