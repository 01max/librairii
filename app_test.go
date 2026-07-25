package main

import (
	"context"
	"testing"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/removal"
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

type facadeEvents struct{}

func (facadeEvents) Emit(context.Context, string, any) {}

type facadeReadiness struct{}

func (facadeReadiness) Check(context.Context) (coreapp.ReadinessReport, error) {
	return coreapp.ReadinessReport{MutationsAllowed: true}, nil
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

func (facadeOperations) Close() error {
	return nil
}

func (facadeOperations) List(
	context.Context,
	library.ListRequest,
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
