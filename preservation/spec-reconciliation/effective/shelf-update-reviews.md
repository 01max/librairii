# Shelf Update Reviews

## Purpose

Define Collection sources for curated Shelves, conservative update proposals, explicit atomic decisions, snapshot integrity, and reviewed source detachment.

## Requirements

### Requirement: Collection-only sources and durable origins
The system SHALL support zero or more unique active Collection sources per Shelf and SHALL NOT establish live Shelf-to-Shelf source links. It SHALL retain historical acceptance origins independently of current matches and detached/deleted Collections. Attaching sources SHALL require an explicit action and SHALL NOT add members automatically.

#### Scenario: Sources overlap
- **WHEN** a story is accepted while matching two linked Collections
- **THEN** it has one membership with both acceptance origins available for explanation

#### Scenario: Matching sources change after acceptance
- **WHEN** a member stops matching its acceptance source and starts matching another source
- **THEN** the original acceptance attribution remains and current matching sources are evaluated independently

### Requirement: Nonmutating update evaluation
The system SHALL evaluate the deduplicated union of active source results without mutating Shelf membership, internal manual retention, exclusions, or origins. It SHALL propose source matches absent from both membership and exclusions as additions, and resolvable source-derived members without manual retention matching no active source as removals. Broken flags SHALL affect neither set. A Shelf with no sources SHALL produce neither set.

#### Scenario: Newly matching story is reviewed
- **WHEN** a linked Collection matches an unexcluded story absent from the Shelf
- **THEN** it appears as an addition with source names and remains absent until approval

#### Scenario: One of several sources still matches
- **WHEN** a source-derived member stops matching one source but still matches another
- **THEN** it is not proposed for removal

#### Scenario: No source still matches
- **WHEN** a resolvable source-derived member without manual retention stops matching every active source
- **THEN** it is proposed for removal but remains on the Shelf until approval

#### Scenario: Manually retained, unavailable, or broken story is reviewed
- **WHEN** a story is manually retained, unavailable, or merely has its broken flag changed
- **THEN** manual retention prevents source removal, unavailability is a separate warning, and the flag alone produces no membership proposal, respectively

#### Scenario: Valid review has no candidates
- **WHEN** all sources evaluate successfully and both proposal sets are empty
- **THEN** the review displays Up to date

### Requirement: Conservative incomplete reviews
The system SHALL report invalid, unavailable, or failed Collection sources as needing attention, SHALL NOT substitute empty or unfiltered results, and SHALL suppress all source-based removal proposals while any active source result is unknown. It SHALL permit additions from healthy sources with an explicit incomplete-review warning rather than claiming Up to date.

#### Scenario: One source loses a referenced tag
- **WHEN** one source is invalid while another has known new matches
- **THEN** healthy additions remain reviewable, source-based removals are suspended, and the invalid source is identified

#### Scenario: Source is repaired
- **WHEN** every active source can again be evaluated
- **THEN** a fresh review can compute both additions and removals using the complete current union

### Requirement: Explicit atomic review decisions
The system SHALL provide per-item and bulk Add, Exclude, Not now, Remove and exclude, and Keep decisions as applicable, followed by a deliberate Apply approved changes action. No addition/removal SHALL be preapproved by default. It SHALL validate all decisions before atomically applying membership and persistent decisions. Keeping SHALL record manual retention; accepted additions SHALL retain source origins; deferrals SHALL write nothing.

#### Scenario: Mixed decisions are applied
- **WHEN** a valid review approves additions, removals, exclusions, keeps, and deferrals
- **THEN** all approved persistent effects commit together and deferred items remain eligible without stored decisions

#### Scenario: Empty or deferred-only review is applied
- **WHEN** no persistent decisions are approved
- **THEN** application succeeds as a no-op without changing Shelf state or revision

#### Scenario: Conflicting or unauthorized decision is supplied
- **WHEN** a request repeats a UUID with conflicting actions or targets an item outside its applicable preview candidates
- **THEN** the complete batch is rejected without partial changes

### Requirement: Snapshot integrity
The system SHALL bind seeding, updates, and detach/delete plans to the previewed Shelf/source/decision state and relevant current query results and availability. It SHALL revalidate those inputs within the application transaction before writing and reject stale, expired, or unknown reviews with an action to refresh. A Shelf revision alone SHALL NOT stand in for source-result validation.

#### Scenario: Source matches change after preview
- **WHEN** an import, metadata/tag edit, source-query edit, or relevant availability change alters reviewed inputs before apply
- **THEN** the old decisions cannot commit and the user is directed to refresh the review

#### Scenario: Concurrent Shelf decision changes
- **WHEN** membership, manual retention, exclusions, or source links change after preview
- **THEN** applying the old token is rejected without overwriting the newer decision

#### Scenario: Only a broken flag changes
- **WHEN** a story's flag changes while the membership/source snapshot remains equivalent
- **THEN** that flag change alone does not invalidate a membership review or change its candidates

### Requirement: Reviewed source detachment and Collection deletion
The system SHALL preview affected Shelf members before source detachment or linked Collection deletion and SHALL offer Keep all or Review affected stories. Retained stories losing source coverage SHALL become manually retained through the explicit confirmation; removals SHALL require per-item or bulk approval and create exclusions. Stories still covered by another healthy source SHALL remain source-derived. Uncertain source coverage SHALL prevent inferred removals. Deletion across multiple linked Shelves SHALL apply the reviewed plans and Collection deletion atomically.

#### Scenario: User keeps all while detaching
- **WHEN** the user confirms Keep all on a fresh detach plan
- **THEN** the link is retired, stories losing coverage acquire manual retention, and no approved member is removed

#### Scenario: Last source is detached
- **WHEN** the user confirms a reviewed last-source detachment
- **THEN** explicitly removed members are excluded and all remaining members acquire manual retention on the resulting manual Shelf

#### Scenario: Detachment is cancelled or deferred
- **WHEN** the user cancels or leaves an affected-member decision deferred
- **THEN** the detachment is not applied and membership remains unchanged

#### Scenario: Linked Collection is deleted
- **WHEN** the user approves fresh detach plans for every linked Shelf and confirms deletion
- **THEN** all source-link and approved membership decisions commit together with deletion, while historical acceptance origins remain explainable

#### Scenario: Remaining source is invalid
- **WHEN** detachment cannot determine remaining coverage
- **THEN** Keep all records conservative manual retention for affected members and coverage-based removal choices remain unavailable

### Requirement: On-demand review presentation
The system SHALL expose separate additions and removals, story previews, source explanations, warnings, explicit Keep decisions, exclusion management, and an apply summary without a protection toggle. It SHALL distinguish approved member counts from suggestions and SHALL NOT display unevaluated or stale pending counts as current. Initial delivery SHALL evaluate reviews on demand without background membership mutation.

#### Scenario: User opens a Shelf without evaluation
- **WHEN** the user browses a Shelf before reviewing current source results
- **THEN** its approved contents and Review updates action are shown without an invented pending-change count

#### Scenario: Review contains many candidates
- **WHEN** a source union produces more candidates than fit in the viewport
- **THEN** the review remains operable with bounded rendering, keyboard access, explicit bulk decisions, and counts representing the full candidate set
