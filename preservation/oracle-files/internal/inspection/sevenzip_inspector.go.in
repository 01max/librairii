package inspection

import (
	"context"
	"strings"

	"github.com/01max/librairii/internal/catalog"
)

type SevenZIPInspector struct{}

func NewSevenZIPInspector() *SevenZIPInspector {
	return &SevenZIPInspector{}
}

func (i *SevenZIPInspector) Inspect(
	ctx context.Context,
	candidate Candidate,
	limits Limits,
) (Result, error) {
	if !strings.HasSuffix(strings.ToLower(candidate.OriginalFilename), ".7z") {
		return Result{}, &ValidationError{Code: CodeUnsupportedFormat}
	}
	archive, err := openValidatedSevenZIP(ctx, candidate, limits)
	if err != nil {
		return Result{}, err
	}
	defer archive.Close()

	if archive.has(studioStoryEntry) {
		return inspectStudio(ctx, archive, limits, catalog.FormatSevenZIP)
	}
	return inspectLuniiPack(ctx, archive, catalog.FormatSevenZIP, limits)
}

type StoryInspector struct {
	zip      *ZIPInspector
	sevenZIP *SevenZIPInspector
}

func NewStoryInspector() *StoryInspector {
	return &StoryInspector{
		zip:      NewZIPInspector(),
		sevenZIP: NewSevenZIPInspector(),
	}
}

func (i *StoryInspector) Inspect(
	ctx context.Context,
	candidate Candidate,
	limits Limits,
) (Result, error) {
	if strings.HasSuffix(strings.ToLower(candidate.OriginalFilename), ".7z") {
		return i.sevenZIP.Inspect(ctx, candidate, limits)
	}
	return i.zip.Inspect(ctx, candidate, limits)
}

var _ Inspector = (*SevenZIPInspector)(nil)
var _ Inspector = (*StoryInspector)(nil)
