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
	state     LifecycleState
	startedAt time.Time
}

func New(deps Dependencies) (*Application, error) {
	if deps.Clock == nil || deps.Dialogs == nil || deps.Events == nil {
		return nil, ErrMissingDependency
	}

	return &Application{
		clock:   deps.Clock,
		dialogs: deps.Dialogs,
		events:  deps.Events,
		state:   StateInitializing,
	}, nil
}

func (a *Application) Start(_ context.Context) error {
	a.mu.Lock()
	defer a.mu.Unlock()

	if a.state == StateReady {
		return nil
	}

	a.startedAt = a.clock.Now().UTC()
	a.state = StateReady
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
	defer a.mu.Unlock()

	a.state = StateStopped
}

func (a *Application) Status() Status {
	a.mu.RLock()
	defer a.mu.RUnlock()

	status := Status{State: a.state}
	if !a.startedAt.IsZero() {
		status.StartedAt = a.startedAt.Format(time.RFC3339Nano)
	}
	return status
}
