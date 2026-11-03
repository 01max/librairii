package tagging

import "errors"

type Kind string

const (
	KindBoolean Kind = "boolean"
	KindChoice  Kind = "choice"
)

type Source string

const (
	SourceUser    Source = "user"
	SourceBuiltIn Source = "builtin"
	SourceDerived Source = "derived"
)

type Presentation string

const (
	PresentationDefault Presentation = "default"
	PresentationWarning Presentation = "warning"
	PresentationSystem  Presentation = "system"
)

var (
	ErrMissingDatabase = errors.New("tag database is required")
)

type Definition struct {
	ID            int64        `json:"id"`
	Key           string       `json:"key"`
	NormalizedKey string       `json:"normalizedKey"`
	Label         string       `json:"label"`
	Color         string       `json:"color"`
	Kind          Kind         `json:"kind"`
	Source        Source       `json:"source"`
	Presentation  Presentation `json:"presentation"`
	Position      int          `json:"position"`
	Protected     bool         `json:"protected"`
}
