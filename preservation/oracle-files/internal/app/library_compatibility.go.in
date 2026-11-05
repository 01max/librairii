package app

import (
	"context"
	"github.com/01max/librairii/internal/library"
)

type LibraryCompatibilityResponse struct {
	Compatibility library.CompatibilityInfo `json:"compatibility"`
	Error         *APIError                 `json:"error,omitempty"`
}

func (a *Application) LibraryCompatibility(ctx context.Context) LibraryCompatibilityResponse {
	provider, ok := a.library.(interface {
		CompatibilityInfo(context.Context) (library.CompatibilityInfo, error)
	})
	if !ok {
		return LibraryCompatibilityResponse{}
	}
	result, err := provider.CompatibilityInfo(ctx)
	if err != nil {
		return LibraryCompatibilityResponse{Error: libraryAPIError(err)}
	}
	return LibraryCompatibilityResponse{Compatibility: result}
}

func (r *ImportRuntime) CompatibilityInfo(ctx context.Context) (library.CompatibilityInfo, error) {
	query, err := r.currentQuery()
	if err != nil {
		return library.CompatibilityInfo{}, err
	}
	return query.CompatibilityInfo(ctx)
}
