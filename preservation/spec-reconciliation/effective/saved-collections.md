# Saved Collections

## Purpose

Define named dynamic Collection queries, migration from saved Shelves, query integrity, management, and deduplicated union selection.

## Requirements

### Requirement: Migrate dynamic Shelves to Collections
The system SHALL migrate existing dynamic Shelves to named Collections while preserving identities, names, ordering, timestamps, validity states, and supported non-Broken query criteria. It SHALL apply the deliberate removal of built-in Broken filtering defined by library-browsing. It SHALL NOT materialize current results as curated Shelf membership or create duplicate groups.

#### Scenario: Existing saved search is upgraded
- **WHEN** a library containing a dynamic Shelf is upgraded
- **THEN** the same identity, name, order, and saved search appear as a Collection with equivalent non-Broken criteria and no newly created curated Shelf

#### Scenario: Existing invalid definition is upgraded
- **WHEN** an existing saved query has unsupported encoding or an unrelated unavailable criterion
- **THEN** it remains visible as a Collection needing attention with its repairable definition retained

### Requirement: Save canonical Collection criteria
The system SHALL save the current story-name search and membership-affecting metadata/tag filters as a uniquely named, versioned Collection, excluding paging, view sort, transient selection, operation state, and built-in Broken filtering. Names SHALL follow the existing normalized uniqueness rules within Collections independently of Shelf names.

#### Scenario: Search and filters are saved
- **WHEN** the user saves a filtered result under a unique Collection name
- **THEN** its canonical criteria reproduce the complete matching set across all pages, including flagged stories

#### Scenario: Empty query is saved
- **WHEN** the user saves without name or membership filters
- **THEN** the Collection represents every active library story, including broken stories

#### Scenario: Duplicate Collection name is submitted
- **WHEN** the submitted name normalizes to another Collection name
- **THEN** creation fails with an inline validation error without changing either definition

### Requirement: Collection management and dynamic evaluation
The system SHALL support listing, opening, renaming, duplicating, reordering, replacing criteria, and deleting Collections without implicitly modifying stories or approved Shelf membership. Opening, counts, previews, and export-scope resolution SHALL evaluate current library state using the same canonical query semantics. Deleting a linked Collection SHALL follow the reviewed detachment requirements in shelf-update-reviews.

#### Scenario: Imported or retagged story changes matches
- **WHEN** a story starts or stops matching a Collection's non-Broken criteria
- **THEN** its next evaluation reflects that change while linked Shelves retain their approved contents

#### Scenario: Story is marked broken
- **WHEN** a matching story's broken flag is set or cleared
- **THEN** its Collection membership and the Collection's count remain unchanged

#### Scenario: Collection is duplicated or its criteria replaced
- **WHEN** the user duplicates a Collection or explicitly replaces its query
- **THEN** duplication produces an independent equivalent definition, or replacement retains the existing identity and name, respectively

#### Scenario: Collection has no matches
- **WHEN** a valid Collection evaluates to zero stories
- **THEN** it remains valid with an empty state offering query editing and import

### Requirement: Collection query integrity
The system SHALL validate and version every saved Collection query, mark unavailable or unmigratable criteria as needing attention, and prevent invalid queries from becoming empty or unfiltered source results. Supported legacy payloads SHALL remove built-in Broken restrictions while preserving other predicate composition and ordinary user-tag semantics.

#### Scenario: Referenced tag value is deleted
- **WHEN** a user confirms deletion of a value referenced by a Collection
- **THEN** that Collection needs attention until repaired and cannot be exported as a valid query

#### Scenario: Supported legacy query is opened
- **WHEN** a supported older query contains nested built-in Broken predicates and unrelated tag predicates
- **THEN** migration removes the built-in Broken restrictions recursively, retains the unrelated predicate structure, and emits the current canonical version

#### Scenario: Ordinary broken-named tag is used
- **WHEN** a supported query refers to an ordinary user tag named broken, including a current-version tag reusing a retired numeric ID
- **THEN** it retains normal user-tag filtering semantics without controlling the story flag

### Requirement: Multi-Collection union selection
The system SHALL let users select Collections as a deduplicated OR union for preview and export and SHALL report each current count, the unique combined count, and collapsed overlap. Source-selection copy SHALL explain that matching any selected Collection is sufficient. Saving a combined Collection SHALL snapshot the selected query definitions rather than establish live Collection dependencies.

#### Scenario: Story matches two Collections
- **WHEN** the selected Collections both match one UUID
- **THEN** their union contains the story once and reports the overlap

#### Scenario: User wants an intersection
- **WHEN** a user needs stories satisfying age and adventure criteria together
- **THEN** those criteria can be combined within one Collection rather than interpreting two selected sources as an intersection

#### Scenario: Combined definition is saved
- **WHEN** a user saves the union as a new Collection and later edits an original Collection
- **THEN** the saved combined definition retains its copied criteria and changes only through its own edits or library changes
