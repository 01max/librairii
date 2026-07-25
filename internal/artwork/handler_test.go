package artwork

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/storage"
)

type testStorageProvider struct {
	layout   storage.Layout
	database *sql.DB
}

func (p testStorageProvider) Layout() storage.Layout {
	return p.layout
}

func (p testStorageProvider) SQL() *sql.DB {
	return p.database
}

type artworkFetcherFunc func(
	context.Context,
	string,
	int64,
) (lunii.ArtworkPayload, error)

func (f artworkFetcherFunc) FetchArtwork(
	ctx context.Context,
	sourceURL string,
	maximumBytes int64,
) (lunii.ArtworkPayload, error) {
	return f(ctx, sourceURL, maximumBytes)
}

func TestAssetHandlerLazilyCachesOpaqueOfficialArtwork(t *testing.T) {
	t.Parallel()

	provider, repository, artworkID, sourceURL := openAssetHandlerFixture(t)
	content := testfixture.PNG()
	var fetches atomic.Int32
	handler, err := NewAssetHandler(provider, artworkFetcherFunc(func(
		ctx context.Context,
		gotURL string,
		maximumBytes int64,
	) (lunii.ArtworkPayload, error) {
		fetches.Add(1)
		if ctx.Err() != nil || gotURL != sourceURL || maximumBytes != DefaultMaximumBytes {
			t.Fatalf("FetchArtwork() = %q, %d, %v", gotURL, maximumBytes, ctx.Err())
		}
		return lunii.ArtworkPayload{
			Content:      content,
			ContentType:  "image/png",
			ETag:         `"fixture"`,
			LastModified: "Sat, 25 Jul 2026 10:00:00 GMT",
		}, nil
	}))
	if err != nil {
		t.Fatal(err)
	}
	handler.now = func() time.Time {
		return time.Date(2026, time.July, 25, 15, 0, 0, 0, time.UTC)
	}

	for iteration := 0; iteration < 2; iteration++ {
		response := requestArtwork(handler, http.MethodGet, assetPathPrefix+artworkID)
		if response.Code != http.StatusOK ||
			response.Header().Get("Content-Type") != "image/png" ||
			response.Header().Get("X-Content-Type-Options") != "nosniff" ||
			response.Header().Get("ETag") == "" ||
			string(response.Body.Bytes()) != string(content) {
			t.Fatalf("GET response = %d, %#v, %q", response.Code, response.Header(), response.Body.String())
		}
	}
	if fetches.Load() != 1 {
		t.Fatalf("FetchArtwork() count = %d", fetches.Load())
	}
	record, err := repository.Artwork(context.Background(), artworkID)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(record.ManagedPath, "catalog/official/"+artworkID+"/") ||
		record.ContentType != "image/png" ||
		record.ByteSize != int64(len(content)) ||
		record.ETag != `"fixture"` ||
		record.CachedAt == "" {
		t.Fatalf("Artwork(after request) = %#v", record)
	}
	response := requestArtwork(handler, http.MethodHead, assetPathPrefix+artworkID)
	if response.Code != http.StatusOK ||
		response.Body.Len() != 0 ||
		response.Header().Get("Content-Length") == "" {
		t.Fatalf("HEAD response = %d, %#v, %q", response.Code, response.Header(), response.Body.String())
	}
}

func TestAssetHandlerCoalescesConcurrentCacheMisses(t *testing.T) {
	t.Parallel()

	provider, _, artworkID, _ := openAssetHandlerFixture(t)
	content := testfixture.PNG()
	entered := make(chan struct{})
	release := make(chan struct{})
	var fetches atomic.Int32
	handler, err := NewAssetHandler(provider, artworkFetcherFunc(func(
		context.Context,
		string,
		int64,
	) (lunii.ArtworkPayload, error) {
		if fetches.Add(1) == 1 {
			close(entered)
		}
		<-release
		return lunii.ArtworkPayload{
			Content:     content,
			ContentType: "image/png",
		}, nil
	}))
	if err != nil {
		t.Fatal(err)
	}

	statuses := make(chan int, 8)
	var group sync.WaitGroup
	request := func() {
		defer group.Done()
		statuses <- requestArtwork(
			handler,
			http.MethodGet,
			assetPathPrefix+artworkID,
		).Code
	}
	group.Add(1)
	go request()
	<-entered
	group.Add(7)
	for index := 0; index < 7; index++ {
		go request()
	}
	close(release)
	group.Wait()
	close(statuses)
	for status := range statuses {
		if status != http.StatusOK {
			t.Fatalf("concurrent GET status = %d", status)
		}
	}
	if fetches.Load() != 1 {
		t.Fatalf("FetchArtwork() count = %d", fetches.Load())
	}
}

func TestAssetHandlerRejectsPathInputsAndInvalidRemoteArtwork(t *testing.T) {
	t.Parallel()

	provider, repository, artworkID, _ := openAssetHandlerFixture(t)
	handler, err := NewAssetHandler(provider, artworkFetcherFunc(func(
		context.Context,
		string,
		int64,
	) (lunii.ArtworkPayload, error) {
		return lunii.ArtworkPayload{
			Content:     []byte("not an image"),
			ContentType: "image/png",
		}, nil
	}))
	if err != nil {
		t.Fatal(err)
	}
	response := requestArtwork(handler, http.MethodGet, assetPathPrefix+artworkID)
	if response.Code != http.StatusBadGateway ||
		response.Body.String() != "artwork unavailable\n" {
		t.Fatalf("invalid remote response = %d, %q", response.Code, response.Body.String())
	}
	record, err := repository.Artwork(context.Background(), artworkID)
	if err != nil {
		t.Fatal(err)
	}
	if record.ManagedPath != "" {
		t.Fatalf("invalid artwork was cached at %q", record.ManagedPath)
	}

	for _, request := range []struct {
		method string
		target string
		status int
	}{
		{method: http.MethodPost, target: assetPathPrefix + artworkID, status: http.StatusMethodNotAllowed},
		{method: http.MethodGet, target: "/artwork/", status: http.StatusNotFound},
		{method: http.MethodGet, target: "/artwork/catalog/official/cover.png", status: http.StatusNotFound},
		{method: http.MethodGet, target: assetPathPrefix + artworkID + "?path=secret", status: http.StatusNotFound},
		{method: http.MethodGet, target: assetPathPrefix + strings.Repeat("f", 64), status: http.StatusNotFound},
	} {
		response := requestArtwork(handler, request.method, request.target)
		if response.Code != request.status {
			t.Fatalf("%s %s status = %d", request.method, request.target, response.Code)
		}
	}
}

func TestAssetHandlerServesContainedEmbeddedArtwork(t *testing.T) {
	t.Parallel()

	provider, _, _, _ := openAssetHandlerFixture(t)
	content := testfixture.PNG()
	relativePath, err := NewRepository(provider.layout).Publish(
		testfixture.StoryUUID,
		"image/png",
		content,
	)
	if err != nil {
		t.Fatal(err)
	}
	result, err := provider.database.Exec(
		`INSERT INTO stories (uuid, embedded_artwork_path)
		 VALUES (?, ?)`,
		testfixture.StoryUUID,
		relativePath,
	)
	if err != nil {
		t.Fatal(err)
	}
	storyID, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	handler, err := NewAssetHandler(provider, artworkFetcherFunc(func(
		context.Context,
		string,
		int64,
	) (lunii.ArtworkPayload, error) {
		return lunii.ArtworkPayload{}, errors.New("unexpected remote fetch")
	}))
	if err != nil {
		t.Fatal(err)
	}
	response := requestArtwork(
		handler,
		http.MethodGet,
		assetPathPrefix+"embedded:"+strconv.FormatInt(storyID, 10),
	)
	if response.Code != http.StatusOK || string(response.Body.Bytes()) != string(content) {
		t.Fatalf("embedded response = %d, %q", response.Code, response.Body.String())
	}
}

func openAssetHandlerFixture(
	t *testing.T,
) (testStorageProvider, *metadata.Repository, string, string) {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(
		context.Background(),
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := opened.Close(); err != nil {
			t.Error(err)
		}
	})
	repository := metadata.NewRepository(opened.SQL())
	sourceURL := "https://storage.googleapis.com/lunii-data-prod/fixture/cover.png"
	digest := sha256.Sum256([]byte(sourceURL))
	artworkID := hex.EncodeToString(digest[:])
	syncID := "123e4567-e89b-42d3-a456-426614174112"
	if _, err := repository.CreateSync(context.Background(), metadata.NewCatalogSync{
		ID:        syncID,
		Locale:    "en-GB",
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := repository.StageSnapshot(
		context.Background(),
		metadata.NewCatalogSnapshot{
			SyncID:    syncID,
			Locale:    "en-GB",
			RawPath:   "catalog/" + syncID + "/catalog.json",
			RawSHA256: strings.Repeat("c", 64),
			ByteSize:  128,
			FetchedAt: time.Now(),
			Artworks: []metadata.NewCatalogArtwork{{
				ID:        artworkID,
				SourceURL: sourceURL,
			}},
			Stories: []metadata.NewOfficialStoryMetadata{{
				StoryUUID: testfixture.StoryUUID,
				ArtworkID: artworkID,
			}},
		},
	); err != nil {
		t.Fatal(err)
	}
	return testStorageProvider{
		layout:   layout,
		database: opened.SQL(),
	}, repository, artworkID, sourceURL
}

func requestArtwork(handler http.Handler, method string, target string) *httptest.ResponseRecorder {
	request := httptest.NewRequest(method, target, nil)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}
