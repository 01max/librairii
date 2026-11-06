# Curated Shelves

## Purpose

Define persistent curated Shelf membership, explicit seeding and curation, durable decisions, unavailable members, and independent duplication.

## Requirements

### Requirement: Persistent curated Shelf identity and membership
The system SHALL provide independently named Shelves whose ordered membership consists of explicit story UUIDs. Shelf names SHALL be normalized and unique within Shelves independently of Collection names. Creating, listing, opening, renaming, reordering, duplicating, and deleting Shelves SHALL NOT change library stories, archives, Collections, or other Shelves. Imports, metadata/tag changes, broken flags, reads, and restarts SHALL NOT implicitly add or remove members.

#### Scenario: Shelf is empty or purely manual
- **WHEN** a user creates an empty Shelf with no sources
- **THEN** it remains valid and supports manual curation without rule-based update suggestions

#### Scenario: Collection matches change
- **WHEN** a linked Collection acquires or loses matching stories
- **THEN** the Shelf's approved UUID membership remains unchanged before explicit approval

#### Scenario: Shelf name duplicates only a Collection name
- **WHEN** a user creates a Shelf whose name equals a Collection name but no Shelf name
- **THEN** the name is accepted under independent namespace rules

#### Scenario: Shelf is deleted
- **WHEN** the user confirms Shelf deletion
- **THEN** only that Shelf and its membership/source/decision records are removed

### Requirement: Explicit initial seeding
The system SHALL permit Shelf creation from manually selected stories, one or more Collection sources, or both, and SHALL display the deduplicated proposed initial members before a deliberate Create with N stories action. Linking a source alone SHALL NOT insert its matches. Manual seeds SHALL be retained independently of source matching; accepted source-only seeds SHALL retain source attribution. Overlapping manual/source seeds SHALL retain their manual-retention decision.

#### Scenario: Collection-backed Shelf is created
- **WHEN** the user previews sources, selects initial members, and confirms Create with N stories
- **THEN** exactly the confirmed UUIDs are stored once and unchecked matches remain eligible for later review

#### Scenario: Initial preview is cancelled or stale
- **WHEN** the user cancels creation or source membership changes before confirmation
- **THEN** cancellation writes nothing, or stale confirmation requests a fresh preview before storing membership, respectively

#### Scenario: Source is linked without seeding
- **WHEN** a user confirms a new Collection source without approving initial additions
- **THEN** the source is attached and membership is unchanged

### Requirement: Manual curation and retention
The system SHALL support explicit single/bulk manual additions, removals and member reordering without a user-facing protection flag or toggle. Manual addition SHALL record independent retention, including for an existing source-derived member, and clear an existing exclusion. Every approved story SHALL remain until explicit removal. New members SHALL append in stable approved order. View-only filtering/sorting SHALL NOT persist membership changes. Source-less Shelves SHALL retain their approved membership. Historical protection fields SHALL remain lossless internal compatibility data.

#### Scenario: User adds a story already supplied by a source
- **WHEN** the user explicitly adds an existing source-derived member manually
- **THEN** its existing membership acquires manual retention without duplication

#### Scenario: User keeps a removal candidate
- **WHEN** the user applies Keep for a proposed removal
- **THEN** the story remains with a manual Keep decision against future source-based removal proposals

#### Scenario: User inspects membership actions
- **WHEN** the user opens approved membership actions
- **THEN** no protection or unprotect setting is exposed, and removal and exclusion remain explicit separate decisions

#### Scenario: User refines or reorders the view
- **WHEN** a user searches, filters, or sorts within a Shelf, or explicitly reorders its members
- **THEN** view refinements leave stored contents/order unchanged, while an explicit member reorder changes only the approved order

### Requirement: Persistent exclusions and reversible decisions
The system SHALL store Shelf-level UUID exclusions separately from membership. Ordinary Remove selected SHALL remove membership only and SHALL NOT create an exclusion. Exclude from suggestions and review Remove and exclude SHALL be separate explicit actions. Historical exclusions SHALL persist until explicitly reversed. Exclusions SHALL suppress proposals across every source until explicitly cleared. Clearing an exclusion SHALL NOT add membership; deferring SHALL persist neither exclusion nor manual retention.

#### Scenario: User rejects an addition permanently
- **WHEN** the user applies Exclude for a proposed addition
- **THEN** the UUID is excluded and later source matches do not propose it again

#### Scenario: User removes a matching member ordinarily
- **WHEN** the user confirms Remove selected
- **THEN** the UUID leaves membership without a new exclusion and may be suggested and accepted again

#### Scenario: User explicitly removes and excludes a matching member
- **WHEN** the user confirms Remove and exclude from suggestions
- **THEN** the UUID leaves membership and future source suggestions are suppressed until the exclusion is reversed

#### Scenario: User reverses an exclusion
- **WHEN** the user clears an exclusion
- **THEN** membership stays unchanged and a currently source-matching story becomes eligible for the next review

#### Scenario: User defers an addition
- **WHEN** a proposed addition is left undecided or marked Not now
- **THEN** the story remains eligible for later review with no stored exclusion

### Requirement: Stable unavailable membership
The system SHALL preserve Shelf membership, order, protection, origins, and exclusions independently of numeric library story IDs. Unresolved member UUIDs SHALL display as Unavailable with a cached title when available and SHALL NOT become source-removal candidates solely because library resolution fails.

#### Scenario: Story is removed and reimported
- **WHEN** a Shelf member is removed from the library and later reimported with the same UUID and a different numeric ID
- **THEN** its approved membership remains throughout and resolves to the reimported story automatically

#### Scenario: Unavailable member is browsed
- **WHEN** a Shelf contains an unresolved UUID
- **THEN** its total membership includes the UUID and the view exposes an unavailable state and explicit removal action

### Requirement: Independent Shelf duplication and persistence
The system SHALL persist Shelf membership, sources, origins, protection, exclusions, and ordering across restart. Duplication SHALL copy those approved states under independent identities without reevaluating source results or copying pending review tokens.

#### Scenario: Sourced Shelf is duplicated
- **WHEN** the user duplicates a Shelf with protected, excluded, and unavailable stories
- **THEN** the new Shelf starts with equivalent approved state and future decisions on either Shelf do not affect the other

#### Scenario: Application restarts
- **WHEN** the application restarts after curation
- **THEN** the same approved contents, decisions, source links, and member order are restored
