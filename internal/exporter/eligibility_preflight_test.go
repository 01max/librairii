package exporter

import (
	"context"
	"testing"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/storage"
)

func TestPreflightExplainsBrokenAndUnavailableWithoutReservingFilenames(t *testing.T) {
	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	broken := writeManagedExportStory(t, layout, 1, "same.zip", []byte("broken"))
	broken.Broken = true
	ready := writeManagedExportStory(t, layout, 2, "same.zip", []byte("ready"))
	missing := library.ExportStory{UUID: "323e4567-e89b-42d3-a456-426614174000", Title: "Missing"}
	resolver, err := NewResolver(&fakeLibraryResolver{stories: exportStories(1)}, fakeShelfResolver{})
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}
	report, err := service.inspect(context.Background(), Scope{Source: operations.ExportSource{Type: operations.ExportSourceCuratedShelf, ShelfIDs: []int64{1}, ShelfNames: []string{"Approved"}}, Stories: []library.ExportStory{broken, ready, missing}}, t.TempDir())
	if err != nil || report.ResolvedCount != 3 || report.ReadyCount != 1 || !report.CanExport || report.TotalBytes != ready.ByteSize {
		t.Fatalf("preflight: %#v %v", report, err)
	}
	if report.Items[0].Issue == nil || report.Items[0].Issue.Code != IssueStoryMarkedBroken || report.Items[1].Disposition != DispositionReady || report.Items[2].Issue == nil || report.Items[2].Issue.Code != IssueStoryUnavailable {
		t.Fatalf("missing reasons: %#v", report.Items)
	}
	work := report.OperationItems()
	if work[0].PlannedStatus != operations.ItemSkipped || work[0].Item.ExportSkipCode != "story_marked_broken" || work[0].Item.TotalBytes != 0 || work[2].Item.StoryID != 0 || work[2].Item.ExportSkipCode != "story_unavailable" {
		t.Fatalf("unsafe operation plan: %#v", work)
	}
	allSkipped, err := service.inspect(context.Background(), Scope{Stories: []library.ExportStory{broken, missing}}, t.TempDir())
	if err != nil || allSkipped.CanExport || !allSkipped.Blocked || allSkipped.ReadyCount != 0 {
		t.Fatalf("empty operation allowed: %#v %v", allSkipped, err)
	}
}
