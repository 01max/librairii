package app

import (
	"context"
	"errors"
	"testing"
	"time"
)

type fixedClock struct {
	now time.Time
}

func (c fixedClock) Now() time.Time {
	return c.now
}

type fakeDialogs struct{}

func (fakeDialogs) OpenFiles(context.Context, FileDialogRequest) ([]string, error) {
	return nil, nil
}

func (fakeDialogs) OpenDirectory(context.Context, string) (string, error) {
	return "", nil
}

type emittedEvent struct {
	name    string
	payload any
}

type fakeEvents struct {
	events []emittedEvent
}

func (e *fakeEvents) Emit(_ context.Context, name string, payload any) {
	e.events = append(e.events, emittedEvent{name: name, payload: payload})
}

func TestApplicationLifecycle(t *testing.T) {
	t.Parallel()

	now := time.Date(2026, time.July, 25, 8, 30, 0, 0, time.FixedZone("CEST", 2*60*60))
	events := &fakeEvents{}
	application, err := New(Dependencies{
		Clock:   fixedClock{now: now},
		Dialogs: fakeDialogs{},
		Events:  events,
	})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	if got := application.Status().State; got != StateInitializing {
		t.Fatalf("initial state = %q, want %q", got, StateInitializing)
	}

	if err := application.Start(context.Background()); err != nil {
		t.Fatalf("Start() error = %v", err)
	}

	status := application.Status()
	if status.State != StateReady {
		t.Fatalf("ready state = %q, want %q", status.State, StateReady)
	}
	if status.StartedAt != "2026-07-25T06:30:00Z" {
		t.Fatalf("startedAt = %q", status.StartedAt)
	}

	application.AnnounceReady(context.Background())
	if len(events.events) != 1 || events.events[0].name != EventApplicationReady {
		t.Fatalf("events = %#v", events.events)
	}

	application.Stop(context.Background())
	if got := application.Status().State; got != StateStopped {
		t.Fatalf("stopped state = %q, want %q", got, StateStopped)
	}
}

func TestNewRequiresReplaceablePorts(t *testing.T) {
	t.Parallel()

	_, err := New(Dependencies{})
	if !errors.Is(err, ErrMissingDependency) {
		t.Fatalf("New() error = %v, want ErrMissingDependency", err)
	}
}

func TestAsAPIErrorPreservesStableErrors(t *testing.T) {
	t.Parallel()

	expected := NewAPIError(ErrorConflict, "Already exists.")
	if got := AsAPIError(expected); got != expected {
		t.Fatalf("AsAPIError() = %#v, want original error", got)
	}

	got := AsAPIError(errors.New("private implementation detail"))
	if got.Code != ErrorInternal || got.Message == "private implementation detail" {
		t.Fatalf("AsAPIError() = %#v", got)
	}
}
