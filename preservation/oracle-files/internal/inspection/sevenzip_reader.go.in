package inspection

import (
	"context"
	"errors"
	"fmt"
	"math"
	"os"
	"path"
	"sort"
	"strings"

	sevenzip "github.com/bodgit/sevenzip"
)

type validatedSevenZIP struct {
	source  *os.File
	reader  *sevenzip.Reader
	entries map[string]*sevenzip.File
}

func openValidatedSevenZIP(
	ctx context.Context,
	candidate Candidate,
	limits Limits,
) (*validatedSevenZIP, error) {
	if err := limits.Validate(); err != nil {
		return nil, err
	}
	source, err := os.Open(candidate.Path)
	if err != nil {
		return nil, &ValidationError{Code: CodeInvalidContainer, Cause: err}
	}
	info, err := source.Stat()
	if err != nil {
		_ = source.Close()
		return nil, &ValidationError{Code: CodeInvalidContainer, Cause: err}
	}
	if !info.Mode().IsRegular() {
		_ = source.Close()
		return nil, &ValidationError{
			Code:  CodeInvalidContainer,
			Cause: fmt.Errorf("7z source is not a regular file"),
		}
	}

	reader, err := sevenzip.NewReader(source, info.Size())
	if err != nil {
		_ = source.Close()
		return nil, &ValidationError{Code: CodeInvalidContainer, Cause: err}
	}
	budget, err := NewBudget(limits)
	if err != nil {
		_ = source.Close()
		return nil, err
	}
	archive := &validatedSevenZIP{
		source:  source,
		reader:  reader,
		entries: make(map[string]*sevenzip.File, len(reader.File)),
	}
	for _, file := range reader.File {
		if err := ctx.Err(); err != nil {
			_ = source.Close()
			return nil, err
		}
		if file.UncompressedSize > math.MaxInt64 {
			_ = source.Close()
			return nil, &ValidationError{
				Code:  CodeInvalidContainer,
				Entry: file.Name,
				Cause: fmt.Errorf("entry size exceeds supported range"),
			}
		}
		fileInfo := file.FileInfo()
		special := fileInfo.Mode().Type() != 0 && !fileInfo.IsDir()
		if err := budget.AccountExpanded(EntryInfo{
			Name:              file.Name,
			UncompressedBytes: int64(file.UncompressedSize),
			IsDirectory:       fileInfo.IsDir(),
			IsLink:            special,
		}); err != nil {
			_ = source.Close()
			return nil, err
		}

		cleanName := strings.TrimSuffix(path.Clean(file.Name), "/")
		if _, exists := archive.entries[cleanName]; exists {
			_ = source.Close()
			return nil, &ValidationError{Code: CodeAmbiguousStructure, Entry: file.Name}
		}
		if !fileInfo.IsDir() && isNestedArchive(cleanName) {
			_ = source.Close()
			return nil, &ValidationError{Code: CodeNestedArchive, Entry: file.Name}
		}
		archive.entries[cleanName] = file
	}
	if err := budget.ValidateCompression(info.Size()); err != nil {
		_ = source.Close()
		return nil, err
	}
	return archive, nil
}

func (a *validatedSevenZIP) Close() error {
	return a.source.Close()
}

func (a *validatedSevenZIP) has(name string) bool {
	file, exists := a.entries[name]
	return exists && !file.FileInfo().IsDir()
}

func (a *validatedSevenZIP) hasFileBelow(prefix string) bool {
	prefix = strings.TrimSuffix(prefix, "/") + "/"
	for name, file := range a.entries {
		if strings.HasPrefix(name, prefix) && !file.FileInfo().IsDir() {
			return true
		}
	}
	return false
}

func (a *validatedSevenZIP) entryNames() []string {
	names := make([]string, 0, len(a.entries))
	for name, file := range a.entries {
		if !file.FileInfo().IsDir() {
			names = append(names, name)
		}
	}
	sort.Strings(names)
	return names
}

func (a *validatedSevenZIP) read(
	ctx context.Context,
	name string,
	limit int64,
	code ErrorCode,
) ([]byte, error) {
	file, exists := a.entries[name]
	if !exists || file.FileInfo().IsDir() {
		return nil, &ValidationError{Code: CodeMissingEntry, Entry: name}
	}
	reader, err := file.Open()
	if err != nil {
		return nil, &ValidationError{Code: CodeInvalidContainer, Entry: name, Cause: err}
	}
	bytes, readErr := ReadLimited(contextReader{ctx: ctx, reader: reader}, limit, code, name)
	closeErr := reader.Close()
	if ctxErr := ctx.Err(); ctxErr != nil && errors.Is(readErr, ctxErr) {
		return nil, ctxErr
	}
	if readErr != nil {
		return nil, readErr
	}
	if closeErr != nil {
		return nil, &ValidationError{
			Code:  CodeInvalidContainer,
			Entry: name,
			Cause: closeErr,
		}
	}
	return bytes, nil
}
