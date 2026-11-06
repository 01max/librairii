# Lunii.QT Export

## Purpose

Define exports of selected stories, current results, dynamic Collections, and curated Shelves into Lunii.QT-compatible output, with eligibility checks, atomic file handling, and progress reporting.

## Requirements

### Requirement: Explicit export scope
The system SHALL let the user prepare an export from explicit story selections, the complete current name-search/filter result, one Collection or a deduplicated union of Collections, or one curated Shelf or a deduplicated union of curated Shelves. It SHALL snapshot the intended story UUID set before copying, then apply export eligibility without altering Collection definitions or approved Shelf membership. Whole-Shelf export SHALL use all approved members independently of view-only refinements; filtered-result and selection export SHALL use their explicitly indicated scopes.

#### Scenario: Selected stories are exported
- **WHEN** one or more stories are explicitly selected and the user chooses export selection
- **THEN** preflight contains exactly that intended set regardless of later view-filter changes and applies the same broken-story exclusion as every other export mode

#### Scenario: Filtered result is exported
- **WHEN** the user chooses export current result
- **THEN** preflight contains every matching story across all pages within the active Library, Collection, or Shelf scope

#### Scenario: One Collection is exported
- **WHEN** the user chooses export for a valid Collection
- **THEN** preflight reevaluates its non-Broken query criteria and reports every current match before eligibility checks

#### Scenario: Multiple Collections are exported
- **WHEN** the user selects two or more valid Collections for one export
- **THEN** preflight resolves their OR union, deduplicates UUIDs, and reports collapsed overlap

#### Scenario: Shelf is exported before updates are approved
- **WHEN** a Shelf has pending addition/removal proposals and the user chooses whole-Shelf export
- **THEN** the intended scope contains its currently approved UUID membership only, including approved members pending removal and excluding unapproved additions

#### Scenario: Multiple curated Shelves are exported
- **WHEN** the user chooses several curated Shelves for one export
- **THEN** the intended set is the deduplicated union of their approved UUID memberships

#### Scenario: Collection needs attention
- **WHEN** an export scope contains a Collection with an unresolved saved criterion
- **THEN** the system blocks that query export and directs the user to repair it instead of broadening its membership

#### Scenario: Shelf source needs attention
- **WHEN** an approved Shelf is exported while one of its source Collections is invalid
- **THEN** the Shelf's stored membership remains exportable under ordinary per-story eligibility without reevaluating the invalid source

#### Scenario: Resolved scope is empty
- **WHEN** the selected mode resolves to no intended members
- **THEN** the system creates no export operation and explains how to choose stories or another scope

### Requirement: Export preflight
The system SHALL present the intended member count, ready file count and bytes, destination, detected formats, broken-story exclusions, unavailable members, incompatible items, and destination filename conflicts before copying. Every skipped/conflicted member SHALL retain its UUID, title when available, and an inspectable reason. Skipped items SHALL NOT consume eligible bytes or reserve destination filenames.

#### Scenario: Preflight is clean
- **WHEN** every intended member is unflagged with a present validated archive and a nonconflicting output filename
- **THEN** export is enabled with the expected ready count and total bytes

#### Scenario: Preflight finds unavailable items
- **WHEN** a member UUID cannot resolve or its archive is missing, invalid, or incompatible with passthrough export
- **THEN** preflight identifies the member and its skipped/blocking reason before copying

#### Scenario: Shelf includes broken stories
- **WHEN** a 25-member Shelf contains 3 flagged members and all others are eligible
- **THEN** preflight shows 25 intended members, 22 ready files, and 3 skipped because they are marked broken

#### Scenario: Scope has no eligible members
- **WHEN** all intended members are broken or otherwise unavailable
- **THEN** preflight explains every exclusion and does not allow an empty export operation

### Requirement: Lunii.QT-compatible passthrough output
The system SHALL export each eligible archive byte-for-byte with a Lunii.QT-supported extension directly into the chosen destination folder and SHALL not add unrelated manifest files to that folder.

#### Scenario: Archive exports successfully
- **WHEN** an eligible managed archive is copied to a writable destination
- **THEN** the destination archive checksum equals the managed checksum and its filename remains suitable for selection or drag-and-drop into Lunii.QT

#### Scenario: Multiple eligible formats are exported
- **WHEN** the scope contains validated archives in different supported Lunii.QT formats
- **THEN** each is exported in its stored format without conversion or decryption

### Requirement: Atomic and non-overwriting export
The system SHALL copy through a destination-local temporary file, verify the resulting checksum, atomically publish the final filename, and SHALL not overwrite an existing destination file by default.

#### Scenario: Destination filename already exists
- **WHEN** an output filename already exists at preflight or publication time
- **THEN** the system leaves the existing file unchanged and records the story as a conflict or skipped item

#### Scenario: Copy fails midway
- **WHEN** a filesystem error or cancellation interrupts a copy
- **THEN** the system removes its incomplete temporary file, leaves any prior destination file unchanged, and records the item outcome

### Requirement: Export progress and final report
The system SHALL expose operation-level and per-story progress and SHALL retain a final report separating exported, skipped, failed, conflicted, and cancelled items.

#### Scenario: Export completes successfully
- **WHEN** all snapshotted archives are published
- **THEN** the report shows every story as exported and offers an action to reveal the destination in the desktop shell

#### Scenario: Export completes partially
- **WHEN** some stories export and others fail, conflict, or are skipped
- **THEN** successful files remain available and the final report gives an inspectable reason for every non-successful story

#### Scenario: User cancels an export
- **WHEN** the user cancels an active export
- **THEN** the operation stops after the current safe cancellation boundary, removes incomplete temporary data, preserves completed files, and reports the remaining items as cancelled

### Requirement: Universal broken-story export exclusion
The system SHALL skip every story marked broken in all export modes, including explicit selections, without removing it from views, Collection matches, or approved Shelf membership. The exclusion SHALL have a dedicated inspectable outcome and SHALL NOT be bypassed by old visibility settings or selection APIs.

#### Scenario: Flagged story is explicitly selected
- **WHEN** a user selects a flagged story alongside an eligible story
- **THEN** the eligible story can export and the flagged story is skipped with a marked-broken reason

#### Scenario: Flag is cleared before a new preview
- **WHEN** a user clears a story's flag and prepares a fresh export
- **THEN** the story is evaluated under ordinary archive/destination eligibility without an extra working-status requirement

### Requirement: Prepared and queued export eligibility validation
The system SHALL revalidate flag states for prepared scope members at export start and require a fresh preview if those states changed, without silently broadening the snapshot. It SHALL check current eligibility again before each queued ready item's copy starts and record newly flagged items as skipped. Previously skipped items SHALL NOT be silently promoted. An item whose copy has already begun SHALL complete under that eligibility boundary; later flagging SHALL NOT remove already copied or published destination files.

#### Scenario: Flag changes between preview and start
- **WHEN** an intended member's flag changes after export preparation and before start
- **THEN** the prepared export is rejected as stale with a refresh action before any copy begins

#### Scenario: Pending item becomes broken during export
- **WHEN** a ready item becomes flagged before its queued copy begins
- **THEN** it is recorded as skipped and no archive bytes are copied for that item

#### Scenario: Skipped item becomes unflagged during export
- **WHEN** a member skipped during preparation has its flag cleared after the operation begins
- **THEN** it remains skipped in that operation and can be included through a new export preview

#### Scenario: Published member is flagged
- **WHEN** the user flags a story whose copy has already begun or been published
- **THEN** the current copy is not retracted and subsequent items and future exports use their applicable eligibility checks

### Requirement: Stable historical export identities
The system SHALL preserve immutable operation snapshots and SHALL interpret historical shelf/shelves source types as legacy dynamic Collections. New Collection and curated-Shelf exports SHALL use unambiguous distinct discriminators and identifiers, enforced by updated persistence constraints and decoded correctly after restart. Unavailable approved UUIDs recorded as skipped SHALL NOT require a current numeric story ID; ready items SHALL retain complete validated copy scope.

#### Scenario: Historical dynamic export is viewed after upgrade
- **WHEN** history contains a pre-upgrade shelf export and a new curated Shelf reuses that numeric ID
- **THEN** history retains the old dynamic Collection identity/name snapshot and is not relinked to the curated Shelf

#### Scenario: Mixed history survives restart
- **WHEN** historical dynamic, new Collection, and new curated-Shelf exports are saved and the application restarts
- **THEN** each record retains its source meaning, intended membership, and per-item outcomes

#### Scenario: Unavailable Shelf UUID is part of a partial export
- **WHEN** approved membership contains an unresolved UUID alongside eligible stories
- **THEN** the operation can retain a skipped UUID/title/reason record without a live numeric ID while eligible stories retain full validated copy fields
