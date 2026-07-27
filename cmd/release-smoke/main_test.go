package main

import (
	"context"
	"errors"
	"path/filepath"
	"strings"
	"testing"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
)

type smokeReadinessFunc func(context.Context) (coreapp.ReadinessReport, error)

func (f smokeReadinessFunc) Check(
	ctx context.Context,
) (coreapp.ReadinessReport, error) {
	return f(ctx)
}

type smokeRuntimeStarterFunc func(context.Context) error

func (f smokeRuntimeStarterFunc) Start(ctx context.Context) error {
	return f(ctx)
}

func TestReleaseSmokeCoversTheCompleteHeadlessComposition(t *testing.T) {
	t.Parallel()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	result, err := runReleaseSmoke(
		ctx,
		filepath.Join(t.TempDir(), "application-data"),
		catalogFixture,
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.ImportedStories != 2 ||
		result.ExportScopes != 4 ||
		!result.Recovered ||
		!result.OfflineRestart {
		t.Fatalf("release smoke result = %#v", result)
	}
}

func TestPrepareSmokeStartupPreservesReadinessCause(t *testing.T) {
	t.Parallel()

	cause := errors.New("open Windows SQLite file URI")
	err := prepareSmokeStartup(
		context.Background(),
		smokeReadinessFunc(
			func(context.Context) (coreapp.ReadinessReport, error) {
				return coreapp.ReadinessReport{}, cause
			},
		),
		smokeRuntimeStarterFunc(func(context.Context) error {
			t.Fatal("runtime started after readiness failure")
			return nil
		}),
	)
	if !errors.Is(err, cause) {
		t.Fatalf("prepareSmokeStartup() error = %v, want cause %v", err, cause)
	}
	if !strings.Contains(err.Error(), "storage readiness") {
		t.Fatalf("prepareSmokeStartup() error = %q", err)
	}
}

func TestPrepareSmokeStartupPreservesRuntimeCause(t *testing.T) {
	t.Parallel()

	cause := errors.New("seed Windows SQLite data")
	err := prepareSmokeStartup(
		context.Background(),
		smokeReadinessFunc(
			func(context.Context) (coreapp.ReadinessReport, error) {
				return coreapp.ReadinessReport{MutationsAllowed: true}, nil
			},
		),
		smokeRuntimeStarterFunc(func(context.Context) error {
			return cause
		}),
	)
	if !errors.Is(err, cause) {
		t.Fatalf("prepareSmokeStartup() error = %v, want cause %v", err, cause)
	}
	if !strings.Contains(err.Error(), "release smoke runtime") {
		t.Fatalf("prepareSmokeStartup() error = %q", err)
	}
}
