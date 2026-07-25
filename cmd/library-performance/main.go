package main

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"image/png"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"time"

	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/performancefixture"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
)

type metric struct {
	MedianMilliseconds  float64 `json:"medianMilliseconds"`
	P95Milliseconds     float64 `json:"p95Milliseconds"`
	MaximumMilliseconds float64 `json:"maximumMilliseconds"`
	BudgetMilliseconds  float64 `json:"budgetMilliseconds"`
	WithinBudget        bool    `json:"withinBudget"`
	ResultCount         int     `json:"resultCount"`
}

type report struct {
	GeneratedAt                   string                  `json:"generatedAt"`
	GoVersion                     string                  `json:"goVersion"`
	OperatingSystem               string                  `json:"operatingSystem"`
	Architecture                  string                  `json:"architecture"`
	Stories                       int                     `json:"stories"`
	Shelves                       int                     `json:"shelves"`
	Samples                       int                     `json:"samples"`
	FixtureGenerationMilliseconds float64                 `json:"fixtureGenerationMilliseconds"`
	Metrics                       map[string]metric       `json:"metrics"`
	QueryPlans                    performancefixture.Plan `json:"queryPlans"`
}

func main() {
	storyCount := flag.Int(
		"stories",
		performancefixture.MinimumLargeLibraryStories,
		"number of synthetic stories",
	)
	samples := flag.Int("samples", 20, "timed samples per scenario")
	flag.Parse()
	if *storyCount < performancefixture.MinimumLargeLibraryStories ||
		*samples < 1 {
		fmt.Fprintln(
			os.Stderr,
			"stories must be at least 5000 and samples must be positive",
		)
		os.Exit(2)
	}
	if err := run(context.Background(), *storyCount, *samples); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(ctx context.Context, storyCount int, samples int) error {
	root, err := os.MkdirTemp("", "librairii-performance-*")
	if err != nil {
		return err
	}
	defer os.RemoveAll(root)
	layout, err := storage.Initialize(root)
	if err != nil {
		return err
	}
	opened, err := database.Open(
		ctx,
		filepath.Join(layout.Database, "librairii.sqlite3"),
	)
	if err != nil {
		return err
	}
	defer opened.Close()

	fixtureStarted := time.Now()
	fixture, err := performancefixture.Generate(
		ctx,
		opened.Writer(),
		layout,
		storyCount,
	)
	if err != nil {
		return err
	}
	fixtureDuration := time.Since(fixtureStarted)
	metadataRepository := metadata.NewRepository(opened.SQL())
	official, err := metadata.NewLibraryProvider(
		metadataRepository,
		metadata.DefaultLocale,
	)
	if err != nil {
		return err
	}
	query := library.NewQuery(opened.SQL(), official)
	shelfService, err := shelves.NewService(opened.SQL())
	if err != nil {
		return err
	}
	evaluator, err := shelves.NewEvaluator(shelfService, query)
	if err != nil {
		return err
	}
	artworkHandler, err := artwork.NewAssetHandler(
		performanceStorage{layout: layout, database: opened},
		unreachableArtworkFetcher{},
	)
	if err != nil {
		return err
	}
	queryPlans, err := performancefixture.QueryPlans(ctx, opened.SQL())
	if err != nil {
		return err
	}

	metrics := make(map[string]metric)
	metrics["collectionQuery"] = measure(
		samples,
		mustBudget("collectionQuery"),
		func() (int, error) {
			page, err := query.Search(ctx, library.StoryLibraryQuery{
				Page:     1,
				PageSize: library.DefaultPageSize,
			})
			return page.TotalItems, err
		},
	)
	metrics["substringSearch"] = measure(
		samples,
		mustBudget("substringSearch"),
		func() (int, error) {
			page, err := query.Search(ctx, library.StoryLibraryQuery{
				Name:     "moon",
				Page:     1,
				PageSize: library.DefaultPageSize,
			})
			return page.TotalItems, err
		},
	)
	metrics["combinedFilters"] = measure(
		samples,
		mustBudget("combinedFilters"),
		func() (int, error) {
			request := performancefixture.CombinedQuery()
			request.Page = 1
			request.PageSize = library.DefaultPageSize
			page, err := query.Search(ctx, request)
			return page.TotalItems, err
		},
	)
	metrics["shelfCounts"] = measure(
		samples,
		mustBudget("shelfCounts"),
		func() (int, error) {
			counts, err := evaluator.Counts(ctx)
			if err != nil {
				return 0, err
			}
			total := 0
			for _, count := range counts {
				total += count.Count
			}
			return total, nil
		},
	)
	metrics["deepPagination"] = measure(
		samples,
		mustBudget("deepPagination"),
		func() (int, error) {
			page, err := query.Search(ctx, library.StoryLibraryQuery{
				Page:     200,
				PageSize: library.DefaultPageSize,
			})
			return len(page.Stories), err
		},
	)
	firstPage, err := query.Search(ctx, library.StoryLibraryQuery{
		Page:     1,
		PageSize: library.DefaultPageSize,
	})
	if err != nil {
		return err
	}
	metrics["artworkLoad"] = measure(
		samples,
		mustBudget("artworkLoad"),
		func() (int, error) {
			loaded := 0
			for _, story := range firstPage.Stories {
				request := httptest.NewRequest(
					http.MethodGet,
					"http://librairii.local/artwork/"+story.ArtworkID,
					nil,
				).WithContext(ctx)
				response := httptest.NewRecorder()
				artworkHandler.ServeHTTP(response, request)
				if response.Code != http.StatusOK {
					return loaded, fmt.Errorf(
						"artwork %q response status %d",
						story.ArtworkID,
						response.Code,
					)
				}
				picture, err := png.Decode(bytes.NewReader(response.Body.Bytes()))
				if err != nil {
					return loaded, err
				}
				bounds := picture.Bounds()
				if bounds.Dx() != performancefixture.SyntheticArtworkWidth ||
					bounds.Dy() != performancefixture.SyntheticArtworkHeight {
					return loaded, fmt.Errorf(
						"artwork %q dimensions %dx%d",
						story.ArtworkID,
						bounds.Dx(),
						bounds.Dy(),
					)
				}
				loaded++
			}
			return loaded, nil
		},
	)
	budgetsMet := true
	for name, result := range metrics {
		if result.ResultCount < 0 {
			return fmt.Errorf("%s measurement failed", name)
		}
		budgetsMet = budgetsMet && result.WithinBudget
	}
	encoded, err := json.MarshalIndent(report{
		GeneratedAt:                   time.Now().UTC().Format(time.RFC3339),
		GoVersion:                     runtime.Version(),
		OperatingSystem:               runtime.GOOS,
		Architecture:                  runtime.GOARCH,
		Stories:                       fixture.StoryCount,
		Shelves:                       len(fixture.ShelfIDs),
		Samples:                       samples,
		FixtureGenerationMilliseconds: milliseconds(fixtureDuration),
		Metrics:                       metrics,
		QueryPlans:                    queryPlans,
	}, "", "  ")
	if err != nil {
		return err
	}
	if _, err = fmt.Fprintln(os.Stdout, string(encoded)); err != nil {
		return err
	}
	if !budgetsMet {
		return errors.New("one or more large-library interaction budgets were missed")
	}
	return nil
}

type performanceStorage struct {
	layout   storage.Layout
	database *database.Database
}

func (s performanceStorage) Layout() storage.Layout {
	return s.layout
}

func (s performanceStorage) SQL() *sql.DB {
	return s.database.SQL()
}

func (s performanceStorage) Writer() *sql.DB {
	return s.database.Writer()
}

type unreachableArtworkFetcher struct{}

func (unreachableArtworkFetcher) FetchArtwork(
	context.Context,
	string,
	int64,
) (lunii.ArtworkPayload, error) {
	return lunii.ArtworkPayload{}, errors.New(
		"performance fixture artwork must remain local",
	)
}

func measure(
	samples int,
	budget time.Duration,
	operation func() (int, error),
) metric {
	if _, err := operation(); err != nil {
		return metric{ResultCount: -1}
	}
	durations := make([]time.Duration, 0, samples)
	resultCount := 0
	for range samples {
		started := time.Now()
		count, err := operation()
		duration := time.Since(started)
		if err != nil {
			return metric{ResultCount: -1}
		}
		resultCount = count
		durations = append(durations, duration)
	}
	sort.Slice(durations, func(i, j int) bool {
		return durations[i] < durations[j]
	})
	p95Index := (95*len(durations)+99)/100 - 1
	return metric{
		MedianMilliseconds:  milliseconds(durations[len(durations)/2]),
		P95Milliseconds:     milliseconds(durations[p95Index]),
		MaximumMilliseconds: milliseconds(durations[len(durations)-1]),
		BudgetMilliseconds:  milliseconds(budget),
		WithinBudget:        durations[p95Index] <= budget,
		ResultCount:         resultCount,
	}
}

func mustBudget(name string) time.Duration {
	budget, found := performancefixture.InteractionBudget(name)
	if !found {
		panic("missing performance budget: " + name)
	}
	return budget
}

func milliseconds(duration time.Duration) float64 {
	return float64(duration.Microseconds()) / 1000
}
