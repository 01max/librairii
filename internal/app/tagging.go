package app

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/tagging"
)

func (a *Application) TagCatalog(ctx context.Context) TagCatalogResponse {
	catalog, err := a.tags.Catalog(ctx)
	if err != nil {
		return TagCatalogResponse{Error: taggingAPIError(err)}
	}
	return TagCatalogResponse{Catalog: &catalog}
}

func (a *Application) CreateTagDefinition(
	ctx context.Context,
	input tagging.CreateDefinition,
) TagDefinitionResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagDefinitionResponse{Error: response}
	}
	definition, err := a.tags.CreateDefinition(ctx, input)
	if err != nil {
		return TagDefinitionResponse{Error: taggingAPIError(err)}
	}
	return TagDefinitionResponse{Definition: &definition}
}

func (a *Application) RenameTagDefinition(
	ctx context.Context,
	definitionID int64,
	label string,
) TagDefinitionResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagDefinitionResponse{Error: response}
	}
	definition, err := a.tags.RenameDefinition(ctx, definitionID, label)
	if err != nil {
		return TagDefinitionResponse{Error: taggingAPIError(err)}
	}
	return TagDefinitionResponse{Definition: &definition}
}

func (a *Application) RecolorTagDefinition(
	ctx context.Context,
	definitionID int64,
	color string,
) TagDefinitionResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagDefinitionResponse{Error: response}
	}
	definition, err := a.tags.RecolorDefinition(ctx, definitionID, color)
	if err != nil {
		return TagDefinitionResponse{Error: taggingAPIError(err)}
	}
	return TagDefinitionResponse{Definition: &definition}
}

func (a *Application) ReorderTagDefinitions(
	ctx context.Context,
	orderedIDs []int64,
) MutationResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return MutationResponse{Error: response}
	}
	if _, err := a.tags.ReorderDefinitions(ctx, orderedIDs); err != nil {
		return MutationResponse{Error: taggingAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) PlanTagDefinitionDeletion(
	ctx context.Context,
	definitionID int64,
) TagDefinitionDeletionPlanResponse {
	plan, err := a.tags.PlanDefinitionDeletion(ctx, definitionID)
	if err != nil {
		return TagDefinitionDeletionPlanResponse{Error: taggingAPIError(err)}
	}
	return TagDefinitionDeletionPlanResponse{Plan: &plan}
}

func (a *Application) DeleteTagDefinition(
	ctx context.Context,
	plan tagging.DefinitionDeletionPlan,
) MutationResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return MutationResponse{Error: response}
	}
	if err := a.tags.DeleteDefinition(ctx, plan); err != nil {
		return MutationResponse{Error: taggingAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) CreateTagValue(
	ctx context.Context,
	input tagging.CreateValue,
) TagValueResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagValueResponse{Error: response}
	}
	value, err := a.tags.CreateValue(ctx, input)
	if err != nil {
		return TagValueResponse{Error: taggingAPIError(err)}
	}
	return TagValueResponse{Value: &value}
}

func (a *Application) RenameTagValue(
	ctx context.Context,
	valueID int64,
	label string,
) TagValueResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagValueResponse{Error: response}
	}
	value, err := a.tags.RenameValue(ctx, valueID, label)
	if err != nil {
		return TagValueResponse{Error: taggingAPIError(err)}
	}
	return TagValueResponse{Value: &value}
}

func (a *Application) ReorderTagValues(
	ctx context.Context,
	definitionID int64,
	orderedIDs []int64,
) MutationResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return MutationResponse{Error: response}
	}
	if _, err := a.tags.ReorderValues(ctx, definitionID, orderedIDs); err != nil {
		return MutationResponse{Error: taggingAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) PlanTagValueDeletion(
	ctx context.Context,
	valueID int64,
) TagValueDeletionPlanResponse {
	plan, err := a.tags.PlanValueDeletion(ctx, valueID)
	if err != nil {
		return TagValueDeletionPlanResponse{Error: taggingAPIError(err)}
	}
	return TagValueDeletionPlanResponse{Plan: &plan}
}

func (a *Application) DeleteTagValue(
	ctx context.Context,
	plan tagging.ValueDeletionPlan,
) MutationResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return MutationResponse{Error: response}
	}
	if err := a.tags.DeleteValue(ctx, plan); err != nil {
		return MutationResponse{Error: taggingAPIError(err)}
	}
	return MutationResponse{Success: true}
}

func (a *Application) TagAssignmentWorkspace(
	ctx context.Context,
	storyIDs []int64,
) TagAssignmentWorkspaceResponse {
	workspace, err := a.tags.AssignmentWorkspace(ctx, storyIDs)
	if err != nil {
		return TagAssignmentWorkspaceResponse{Error: taggingAPIError(err)}
	}
	return TagAssignmentWorkspaceResponse{Workspace: &workspace}
}

func (a *Application) SetBooleanTag(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	assigned bool,
) TagAssignmentResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagAssignmentResponse{Error: response}
	}
	result, err := a.tags.SetBulkBoolean(ctx, storyIDs, definitionID, assigned)
	if err != nil {
		return TagAssignmentResponse{Error: taggingAPIError(err)}
	}
	return TagAssignmentResponse{Result: &result}
}

func (a *Application) SetChoiceTagValues(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	valueIDs []int64,
) TagAssignmentResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagAssignmentResponse{Error: response}
	}
	result, err := a.tags.SetBulkChoiceValues(ctx, storyIDs, definitionID, valueIDs)
	if err != nil {
		return TagAssignmentResponse{Error: taggingAPIError(err)}
	}
	return TagAssignmentResponse{Result: &result}
}

func (a *Application) SetChoiceTagValue(
	ctx context.Context,
	storyIDs []int64,
	definitionID int64,
	valueID int64,
	assigned bool,
) TagAssignmentResponse {
	if response := a.tagMutationReadiness(); response != nil {
		return TagAssignmentResponse{Error: response}
	}
	result, err := a.tags.SetBulkChoiceValue(
		ctx,
		storyIDs,
		definitionID,
		valueID,
		assigned,
	)
	if err != nil {
		return TagAssignmentResponse{Error: taggingAPIError(err)}
	}
	return TagAssignmentResponse{Result: &result}
}

func (a *Application) tagMutationReadiness() *APIError {
	if a.Status().MutationsAllowed {
		return nil
	}
	return NewAPIError(ErrorNotReady, "Tags are unavailable until storage is ready.")
}

func taggingAPIError(err error) *APIError {
	switch {
	case errors.Is(err, tagging.ErrInvalidDefinition),
		errors.Is(err, tagging.ErrInvalidValue),
		errors.Is(err, tagging.ErrInvalidAssignment),
		errors.Is(err, tagging.ErrAssignmentKind),
		errors.Is(err, tagging.ErrDerivedAssignment),
		errors.Is(err, tagging.ErrStoryNotFound),
		errors.Is(err, tagging.ErrInvalidOrder),
		errors.Is(err, tagging.ErrInvalidValueOrder):
		return NewAPIError(ErrorInvalidInput, err.Error())
	case errors.Is(err, tagging.ErrDuplicateDefinition),
		errors.Is(err, tagging.ErrDuplicateValue):
		return NewAPIError(ErrorConflict, err.Error())
	case errors.Is(err, tagging.ErrProtectedDefinition):
		return NewAPIError(ErrorConflict, "Built-in and derived tags cannot be changed.")
	case errors.Is(err, tagging.ErrDeletePlanStale),
		errors.Is(err, tagging.ErrValueDeletePlanStale):
		return NewAPIError(ErrorConflict, "Tag assignments changed; review the deletion impact again.")
	case errors.Is(err, tagging.ErrDefinitionNotFound),
		errors.Is(err, tagging.ErrValueNotFound):
		return NewAPIError(ErrorInvalidInput, "The tag no longer exists.")
	default:
		return NewAPIError(ErrorInternal, "The tag change could not be completed.")
	}
}
