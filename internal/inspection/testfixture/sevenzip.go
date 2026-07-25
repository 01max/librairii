package testfixture

import (
	"embed"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type SevenZIPFixture string

const (
	SevenZIPGeneric  SevenZIPFixture = "generic"
	SevenZIPStudio   SevenZIPFixture = "studio"
	SevenZIPBomb     SevenZIPFixture = "bomb"
	SevenZIPDeepPath SevenZIPFixture = "deep_path"
	SevenZIPSymlink  SevenZIPFixture = "symlink"
	SevenZIPNested   SevenZIPFixture = "nested"
	SevenZIPMissing  SevenZIPFixture = "missing"
)

//go:embed testdata
var sevenZIPFixtures embed.FS

func SevenZIPFixtureKinds() []SevenZIPFixture {
	return []SevenZIPFixture{
		SevenZIPGeneric,
		SevenZIPStudio,
		SevenZIPBomb,
		SevenZIPDeepPath,
		SevenZIPSymlink,
		SevenZIPNested,
		SevenZIPMissing,
	}
}

func SevenZIPArchive(fixture SevenZIPFixture) (Archive, error) {
	switch fixture {
	case SevenZIPGeneric:
		return GenericSevenZIP(), nil
	case SevenZIPStudio:
		return StudioSevenZIP(), nil
	case SevenZIPBomb:
		return WithEntries(
			GenericSevenZIP(),
			Entry{
				Name:  "padding.bin",
				Bytes: []byte(strings.Repeat("0", 256<<10)),
			},
		), nil
	case SevenZIPDeepPath:
		return WithEntries(
			GenericSevenZIP(),
			Entry{
				Name:  strings.Repeat("deep/", 20) + "asset.bin",
				Bytes: []byte("deep synthetic asset"),
			},
		), nil
	case SevenZIPSymlink:
		return WithEntries(
			GenericSevenZIP(),
			Entry{
				Name:  "synthetic-link",
				Bytes: []byte("../outside"),
				Mode:  os.ModeSymlink | 0o777,
			},
		), nil
	case SevenZIPNested:
		return WithEntries(
			GenericSevenZIP(),
			Entry{
				Name:  "nested.pk",
				Bytes: []byte("synthetic nested archive"),
			},
		), nil
	case SevenZIPMissing:
		return WithoutEntry(
			GenericSevenZIP(),
			StoryUUID+"/si",
		), nil
	default:
		return Archive{}, fmt.Errorf("unknown synthetic 7z fixture %q", fixture)
	}
}

func SevenZIPBytes(fixture SevenZIPFixture) ([]byte, error) {
	filename, err := sevenZIPTestdataName(fixture)
	if err != nil {
		return nil, err
	}
	bytes, err := sevenZIPFixtures.ReadFile(filepath.ToSlash(filepath.Join("testdata", filename)))
	if err != nil {
		return nil, fmt.Errorf("read synthetic 7z fixture %q: %w", fixture, err)
	}
	return bytes, nil
}

func WriteSevenZIP(directory string, fixture SevenZIPFixture) (string, error) {
	archive, err := SevenZIPArchive(fixture)
	if err != nil {
		return "", err
	}
	bytes, err := SevenZIPBytes(fixture)
	if err != nil {
		return "", err
	}
	path := filepath.Join(directory, archive.Filename)
	if err := os.WriteFile(path, bytes, 0o600); err != nil {
		return "", fmt.Errorf("write synthetic 7z fixture: %w", err)
	}
	return path, nil
}

func sevenZIPTestdataName(fixture SevenZIPFixture) (string, error) {
	switch fixture {
	case SevenZIPGeneric:
		return "generic.7z", nil
	case SevenZIPStudio:
		return "studio.7z", nil
	case SevenZIPBomb:
		return "bomb.7z", nil
	case SevenZIPDeepPath:
		return "deep-path.7z", nil
	case SevenZIPSymlink:
		return "symlink.7z", nil
	case SevenZIPNested:
		return "nested.7z", nil
	case SevenZIPMissing:
		return "missing.7z", nil
	default:
		return "", fmt.Errorf("unknown synthetic 7z fixture %q", fixture)
	}
}
