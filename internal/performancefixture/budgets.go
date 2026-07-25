package performancefixture

import "time"

var interactionBudgets = map[string]time.Duration{
	"collectionQuery": 100 * time.Millisecond,
	"substringSearch": 120 * time.Millisecond,
	"combinedFilters": 150 * time.Millisecond,
	"shelfCounts":     250 * time.Millisecond,
	"deepPagination":  120 * time.Millisecond,
	"artworkLoad":     50 * time.Millisecond,
}

func InteractionBudget(name string) (time.Duration, bool) {
	budget, found := interactionBudgets[name]
	return budget, found
}
