# Library Browsing

## Purpose

Define the desktop story collection experience, including story details, name search, filters, navigation state, accessible controls, and visible operation feedback.

## Requirements

### Requirement: Collection-first desktop interface
The system SHALL open to a desktop-oriented collection surface that shows story artwork or a fallback, title, author when available, UUID, compatibility status, tag values, selection state, result count, and access to story details.

#### Scenario: Collection contains stories
- **WHEN** the user opens the application with imported stories
- **THEN** the collection presents paginated or incrementally rendered story cards or rows with the required summary information and a usable default sort

#### Scenario: User opens story details
- **WHEN** the user activates a story from the collection
- **THEN** the interface shows its complete available metadata, archive details, metadata provenance, tags, and relevant actions without losing the collection context

#### Scenario: Collection is empty
- **WHEN** the user opens a library with no active stories
- **THEN** the system explains the collection purpose and provides a prominent import action rather than showing an empty results grid

### Requirement: Story name substring search
The system SHALL provide a visible story-name search that applies a safely escaped, case- and accent-insensitive substring match to each story's resolved display title/name and combines with all active filters.

#### Scenario: Name contains the search text
- **WHEN** the user enters `dragon` and a story's resolved name is `Le Dragon des montagnes`
- **THEN** that story matches the name search even though the search text is not the complete name

#### Scenario: Name differs only by case or accents
- **WHEN** the user enters an unaccented or differently cased substring of a story name
- **THEN** the story matches after the same documented case and accent normalization is applied to both values

#### Scenario: Search text contains wildcard characters
- **WHEN** the user enters characters such as `%` or `_`
- **THEN** the system treats them as literal search text rather than executable SQL wildcard syntax

### Requirement: Search and metadata filtering
The system SHALL let the user combine story-name search with official metadata fields, archive compatibility/import state, and available derived or user tag values, and SHALL display the active search, filters, and result count.

#### Scenario: User combines name and tag criteria
- **WHEN** the user searches for a name substring and selects a tag value
- **THEN** the result set contains only stories whose names contain the substring and whose tags satisfy the selected value

#### Scenario: User removes an active filter
- **WHEN** the user removes one filter from the active-filter summary
- **THEN** the result set is recomputed from all remaining filters and focus stays in a predictable location

#### Scenario: User clears all filters
- **WHEN** the user activates clear-all
- **THEN** the name search and facet filters reset while the user's explicit sort preference remains in effect

#### Scenario: User searches inside a Shelf
- **WHEN** the user refines an approved Shelf with a name search or tag filter
- **THEN** only matching stored members appear in that view and neither approved membership nor source criteria change

### Requirement: Predictable tag filter composition
The system SHALL OR selected values within one choice tag definition, SHALL give boolean tags ignored/true/false filter states, and SHALL AND selections across different definitions and other filter groups.

#### Scenario: Multiple values in one definition
- **WHEN** the user selects `3-5` and `6-8` within the `age` definition
- **THEN** a story matches that definition when it has either selected value

#### Scenario: Values across definitions
- **WHEN** the user selects an `age` value and a `language` value
- **THEN** a story appears only when it matches at least one selected age value and at least one selected language value

#### Scenario: Boolean false state is composed with a choice
- **WHEN** the user filters for the false state of a user-defined boolean tag and selects an `age` value
- **THEN** a story appears only when it lacks that boolean tag assignment and matches the selected age value

### Requirement: Durable navigation and filter state
The system SHALL encode Library, named Collection, curated Shelf, or custom-query scope with unambiguous identity and durable non-Broken search/filter, sort, and page state so reload, back, forward, creation, and export retain the intended scope. Supported legacy URLs and history SHALL remove built-in Broken restrictions recursively, preserve unrelated query structure, and canonicalize to the current version. Legacy dynamic-Shelf identities SHALL map to Collections rather than new curated Shelves.

#### Scenario: User returns with back navigation
- **WHEN** the user changes scope or filters and navigates back
- **THEN** the prior scope, non-Broken criteria, sort, and page state are restored

#### Scenario: Collection or Shelf reloads
- **WHEN** a user reloads a Collection or filtered Shelf
- **THEN** the same group identity and applicable non-Broken view criteria are restored

#### Scenario: Supported legacy URL contains Broken restrictions
- **WHEN** an old URL or history entry contains hide/only visibility or a retired built-in Broken reference at any query depth
- **THEN** canonicalization removes those restrictions and preserves other predicates and AND/OR structure

#### Scenario: Current user tag reuses a retired identity
- **WHEN** a current-version query filters an ordinary user tag named broken or reusing the retired built-in tag ID
- **THEN** ordinary user-tag filtering is preserved without converting it into flag filtering

#### Scenario: Legacy Shelf and new Shelf IDs overlap
- **WHEN** an old dynamic-Shelf history entry and a new curated Shelf have the same numeric ID
- **THEN** the old entry resolves only to the migrated Collection

### Requirement: Visible operation and failure states
The system SHALL provide nonblocking progress, completion, cancellation, and actionable failure feedback for imports, metadata refreshes, removals, and exports.

#### Scenario: Long-running operation is active
- **WHEN** an import, metadata refresh, or export continues beyond immediate response time
- **THEN** the interface shows its operation type, progress or indeterminate state, and a safe cancel action when cancellation is supported

#### Scenario: Operation partially fails
- **WHEN** a multi-story operation completes with both successes and failures
- **THEN** the interface summarizes both groups and makes per-item failure reasons inspectable

### Requirement: Canonical archive shelves UI parity
The system SHALL reproduce the product UI inside `.app` from `testdata/ui-prototypes/05-archive-shelves.html` using the canonical checksum recorded by the prototype verification contract, with one-to-one visual, responsive, content-hierarchy, state, and interaction parity; the prototype-gallery `.back` link SHALL be excluded. The accepted Collections/Shelves navigation, curation/source/review surfaces, always-visible broken warnings and controls, export eligibility feedback, and removal of broken visibility controls SHALL be incorporated into the canonical prototype, its checksum contract, fixtures, and affected visual baselines together; the stored checksum SHALL then identify the revised prototype. Unrelated layout and interaction hierarchy SHALL retain parity.

#### Scenario: Desktop shell is rendered
- **WHEN** the deterministic parity fixture is rendered at a viewport wider than 1180px
- **THEN** the labeled action rail, Collections/Shelves/refinement sidebar, search/import bar and official Lunii sync panel, shelf headings and six-cover rows, shelf ledges, selected story, and fixed detail drawer with readable tags at every window size match the canonical prototype

#### Scenario: Intermediate responsive states are rendered
- **WHEN** the parity fixture is rendered at the prototype's 1180px or 850px breakpoints
- **THEN** visible cover counts, hidden drawer tags, hidden refinement sidebar, rail width, main layout, and drawer offset match the corresponding canonical CSS behavior

#### Scenario: Compact responsive state is rendered
- **WHEN** the parity fixture is rendered at the prototype's 560px breakpoint
- **THEN** the sticky top rail with all four labeled actions, visible sync action, main padding, three visible covers, compact drawer, and hidden secondary detail/actions match the canonical CSS behavior

#### Scenario: Canonical interaction states are exercised
- **WHEN** the user hovers or selects a story, switches a Collection or Shelf, reviews source proposals, changes a facet, searches, sorts, syncs, imports, manages tags, opens details, or adds a story to export
- **THEN** the available control, placement, visible state change, and resulting domain action follow the prototype's hierarchy and interaction affordance

#### Scenario: Visual deviation is proposed
- **WHEN** implementation would change the canonical layout, tokens, hierarchy, responsive behavior, or interaction placement
- **THEN** changes outside the accepted Collections/Shelves and broken export-policy scope remain blocked until the user explicitly approves an updated prototype and its checksum, specifications, and baselines are updated; changes within that scope SHALL update the prototype, checksum contract, specifications, and affected baselines together

### Requirement: Accessible implementation without visual drift
The system SHALL add keyboard navigation, semantic labels, sufficient contrast, reduced-motion support, visible focus, and tag identification that does not rely on color alone without changing canonical visible layout or interaction hierarchy.

#### Scenario: User navigates without a pointer
- **WHEN** the user operates collection, filters, details, dialogs, and tag controls using a keyboard
- **THEN** every interactive action is reachable in a logical order with visible focus and no keyboard trap

#### Scenario: Tag colors are indistinguishable
- **WHEN** tag chips are viewed without reliable color perception
- **THEN** each chip remains identifiable through its definition and value text

#### Scenario: Nonvisual semantics are added
- **WHEN** implementation adds ARIA, semantic elements, test hooks, or equivalent nonvisual structure
- **THEN** assistive behavior improves while screenshot parity with the canonical fixture remains within the approved tolerance

### Requirement: Clear family browsing controls

The system SHALL offer a prominent first-sync action that explains the official Lunii API enriches matching stories with descriptions and cover images. The last successful sync SHALL remain visible in local date and time during unrelated operations and failed refreshes. Library, Tags, Import and Export rail shortcuts SHALL have visible labels and working handlers.

#### Scenario: Story artwork is available
- **WHEN** a shelf story has validated artwork
- **THEN** its image-only button follows the image's intrinsic ratio, without title overlays or author captions, and the story title appears below the button

#### Scenario: No successful sync yet
- **WHEN** the official catalog has never been synced
- **THEN** the main view prominently offers Sync with Lunii with an explanation of contextual descriptions and cover images and reports the last successful sync as Never

### Requirement: Viewport scrolling revision (2026-10-15)

The application grid SHALL fit the window height. Story content and long sidebar filter lists SHALL scroll independently without moving the navigation or scrolling the document. All four navigation actions SHALL remain reachable in short windows. The decorative ML profile badge SHALL be removed because it has no account or navigation function.

#### Scenario: Short window contains overflowing stories and filters
- **WHEN** a short application window contains overflowing story content and sidebar filter lists
- **THEN** the application grid fits the window height, story content and filter lists scroll independently, navigation stays fixed with all four actions reachable, and the document does not scroll

### Requirement: Popup and tag usability revision (2026-10-15)

Application popups SHALL dismiss on backdrop clicks. Story details SHALL show the selected story's tags and a visible removal button. The sidebar SHALL distinguish Collections from curated Shelves without offering built-in Broken visibility or membership filters. Flagged stories SHALL remain visible and counted under other active criteria, with an accessible badge and muted artwork. The Lunii sync panel SHALL use a contrasting treatment, the top-right import button SHALL be removed, age tags SHALL default to green, and every remaining tag SHALL permit user color changes, including protected derived tags. The dedicated broken flag SHALL use fixed warning styling outside tag color customization. Protected identity and assignment rules SHALL remain intact, and colors SHALL survive seeding and sync.

#### Scenario: All stories after saving a search
- **WHEN** a user saves a filtered search as a Collection and returns to All stories
- **THEN** the search and tag filters clear, the complete library appears above group previews, and reload preserves the reset

#### Scenario: Story tags and small shelves remain readable
- **WHEN** the window narrows or a shelf contains fewer cards than the row capacity
- **THEN** all available preview cards up to the row capacity remain visible, selected-story tags and detail controls remain available, and choice-tag values appear in story details

#### Scenario: Long story opens and tag filters collapse
- **WHEN** a long story is opened or reopened
- **THEN** its details begin at scroll position zero
- **AND** each tag-filter heading supports mouse and keyboard collapse without clearing selected values

#### Scenario: Edit story tags from details
- **WHEN** the user opens story details
- **THEN** Edit tags is available inside that popup and absent from the collection index
- **AND** closing the tag editor returns to refreshed details when the story remains in the current result

#### Scenario: User distinguishes flagging from tag editing
- **WHEN** the user browses stories or opens details
- **THEN** dedicated broken-flag controls are available independently of Edit tags, and the tag manager, assignment editor, and tag chips do not expose the retired built-in Broken definition

### Requirement: Distinct Library Collection and Shelf navigation
The system SHALL distinguish All stories, dynamic Collections, and curated Shelves in navigation and action labels. Collection actions SHALL manage saved criteria; Shelf actions SHALL manage approved membership and Collection sources. A source picker SHALL explain that matching any selected Collection is sufficient. The interface SHALL distinguish approved member counts, broken/unavailable indicators, and reviewed suggestions without implying automatic updates. All new copy and accessible labels SHALL be available in English and French.

#### Scenario: User browses both group types
- **WHEN** a library contains Collections and curated Shelves
- **THEN** separate labeled sections expose the correct actions and a Shelf cannot accidentally replace its membership with current search results

#### Scenario: User chooses export from a refined Shelf
- **WHEN** a Shelf view has temporary filters
- **THEN** whole-Shelf, current-result, and selection export actions identify their different scopes clearly

#### Scenario: User operates review without a pointer
- **WHEN** the user opens source management or update review with a keyboard
- **THEN** decisions, bulk controls, apply/cancel, error feedback, and focus return are usable without relying on color or pointer hover

### Requirement: Broken state does not filter browsing or source matching
The system SHALL show and count broken stories in Library, Collection, and Shelf views under the same non-Broken criteria as unflagged stories. It SHALL remove built-in Broken filtering from supported saved definitions, URLs, and new query contracts while preserving genuine user-tag filtering. Marking or clearing a flag SHALL NOT change matching, membership, source-review candidates, pagination, or selection by itself. Current warnings, direct actions, and Undo SHALL remain available.

#### Scenario: Story is flagged while selected
- **WHEN** a user marks a visible selected story as broken
- **THEN** it remains visible and selected with unchanged result/member counts, its warning updates everywhere, and Undo remains available

#### Scenario: Broken story belongs to several groups
- **WHEN** the flag changes for a story appearing in All stories, multiple Collections, and Shelves
- **THEN** all instances show the flag consistently and retain their existing non-Broken membership

#### Scenario: New request tries to restore built-in flag filtering
- **WHEN** a current-version request supplies a dedicated hide/only or flag-membership restriction
- **THEN** the query contract rejects the unsupported restriction rather than silently filtering flagged stories
