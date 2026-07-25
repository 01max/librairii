package main

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	coreapp "github.com/01max/librairii/internal/app"
)

func TestPackagedAcceptanceKeepsFixturePathsInsideGo(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "story.7z")
	destination := filepath.Join(root, "export")
	checkpoints := filepath.Join(root, "checkpoints.log")
	if err := os.WriteFile(source, []byte("synthetic"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(destination, 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("LIBRAIRII_PACKAGED_ACCEPTANCE", "1")
	t.Setenv("LIBRAIRII_ACCEPTANCE_SOURCE", source)
	t.Setenv("LIBRAIRII_ACCEPTANCE_DESTINATION", destination)
	t.Setenv("LIBRAIRII_ACCEPTANCE_CHECKPOINTS", checkpoints)

	acceptance, err := newPackagedAcceptanceFromEnvironment()
	if err != nil {
		t.Fatalf("newPackagedAcceptanceFromEnvironment() error = %v", err)
	}
	paths, err := acceptance.OpenFiles(
		context.Background(),
		coreapp.FileDialogRequest{
			Extensions: []string{".zip", ".7z"},
			Multiple:   true,
		},
	)
	if err != nil || len(paths) != 1 || paths[0] != source {
		t.Fatalf("OpenFiles() = %#v, %v", paths, err)
	}
	selectedDestination, err := acceptance.OpenDirectory(
		context.Background(),
		"Export story archives",
	)
	resolvedDestination, resolveErr := filepath.EvalSymlinks(destination)
	if err != nil ||
		resolveErr != nil ||
		selectedDestination != resolvedDestination {
		t.Fatalf("OpenDirectory() = %q, %v", selectedDestination, err)
	}
	if err := acceptance.RevealDirectory(
		context.Background(),
		destination,
	); err != nil {
		t.Fatalf("RevealDirectory() error = %v", err)
	}
	if err := acceptance.record(acceptanceCheckpointStarted); err != nil {
		t.Fatalf("record() error = %v", err)
	}
	body, err := os.ReadFile(checkpoints)
	if err != nil {
		t.Fatal(err)
	}
	if string(body) != acceptanceCheckpointStarted+"\n" {
		t.Fatalf("checkpoints = %q", body)
	}
	if err := acceptance.record("absolute/private/path"); err == nil {
		t.Fatal("record(invalid checkpoint) error = nil")
	}
}

func TestAppPackagedAcceptanceIsExplicitAndTerminates(t *testing.T) {
	t.Parallel()

	disabled := NewApp(nil)
	if disabled.PackagedAcceptanceMode() {
		t.Fatal("packaged acceptance enabled without an explicit option")
	}
	if response := disabled.RecordPackagedAcceptance(
		acceptanceCheckpointStarted,
	); response.Error == nil {
		t.Fatal("disabled packaged acceptance accepted a checkpoint")
	}

	var checkpoints []string
	finished := false
	enabled := NewApp(nil, WithPackagedAcceptance(
		func(checkpoint string) error {
			checkpoints = append(checkpoints, checkpoint)
			return nil
		},
		func(context.Context) {
			finished = true
		},
	))
	if !enabled.PackagedAcceptanceMode() {
		t.Fatal("packaged acceptance option did not enable the bound driver")
	}
	response := enabled.RecordPackagedAcceptance(acceptanceCheckpointComplete)
	if response.Error != nil || !response.Success || !finished {
		t.Fatalf("RecordPackagedAcceptance() = %#v, finished = %v", response, finished)
	}
	if strings.Join(checkpoints, ",") != acceptanceCheckpointComplete {
		t.Fatalf("checkpoints = %v", checkpoints)
	}
}
