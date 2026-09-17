package app

import (
	"fmt"

	"github.com/01max/librairii/internal/metadata"
)

// CompositionStorage owns the local database and its readiness lifecycle.
type CompositionStorage interface {
	StorageProvider
	ReadinessPort
	ResourcePort
}

type CompositionOptions struct {
	Storage         CompositionStorage
	Clock           Clock
	Dialogs         DialogPort
	Events          EventPort
	MetadataFetcher metadata.CatalogFetcher
	Workers         int
}

type Composition struct {
	Application *Application
	Runtime     *ImportRuntime
}

// NewComposition wires the application once for both native and test hosts.
// The Application owns Storage and Runtime and closes them when it stops.
func NewComposition(options CompositionOptions) (*Composition, error) {
	if options.Storage == nil || options.Clock == nil ||
		options.Dialogs == nil || options.Events == nil ||
		options.MetadataFetcher == nil || options.Workers < 1 {
		return nil, ErrMissingDependency
	}
	runtime, err := NewImportRuntime(
		options.Storage,
		options.Clock,
		options.Events,
		options.Workers,
		WithMetadataFetcher(options.MetadataFetcher),
	)
	if err != nil {
		return nil, fmt.Errorf("construct import runtime: %w", err)
	}
	application, err := New(Dependencies{
		Clock:       options.Clock,
		Dialogs:     options.Dialogs,
		Events:      options.Events,
		Readiness:   options.Storage,
		Operations:  runtime,
		Library:     runtime,
		Removal:     runtime,
		Tags:        runtime,
		Shelves:     runtime,
		Diagnostics: runtime,
		Resources:   []ResourcePort{options.Storage},
	})
	if err != nil {
		return nil, fmt.Errorf("construct application: %w", err)
	}
	return &Composition{Application: application, Runtime: runtime}, nil
}
