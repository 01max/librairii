package shelves

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"
)

var ErrStaleReview = fmt.Errorf("%w: this review is stale or expired; refresh it", ErrConflict)

const maxReviewTokens = 64
const reviewLifetime = 15 * time.Minute

type reviewArguments struct {
	CollectionIDs []int64
	ManualUUIDs   []string
	Name          string
	CollectionID  int64
}

type reviewTicket struct {
	Kind        string
	ShelfID     int64
	Fingerprint [32]byte
	ExpiresAt   time.Time
	Arguments   reviewArguments
}

func fingerprint(e reviewEvaluation) ([32]byte, error) {
	// Broken state changes export eligibility, never membership review validity.
	e.Shelf.BrokenCount = 0
	data, err := json.Marshal(struct {
		Shelf      Shelf
		Members    []Member
		Exclusions []Exclusion
		Sources    []evaluatedSource
		Available  []string
	}{e.Shelf, e.Members, e.Exclusions, e.Sources, e.Available})
	if err != nil {
		return [32]byte{}, err
	}
	return sha256.Sum256(data), nil
}

func (s *Service) issueTicket(kind string, id int64, hash [32]byte, args reviewArguments) string {
	s.reviewsMu.Lock()
	defer s.reviewsMu.Unlock()
	now := s.now()
	for token, ticket := range s.reviews {
		if !ticket.ExpiresAt.After(now) {
			delete(s.reviews, token)
		}
	}
	if len(s.reviews) >= maxReviewTokens {
		var oldest string
		var expiry time.Time
		for token, ticket := range s.reviews {
			if oldest == "" || ticket.ExpiresAt.Before(expiry) {
				oldest = token
				expiry = ticket.ExpiresAt
			}
		}
		delete(s.reviews, oldest)
	}
	args.CollectionIDs = append([]int64(nil), args.CollectionIDs...)
	args.ManualUUIDs = append([]string(nil), args.ManualUUIDs...)
	token := uuid.NewString()
	s.reviews[token] = reviewTicket{Kind: kind, ShelfID: id, Fingerprint: hash, ExpiresAt: now.Add(reviewLifetime), Arguments: args}
	return token
}

func (s *Service) ticket(token, kind string, id int64) (reviewTicket, error) {
	s.reviewsMu.Lock()
	defer s.reviewsMu.Unlock()
	ticket, exists := s.reviews[token]
	if !exists || ticket.Kind != kind || ticket.ShelfID != id || !ticket.ExpiresAt.After(s.now()) {
		return reviewTicket{}, ErrStaleReview
	}
	return ticket, nil
}

func (s *Service) consumeTicket(token string) {
	s.reviewsMu.Lock()
	delete(s.reviews, token)
	s.reviewsMu.Unlock()
}

func (s *Service) validatedReview(ctx context.Context, tx *sql.Tx, id int64, token string) (reviewEvaluation, error) {
	ticket, err := s.ticket(token, "update", id)
	if err != nil {
		return reviewEvaluation{}, err
	}
	evaluation, err := s.evaluateReview(ctx, tx, id)
	if err != nil {
		return reviewEvaluation{}, err
	}
	hash, err := fingerprint(evaluation)
	if err != nil {
		return reviewEvaluation{}, err
	}
	if hash != ticket.Fingerprint {
		return reviewEvaluation{}, ErrStaleReview
	}
	return evaluation, nil
}

type Decision struct {
	UUID   string `json:"uuid"`
	Action string `json:"action"`
}

func validateDecisions(review Review, decisions []Decision) (map[string]string, error) {
	allowed := map[string]map[string]bool{}
	for _, candidate := range review.Additions {
		allowed[candidate.UUID] = map[string]bool{"add": true, "exclude": true, "defer": true}
	}
	for _, candidate := range review.Removals {
		allowed[candidate.UUID] = map[string]bool{"remove": true, "keep": true, "defer": true}
	}
	actions := make(map[string]string, len(decisions))
	for _, decision := range decisions {
		if _, duplicate := actions[decision.UUID]; duplicate || !allowed[decision.UUID][decision.Action] {
			return nil, ErrInvalidInput
		}
		actions[decision.UUID] = decision.Action
	}
	return actions, nil
}
