package app

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/library"
)

type StoryFlagsResponse struct {
	Result *library.FlagResult `json:"result,omitempty"`
	Error  *APIError           `json:"error,omitempty"`
}

type StoryFlagsPort interface {
	SetStoriesBroken(context.Context, []int64, bool) (library.FlagResult, error)
	RestoreStoryFlags(context.Context, []library.FlagChange) (library.FlagResult, error)
}

func (a *Application) SetStoriesBroken(ctx context.Context, ids []int64, broken bool) StoryFlagsResponse {
	return a.mutateStoryFlags(func(port StoryFlagsPort) (library.FlagResult, error) { return port.SetStoriesBroken(ctx, ids, broken) })
}

func (a *Application) RestoreStoryFlags(ctx context.Context, changes []library.FlagChange) StoryFlagsResponse {
	return a.mutateStoryFlags(func(port StoryFlagsPort) (library.FlagResult, error) { return port.RestoreStoryFlags(ctx, changes) })
}

func (a *Application) mutateStoryFlags(mutate func(StoryFlagsPort) (library.FlagResult, error)) StoryFlagsResponse {
	port, ok := a.library.(StoryFlagsPort)
	if !ok || !a.Status().MutationsAllowed {
		return StoryFlagsResponse{Error: NewAPIError(ErrorNotReady, "Story flags are unavailable until storage is ready.")}
	}
	result, err := mutate(port)
	if err != nil {
		switch {
		case errors.Is(err, library.ErrInvalidFlagRequest):
			return StoryFlagsResponse{Error: NewAPIError(ErrorInvalidInput, "Select valid stories to change their broken flags.")}
		case errors.Is(err, library.ErrFlagStoryUnavailable):
			return StoryFlagsResponse{Error: NewAPIError(ErrorInvalidInput, "A story is no longer available. Refresh the collection and try again.")}
		case errors.Is(err, library.ErrFlagRestoreStale):
			return StoryFlagsResponse{Error: NewAPIError(ErrorInvalidInput, "These flags changed after this action. Refresh the collection before retrying.")}
		default:
			return StoryFlagsResponse{Error: NewAPIError(ErrorInternal, "Story flags could not be updated. Try again.")}
		}
	}
	return StoryFlagsResponse{Result: &result}
}

func (r *ImportRuntime) currentStoryFlags() (*library.BrokenFlags, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.flags == nil {
		return nil, ErrImportRuntimeNotReady
	}
	return r.flags, nil
}

func (r *ImportRuntime) SetStoriesBroken(ctx context.Context, ids []int64, broken bool) (library.FlagResult, error) {
	flags, err := r.currentStoryFlags()
	if err != nil {
		return library.FlagResult{}, err
	}
	return flags.Set(ctx, ids, broken)
}

func (r *ImportRuntime) RestoreStoryFlags(ctx context.Context, changes []library.FlagChange) (library.FlagResult, error) {
	flags, err := r.currentStoryFlags()
	if err != nil {
		return library.FlagResult{}, err
	}
	return flags.Restore(ctx, changes)
}
