package main

import (
	"context"
	"path/filepath"
	"testing"
	"time"
)

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
