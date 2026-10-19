package app

import (
	"context"
	"github.com/01max/librairii/internal/library"
	"testing"
	"time"
)

type flagTestPort struct {
	*fakeOperations
	calls int
}

func (p *flagTestPort) SetStoriesBroken(context.Context, []int64, bool) (library.FlagResult, error) {
	p.calls++
	return library.FlagResult{RequestedStories: 1, ChangedStories: 1}, p.err
}
func (p *flagTestPort) RestoreStoryFlags(context.Context, []library.FlagChange) (library.FlagResult, error) {
	p.calls++
	return library.FlagResult{ChangedStories: 1}, p.err
}

func TestStoryFlagBindingsRequireReadinessAndExposeResults(t *testing.T) {
	ctx := context.Background()
	port := &flagTestPort{fakeOperations: &fakeOperations{}}
	application, err := New(Dependencies{Clock: fixedClock{now: time.Now()}, Dialogs: fakeDialogs{}, Events: &fakeEvents{}, Readiness: fakeReadiness{report: ReadinessReport{MutationsAllowed: true}}, Operations: port, Library: port, Removal: port, Tags: port, Shelves: port})
	if err != nil {
		t.Fatal(err)
	}
	if response := application.SetStoriesBroken(ctx, []int64{1}, true); response.Error == nil || port.calls != 0 {
		t.Fatalf("not ready: %#v", response)
	}
	if err := application.Start(ctx); err != nil {
		t.Fatal(err)
	}
	defer application.Stop(ctx)
	if response := application.SetStoriesBroken(ctx, []int64{1}, true); response.Error != nil || response.Result == nil || response.Result.ChangedStories != 1 {
		t.Fatalf("set: %#v", response)
	}
	if response := application.RestoreStoryFlags(ctx, []library.FlagChange{{StoryID: 1, Current: true}}); response.Error != nil || response.Result == nil {
		t.Fatalf("restore: %#v", response)
	}
	for _, failure := range []error{library.ErrInvalidFlagRequest, library.ErrFlagStoryUnavailable, library.ErrFlagRestoreStale} {
		port.err = failure
		if response := application.SetStoriesBroken(ctx, []int64{1}, true); response.Error == nil || response.Result != nil {
			t.Fatalf("failed set: %#v", response)
		}
	}
	port.err = nil
}
