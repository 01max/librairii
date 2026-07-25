package platform

import (
	"context"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/storage"
)

type StorageReadiness struct {
	override string

	mu     sync.RWMutex
	layout storage.Layout
}

func NewStorageReadiness(override string) *StorageReadiness {
	return &StorageReadiness{override: override}
}

func (r *StorageReadiness) Check(ctx context.Context) (coreapp.ReadinessReport, error) {
	root, err := storage.ResolveRoot(r.override)
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}
	layout, err := storage.Initialize(root)
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}

	report, err := storage.NewReadinessChecker(layout, storage.FileSchemaProbe{}).Check(ctx)
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}

	r.mu.Lock()
	r.layout = layout
	r.mu.Unlock()

	issues := make([]coreapp.ReadinessIssue, 0, len(report.Issues))
	for _, issue := range report.Issues {
		issues = append(issues, coreapp.ReadinessIssue{Code: string(issue.Code)})
	}
	return coreapp.ReadinessReport{
		MutationsAllowed: report.MutationsAllowed,
		Issues:           issues,
	}, nil
}

func (r *StorageReadiness) Layout() storage.Layout {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.layout
}
