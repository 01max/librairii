package shelves

import (
	"context"
	"database/sql"
	"errors"
	"sort"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

type Candidate struct {
	UUID    string                `json:"uuid"`
	Title   string                `json:"title"`
	Story   *library.StorySummary `json:"story,omitempty"`
	Sources []Source              `json:"sources"`
	Origins []Source              `json:"origins"`
	Reason  string                `json:"reason"`
	Manual  bool                  `json:"manual,omitempty"`
}

type ReviewWarning struct {
	SourceID     int64  `json:"sourceId,omitempty"`
	CollectionID int64  `json:"collectionId,omitempty"`
	StoryUUID    string `json:"storyUuid,omitempty"`
	Name         string `json:"name,omitempty"`
	Code         string `json:"code"`
	Message      string `json:"message"`
}

type Review struct {
	ShelfID    int64           `json:"shelfId"`
	Revision   int64           `json:"revision"`
	Token      string          `json:"token"`
	Additions  []Candidate     `json:"additions"`
	Removals   []Candidate     `json:"removals"`
	Warnings   []ReviewWarning `json:"warnings"`
	Incomplete bool            `json:"incomplete"`
	UpToDate   bool            `json:"upToDate"`
}

type evaluatedSource struct {
	Source     Source
	Definition collections.Collection
	Matches    []string
	Warning    string
}

type reviewEvaluation struct {
	Review     Review
	Shelf      Shelf
	Members    []Member
	Exclusions []Exclusion
	Sources    []evaluatedSource
	Available  []string
}

func sortedKeys[V any](values map[string]V) []string {
	result := make([]string, 0, len(values))
	for id := range values {
		result = append(result, id)
	}
	sort.Strings(result)
	return result
}

func evaluateSources(ctx context.Context, tx *sql.Tx, query *library.Query, sources []Source) ([]evaluatedSource, map[string][]Source, error) {
	states := make([]evaluatedSource, 0, len(sources))
	union := map[string][]Source{}
	for _, source := range sources {
		opened, err := collections.InspectInTransaction(ctx, tx, source.CollectionID)
		state := evaluatedSource{Source: source, Definition: opened.Collection, Matches: []string{}}
		if err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return nil, nil, err
			}
			state.Warning = "source_needs_attention"
			states = append(states, state)
			continue
		}
		matches, err := query.MatchingUUIDs(ctx, opened.Query.StoryLibraryQuery())
		if err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return nil, nil, err
			}
			state.Warning = "source_evaluation_failed"
			states = append(states, state)
			continue
		}
		state.Matches = matches
		states = append(states, state)
		for _, storyUUID := range matches {
			union[storyUUID] = append(union[storyUUID], source)
		}
	}
	return states, union, nil
}

func (s *Service) evaluateReview(ctx context.Context, tx *sql.Tx, id int64) (reviewEvaluation, error) {
	shelf, err := readShelf(ctx, tx, id)
	if err != nil {
		return reviewEvaluation{}, err
	}
	members, err := readMembers(ctx, tx, id)
	if err != nil {
		return reviewEvaluation{}, err
	}
	exclusions, err := readExclusions(ctx, tx, id)
	if err != nil {
		return reviewEvaluation{}, err
	}
	sources, err := readSources(ctx, tx, id, true)
	if err != nil {
		return reviewEvaluation{}, err
	}
	query := s.library.WithTransaction(tx)
	states, union, err := evaluateSources(ctx, tx, query, sources)
	if err != nil {
		return reviewEvaluation{}, err
	}
	memberIDs := make([]string, 0, len(members))
	for _, member := range members {
		memberIDs = append(memberIDs, member.UUID)
	}
	available, err := activeTitles(ctx, tx, memberIDs)
	if err != nil {
		return reviewEvaluation{}, err
	}
	origins, err := readOrigins(ctx, tx, id)
	if err != nil {
		return reviewEvaluation{}, err
	}
	allIDs := map[string]bool{}
	for _, id := range memberIDs {
		allIDs[id] = true
	}
	for id := range union {
		allIDs[id] = true
	}
	stories, err := query.StoriesInScope(ctx, sortedKeys(allIDs), library.StoryLibraryQuery{})
	if err != nil {
		return reviewEvaluation{}, err
	}
	byUUID := map[string]*library.StorySummary{}
	for i := range stories {
		byUUID[stories[i].UUID] = &stories[i]
	}
	current := map[string]bool{}
	for _, member := range members {
		current[member.UUID] = true
	}
	excluded := map[string]bool{}
	for _, item := range exclusions {
		excluded[item.UUID] = true
	}
	review := Review{ShelfID: id, Revision: shelf.Revision, Additions: make([]Candidate, 0), Removals: make([]Candidate, 0), Warnings: make([]ReviewWarning, 0)}
	for _, state := range states {
		if state.Warning != "" {
			review.Incomplete = true
			review.Warnings = append(review.Warnings, ReviewWarning{SourceID: state.Source.ID, CollectionID: state.Source.CollectionID, Name: state.Source.Name, Code: state.Warning, Message: "A source needs attention. Removal suggestions are paused."})
		}
	}
	for _, story := range stories {
		matching := union[story.UUID]
		if len(matching) > 0 && !current[story.UUID] && !excluded[story.UUID] {
			review.Additions = append(review.Additions, Candidate{UUID: story.UUID, Title: story.Title, Story: byUUID[story.UUID], Sources: matching, Reason: "Matches a linked Collection."})
		}
	}
	for _, member := range members {
		if _, exists := available[member.UUID]; !exists {
			review.Warnings = append(review.Warnings, ReviewWarning{StoryUUID: member.UUID, Code: "story_unavailable", Message: "This approved story is unavailable in the library."})
			continue
		}
		if review.Incomplete || len(sources) == 0 || member.Protected || len(union[member.UUID]) > 0 {
			continue
		}
		title := member.CachedTitle
		if story := byUUID[member.UUID]; story != nil {
			title = story.Title
		}
		review.Removals = append(review.Removals, Candidate{UUID: member.UUID, Title: title, Story: byUUID[member.UUID], Origins: origins[member.UUID], Reason: "No longer matches any linked Collection."})
	}
	review.UpToDate = !review.Incomplete && len(review.Additions) == 0 && len(review.Removals) == 0
	return reviewEvaluation{Review: review, Shelf: shelf, Members: members, Exclusions: exclusions, Sources: states, Available: sortedKeys(available)}, nil
}

func (s *Service) Preview(ctx context.Context, id int64) (Review, error) {
	tx, err := s.database.BeginTx(ctx, nil)
	if err != nil {
		return Review{}, err
	}
	defer tx.Rollback()
	evaluation, err := s.evaluateReview(ctx, tx, id)
	if err != nil {
		return Review{}, err
	}
	hash, err := fingerprint(evaluation)
	if err != nil {
		return Review{}, err
	}
	if err := tx.Commit(); err != nil {
		return Review{}, err
	}
	evaluation.Review.Token = s.issueTicket("update", id, hash, reviewArguments{})
	return evaluation.Review, nil
}
