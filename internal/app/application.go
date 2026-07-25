package app

import (
	"context"
	"errors"
	"sync"
	"time"
)

const EventApplicationReady = "application:ready"

var ErrMissingDependency = errors.New("application dependency is required")

type Application struct {
	mu        sync.RWMutex
	clock     Clock
	dialogs   DialogPort
	events    EventPort
	readiness ReadinessPort
	resources []ResourcePort
	state     LifecycleState
	startedAt time.Time
	ready     bool
	lastError *APIError
}

func New(deps Dependencies) (*Application, error) {
	if deps.Clock == nil || deps.Dialogs == nil || deps.Events == nil || deps.Readiness == nil {
		return nil, ErrMissingDependency
	}

	return &Application{
		clock:     deps.Clock,
		dialogs:   deps.Dialogs,
		events:    deps.Events,
		readiness: deps.Readiness,
		resources: append([]ResourcePort(nil), deps.Resources...),
		state:     StateInitializing,
	}, nil
}

func (a *Application) Start(ctx context.Context) error {
	report, err := a.readiness.Check(ctx)

	a.mu.Lock()
	defer a.mu.Unlock()

	if a.state == StateReady || a.state == StateRecovery {
		return nil
	}

	a.startedAt = a.clock.Now().UTC()
	if err != nil {
		a.state = StateRecovery
		a.ready = false
		a.lastError = NewAPIError(ErrorStorageUnavailable, "Application storage is unavailable.")
		return nil
	}
	if !report.MutationsAllowed {
		a.state = StateRecovery
		a.ready = false
		a.lastError = NewAPIError(ErrorStorageUnavailable, "Application storage requires recovery.")
		return nil
	}

	a.state = StateReady
	a.ready = true
	a.lastError = nil
	return nil
}

func (a *Application) AnnounceReady(ctx context.Context) {
	status := a.Status()
	if status.State == StateReady {
		a.events.Emit(ctx, EventApplicationReady, status)
	}
}

func (a *Application) Stop(_ context.Context) {
	a.mu.Lock()
	a.state = StateStopped
	a.ready = false
	resources := append([]ResourcePort(nil), a.resources...)
	a.mu.Unlock()

	for index := len(resources) - 1; index >= 0; index-- {
		_ = resources[index].Close()
	}
}

func (a *Application) Status() Status {
	a.mu.RLock()
	defer a.mu.RUnlock()

	return a.statusLocked()
}

func (a *Application) StatusResponse() StatusResponse {
	a.mu.RLock()
	defer a.mu.RUnlock()

	return StatusResponse{Status: a.statusLocked(), Error: a.lastError}
}

func (a *Application) statusLocked() Status {
	status := Status{
		State:            a.state,
		MutationsAllowed: a.ready,
	}
	if !a.startedAt.IsZero() {
		status.StartedAt = a.startedAt.Format(time.RFC3339Nano)
	}
	return status
}
