package app

// GroupScope keeps saved-query identities separate from curated membership
// identities even when their database IDs happen to be equal.
type GroupScope struct {
	Kind string `json:"kind"`
	ID   int64  `json:"id,omitempty"`
}

const (
	ScopeLibrary    = "library"
	ScopeCollection = "collection"
	ScopeShelf      = "shelf"
)
