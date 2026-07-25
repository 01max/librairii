// Command generate-sevenzip-fixtures is a developer-only fixture generator.
// Librairii embeds its output and never invokes bsdtar in the application.
package main

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/01max/librairii/internal/inspection/testfixture"
)

const outputDirectory = "internal/inspection/testfixture/testdata"

func main() {
	if _, err := exec.LookPath("bsdtar"); err != nil {
		fatal(fmt.Errorf("find bsdtar: %w", err))
	}
	absoluteOutput, err := filepath.Abs(outputDirectory)
	if err != nil {
		fatal(err)
	}
	if err := os.MkdirAll(absoluteOutput, 0o755); err != nil {
		fatal(err)
	}

	for _, fixture := range testfixture.SevenZIPFixtureKinds() {
		if err := generate(absoluteOutput, fixture); err != nil {
			fatal(err)
		}
	}
}

func generate(output string, fixture testfixture.SevenZIPFixture) error {
	archive, err := testfixture.SevenZIPArchive(fixture)
	if err != nil {
		return err
	}
	directory, err := os.MkdirTemp("", "librairii-sevenzip-fixture-*")
	if err != nil {
		return err
	}
	defer os.RemoveAll(directory)

	names := make([]string, 0, len(archive.Entries))
	for _, entry := range archive.Entries {
		target := filepath.Join(directory, filepath.FromSlash(entry.Name))
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			return err
		}
		if entry.Mode&os.ModeSymlink != 0 {
			if err := os.Symlink(string(entry.Bytes), target); err != nil {
				return err
			}
		} else {
			if err := os.WriteFile(target, entry.Bytes, 0o644); err != nil {
				return err
			}
			fixedTime := time.Date(2000, time.January, 1, 0, 0, 0, 0, time.UTC)
			if err := os.Chtimes(target, fixedTime, fixedTime); err != nil {
				return err
			}
		}
		names = append(names, entry.Name)
	}

	filename := fixtureFilename(fixture)
	target := filepath.Join(output, filename)
	if err := os.Remove(target); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	intermediate := filepath.Join(directory, "fixture.tar")
	tarArguments := []string{
		"--format", "ustar",
		"--no-xattrs",
		"--no-mac-metadata",
		"-cf", intermediate,
	}
	tarArguments = append(tarArguments, names...)
	if err := runBSDTar(directory, tarArguments); err != nil {
		return fmt.Errorf("stage %s: %w", fixture, err)
	}

	sevenZIPArguments := []string{
		"--format", "7zip",
		"--options", "compression=lzma2,compression-level=9",
		"-cf", target,
		"@" + intermediate,
	}
	if err := runBSDTar(directory, sevenZIPArguments); err != nil {
		return fmt.Errorf("generate %s: %w", fixture, err)
	}
	return nil
}

func runBSDTar(directory string, arguments []string) error {
	command := exec.Command("bsdtar", arguments...)
	command.Dir = directory
	command.Env = append(os.Environ(), "COPYFILE_DISABLE=1", "LC_ALL=C", "TZ=UTC")
	if output, err := command.CombinedOutput(); err != nil {
		return fmt.Errorf("%w: %s", err, output)
	}
	return nil
}

func fixtureFilename(fixture testfixture.SevenZIPFixture) string {
	switch fixture {
	case testfixture.SevenZIPGeneric:
		return "generic.7z"
	case testfixture.SevenZIPStudio:
		return "studio.7z"
	case testfixture.SevenZIPBomb:
		return "bomb.7z"
	case testfixture.SevenZIPDeepPath:
		return "deep-path.7z"
	case testfixture.SevenZIPSymlink:
		return "symlink.7z"
	case testfixture.SevenZIPNested:
		return "nested.7z"
	case testfixture.SevenZIPMissing:
		return "missing.7z"
	default:
		panic(fmt.Sprintf("unknown fixture %q", fixture))
	}
}

func fatal(err error) {
	_, _ = fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}
