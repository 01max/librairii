package inspection

import (
	"errors"
	"testing"
)

func FuzzBudgetRejectsUnsafePathsAndLimitOverflows(f *testing.F) {
	for _, seed := range []struct {
		name         string
		compressed   int64
		uncompressed int64
		link         bool
	}{
		{name: "story/metadata.json", compressed: 10, uncompressed: 20},
		{name: "../outside", compressed: 1, uncompressed: 1},
		{name: "/absolute", compressed: 1, uncompressed: 1},
		{name: `C:\private`, compressed: 1, uncompressed: 1},
		{name: "story\x00/file", compressed: 1, uncompressed: 1},
		{name: "story/link", compressed: 1, uncompressed: 1, link: true},
		{name: "story/large", compressed: 1, uncompressed: 2 << 30},
		{name: "story/negative", compressed: -1, uncompressed: -1},
	} {
		f.Add(seed.name, seed.compressed, seed.uncompressed, seed.link)
	}

	f.Fuzz(func(
		t *testing.T,
		name string,
		compressed int64,
		uncompressed int64,
		link bool,
	) {
		limits := DefaultLimits()
		budget, err := NewBudget(limits)
		if err != nil {
			t.Fatal(err)
		}
		entry := EntryInfo{
			Name:              name,
			CompressedBytes:   compressed,
			UncompressedBytes: uncompressed,
			IsLink:            link,
		}
		err = budget.Account(entry)
		if err != nil {
			var validationError *ValidationError
			if !errors.As(err, &validationError) {
				t.Fatalf("Account(%#v) returned unclassified error %v", entry, err)
			}
			return
		}
		if validateEntryPath(name, limits) != nil ||
			link ||
			compressed < 0 ||
			uncompressed < 0 ||
			uncompressed > limits.MaxExpandedBytes ||
			float64(uncompressed)/float64(max(compressed, 1)) >
				limits.MaxCompressionRatio {
			t.Fatalf("Account(%#v) accepted an unsafe entry", entry)
		}
	})
}
