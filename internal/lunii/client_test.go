package lunii

import (
	"context"
	"crypto/tls"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/01max/librairii/internal/inspection/testfixture"
)

func TestCatalogClientMatchesSanitizedGuestAndCatalogContracts(t *testing.T) {
	t.Parallel()

	tokenFixture := readFixture(t, "guest_token.json")
	catalogFixture := readFixture(t, "catalog.json")
	var mutex sync.Mutex
	requests := make([]string, 0, 2)
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		mutex.Lock()
		requests = append(requests, request.URL.Path)
		mutex.Unlock()

		if request.Method != http.MethodGet {
			t.Errorf("request method = %s", request.Method)
		}
		if request.Header.Get("Accept") != "application/json" {
			t.Errorf("Accept = %q", request.Header.Get("Accept"))
		}
		if request.Header.Get("User-Agent") != userAgent {
			t.Errorf("User-Agent = %q", request.Header.Get("User-Agent"))
		}
		if request.Header.Get("Authorization") != "" || len(request.Cookies()) != 0 {
			t.Errorf("personal credential header was sent: %#v", request.Header)
		}

		response.Header().Set("Content-Type", "application/json; charset=utf-8")
		switch request.URL.Path {
		case "/guest/create":
			if request.Header.Get("X-AUTH-TOKEN") != "" ||
				request.Header.Get("Application-Sender") != "" {
				t.Errorf("guest token request headers = %#v", request.Header)
			}
			_, _ = response.Write(tokenFixture)
		case "/v2/packs":
			if request.Header.Get("Application-Sender") != applicationSender {
				t.Errorf("Application-Sender = %q", request.Header.Get("Application-Sender"))
			}
			if request.Header.Get("X-AUTH-TOKEN") != "sanitized-fixture-token" {
				t.Errorf("X-AUTH-TOKEN = %q", request.Header.Get("X-AUTH-TOKEN"))
			}
			_, _ = response.Write(catalogFixture)
		default:
			http.NotFound(response, request)
		}
	}))
	defer server.Close()

	client := newTestClient(t, server.URL, time.Second, 64*1024, 1024*1024)
	got, err := client.FetchCatalog(context.Background())
	if err != nil {
		t.Fatalf("FetchCatalog() error = %v", err)
	}
	if string(got) != string(catalogFixture) {
		t.Fatalf("FetchCatalog() = %s", got)
	}
	mutex.Lock()
	defer mutex.Unlock()
	if len(requests) != 2 ||
		requests[0] != "/guest/create" ||
		requests[1] != "/v2/packs" {
		t.Fatalf("request paths = %#v", requests)
	}
}

func TestCatalogClientBoundsEveryResponse(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name         string
		tokenLimit   int64
		catalogLimit int64
	}{
		{
			name:         "guest token",
			tokenLimit:   8,
			catalogLimit: 1024,
		},
		{
			name:         "catalog",
			tokenLimit:   1024,
			catalogLimit: 8,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()

			server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
				response.Header().Set("Content-Type", "application/json")
				if request.URL.Path == "/guest/create" {
					_, _ = response.Write(readFixture(t, "guest_token.json"))
					return
				}
				_, _ = response.Write(readFixture(t, "catalog.json"))
			}))
			defer server.Close()

			client := newTestClient(
				t,
				server.URL,
				time.Second,
				test.tokenLimit,
				test.catalogLimit,
			)
			_, err := client.FetchCatalog(context.Background())
			if !errors.Is(err, ErrResponseTooLarge) {
				t.Fatalf("FetchCatalog() error = %v", err)
			}
		})
	}
}

func TestCatalogClientUsesBoundedTimeout(t *testing.T) {
	t.Parallel()

	cancelled := make(chan struct{})
	server := httptest.NewServer(http.HandlerFunc(func(_ http.ResponseWriter, request *http.Request) {
		<-request.Context().Done()
		close(cancelled)
	}))
	defer server.Close()

	client := newTestClient(t, server.URL, 25*time.Millisecond, 1024, 1024)
	_, err := client.FetchCatalog(context.Background())
	if err == nil {
		t.Fatal("FetchCatalog() error = nil")
	}
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("FetchCatalog() error = %v", err)
	}
	select {
	case <-cancelled:
	case <-time.After(time.Second):
		t.Fatal("server request context was not cancelled")
	}
}

func TestCatalogClientRejectsStatusesContentTypesAndMalformedTokens(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name        string
		status      int
		contentType string
		body        string
		want        error
	}{
		{
			name:        "upstream status",
			status:      http.StatusServiceUnavailable,
			contentType: "application/json",
			body:        `{"secret":"must not leak"}`,
			want:        ErrUnexpectedStatus,
		},
		{
			name:        "non json",
			status:      http.StatusOK,
			contentType: "text/html",
			body:        `<html>not json</html>`,
			want:        ErrInvalidJSON,
		},
		{
			name:        "missing token",
			status:      http.StatusOK,
			contentType: "application/json",
			body:        `{"response":{"token":{}}}`,
			want:        ErrInvalidGuestToken,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()

			server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
				response.Header().Set("Content-Type", test.contentType)
				response.WriteHeader(test.status)
				_, _ = response.Write([]byte(test.body))
			}))
			defer server.Close()

			client := newTestClient(t, server.URL, time.Second, 1024, 1024)
			_, err := client.FetchCatalog(context.Background())
			if !errors.Is(err, test.want) {
				t.Fatalf("FetchCatalog() error = %v, want %v", err, test.want)
			}
			if strings.Contains(err.Error(), "must not leak") {
				t.Fatalf("FetchCatalog() leaked response body: %v", err)
			}
		})
	}
}

func TestProductionClientUsesVerifiedTLSDefaults(t *testing.T) {
	t.Parallel()

	client, err := NewCatalogClient(ProductionConfig())
	if err != nil {
		t.Fatal(err)
	}
	transport, ok := client.httpClient.Transport.(*http.Transport)
	if !ok {
		t.Fatalf("Transport = %T", client.httpClient.Transport)
	}
	if transport.TLSClientConfig == nil ||
		transport.TLSClientConfig.MinVersion < tls.VersionTLS12 ||
		transport.TLSClientConfig.InsecureSkipVerify {
		t.Fatalf("TLS config = %#v", transport.TLSClientConfig)
	}
	if transport.DialContext == nil ||
		transport.TLSHandshakeTimeout <= 0 ||
		transport.ResponseHeaderTimeout <= 0 ||
		client.httpClient.Timeout != defaultRequestTimeout {
		t.Fatalf("transport deadlines = %#v, client timeout = %s", transport, client.httpClient.Timeout)
	}
}

func TestCatalogClientFetchesOnlyBoundedApprovedArtwork(t *testing.T) {
	t.Parallel()

	content := testfixture.PNG()
	var requests atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		requests.Add(1)
		if request.URL.Path != "/artwork/fixture.png" ||
			request.Header.Get("Accept") != "image/png, image/jpeg, image/webp" ||
			request.Header.Get("Authorization") != "" {
			t.Errorf("artwork request = %s %#v", request.URL.Path, request.Header)
		}
		response.Header().Set("Content-Type", "image/png")
		response.Header().Set("ETag", `"fixture"`)
		response.Header().Set("Last-Modified", "Sat, 25 Jul 2026 10:00:00 GMT")
		_, _ = response.Write(content)
	}))
	defer server.Close()

	client := newTestClient(t, server.URL, time.Second, 1024, 1024)
	payload, err := client.FetchArtwork(
		context.Background(),
		server.URL+"/artwork/fixture.png",
		int64(len(content)),
	)
	if err != nil {
		t.Fatal(err)
	}
	if string(payload.Content) != string(content) ||
		payload.ContentType != "image/png" ||
		payload.ETag != `"fixture"` ||
		payload.LastModified == "" {
		t.Fatalf("FetchArtwork() = %#v", payload)
	}
	for _, invalid := range []string{
		"https://example.test/fixture.png",
		server.URL + "/outside/fixture.png",
		server.URL + "/artwork/../secret.png",
		server.URL + "/artwork/%2e%2e/secret.png",
		server.URL + "/artwork/fixture.png?token=secret",
	} {
		if _, err := client.FetchArtwork(
			context.Background(),
			invalid,
			int64(len(content)),
		); !errors.Is(err, ErrInvalidArtworkURL) {
			t.Fatalf("FetchArtwork(%q) error = %v", invalid, err)
		}
	}
	if _, err := client.FetchArtwork(
		context.Background(),
		server.URL+"/artwork/fixture.png",
		int64(len(content)-1),
	); !errors.Is(err, ErrResponseTooLarge) {
		t.Fatalf("FetchArtwork(oversized) error = %v", err)
	}
	if requests.Load() != 2 {
		t.Fatalf("artwork request count = %d", requests.Load())
	}
}

func TestResolveArtworkURLAllowsOnlyCatalogRelativePaths(t *testing.T) {
	t.Parallel()

	got, err := ResolveArtworkURL("/fixture/cover.png")
	if err != nil ||
		got != productionArtworkURL+"/fixture/cover.png" {
		t.Fatalf("ResolveArtworkURL() = %q, %v", got, err)
	}
	for _, invalid := range []string{
		"",
		"relative.png",
		"https://example.test/cover.png",
		"/../secret.png",
		"/fixture/cover.png?token=secret",
	} {
		if _, err := ResolveArtworkURL(invalid); !errors.Is(err, ErrInvalidArtworkURL) {
			t.Fatalf("ResolveArtworkURL(%q) error = %v", invalid, err)
		}
	}
}

func TestCatalogClientRejectsUnboundedOrInvalidConfiguration(t *testing.T) {
	t.Parallel()

	tests := []Config{
		{},
		{
			GuestTokenURL:   "file:///tmp/token",
			CatalogURL:      "https://example.test/catalog",
			RequestTimeout:  time.Second,
			GuestTokenLimit: 1,
			CatalogLimit:    1,
		},
		{
			GuestTokenURL:   "https://example.test/token",
			CatalogURL:      "https://example.test/catalog",
			RequestTimeout:  0,
			GuestTokenLimit: 1,
			CatalogLimit:    1,
		},
		{
			GuestTokenURL:   "http://example.test/token",
			CatalogURL:      "https://example.test/catalog",
			RequestTimeout:  time.Second,
			GuestTokenLimit: 1,
			CatalogLimit:    1,
		},
	}
	for _, config := range tests {
		if _, err := NewCatalogClient(config); !errors.Is(err, ErrInvalidConfiguration) {
			t.Fatalf("NewCatalogClient(%#v) error = %v", config, err)
		}
	}
}

func newTestClient(
	t *testing.T,
	baseURL string,
	timeout time.Duration,
	tokenLimit int64,
	catalogLimit int64,
) *CatalogClient {
	t.Helper()

	client, err := NewCatalogClient(Config{
		GuestTokenURL:   baseURL + "/guest/create",
		CatalogURL:      baseURL + "/v2/packs",
		ArtworkBaseURL:  baseURL + "/artwork",
		RequestTimeout:  timeout,
		GuestTokenLimit: tokenLimit,
		CatalogLimit:    catalogLimit,
	})
	if err != nil {
		t.Fatal(err)
	}
	return client
}

func readFixture(t *testing.T, name string) []byte {
	t.Helper()

	body, err := os.ReadFile("testdata/" + name)
	if err != nil {
		t.Fatal(err)
	}
	return body
}
