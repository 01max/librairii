package exporter

import (
	"context"
	"errors"

	"github.com/01max/librairii/internal/library"
)

var ErrPreparedExportStale = errors.New("stories changed after export preview; refresh the preview")

type exportStateResolver interface {
	ExportStates(context.Context, []string) (map[string]library.ExportState, error)
}

func (s *PreflightService) ValidatePrepared(ctx context.Context, report PreflightReport) error {
	if len(report.Scope.Stories) == 0 {
		return nil
	}
	resolver, ok := s.resolver.library.(exportStateResolver)
	if !ok {
		return ErrMissingLibrary
	}
	uuids := make([]string, 0, len(report.Scope.Stories))
	for _, story := range report.Scope.Stories {
		uuids = append(uuids, story.UUID)
	}
	states, err := resolver.ExportStates(ctx, uuids)
	if err != nil {
		return err
	}
	for _, story := range report.Scope.Stories {
		current := states[story.UUID]
		if current.ID != story.ID || current.Broken != story.Broken {
			return ErrPreparedExportStale
		}
	}
	return nil
}
