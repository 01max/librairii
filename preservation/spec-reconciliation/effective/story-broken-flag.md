# Story Broken Flag

## Purpose

Define independent manual broken-story flags, accessible controls, feedback, and migration.

## Requirements

### Requirement: Independent manual broken flag
The system SHALL persist one boolean broken flag on each story independently of tag definitions, tag assignments, official metadata, and archive validation. New stories SHALL default to unflagged. Flag changes SHALL preserve managed archive bytes, other story metadata, and tags. Clearing the flag SHALL make no claim of verified playback.

#### Scenario: New story is imported
- **WHEN** a previously absent story is imported
- **THEN** its manual broken flag is false

#### Scenario: Existing flagged story is encountered again
- **WHEN** import or metadata refresh encounters an existing flagged story
- **THEN** the flag remains true

#### Scenario: Archive verification succeeds
- **WHEN** archive verification succeeds for a flagged story
- **THEN** the manual broken flag remains set independently of archive validation

#### Scenario: User clears a flag on an invalid archive
- **WHEN** the user clears the broken flag on a story whose archive is invalid or missing
- **THEN** the flag is cleared and archive validation retains its existing state

### Requirement: Explicit atomic flag mutation
The system SHALL provide a dedicated operation to set one or several active stories to an explicit broken state, SHALL deduplicate story IDs, SHALL validate application mutation readiness and every target before committing, and SHALL report requested and changed counts. A successful result SHALL include the prior flags for changed stories so Undo can restore them. An invalid target or failed operation SHALL leave the entire batch unchanged.

#### Scenario: Story is marked broken
- **WHEN** the user marks an unflagged story as broken
- **THEN** its flag becomes true and one changed story is reported

#### Scenario: Mixed selection is marked in bulk
- **WHEN** the user marks a selection containing flagged and unflagged stories as broken
- **THEN** every distinct selected story is flagged and the changed count includes only previously unflagged stories

#### Scenario: Flags are cleared in bulk
- **WHEN** the user clears flags for several selected stories
- **THEN** each target becomes unflagged and only actual changes contribute to the changed count

#### Scenario: Duplicate IDs or already-matching flags are supplied
- **WHEN** an otherwise valid request repeats a story ID or targets an already-matching state
- **THEN** each distinct story is counted once and already-matching flags are successful no-ops

#### Scenario: Batch contains an unavailable story
- **WHEN** a requested story no longer exists or is not active
- **THEN** the whole mutation is rejected with actionable feedback and no target flag changes

#### Scenario: Application is not ready for mutation
- **WHEN** a flag action is requested while application mutation readiness prevents library writes
- **THEN** the operation is rejected without changes and the interface explains why it is unavailable

### Requirement: Direct flag controls throughout story views
The system SHALL provide an independent Broken switch in the selected-story panel and details, with bulk selection and flagged counts. The switch SHALL apply to the selected available story or frozen bulk targets. Story cards in List, Small covers and Large covers, including Collection and Shelf previews, SHALL retain warning-only presentation without extra flag action buttons. Details SHALL place the switch outside tag editing. Available Shelf members SHALL expose details and a bulk switch through selection.

#### Scenario: User flags a selected story
- **WHEN** the user toggles Broken in the selected-story panel or details while using List, Small covers, Large covers or a Shelf view
- **THEN** only the explicit single-story or selected bulk targets are changed independently of tag editing

#### Scenario: Bulk switch is used during multi-selection
- **WHEN** multiple stories are selected and the user toggles the selection's Broken switch
- **THEN** the explicit selected targets are changed atomically
- **AND** flagging alone preserves visible results, membership counts, and selection

#### Scenario: User navigates without hover
- **WHEN** the user selects a story and reaches the panel or details with the keyboard or touch
- **THEN** the Broken switch has a story-specific accessible name, current state and visible keyboard focus without requiring hover

#### Scenario: User inspects a mixed selection
- **WHEN** the selected-story panel represents flagged and unflagged stories
- **THEN** it shows the selection and flagged counts and explicit actions to mark all selected stories or clear their flags

#### Scenario: User changes a flag from details
- **WHEN** the user opens a story's details and changes its broken flag
- **THEN** the action appears outside tag editing and the story and details remain available under unchanged non-Broken criteria

### Requirement: Distinct story warning presentation
The system SHALL present flagged stories with a consistent warning icon, Broken text, and muted artwork where covers are shown. Warning presentation SHALL use an accessible fixed system style independent of tag colors. Broken flag state SHALL appear separately from tags in the selected-story panel and details. Unflagged stories SHALL offer Mark as broken without being described as verified working.

#### Scenario: Flagged story is visible
- **WHEN** a flagged story appears in a collection or preview
- **THEN** its warning is identifiable through icon and text without depending on color and its cover is subtly muted when present

#### Scenario: User inspects tags on a flagged story
- **WHEN** the user views the selected-story panel or details for a flagged story
- **THEN** the warning is separate from tag chips and does not depend on loading the tag catalog

### Requirement: Reversible feedback and consistent reconciliation
After a successful flag mutation, the system SHALL announce nonblocking feedback, refresh every visible instance and flag-dependent export feedback, and preserve browsing selection, counts, pagination, focus, and approved membership when no other criteria changed. Feedback SHALL offer Undo for the latest successful action that changed flags. Undo SHALL atomically restore only changed stories to their recorded prior values after checking target availability and expected current flag states; a stale or unavailable batch SHALL fail without partial restoration. A failed mutation SHALL preserve existing flags and report an actionable error. Controls SHALL prevent conflicting repeated flag mutations while a flag action is pending.

#### Scenario: Story is marked or cleared in a view
- **WHEN** the user marks or clears a story's broken flag without another membership change
- **THEN** its card remains in place, selected state and counts remain stable, and feedback announces an actual flag change with Undo; closing details clears its local notice

#### Scenario: Story appears in multiple group rows
- **WHEN** a visible story's flag changes
- **THEN** every visible copy, selected-story panel, and open details reflect the current warning without flag-driven disappearance

#### Scenario: User undoes a bulk action
- **WHEN** the user activates Undo after a successful action on a mixed selection and all changed targets still have their expected resulting flags
- **THEN** only the originally changed stories regain their prior flags and stories originally matching the target state remain unchanged

#### Scenario: Undo target changed or became unavailable
- **WHEN** a restore target is unavailable or its current flag differs from the expected resulting flag
- **THEN** Undo rejects the whole restore batch and explains the conflict without altering any flags

#### Scenario: Flag write fails or changes nothing
- **WHEN** persistence fails or successfully changes zero stories
- **THEN** failure retains the original displayed state with actionable feedback, or success reports zero changes, respectively
- **AND** neither result offers an Undo action for that attempt

### Requirement: Lossless migration from the built-in Broken tag
The system SHALL migrate existing assignments of the canonical built-in Broken tag into the dedicated story flag, SHALL preserve the retired definition identity for supported legacy query translation, and SHALL remove the built-in definition and its assignments after preserving their state. Migration SHALL preserve unrelated tags, colors, assignments, stories, and archives. Subsequent starts SHALL neither recreate the built-in tag nor reset migrated flags. The retired color customization SHALL be replaced by the fixed warning presentation.

#### Scenario: Existing library is upgraded
- **WHEN** a library with built-in Broken assignments is migrated
- **THEN** exactly the previously assigned stories become flagged, other stories remain unflagged, and the built-in definition and assignments are removed

#### Scenario: Upgraded library restarts
- **WHEN** the application restarts after migration
- **THEN** flags persist and the built-in Broken definition is not reseeded

#### Scenario: Fresh library is initialized
- **WHEN** a new library is created
- **THEN** it supports independent broken flags without seeding a built-in Broken tag

#### Scenario: Tag ID is reused after migration
- **WHEN** a new user tag receives the retired definition's numeric ID
- **THEN** supported legacy decoding recognizes and recursively removes retired built-in restrictions without losing unrelated predicates or AND/OR alternatives, while current query versions interpret the new tag reference normally
