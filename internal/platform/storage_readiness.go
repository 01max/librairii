package platform

import (
	"context"
	"database/sql"
	"path/filepath"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/storage"
)

type StorageReadiness struct {
	override string

	mu     sync.RWMutex
	layout storage.Layout
	db     *database.Database
}

func NewStorageReadiness(override string) *StorageReadiness {
	return &StorageReadiness{override: override}
}

func (r *StorageReadiness) Check(ctx context.Context) (coreapp.ReadinessReport, error) {
	r.mu.RLock()
	if r.db != nil {
		r.mu.RUnlock()
		return coreapp.ReadinessReport{MutationsAllowed: true}, nil
	}
	r.mu.RUnlock()

	root, err := storage.ResolveRoot(r.override)
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}
	layout, err := storage.Initialize(root)
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}

	report, err := storage.NewReadinessChecker(layout, sqliteSchemaProbe{}).Check(ctx)
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}

	issues := make([]coreapp.ReadinessIssue, 0, len(report.Issues))
	for _, issue := range report.Issues {
		issues = append(issues, coreapp.ReadinessIssue{Code: string(issue.Code)})
	}
	applicationReport := coreapp.ReadinessReport{
		MutationsAllowed: report.MutationsAllowed,
		Issues:           issues,
	}
	if !report.MutationsAllowed {
		return applicationReport, nil
	}

	connection, err := database.Open(ctx, filepath.Join(layout.Database, "librairii.sqlite3"))
	if err != nil {
		return coreapp.ReadinessReport{}, err
	}

	r.mu.Lock()
	if r.db != nil {
		r.mu.Unlock()
		_ = connection.Close()
		return coreapp.ReadinessReport{MutationsAllowed: true}, nil
	}
	r.layout = layout
	r.db = connection
	r.mu.Unlock()
	return applicationReport, nil
}

func (r *StorageReadiness) Layout() storage.Layout {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.layout
}

func (r *StorageReadiness) SQL() *sql.DB {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.db == nil {
		return nil
	}
	return r.db.SQL()
}

func (r *StorageReadiness) Close() error {
	r.mu.Lock()
	connection := r.db
	r.db = nil
	r.mu.Unlock()
	if connection == nil {
		return nil
	}
	return connection.Close()
}

type sqliteSchemaProbe struct{}

func (sqliteSchemaProbe) Inspect(ctx context.Context, path string) (storage.SchemaIdentity, error) {
	identity, err := (database.SchemaProbe{}).Inspect(ctx, path)
	if err != nil {
		return storage.SchemaIdentity{}, err
	}
	switch identity {
	case database.IdentityAbsent:
		return storage.SchemaIdentity{State: storage.SchemaAbsent}, nil
	case database.IdentityCompatible:
		return storage.SchemaIdentity{State: storage.SchemaCompatible}, nil
	default:
		return storage.SchemaIdentity{State: storage.SchemaForeign}, nil
	}
}
