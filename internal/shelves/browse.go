package shelves

import (
	"context"
	"sort"

	"github.com/01max/librairii/internal/library"
)

type Evaluation struct {
	Shelf      Shelf       `json:"shelf"`
	Members    []Member    `json:"members"`
	Sources    []Source    `json:"sources"`
	Exclusions []Exclusion `json:"exclusions"`
	Page       int         `json:"page"`
	PageSize   int         `json:"pageSize"`
	TotalItems int         `json:"totalItems"`
	TotalPages int         `json:"totalPages"`
}

func (s *Service) Open(ctx context.Context, id int64, query library.StoryLibraryQuery) (Evaluation, error) {
	shelf, err := s.Get(ctx, id)
	if err != nil {
		return Evaluation{}, err
	}
	members, err := s.Members(ctx, id)
	if err != nil {
		return Evaluation{}, err
	}
	uuids := make([]string, 0, len(members))
	for _, member := range members {
		uuids = append(uuids, member.UUID)
	}
	stories, err := s.library.StoriesInScope(ctx, uuids, query)
	if err != nil {
		return Evaluation{}, err
	}
	canonical, err := library.CanonicalStoryLibraryMembershipQuery(query)
	if err != nil {
		return Evaluation{}, err
	}
	filtered := canonical.Name != "" || len(canonical.AnyOf) > 0 || len(canonical.Languages) > 0 || len(canonical.Compatibilities) > 0 || len(canonical.BooleanFilters) > 0 || len(canonical.ChoiceFilters) > 0
	byUUID := map[string]*library.StorySummary{}
	storyOrder := map[string]int{}
	for i := range stories {
		byUUID[stories[i].UUID] = &stories[i]
		storyOrder[stories[i].UUID] = i
	}
	visible := make([]Member, 0, len(members))
	origins, err := readOrigins(ctx, s.database, id)
	if err != nil {
		return Evaluation{}, err
	}
	for _, member := range members {
		member.Story = byUUID[member.UUID]
		member.Origins = origins[member.UUID]
		if member.Story == nil && filtered {
			continue
		}
		visible = append(visible, member)
	}
	if query.Sort != "" {
		sort.SliceStable(visible, func(i, j int) bool {
			if visible[i].Story == nil {
				return false
			}
			if visible[j].Story == nil {
				return true
			}
			return storyOrder[visible[i].UUID] < storyOrder[visible[j].UUID]
		})
	}
	pageSize := query.PageSize
	if pageSize == 0 {
		pageSize = library.DefaultPageSize
	}
	page := query.Page
	if page < 1 {
		page = 1
	}
	totalPages := (len(visible) + pageSize - 1) / pageSize
	if totalPages > 0 && page > totalPages {
		page = totalPages
	}
	start := (page - 1) * pageSize
	if start > len(visible) {
		start = len(visible)
	}
	end := min(start+pageSize, len(visible))
	sources, err := s.Sources(ctx, id)
	if err != nil {
		return Evaluation{}, err
	}
	exclusions, err := s.Exclusions(ctx, id)
	if err != nil {
		return Evaluation{}, err
	}
	return Evaluation{Shelf: shelf, Members: visible[start:end], Sources: sources, Exclusions: exclusions, Page: page, PageSize: pageSize, TotalItems: len(visible), TotalPages: totalPages}, nil
}

func readOrigins(ctx context.Context, q reader, id int64) (map[string][]Source, error) {
	rows, err := q.QueryContext(ctx, `SELECT o.story_uuid,src.id,coalesce(src.collection_id,0),src.collection_name,src.active
	 FROM shelf_story_origins o JOIN shelf_sources src ON src.id=o.source_id AND src.shelf_id=o.shelf_id
	 WHERE o.shelf_id=? ORDER BY src.id`, id)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := map[string][]Source{}
	for rows.Next() {
		var storyUUID string
		var source Source
		if err := rows.Scan(&storyUUID, &source.ID, &source.CollectionID, &source.Name, &source.Active); err != nil {
			return nil, err
		}
		result[storyUUID] = append(result[storyUUID], source)
	}
	return result, rows.Err()
}
