// Package lunii contains the only adapter that knows the official catalog
// endpoints and their wire protocol.
package lunii

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"mime"
	"net"
	"net/http"
	"net/url"
	"path"
	"strings"
	"time"
)

const (
	productionGuestTokenURL = "https://server-auth-prod.lunii.com/guest/create"
	productionCatalogURL    = "https://server-data-prod.lunii.com/v2/packs"
	productionArtworkURL    = "https://storage.googleapis.com/lunii-data-prod"

	applicationSender = "luniistore_desktop"
	userAgent         = "Librairii/metadata-sync"

	defaultRequestTimeout      = 30 * time.Second
	defaultDialTimeout         = 10 * time.Second
	defaultTLSHandshakeTimeout = 10 * time.Second
	defaultHeaderTimeout       = 15 * time.Second
	defaultIdleTimeout         = 30 * time.Second
	defaultGuestTokenLimit     = 64 * 1024
	defaultCatalogLimit        = 32 * 1024 * 1024
)

var (
	ErrInvalidConfiguration = errors.New("invalid Lunii catalog client configuration")
	ErrUnexpectedRedirect   = errors.New("unexpected Lunii catalog redirect")
	ErrUnexpectedStatus     = errors.New("unexpected Lunii catalog response status")
	ErrResponseTooLarge     = errors.New("Lunii catalog response exceeds its size limit")
	ErrInvalidJSON          = errors.New("invalid Lunii catalog JSON response")
	ErrInvalidGuestToken    = errors.New("invalid Lunii guest token response")
	ErrInvalidArtworkURL    = errors.New("invalid Lunii artwork URL")
	ErrInvalidArtwork       = errors.New("invalid Lunii artwork response")
)

type Config struct {
	GuestTokenURL   string
	CatalogURL      string
	ArtworkBaseURL  string
	RequestTimeout  time.Duration
	GuestTokenLimit int64
	CatalogLimit    int64
}

func ProductionConfig() Config {
	return Config{
		GuestTokenURL:   productionGuestTokenURL,
		CatalogURL:      productionCatalogURL,
		ArtworkBaseURL:  productionArtworkURL,
		RequestTimeout:  defaultRequestTimeout,
		GuestTokenLimit: defaultGuestTokenLimit,
		CatalogLimit:    defaultCatalogLimit,
	}
}

type CatalogClient struct {
	httpClient     *http.Client
	guestTokenURL  string
	catalogURL     string
	artworkBaseURL *url.URL
	tokenLimit     int64
	catalogLimit   int64
}

func NewCatalogClient(config Config) (*CatalogClient, error) {
	return newCatalogClient(config, nil)
}

func newCatalogClient(config Config, roundTripper http.RoundTripper) (*CatalogClient, error) {
	if err := validateConfig(config); err != nil {
		return nil, err
	}
	if roundTripper == nil {
		roundTripper = productionTransport()
	}
	parsedArtworkBase, err := url.Parse(config.ArtworkBaseURL)
	if err != nil {
		return nil, ErrInvalidConfiguration
	}
	return &CatalogClient{
		httpClient: &http.Client{
			Transport: roundTripper,
			Timeout:   config.RequestTimeout,
			CheckRedirect: func(
				_ *http.Request,
				_ []*http.Request,
			) error {
				return ErrUnexpectedRedirect
			},
		},
		guestTokenURL:  config.GuestTokenURL,
		catalogURL:     config.CatalogURL,
		artworkBaseURL: parsedArtworkBase,
		tokenLimit:     config.GuestTokenLimit,
		catalogLimit:   config.CatalogLimit,
	}, nil
}

func validateConfig(config Config) error {
	if config.RequestTimeout <= 0 ||
		config.GuestTokenLimit <= 0 ||
		config.CatalogLimit <= 0 {
		return ErrInvalidConfiguration
	}
	for _, rawURL := range []string{
		config.GuestTokenURL,
		config.CatalogURL,
		config.ArtworkBaseURL,
	} {
		parsed, err := url.ParseRequestURI(rawURL)
		if err != nil || parsed.Host == "" || (parsed.Scheme != "https" && parsed.Scheme != "http") {
			return ErrInvalidConfiguration
		}
		if parsed.Scheme == "http" && !isLoopbackHost(parsed.Hostname()) {
			return ErrInvalidConfiguration
		}
	}
	return nil
}

func isLoopbackHost(host string) bool {
	if strings.EqualFold(host, "localhost") {
		return true
	}
	address := net.ParseIP(host)
	return address != nil && address.IsLoopback()
}

func productionTransport() *http.Transport {
	transport := http.DefaultTransport.(*http.Transport).Clone()
	transport.DialContext = (&net.Dialer{
		Timeout:   defaultDialTimeout,
		KeepAlive: defaultIdleTimeout,
	}).DialContext
	transport.TLSClientConfig = &tls.Config{
		MinVersion: tls.VersionTLS12,
	}
	transport.TLSHandshakeTimeout = defaultTLSHandshakeTimeout
	transport.ResponseHeaderTimeout = defaultHeaderTimeout
	transport.IdleConnTimeout = defaultIdleTimeout
	return transport
}

func (c *CatalogClient) FetchCatalog(ctx context.Context) ([]byte, error) {
	token, err := c.fetchGuestToken(ctx)
	if err != nil {
		return nil, err
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, c.catalogURL, nil)
	if err != nil {
		return nil, fmt.Errorf("create catalog request: %w", err)
	}
	setJSONHeaders(request)
	request.Header.Set("Application-Sender", applicationSender)
	request.Header.Set("X-AUTH-TOKEN", token)

	body, err := c.doJSON(request, c.catalogLimit)
	if err != nil {
		return nil, fmt.Errorf("fetch Lunii catalog: %w", err)
	}
	return body, nil
}

type ArtworkPayload struct {
	Content      []byte
	ContentType  string
	ETag         string
	LastModified string
}

func (c *CatalogClient) FetchArtwork(
	ctx context.Context,
	sourceURL string,
	maximumBytes int64,
) (ArtworkPayload, error) {
	if maximumBytes <= 0 {
		return ArtworkPayload{}, ErrResponseTooLarge
	}
	parsed, err := url.ParseRequestURI(sourceURL)
	if err != nil || !urlWithinBase(parsed, c.artworkBaseURL) {
		return ArtworkPayload{}, ErrInvalidArtworkURL
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, sourceURL, nil)
	if err != nil {
		return ArtworkPayload{}, ErrInvalidArtworkURL
	}
	request.Header.Set("Accept", "image/png, image/jpeg, image/webp")
	request.Header.Set("User-Agent", userAgent)
	response, err := c.httpClient.Do(request)
	if err != nil {
		return ArtworkPayload{}, err
	}
	defer response.Body.Close()
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return ArtworkPayload{}, fmt.Errorf("%w: %d", ErrUnexpectedStatus, response.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(response.Body, maximumBytes+1))
	if err != nil {
		return ArtworkPayload{}, fmt.Errorf("read artwork response: %w", err)
	}
	if int64(len(body)) > maximumBytes {
		return ArtworkPayload{}, ErrResponseTooLarge
	}
	if len(body) == 0 {
		return ArtworkPayload{}, ErrInvalidArtwork
	}
	contentType, _, err := mime.ParseMediaType(response.Header.Get("Content-Type"))
	if err != nil || !supportedArtworkType(contentType) {
		return ArtworkPayload{}, ErrInvalidArtwork
	}
	detectedType := http.DetectContentType(body)
	if detectedType != contentType {
		return ArtworkPayload{}, ErrInvalidArtwork
	}
	return ArtworkPayload{
		Content:      body,
		ContentType:  contentType,
		ETag:         response.Header.Get("ETag"),
		LastModified: response.Header.Get("Last-Modified"),
	}, nil
}

func ResolveArtworkURL(rawPath string) (string, error) {
	if rawPath == "" || !strings.HasPrefix(rawPath, "/") {
		return "", ErrInvalidArtworkURL
	}
	parsed, err := url.Parse(rawPath)
	if err != nil ||
		parsed.IsAbs() ||
		parsed.Host != "" ||
		parsed.User != nil ||
		parsed.RawQuery != "" ||
		parsed.Fragment != "" ||
		parsed.Path != rawPath ||
		!canonicalArtworkPath(parsed.Path) {
		return "", ErrInvalidArtworkURL
	}
	return productionArtworkURL + rawPath, nil
}

func urlWithinBase(candidate *url.URL, base *url.URL) bool {
	if candidate == nil || base == nil ||
		candidate.Scheme != base.Scheme ||
		!strings.EqualFold(candidate.Host, base.Host) ||
		candidate.User != nil ||
		candidate.RawQuery != "" ||
		candidate.Fragment != "" {
		return false
	}
	basePath := strings.TrimSuffix(base.EscapedPath(), "/")
	candidatePath := candidate.EscapedPath()
	decodedPath, err := url.PathUnescape(candidatePath)
	if err != nil || !canonicalArtworkPath(decodedPath) {
		return false
	}
	return candidatePath != basePath &&
		strings.HasPrefix(candidatePath, basePath+"/")
}

func canonicalArtworkPath(value string) bool {
	if value == "" ||
		!strings.HasPrefix(value, "/") ||
		strings.Contains(value, "\\") ||
		path.Clean(value) != value {
		return false
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "." || segment == ".." {
			return false
		}
	}
	return true
}

func supportedArtworkType(contentType string) bool {
	switch contentType {
	case "image/png", "image/jpeg", "image/webp":
		return true
	default:
		return false
	}
}

func (c *CatalogClient) fetchGuestToken(ctx context.Context) (string, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, c.guestTokenURL, nil)
	if err != nil {
		return "", fmt.Errorf("create guest token request: %w", err)
	}
	setJSONHeaders(request)

	body, err := c.doJSON(request, c.tokenLimit)
	if err != nil {
		return "", fmt.Errorf("fetch Lunii guest token: %w", err)
	}
	var envelope struct {
		Response struct {
			Token struct {
				Server string `json:"server"`
			} `json:"token"`
		} `json:"response"`
	}
	decoder := json.NewDecoder(bytes.NewReader(body))
	if err := decoder.Decode(&envelope); err != nil {
		return "", ErrInvalidGuestToken
	}
	token := strings.TrimSpace(envelope.Response.Token.Server)
	if token == "" || strings.ContainsAny(token, "\r\n") {
		return "", ErrInvalidGuestToken
	}
	return token, nil
}

func setJSONHeaders(request *http.Request) {
	request.Header.Set("Accept", "application/json")
	request.Header.Set("User-Agent", userAgent)
}

func (c *CatalogClient) doJSON(request *http.Request, limit int64) ([]byte, error) {
	response, err := c.httpClient.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()

	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return nil, fmt.Errorf("%w: %d", ErrUnexpectedStatus, response.StatusCode)
	}
	contentType := response.Header.Get("Content-Type")
	if contentType != "" {
		mediaType, _, err := mime.ParseMediaType(contentType)
		if err != nil ||
			(mediaType != "application/json" && !strings.HasSuffix(mediaType, "+json")) {
			return nil, ErrInvalidJSON
		}
	}

	body, err := io.ReadAll(io.LimitReader(response.Body, limit+1))
	if err != nil {
		return nil, fmt.Errorf("read response body: %w", err)
	}
	if int64(len(body)) > limit {
		return nil, ErrResponseTooLarge
	}
	if !json.Valid(body) {
		return nil, ErrInvalidJSON
	}
	return body, nil
}
