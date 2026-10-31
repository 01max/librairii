# Library Location

## Purpose

Define safe library folder selection, complete and verified relocation, exclusive storage access, deterministic interruption recovery, and explicit recovery of missing libraries.

## Requirements

### Requirement: Library folders are selected natively and validated
The system SHALL select library roots through a native folder picker and SHALL validate the resolved location before setup or relocation. A new target SHALL be absent or empty, writable, outside the configuration directory and its ancestors, and neither an ancestor nor descendant of the current root. Alias-equivalent paths SHALL be treated as the same root. The system SHALL reject unsafe symlink traversal, another existing library, unrelated destination content, and insufficient copy space when determinable. The selected folder itself SHALL be the library root.

#### Scenario: Selected destination is valid
- **WHEN** the user selects an empty writable folder with sufficient space, including a folder with spaces or accented characters
- **THEN** the application offers that exact resolved folder as the new library root

#### Scenario: Destination already contains files
- **WHEN** the selected destination contains another library or unrelated files
- **THEN** the application rejects it with an explanation and does not merge, overwrite, or move either collection

#### Scenario: Destination overlaps protected paths
- **WHEN** the selected target contains the configuration directory, is inside that directory, or is nested with the current library root
- **THEN** selection is rejected without copying or removing library data

#### Scenario: User selects the current folder through an alias
- **WHEN** the selected path resolves to the current library root
- **THEN** the application treats the selection as a no-op without moving files

#### Scenario: Picker is cancelled
- **WHEN** the user cancels the change-location picker
- **THEN** the current library location and files remain unchanged

#### Scenario: Destination changes after preview
- **WHEN** files appear at the prepared destination before the move begins or the source identity has changed
- **THEN** execution rejects the stale preparation without overwriting files or changing the configured root

### Requirement: Relocation preserves the complete library
The system SHALL move the complete selected library, including the database and recovery files, managed archives, catalog snapshots, artwork cache, staging, trash, logs, and additional existing root files. Story identities, archive bytes, tags, assignments, shelves, metadata state, and operation history SHALL be preserved. External original imports and export destinations SHALL remain untouched. The system SHALL show source, destination, and total bytes before the user starts the move and show progress until a terminal result.

#### Scenario: Library moves successfully
- **WHEN** the user starts a validated relocation and copying, verification, reopening, and location persistence succeed
- **THEN** the same collection is usable at the destination, settings display the new root, and recorded old library contents are removed after commitment

#### Scenario: User restarts after a successful move
- **WHEN** the application restarts after relocation
- **THEN** it opens the same library at the new root with its organization and history intact

#### Scenario: Move crosses volumes
- **WHEN** source and destination are on different writable local volumes
- **THEN** relocation preserves the same data and verification guarantees as a same-volume move

### Requirement: Relocation has exclusive access to library resources
The system SHALL block starting relocation while library operations or mutations are active, SHALL explain the busy state, and SHALL require the user to retry after activity finishes. During relocation it SHALL reject new library operations and drain existing resource users, including artwork and diagnostics, before copying a closed, consistent database and library. Services SHALL reopen against the destination as one coherent storage generation and stale prepared exports SHALL be invalidated.

#### Scenario: Import or export is running
- **WHEN** the user attempts to change location during an active import, export, or metadata sync
- **THEN** the system explains why the move cannot start and leaves the current operation and location unchanged

#### Scenario: Mutation races with relocation
- **WHEN** a new mutation and relocation attempt to begin concurrently
- **THEN** only one acquires library access and the losing request receives a controlled busy response without changing data

#### Scenario: Artwork request is in flight
- **WHEN** a cache-writing artwork request is active as relocation acquires exclusive access
- **THEN** copying waits until that request releases its resources and no artwork or log writes occur against the closed source during copying

#### Scenario: Collection resumes after relocation
- **WHEN** relocation commits the new root and resumes the collection
- **THEN** queries, imports, exports, removals, artwork, and diagnostics all use the new library and any pre-move export preparation cannot be executed

### Requirement: Relocation verifies data before committing authority
The system SHALL retain the source as authoritative until it verifies a complete copied manifest, checks file sizes and checksums and database integrity and identity, successfully opens the destination with mutations gated, and durably commits the new root in independent configuration. It SHALL use an exclusive move-owned workspace and SHALL never overwrite destination content. Cancellation SHALL be supported before commitment and SHALL be unavailable after commitment.

#### Scenario: Copying or verification fails
- **WHEN** a copy fails, the disk fills, or destination verification detects corruption before commitment
- **THEN** the saved location remains the source, original data is retained, and the source is reopened when available with a visible failure result

#### Scenario: Destination cannot be opened or settings cannot be saved
- **WHEN** destination reopening or durable root persistence fails before commitment
- **THEN** the system closes the attempted destination, preserves the source as authoritative, and does not remove source data

#### Scenario: User cancels during copying
- **WHEN** the user cancels before the root commit
- **THEN** copying stops, only verified move-owned temporary artifacts are cleaned, and the source remains the configured usable library

### Requirement: Interrupted relocation recovers deterministically
The system SHALL durably journal relocation outside source and destination and reconcile it before starting library workers. The atomic configured root SHALL determine authority. Before commitment the source SHALL remain authoritative; after commitment the destination SHALL remain authoritative. Cleanup SHALL be idempotent, limited to verified move-owned or recorded source files, and SHALL never delete newly introduced unrelated content. Inconsistent evidence or an unavailable authoritative root SHALL enter recovery while preserving remaining copies.

#### Scenario: Application exits before commitment
- **WHEN** the application restarts after interruption during copy, verification, or destination publication before the configured root changes
- **THEN** it retains the source as authoritative and cleans only proven move-owned destination artifacts without opening them as a second live library

#### Scenario: Application exits after commitment
- **WHEN** the application restarts after the configured root changes but before old-source cleanup completes
- **THEN** it opens the committed destination and resumes safe cleanup without reverting to the potentially stale source

#### Scenario: Source cleanup fails
- **WHEN** the destination has committed successfully but some old-source files cannot be removed safely
- **THEN** the destination remains authoritative, the UI reports the old location and pending cleanup, and later cleanup cannot delete unrecorded or changed files

#### Scenario: Destination disappears after commitment
- **WHEN** the committed destination is unavailable on restart while the old source still exists
- **THEN** the system enters missing-library recovery and does not automatically open the old source as a current library

### Requirement: Missing libraries require explicit recovery
The system SHALL show the saved root and offer retry or native selection of the same library when the configured root or expected database is unavailable. It SHALL validate the expected stable library identity before accepting a recovery location, SHALL preserve the UI-language setting, and SHALL not create a replacement empty library or silently switch to another library.

#### Scenario: External library drive is disconnected
- **WHEN** the application starts while its configured library drive is unavailable
- **THEN** recovery shows the configured path in the saved UI language and leaves library operations disabled without creating replacement directories

#### Scenario: User reconnects the drive
- **WHEN** the expected library becomes available and the user retries
- **THEN** the system validates and opens that library with its data intact

#### Scenario: User locates the same library after moving it externally
- **WHEN** the user selects a readable location containing the expected library identity and compatible database
- **THEN** the application persists that root and reopens the same library without relocating or deleting unrelated folders

#### Scenario: User selects a different library for recovery
- **WHEN** a selected recovery folder has a different identity or lacks the expected database
- **THEN** recovery rejects it without changing the saved location or initializing new data
