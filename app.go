package main

import (
	"context"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/localization"
	"github.com/01max/librairii/internal/tagging"
)

const frontendRenderedEvent = "frontend:rendered"

// App is the narrow Wails binding facade. Domain behaviour stays in internal
// application services and only stable DTOs cross this boundary.
type App struct {
	libraryGate                sync.RWMutex
	settings                   *settingsController
	core                       *coreapp.Application
	quit                       func(context.Context)
	frontendRendered           func()
	packagedAcceptance         func(string) error
	packagedAcceptanceFinished func(context.Context)
	mu                         sync.RWMutex
	ctx                        context.Context
}

type AppOption func(*App)

func WithQuitAfterDOMReady(quit func(context.Context)) AppOption {
	return func(app *App) {
		app.quit = quit
	}
}

func WithFrontendRendered(callback func()) AppOption {
	return func(app *App) {
		app.frontendRendered = callback
	}
}

func WithPackagedAcceptance(
	record func(string) error,
	finished func(context.Context),
) AppOption {
	return func(app *App) {
		app.packagedAcceptance = record
		app.packagedAcceptanceFinished = finished
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
	if a.settings != nil {
		a.settings.mu.Lock()
		a.settings.changed = a.refreshSettingsMenu
		a.settings.mu.Unlock()
		a.refreshSettingsMenu(ctx)
	}
	a.core.AnnounceReady(ctx)
	if a.quit != nil {
		a.quit(ctx)
	}
}

func (a *App) shutdown(ctx context.Context) {
	a.libraryGate.Lock()
	defer a.libraryGate.Unlock()
	a.core.Stop(ctx)
}

// ApplicationStatus returns a stable lifecycle snapshot for the frontend.
func (a *App) ApplicationStatus() coreapp.StatusResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.StatusResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.StatusResponse()
}

func (a *App) RecoverStorage() coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RecoverStorage(a.runtimeContext())
}

// FrontendRendered acknowledges that React completed its first paint. The
// optional callback is configured only by packaged acceptance.
func (a *App) FrontendRendered() {
	if a.frontendRendered != nil {
		a.frontendRendered()
	}
}

func (a *App) PackagedAcceptanceMode() bool {
	return a.packagedAcceptance != nil
}

func (a *App) RecordPackagedAcceptance(checkpoint string) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	if a.packagedAcceptance == nil {
		return coreapp.MutationResponse{
			Error: coreapp.NewAPIError(
				coreapp.ErrorNotReady,
				"Packaged acceptance is not enabled.",
			),
		}
	}
	if checkpoint == "settings_menu_selected" || checkpoint == "settings_menu_french_selected" {
		if err := automateHostSettingsMenu(a.runtimeContext(), localization.Text(a.interfaceLanguage(), "Settings…")); err != nil {
			return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorInternal, err.Error())}
		}
	}
	if err := a.packagedAcceptance(checkpoint); err != nil {
		return coreapp.MutationResponse{
			Error: coreapp.NewAPIError(
				coreapp.ErrorInvalidInput,
				"Packaged acceptance rejected the checkpoint.",
			),
		}
	}
	if (checkpoint == acceptanceCheckpointComplete ||
		checkpoint == acceptanceCheckpointFailed) &&
		a.packagedAcceptanceFinished != nil {
		a.packagedAcceptanceFinished(a.runtimeContext())
	}
	return coreapp.MutationResponse{Success: true}
}

func (a *App) SelectAndExportDiagnostics() coreapp.DiagnosticExportResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.DiagnosticExportResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.SelectAndExportDiagnostics(a.runtimeContext())
}

// SelectAndImportStories opens the native picker and starts one durable import
// operation without exposing selected filesystem paths to the frontend.
func (a *App) SelectAndImportStories() coreapp.OperationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.SelectAndStartImport(a.runtimeContext())
}

// SelectAndPreflightExport keeps the chosen absolute destination in Go while
// returning only a display label and eligibility report to the frontend.
func (a *App) SelectAndPreflightExport(
	request exporter.PreflightRequest,
) coreapp.ExportPreflightResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.ExportPreflightResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.SelectAndPreflightExport(a.runtimeContext(), request)
}

func (a *App) StartPreparedExport(preparationID string) coreapp.OperationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.StartPreparedExport(a.runtimeContext(), preparationID)
}

func (a *App) RevealExportDestination(operationID string) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RevealExportDestination(a.runtimeContext(), operationID)
}

func (a *App) RefreshOfficialMetadata() coreapp.OperationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RefreshOfficialMetadata(a.runtimeContext())
}

func (a *App) OfficialMetadataStatus() coreapp.MetadataStatusResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MetadataStatusResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.OfficialMetadataStatus(a.runtimeContext())
}

func (a *App) CancelOperation(operationID string) coreapp.OperationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.CancelOperation(a.runtimeContext(), operationID)
}

func (a *App) OperationSnapshot(operationID string) coreapp.OperationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.OperationSnapshot(a.runtimeContext(), operationID)
}

func (a *App) OperationHistory(before string) coreapp.OperationHistoryResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationHistoryResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.OperationHistory(a.runtimeContext(), before)
}

func (a *App) ActiveOperations() coreapp.OperationListResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.OperationListResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ActiveOperations(a.runtimeContext())
}

func (a *App) ListStories(request library.ListRequest) coreapp.LibraryPageResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.LibraryPageResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ListStories(a.runtimeContext(), request)
}

func (a *App) QueryStories(request library.StoryLibraryQuery) coreapp.LibraryPageResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.LibraryPageResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.QueryStories(a.runtimeContext(), request)
}

func (a *App) StoryDetail(storyID int64) coreapp.StoryDetailResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.StoryDetailResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.StoryDetail(a.runtimeContext(), storyID)
}

func (a *App) RemoveStory(storyID int64) coreapp.RemovalResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.RemovalResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RemoveStory(a.runtimeContext(), storyID)
}

func (a *App) ListCollections(includeBroken library.BrokenVisibility) coreapp.CollectionListResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionListResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ListCollections(a.runtimeContext(), includeBroken)
}

func (a *App) CreateCollection(
	name string,
	query library.StoryLibraryQuery,
) coreapp.CollectionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.CreateCollection(a.runtimeContext(), name, query)
}

func (a *App) OpenCollection(
	shelfID int64,
	request library.ListRequest,
) coreapp.CollectionEvaluationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionEvaluationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.OpenCollection(a.runtimeContext(), shelfID, request)
}

func (a *App) RenameCollection(shelfID int64, name string) coreapp.CollectionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RenameCollection(a.runtimeContext(), shelfID, name)
}

func (a *App) DuplicateCollection(shelfID int64, name string) coreapp.CollectionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.DuplicateCollection(a.runtimeContext(), shelfID, name)
}

func (a *App) ReplaceCollectionQuery(
	shelfID int64,
	query library.StoryLibraryQuery,
) coreapp.CollectionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ReplaceCollectionQuery(a.runtimeContext(), shelfID, query)
}

func (a *App) ReorderCollections(orderedIDs []int64) coreapp.CollectionListResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionListResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ReorderCollections(a.runtimeContext(), orderedIDs)
}

func (a *App) DeleteCollection(shelfID int64) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.DeleteCollection(a.runtimeContext(), shelfID)
}

func (a *App) PreviewCollections(
	shelfIDs []int64,
	includeBroken library.BrokenVisibility,
) coreapp.CollectionSelectionPreviewResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.CollectionSelectionPreviewResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.PreviewCollections(a.runtimeContext(), shelfIDs, includeBroken)
}

func (a *App) TagCatalog() coreapp.TagCatalogResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagCatalogResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.TagCatalog(a.runtimeContext())
}

func (a *App) CreateTagDefinition(
	input tagging.CreateDefinition,
) coreapp.TagDefinitionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagDefinitionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.CreateTagDefinition(a.runtimeContext(), input)
}

func (a *App) RenameTagDefinition(
	definitionID int64,
	label string,
) coreapp.TagDefinitionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagDefinitionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RenameTagDefinition(a.runtimeContext(), definitionID, label)
}

func (a *App) RecolorTagDefinition(
	definitionID int64,
	color string,
) coreapp.TagDefinitionResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagDefinitionResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RecolorTagDefinition(a.runtimeContext(), definitionID, color)
}

func (a *App) ReorderTagDefinitions(orderedIDs []int64) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ReorderTagDefinitions(a.runtimeContext(), orderedIDs)
}

func (a *App) PlanTagDefinitionDeletion(
	definitionID int64,
) coreapp.TagDefinitionDeletionPlanResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagDefinitionDeletionPlanResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.PlanTagDefinitionDeletion(a.runtimeContext(), definitionID)
}

func (a *App) DeleteTagDefinition(
	plan tagging.DefinitionDeletionPlan,
) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.DeleteTagDefinition(a.runtimeContext(), plan)
}

func (a *App) CreateTagValue(input tagging.CreateValue) coreapp.TagValueResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagValueResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.CreateTagValue(a.runtimeContext(), input)
}

func (a *App) RenameTagValue(
	valueID int64,
	label string,
) coreapp.TagValueResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagValueResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RenameTagValue(a.runtimeContext(), valueID, label)
}

func (a *App) ReorderTagValues(
	definitionID int64,
	orderedIDs []int64,
) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.ReorderTagValues(a.runtimeContext(), definitionID, orderedIDs)
}

func (a *App) PlanTagValueDeletion(
	valueID int64,
) coreapp.TagValueDeletionPlanResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagValueDeletionPlanResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.PlanTagValueDeletion(a.runtimeContext(), valueID)
}

func (a *App) DeleteTagValue(plan tagging.ValueDeletionPlan) coreapp.MutationResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.DeleteTagValue(a.runtimeContext(), plan)
}

func (a *App) TagAssignmentWorkspace(
	storyIDs []int64,
) coreapp.TagAssignmentWorkspaceResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagAssignmentWorkspaceResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.TagAssignmentWorkspace(a.runtimeContext(), storyIDs)
}

func (a *App) SetBooleanTag(
	storyIDs []int64,
	definitionID int64,
	assigned bool,
) coreapp.TagAssignmentResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagAssignmentResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.SetBooleanTag(a.runtimeContext(), storyIDs, definitionID, assigned)
}

func (a *App) SetChoiceTagValues(
	storyIDs []int64,
	definitionID int64,
	valueIDs []int64,
) coreapp.TagAssignmentResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.TagAssignmentResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
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
	if !a.libraryGate.TryRLock() {
		return coreapp.TagAssignmentResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
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

func (a *App) LibraryCompatibility() coreapp.LibraryCompatibilityResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.LibraryCompatibilityResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.LibraryCompatibility(a.runtimeContext())
}

func (a *App) SetStoriesBroken(storyIDs []int64, broken bool) coreapp.StoryFlagsResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.StoryFlagsResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.SetStoriesBroken(a.runtimeContext(), storyIDs, broken)
}

func (a *App) RestoreStoryFlags(changes []library.FlagChange) coreapp.StoryFlagsResponse {
	if !a.libraryGate.TryRLock() {
		return coreapp.StoryFlagsResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.RUnlock()
	return a.core.RestoreStoryFlags(a.runtimeContext(), changes)
}
