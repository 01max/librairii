package artwork

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/storage"
)

func TestRepositoryPublishesIdempotentRelativeArtwork(t *testing.T) {
	t.Parallel()

	repository, layout := newTestRepository(t)
	content := testfixture.PNG()
	relative, err := repository.Publish(testfixture.StoryUUID, "image/png", content)
	if err != nil {
		t.Fatalf("Publish() error = %v", err)
	}
	if filepath.IsAbs(relative) || !filepath.IsLocal(filepath.FromSlash(relative)) {
		t.Fatalf("Publish() path = %q", relative)
	}
	second, err := repository.Publish(testfixture.StoryUUID, "image/png", content)
	if err != nil || second != relative {
		t.Fatalf("Publish(second) = %q, %v", second, err)
	}

	resolved, err := repository.Resolve(relative)
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	bytes, err := os.ReadFile(resolved)
	if err != nil {
		t.Fatal(err)
	}
	if string(bytes) != string(content) {
		t.Fatal("published artwork bytes changed")
	}
	if filepath.Dir(filepath.Dir(filepath.Dir(resolved))) != layout.Catalog {
		t.Fatalf("published artwork path = %q", resolved)
	}
}

func TestRepositoryRejectsInvalidArtworkAndRemovesManagedBytes(t *testing.T) {
	t.Parallel()

	repository, _ := newTestRepository(t)
	for _, input := range []struct {
		uuid      string
		mediaType string
		content   []byte
	}{
		{uuid: "not-a-uuid", mediaType: "image/png", content: []byte("image")},
		{uuid: testfixture.StoryUUID, mediaType: "image/jpeg", content: []byte("image")},
		{uuid: testfixture.StoryUUID, mediaType: "image/png"},
	} {
		if _, err := repository.Publish(
			input.uuid,
			input.mediaType,
			input.content,
		); !errors.Is(err, ErrInvalidArtwork) {
			t.Fatalf("Publish(%#v) error = %v", input, err)
		}
	}

	relative, err := repository.Publish(
		testfixture.StoryUUID,
		"image/png",
		testfixture.PNG(),
	)
	if err != nil {
		t.Fatal(err)
	}
	resolved, err := repository.Resolve(relative)
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.Remove(relative); err != nil {
		t.Fatalf("Remove() error = %v", err)
	}
	if _, err := os.Stat(resolved); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("Remove() left artwork: %v", err)
	}
	if err := repository.Remove("../outside"); !errors.Is(err, ErrInvalidArtwork) {
		t.Fatalf("Remove(escape) error = %v", err)
	}
}

func newTestRepository(t *testing.T) (*Repository, storage.Layout) {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	return NewRepository(layout), layout
}
