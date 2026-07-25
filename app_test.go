package main

import (
	"context"
	"testing"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
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

func TestAppExposesTypedLifecycleStatus(t *testing.T) {
	t.Parallel()

	core, err := coreapp.New(coreapp.Dependencies{
		Clock:   facadeClock{},
		Dialogs: facadeDialogs{},
		Events:  facadeEvents{},
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
