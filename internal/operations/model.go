package operations

type Kind string

const (
	KindImport       Kind = "import"
	KindMetadataSync Kind = "metadata_sync"
	KindExport       Kind = "export"
)

type ExportSourceType string

const (
	ExportSourceSelection    ExportSourceType = "selection"
	ExportSourceCurrentQuery ExportSourceType = "current_query"
	ExportSourceShelf        ExportSourceType = "shelf"
	ExportSourceShelves      ExportSourceType = "shelves"
)

type ExportSource struct {
	Type       ExportSourceType `json:"type"`
	ShelfIDs   []int64          `json:"shelfIds,omitempty"`
	ShelfNames []string         `json:"shelfNames,omitempty"`
}

type Status string

const (
	StatusQueued             Status = "queued"
	StatusRunning            Status = "running"
	StatusSucceeded          Status = "succeeded"
	StatusPartiallySucceeded Status = "partially_succeeded"
	StatusFailed             Status = "failed"
	StatusCancelled          Status = "cancelled"
	StatusInterrupted        Status = "interrupted"
)

type ItemStatus string

const (
	ItemPending    ItemStatus = "pending"
	ItemRunning    ItemStatus = "running"
	ItemSucceeded  ItemStatus = "succeeded"
	ItemSkipped    ItemStatus = "skipped"
	ItemConflicted ItemStatus = "conflicted"
	ItemFailed     ItemStatus = "failed"
	ItemCancelled  ItemStatus = "cancelled"
)

type NewItem struct {
	StoryID             int64
	StoryUUID           string
	StoryTitle          string
	SourceName          string
	OutputName          string
	ArchiveRelativePath string
	ArchiveSHA256       string
	TotalBytes          int64
}

type ItemSnapshot struct {
	ID                  int64      `json:"id"`
	StoryID             int64      `json:"storyId,omitempty"`
	StoryUUID           string     `json:"storyUuid,omitempty"`
	StoryTitle          string     `json:"storyTitle,omitempty"`
	SourceName          string     `json:"sourceName"`
	OutputName          string     `json:"outputName,omitempty"`
	ArchiveRelativePath string     `json:"archiveRelativePath,omitempty"`
	ArchiveSHA256       string     `json:"archiveSha256,omitempty"`
	Status              ItemStatus `json:"status"`
	OutcomeCode         string     `json:"outcomeCode,omitempty"`
	OutcomeMessage      string     `json:"outcomeMessage,omitempty"`
	CompletedBytes      int64      `json:"completedBytes"`
	TotalBytes          int64      `json:"totalBytes"`
}

type Snapshot struct {
	ID               string           `json:"id"`
	Kind             Kind             `json:"kind"`
	Status           Status           `json:"status"`
	ExportSourceType ExportSourceType `json:"exportSourceType,omitempty"`
	SourceShelfIDs   []int64          `json:"sourceShelfIds,omitempty"`
	SourceShelfNames []string         `json:"sourceShelfNames,omitempty"`
	Destination      string           `json:"destination,omitempty"`
	CompletedItems   int              `json:"completedItems"`
	TotalItems       int              `json:"totalItems"`
	TotalBytes       int64            `json:"totalBytes"`
	CancelRequested  bool             `json:"cancelRequested"`
	ErrorCode        string           `json:"errorCode,omitempty"`
	ErrorMessage     string           `json:"errorMessage,omitempty"`
	CreatedAt        string           `json:"createdAt"`
	StartedAt        string           `json:"startedAt,omitempty"`
	FinishedAt       string           `json:"finishedAt,omitempty"`
	Items            []ItemSnapshot   `json:"items"`
}

func (s Snapshot) Terminal() bool {
	switch s.Status {
	case StatusSucceeded,
		StatusPartiallySucceeded,
		StatusFailed,
		StatusCancelled,
		StatusInterrupted:
		return true
	default:
		return false
	}
}
