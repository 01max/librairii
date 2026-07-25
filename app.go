package main

import (
	"context"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/tagging"
)

// App is the narrow Wails binding facade. Domain behaviour stays in internal
// application services and only stable DTOs cross this boundary.
type App struct {
	core *coreapp.Application
	quit func(context.Context)
	mu   sync.RWMutex
	ctx  context.Context
}

type AppOption func(*App)

func WithQuitAfterDOMReady(quit func(context.Context)) AppOption {
	return func(app *App) {
		app.quit = quit
	}
}

func NewApp(core *coreapp.Application, options ...AppOption) *App {
	app := &App{core: core}
	for _, option := range options {
		option(app)
	}
	return app
}

func (a *App) startup(ctx context.Context) {
	a.mu.Lock()
	a.ctx = ctx
	a.mu.Unlock()
	_ = a.core.Start(ctx)
}

func (a *App) domReady(ctx context.Context) {
	a.core.AnnounceReady(ctx)
	if a.quit != nil {
		a.quit(ctx)
	}
}

func (a *App) shutdown(ctx context.Context) {
	a.core.Stop(ctx)
}

// ApplicationStatus returns a stable lifecycle snapshot for the frontend.
func (a *App) ApplicationStatus() coreapp.StatusResponse {
	return a.core.StatusResponse()
}

// SelectAndImportStories opens the native picker and starts one durable import
// operation without exposing selected filesystem paths to the frontend.
func (a *App) SelectAndImportStories() coreapp.OperationResponse {
	return a.core.SelectAndStartImport(a.runtimeContext())
}

// SelectAndPreflightExport keeps the chosen absolute destination in Go while
// returning only a display label and eligibility report to the frontend.
func (a *App) SelectAndPreflightExport(
	request exporter.PreflightRequest,
) coreapp.ExportPreflightResponse {
	return a.core.SelectAndPreflightExport(a.runtimeContext(), request)
}

func (a *App) StartPreparedExport(preparationID string) coreapp.OperationResponse {
	return a.core.StartPreparedExport(a.runtimeContext(), preparationID)
}

func (a *App) RevealExportDestination(operationID string) coreapp.MutationResponse {
	return a.core.RevealExportDestination(a.runtimeContext(), operationID)
}

func (a *App) RefreshOfficialMetadata() coreapp.OperationResponse {
	return a.core.RefreshOfficialMetadata(a.runtimeContext())
}

func (a *App) OfficialMetadataStatus() coreapp.MetadataStatusResponse {
	return a.core.OfficialMetadataStatus(a.runtimeContext())
}

func (a *App) CancelOperation(operationID string) coreapp.OperationResponse {
	return a.core.CancelOperation(a.runtimeContext(), operationID)
}

func (a *App) OperationSnapshot(operationID string) coreapp.OperationResponse {
	return a.core.OperationSnapshot(a.runtimeContext(), operationID)
}

func (a *App) ActiveOperations() coreapp.OperationListResponse {
	return a.core.ActiveOperations(a.runtimeContext())
}

func (a *App) ListStories(request library.ListRequest) coreapp.LibraryPageResponse {
	return a.core.ListStories(a.runtimeContext(), request)
}

func (a *App) QueryStories(request library.StoryLibraryQuery) coreapp.LibraryPageResponse {
	return a.core.QueryStories(a.runtimeContext(), request)
}

func (a *App) StoryDetail(storyID int64) coreapp.StoryDetailResponse {
	return a.core.StoryDetail(a.runtimeContext(), storyID)
}

func (a *App) RemoveStory(storyID int64) coreapp.RemovalResponse {
	return a.core.RemoveStory(a.runtimeContext(), storyID)
}

func (a *App) ListShelves() coreapp.ShelfListResponse {
	return a.core.ListShelves(a.runtimeContext())
}

func (a *App) CreateShelf(
	name string,
	query library.StoryLibraryQuery,
) coreapp.ShelfResponse {
	return a.core.CreateShelf(a.runtimeContext(), name, query)
}

func (a *App) OpenShelf(
	shelfID int64,
	request library.ListRequest,
) coreapp.ShelfEvaluationResponse {
	return a.core.OpenShelf(a.runtimeContext(), shelfID, request)
}

func (a *App) RenameShelf(shelfID int64, name string) coreapp.ShelfResponse {
	return a.core.RenameShelf(a.runtimeContext(), shelfID, name)
}

func (a *App) DuplicateShelf(shelfID int64, name string) coreapp.ShelfResponse {
	return a.core.DuplicateShelf(a.runtimeContext(), shelfID, name)
}

func (a *App) ReplaceShelfQuery(
	shelfID int64,
	query library.StoryLibraryQuery,
) coreapp.ShelfResponse {
	return a.core.ReplaceShelfQuery(a.runtimeContext(), shelfID, query)
}

func (a *App) ReorderShelves(orderedIDs []int64) coreapp.ShelfListResponse {
	return a.core.ReorderShelves(a.runtimeContext(), orderedIDs)
}

func (a *App) DeleteShelf(shelfID int64) coreapp.MutationResponse {
	return a.core.DeleteShelf(a.runtimeContext(), shelfID)
}

func (a *App) PreviewShelves(
	shelfIDs []int64,
) coreapp.ShelfSelectionPreviewResponse {
	return a.core.PreviewShelves(a.runtimeContext(), shelfIDs)
}

func (a *App) TagCatalog() coreapp.TagCatalogResponse {
	return a.core.TagCatalog(a.runtimeContext())
}

func (a *App) CreateTagDefinition(
	input tagging.CreateDefinition,
) coreapp.TagDefinitionResponse {
	return a.core.CreateTagDefinition(a.runtimeContext(), input)
}

func (a *App) RenameTagDefinition(
	definitionID int64,
	label string,
) coreapp.TagDefinitionResponse {
	return a.core.RenameTagDefinition(a.runtimeContext(), definitionID, label)
}

func (a *App) RecolorTagDefinition(
	definitionID int64,
	color string,
) coreapp.TagDefinitionResponse {
	return a.core.RecolorTagDefinition(a.runtimeContext(), definitionID, color)
}

func (a *App) ReorderTagDefinitions(orderedIDs []int64) coreapp.MutationResponse {
	return a.core.ReorderTagDefinitions(a.runtimeContext(), orderedIDs)
}

func (a *App) PlanTagDefinitionDeletion(
	definitionID int64,
) coreapp.TagDefinitionDeletionPlanResponse {
	return a.core.PlanTagDefinitionDeletion(a.runtimeContext(), definitionID)
}

func (a *App) DeleteTagDefinition(
	plan tagging.DefinitionDeletionPlan,
) coreapp.MutationResponse {
	return a.core.DeleteTagDefinition(a.runtimeContext(), plan)
}

func (a *App) CreateTagValue(input tagging.CreateValue) coreapp.TagValueResponse {
	return a.core.CreateTagValue(a.runtimeContext(), input)
}

func (a *App) RenameTagValue(
	valueID int64,
	label string,
) coreapp.TagValueResponse {
	return a.core.RenameTagValue(a.runtimeContext(), valueID, label)
}

func (a *App) ReorderTagValues(
	definitionID int64,
	orderedIDs []int64,
) coreapp.MutationResponse {
	return a.core.ReorderTagValues(a.runtimeContext(), definitionID, orderedIDs)
}

func (a *App) PlanTagValueDeletion(
	valueID int64,
) coreapp.TagValueDeletionPlanResponse {
	return a.core.PlanTagValueDeletion(a.runtimeContext(), valueID)
}

func (a *App) DeleteTagValue(plan tagging.ValueDeletionPlan) coreapp.MutationResponse {
	return a.core.DeleteTagValue(a.runtimeContext(), plan)
}

func (a *App) TagAssignmentWorkspace(
	storyIDs []int64,
) coreapp.TagAssignmentWorkspaceResponse {
	return a.core.TagAssignmentWorkspace(a.runtimeContext(), storyIDs)
}

func (a *App) SetBooleanTag(
	storyIDs []int64,
	definitionID int64,
	assigned bool,
) coreapp.TagAssignmentResponse {
	return a.core.SetBooleanTag(a.runtimeContext(), storyIDs, definitionID, assigned)
}

func (a *App) SetChoiceTagValues(
	storyIDs []int64,
	definitionID int64,
	valueIDs []int64,
) coreapp.TagAssignmentResponse {
	return a.core.SetChoiceTagValues(
		a.runtimeContext(),
		storyIDs,
		definitionID,
		valueIDs,
	)
}

func (a *App) SetChoiceTagValue(
	storyIDs []int64,
	definitionID int64,
	valueID int64,
	assigned bool,
) coreapp.TagAssignmentResponse {
	return a.core.SetChoiceTagValue(
		a.runtimeContext(),
		storyIDs,
		definitionID,
		valueID,
		assigned,
	)
}

func (a *App) runtimeContext() context.Context {
	a.mu.RLock()
	defer a.mu.RUnlock()
	if a.ctx == nil {
		return context.Background()
	}
	return a.ctx
}
