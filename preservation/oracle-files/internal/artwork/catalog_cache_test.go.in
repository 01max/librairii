package artwork

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/storage"
)

func TestCatalogRepositoryPublishesAndValidatesOpaqueArtwork(t *testing.T) {
	t.Parallel()

	repository, layout := newTestRepository(t)
	artworkID := strings.Repeat("a", 64)
	content := testfixture.PNG()
	relative, asset, err := repository.PublishCatalog(
		artworkID,
		"image/png",
		content,
		int64(len(content)),
	)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(relative, "catalog/official/"+artworkID+"/") ||
		asset.ContentType != "image/png" ||
		len(asset.SHA256) != 64 {
		t.Fatalf("PublishCatalog() = %q, %#v", relative, asset)
	}
	loaded, err := repository.LoadCatalog(metadata.CatalogArtwork{
		ID:          artworkID,
		ManagedPath: relative,
		ContentType: "image/png",
		SHA256:      asset.SHA256,
		ByteSize:    int64(len(content)),
	}, int64(len(content)))
	if err != nil {
		t.Fatal(err)
	}
	if string(loaded.Content) != string(content) {
		t.Fatal("LoadCatalog() changed artwork bytes")
	}
	resolved := filepath.Join(layout.Root, filepath.FromSlash(relative))
	if info, err := os.Stat(resolved); err != nil || !info.Mode().IsRegular() {
		t.Fatalf("published artwork = %#v, %v", info, err)
	}
}

func TestCatalogRepositoryRejectsInvalidContentAndSymlinkedAncestor(t *testing.T) {
	t.Parallel()

	repository, layout := newTestRepository(t)
	artworkID := strings.Repeat("b", 64)
	for _, input := range []struct {
		id        string
		mediaType string
		content   []byte
		limit     int64
	}{
		{id: "catalog-path", mediaType: "image/png", content: testfixture.PNG(), limit: 1024},
		{id: artworkID, mediaType: "text/plain", content: testfixture.PNG(), limit: 1024},
		{id: artworkID, mediaType: "image/png", content: []byte("not an image"), limit: 1024},
		{id: artworkID, mediaType: "image/png", content: testfixture.PNG(), limit: 1},
	} {
		if _, _, err := repository.PublishCatalog(
			input.id,
			input.mediaType,
			input.content,
			input.limit,
		); !errors.Is(err, ErrInvalidArtwork) {
			t.Fatalf("PublishCatalog(%#v) error = %v", input, err)
		}
	}

	outside := t.TempDir()
	redirect := filepath.Join(layout.Catalog, "official", artworkID)
	if err := os.MkdirAll(filepath.Dir(redirect), storage.DirectoryMode); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, redirect); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	if _, _, err := repository.PublishCatalog(
		artworkID,
		"image/png",
		testfixture.PNG(),
		DefaultMaximumBytes,
	); err == nil {
		t.Fatal("PublishCatalog(symlink ancestor) error = nil")
	}
	entries, err := os.ReadDir(outside)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("catalog artwork escaped through symlink: %#v", entries)
	}
}
