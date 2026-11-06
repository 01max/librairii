package artwork

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/storage"
)

const assetPathPrefix = "/artwork/"

type StorageProvider interface {
	Layout() storage.Layout
	SQL() *sql.DB
	Writer() *sql.DB
}

type RemoteFetcher interface {
	FetchArtwork(context.Context, string, int64) (lunii.ArtworkPayload, error)
}

type AssetHandler struct {
	storage      StorageProvider
	fetcher      RemoteFetcher
	maximumBytes int64
	now          func() time.Time
	cacheMiss    sync.Mutex
}

func NewAssetHandler(storage StorageProvider, fetcher RemoteFetcher) (*AssetHandler, error) {
	if storage == nil || fetcher == nil {
		return nil, errors.New("artwork asset dependency is nil")
	}
	return &AssetHandler{
		storage:      storage,
		fetcher:      fetcher,
		maximumBytes: DefaultMaximumBytes,
		now:          time.Now,
	}, nil
}

func (h *AssetHandler) ServeHTTP(response http.ResponseWriter, request *http.Request) {
	if request.Method != http.MethodGet && request.Method != http.MethodHead {
		response.Header().Set("Allow", "GET, HEAD")
		http.Error(response, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if !strings.HasPrefix(request.URL.Path, assetPathPrefix) {
		http.NotFound(response, request)
		return
	}
	opaqueID := strings.TrimPrefix(request.URL.Path, assetPathPrefix)
	if opaqueID == "" || strings.Contains(opaqueID, "/") || request.URL.RawQuery != "" {
		http.NotFound(response, request)
		return
	}
	readDatabase := h.storage.SQL()
	writeDatabase := h.storage.Writer()
	layout := h.storage.Layout()
	if readDatabase == nil || writeDatabase == nil || layout.Root == "" {
		http.Error(response, "artwork storage unavailable", http.StatusServiceUnavailable)
		return
	}

	asset, err := h.load(
		request.Context(),
		readDatabase,
		writeDatabase,
		layout,
		opaqueID,
	)
	if err != nil {
		if errors.Is(err, ErrArtworkNotFound) || errors.Is(err, sql.ErrNoRows) {
			http.NotFound(response, request)
			return
		}
		http.Error(response, "artwork unavailable", http.StatusBadGateway)
		return
	}
	response.Header().Set("Content-Type", asset.ContentType)
	response.Header().Set("Content-Length", strconv.Itoa(len(asset.Content)))
	response.Header().Set("Cache-Control", "private, max-age=86400")
	response.Header().Set("ETag", `"`+asset.SHA256+`"`)
	response.Header().Set("X-Content-Type-Options", "nosniff")
	response.WriteHeader(http.StatusOK)
	if request.Method == http.MethodGet {
		_, _ = response.Write(asset.Content)
	}
}

func (h *AssetHandler) load(
	ctx context.Context,
	readDatabase *sql.DB,
	writeDatabase *sql.DB,
	layout storage.Layout,
	opaqueID string,
) (Asset, error) {
	files := NewRepository(layout)
	if strings.HasPrefix(opaqueID, "embedded:") {
		return files.LoadEmbedded(ctx, readDatabase, opaqueID, h.maximumBytes)
	}
	if !validOpaqueID(opaqueID) {
		return Asset{}, ErrArtworkNotFound
	}
	readCatalog := metadata.NewRepository(readDatabase)
	record, err := readCatalog.Artwork(ctx, opaqueID)
	if err != nil {
		return Asset{}, err
	}
	if record.ManagedPath != "" {
		return files.LoadCatalog(record, h.maximumBytes)
	}

	h.cacheMiss.Lock()
	defer h.cacheMiss.Unlock()
	record, err = readCatalog.Artwork(ctx, opaqueID)
	if err != nil {
		return Asset{}, err
	}
	if record.ManagedPath != "" {
		return files.LoadCatalog(record, h.maximumBytes)
	}
	downloaded, err := h.fetcher.FetchArtwork(ctx, record.SourceURL, h.maximumBytes)
	if err != nil {
		return Asset{}, err
	}
	managedPath, asset, err := files.PublishCatalog(
		opaqueID,
		downloaded.ContentType,
		downloaded.Content,
		h.maximumBytes,
	)
	if err != nil {
		return Asset{}, err
	}
	if err := metadata.NewRepository(writeDatabase).CacheArtwork(
		ctx,
		opaqueID,
		managedPath,
		asset.ContentType,
		asset.SHA256,
		int64(len(asset.Content)),
		downloaded.ETag,
		downloaded.LastModified,
		h.now().UTC(),
	); err != nil {
		removeErr := files.RemoveCatalog(managedPath)
		if removeErr != nil {
			err = errors.Join(err, removeErr)
		}
		return Asset{}, fmt.Errorf("record artwork cache: %w", err)
	}
	return asset, nil
}
