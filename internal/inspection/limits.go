package inspection

import (
	"fmt"
	"math"
	"path"
	"strings"
	"unicode/utf8"
)

type Limits struct {
	MaxPathBytes        int
	MaxPathDepth        int
	MaxEntries          int
	MaxExpandedBytes    int64
	MaxCompressionRatio float64
	MaxMetadataBytes    int64
	MaxArtworkBytes     int64
}

func DefaultLimits() Limits {
	return Limits{
		MaxPathBytes:        512,
		MaxPathDepth:        16,
		MaxEntries:          10_000,
		MaxExpandedBytes:    2 << 30,
		MaxCompressionRatio: 200,
		MaxMetadataBytes:    1 << 20,
		MaxArtworkBytes:     10 << 20,
	}
}

func (l Limits) Validate() error {
	if l.MaxPathBytes <= 0 ||
		l.MaxPathDepth <= 0 ||
		l.MaxEntries <= 0 ||
		l.MaxExpandedBytes <= 0 ||
		l.MaxCompressionRatio < 1 ||
		math.IsNaN(l.MaxCompressionRatio) ||
		math.IsInf(l.MaxCompressionRatio, 0) ||
		l.MaxMetadataBytes <= 0 ||
		l.MaxArtworkBytes <= 0 {
		return &ValidationError{Code: CodeInvalidLimits}
	}
	return nil
}

type EntryInfo struct {
	Name              string
	CompressedBytes   int64
	UncompressedBytes int64
	IsDirectory       bool
	IsLink            bool
}

type Budget struct {
	limits          Limits
	entries         int
	expandedBytes   int64
	compressedBytes int64
}

func NewBudget(limits Limits) (*Budget, error) {
	if err := limits.Validate(); err != nil {
		return nil, err
	}
	return &Budget{limits: limits}, nil
}

func (b *Budget) Account(entry EntryInfo) error {
	if err := b.accountExpanded(entry); err != nil {
		return err
	}
	b.compressedBytes += entry.CompressedBytes
	return b.validateCompression(b.compressedBytes, entry.Name)
}

func (b *Budget) AccountExpanded(entry EntryInfo) error {
	return b.accountExpanded(entry)
}

func (b *Budget) ValidateCompression(compressedBytes int64) error {
	return b.validateCompression(compressedBytes, "")
}

func (b *Budget) accountExpanded(entry EntryInfo) error {
	if err := validateEntryPath(entry.Name, b.limits); err != nil {
		return err
	}
	if entry.IsLink {
		return &ValidationError{Code: CodeLinkEntry, Entry: entry.Name}
	}
	if entry.CompressedBytes < 0 || entry.UncompressedBytes < 0 {
		return &ValidationError{
			Code:  CodeInvalidContainer,
			Entry: entry.Name,
			Cause: fmt.Errorf("negative entry size"),
		}
	}

	b.entries++
	if b.entries > b.limits.MaxEntries {
		return &ValidationError{Code: CodeEntryLimit, Entry: entry.Name}
	}
	if entry.UncompressedBytes > b.limits.MaxExpandedBytes-b.expandedBytes {
		return &ValidationError{Code: CodeExpandedSizeLimit, Entry: entry.Name}
	}
	b.expandedBytes += entry.UncompressedBytes
	return nil
}

func (b *Budget) validateCompression(compressedBytes int64, entry string) error {
	if compressedBytes < 0 {
		return &ValidationError{
			Code:  CodeInvalidContainer,
			Entry: entry,
			Cause: fmt.Errorf("negative compressed size"),
		}
	}
	compressedForRatio := compressedBytes
	if compressedForRatio == 0 && b.expandedBytes > 0 {
		compressedForRatio = 1
	}
	if float64(b.expandedBytes)/float64(compressedForRatio) > b.limits.MaxCompressionRatio {
		return &ValidationError{Code: CodeCompressionRatioLimit, Entry: entry}
	}
	return nil
}

func validateEntryPath(name string, limits Limits) error {
	if name == "" ||
		!utf8.ValidString(name) ||
		strings.ContainsRune(name, '\x00') ||
		strings.Contains(name, `\`) ||
		len([]byte(name)) > limits.MaxPathBytes {
		return &ValidationError{Code: CodeUnsafePath, Entry: name}
	}

	normalized := strings.ReplaceAll(name, `\`, "/")
	clean := path.Clean(normalized)
	if path.IsAbs(normalized) ||
		clean == "." ||
		clean == ".." ||
		strings.HasPrefix(clean, "../") ||
		hasWindowsDrivePrefix(normalized) {
		return &ValidationError{Code: CodeUnsafePath, Entry: name}
	}

	depth := 0
	for _, component := range strings.Split(strings.TrimSuffix(clean, "/"), "/") {
		if component != "" {
			depth++
		}
	}
	if depth > limits.MaxPathDepth {
		return &ValidationError{Code: CodeUnsafePath, Entry: name}
	}
	return nil
}

func hasWindowsDrivePrefix(name string) bool {
	return len(name) >= 2 &&
		((name[0] >= 'A' && name[0] <= 'Z') || (name[0] >= 'a' && name[0] <= 'z')) &&
		name[1] == ':'
}
