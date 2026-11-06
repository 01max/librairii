package inspection

import (
	"errors"
	"strings"
	"testing"
)

func TestDefaultLimitsAreValid(t *testing.T) {
	t.Parallel()

	if err := DefaultLimits().Validate(); err != nil {
		t.Fatalf("DefaultLimits().Validate() error = %v", err)
	}
}

func TestLimitsRejectInvalidValues(t *testing.T) {
	t.Parallel()

	tests := map[string]func(*Limits){
		"path bytes":        func(l *Limits) { l.MaxPathBytes = 0 },
		"path depth":        func(l *Limits) { l.MaxPathDepth = 0 },
		"entries":           func(l *Limits) { l.MaxEntries = 0 },
		"expanded bytes":    func(l *Limits) { l.MaxExpandedBytes = 0 },
		"compression ratio": func(l *Limits) { l.MaxCompressionRatio = 0 },
		"metadata":          func(l *Limits) { l.MaxMetadataBytes = 0 },
		"artwork":           func(l *Limits) { l.MaxArtworkBytes = 0 },
	}
	for name, mutate := range tests {
		name, mutate := name, mutate
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			limits := DefaultLimits()
			mutate(&limits)
			if err := limits.Validate(); !ErrorHasCode(err, CodeInvalidLimits) {
				t.Fatalf("Validate() error = %v", err)
			}
		})
	}
}

func TestBudgetEnforcesArchiveSafetyLimits(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name   string
		limits func() Limits
		entry  EntryInfo
		code   ErrorCode
	}{
		{
			name:   "unsafe parent path",
			limits: DefaultLimits,
			entry:  EntryInfo{Name: "../story.json"},
			code:   CodeUnsafePath,
		},
		{
			name:   "drive-letter absolute path",
			limits: DefaultLimits,
			entry:  EntryInfo{Name: `C:\story.json`},
			code:   CodeUnsafePath,
		},
		{
			name:   "link",
			limits: DefaultLimits,
			entry:  EntryInfo{Name: "story.json", IsLink: true},
			code:   CodeLinkEntry,
		},
		{
			name: "entry count",
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxEntries = 1
				return limits
			},
			entry: EntryInfo{Name: "second", CompressedBytes: 1},
			code:  CodeEntryLimit,
		},
		{
			name: "expanded size",
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxExpandedBytes = 2
				return limits
			},
			entry: EntryInfo{Name: "large", CompressedBytes: 2, UncompressedBytes: 3},
			code:  CodeExpandedSizeLimit,
		},
		{
			name: "compression ratio",
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxCompressionRatio = 2
				return limits
			},
			entry: EntryInfo{Name: "compressed", CompressedBytes: 2, UncompressedBytes: 5},
			code:  CodeCompressionRatioLimit,
		},
	}

	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			budget, err := NewBudget(test.limits())
			if err != nil {
				t.Fatal(err)
			}
			if test.name == "entry count" {
				if err := budget.Account(EntryInfo{Name: "first", CompressedBytes: 1}); err != nil {
					t.Fatal(err)
				}
			}
			if err := budget.Account(test.entry); !ErrorHasCode(err, test.code) {
				t.Fatalf("Account() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestBudgetSupportsAggregateCompressionAccounting(t *testing.T) {
	t.Parallel()

	limits := DefaultLimits()
	limits.MaxCompressionRatio = 2
	budget, err := NewBudget(limits)
	if err != nil {
		t.Fatal(err)
	}
	if err := budget.AccountExpanded(EntryInfo{
		Name:              "story",
		UncompressedBytes: 5,
	}); err != nil {
		t.Fatal(err)
	}
	if err := budget.ValidateCompression(2); !ErrorHasCode(err, CodeCompressionRatioLimit) {
		t.Fatalf("ValidateCompression() error = %v", err)
	}
}

func TestReadLimitedUsesTypedBoundaryErrors(t *testing.T) {
	t.Parallel()

	got, err := ReadLimited(strings.NewReader("story"), 5, CodeMetadataLimit, "story.json")
	if err != nil || string(got) != "story" {
		t.Fatalf("ReadLimited() = %q, %v", got, err)
	}
	if _, err := ReadLimited(
		strings.NewReader("story!"),
		5,
		CodeMetadataLimit,
		"story.json",
	); !ErrorHasCode(err, CodeMetadataLimit) {
		t.Fatalf("ReadLimited(over limit) error = %v", err)
	}

	expected := errors.New("read failure")
	failing := errorReader{err: expected}
	if _, err := ReadLimited(failing, 5, CodeArtworkLimit, "thumbnail.png"); !errors.Is(err, expected) {
		t.Fatalf("ReadLimited(failure) error = %v", err)
	}
}

type errorReader struct {
	err error
}

func (r errorReader) Read([]byte) (int, error) {
	return 0, r.err
}
