package inspection

import (
	"archive/zip"
	"context"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"path"
	"sort"
	"strings"
)

type validatedZIP struct {
	reader  *zip.ReadCloser
	entries map[string]*zip.File
}

func openValidatedZIP(ctx context.Context, candidate Candidate, limits Limits) (*validatedZIP, error) {
	if err := limits.Validate(); err != nil {
		return nil, err
	}
	reader, err := zip.OpenReader(candidate.Path)
	if err != nil {
		return nil, &ValidationError{Code: CodeInvalidContainer, Cause: err}
	}

	budget, err := NewBudget(limits)
	if err != nil {
		_ = reader.Close()
		return nil, err
	}
	archive := &validatedZIP{
		reader:  reader,
		entries: make(map[string]*zip.File, len(reader.File)),
	}
	for _, file := range reader.File {
		if err := ctx.Err(); err != nil {
			_ = reader.Close()
			return nil, err
		}
		if file.UncompressedSize64 > math.MaxInt64 || file.CompressedSize64 > math.MaxInt64 {
			_ = reader.Close()
			return nil, &ValidationError{
				Code:  CodeInvalidContainer,
				Entry: file.Name,
				Cause: fmt.Errorf("entry size exceeds supported range"),
			}
		}

		info := file.FileInfo()
		if err := budget.Account(EntryInfo{
			Name:              file.Name,
			CompressedBytes:   int64(file.CompressedSize64),
			UncompressedBytes: int64(file.UncompressedSize64),
			IsDirectory:       info.IsDir(),
			IsLink:            info.Mode()&os.ModeSymlink != 0,
		}); err != nil {
			_ = reader.Close()
			return nil, err
		}

		cleanName := strings.TrimSuffix(path.Clean(file.Name), "/")
		if _, exists := archive.entries[cleanName]; exists {
			_ = reader.Close()
			return nil, &ValidationError{Code: CodeAmbiguousStructure, Entry: file.Name}
		}
		if !info.IsDir() && isNestedArchive(cleanName) {
			_ = reader.Close()
			return nil, &ValidationError{Code: CodeNestedArchive, Entry: file.Name}
		}
		archive.entries[cleanName] = file
	}
	return archive, nil
}

func (a *validatedZIP) Close() error {
	return a.reader.Close()
}

func (a *validatedZIP) has(name string) bool {
	file, exists := a.entries[name]
	return exists && !file.FileInfo().IsDir()
}

func (a *validatedZIP) entryNames() []string {
	names := make([]string, 0, len(a.entries))
	for name, file := range a.entries {
		if !file.FileInfo().IsDir() {
			names = append(names, name)
		}
	}
	sort.Strings(names)
	return names
}

func (a *validatedZIP) hasFileBelow(prefix string) bool {
	prefix = strings.TrimSuffix(prefix, "/") + "/"
	for name, file := range a.entries {
		if strings.HasPrefix(name, prefix) && !file.FileInfo().IsDir() {
			return true
		}
	}
	return false
}

func (a *validatedZIP) read(
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
	defer reader.Close()

	bytes, err := ReadLimited(contextReader{ctx: ctx, reader: reader}, limit, code, name)
	if ctxErr := ctx.Err(); ctxErr != nil && errors.Is(err, ctxErr) {
		return nil, ctxErr
	}
	if err != nil {
		return nil, err
	}
	return bytes, nil
}

func isNestedArchive(name string) bool {
	lowerName := strings.ToLower(name)
	return strings.HasSuffix(lowerName, ".zip") ||
		strings.HasSuffix(lowerName, ".7z") ||
		strings.HasSuffix(lowerName, ".plain.pk") ||
		strings.HasSuffix(lowerName, ".v1.pk") ||
		strings.HasSuffix(lowerName, ".v2.pk") ||
		strings.HasSuffix(lowerName, ".pk")
}

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r contextReader) Read(buffer []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.reader.Read(buffer)
}
