# Local Story Catalog

## Purpose

Define application-managed storage and local story archive custody, including safe import, duplicate handling, metadata fallback, and recoverable removal.

## Requirements

### Requirement: Application-managed local storage
The system SHALL keep its database, imported archives, cache, staging files, trash, and logs beneath the configured library root and SHALL refuse mutating operations when that root is unavailable or not writable. New production installations SHALL require user confirmation of a library location before initializing a collection. Existing identifiable libraries at the prior platform-specific application-data root SHALL be adopted in place when no settings profile exists. A saved library root SHALL not be silently initialized as an empty replacement when the expected library is unavailable. Application settings and relocation recovery state SHALL be stored separately from the movable library. An explicit `LIBRAIRII_DATA_ROOT` development or test override SHALL preserve isolated initialization without production onboarding or profile access.

#### Scenario: First launch initializes storage
- **WHEN** a new production installation has no existing library and the user confirms a writable library root in first-launch setup
- **THEN** it creates the required storage areas at that root, persists the selected location, and opens an empty library

#### Scenario: First launch awaits confirmation
- **WHEN** a new production installation starts and the user has not confirmed a library root
- **THEN** it presents setup, leaves library mutations disabled, and does not initialize a collection at the suggested default

#### Scenario: Existing library is adopted
- **WHEN** an installation without a settings profile starts with an identifiable existing library at the prior platform-specific application-data root
- **THEN** it preserves and opens that library in place and records its location without requiring folder selection

#### Scenario: Storage is not writable
- **WHEN** the application starts and cannot safely write to its configured library root
- **THEN** it shows a recovery-oriented error and does not attempt an import, removal, metadata refresh, or export

#### Scenario: Configured library is missing
- **WHEN** a configured library root or its expected database is unavailable
- **THEN** the system offers recovery for that library without silently creating an empty collection

#### Scenario: Explicit development root initializes storage
- **WHEN** the application starts with a writable uninitialized `LIBRAIRII_DATA_ROOT` override
- **THEN** it initializes an isolated library there without first-launch folder selection and without reading or changing production preferences

### Requirement: Supported archive import
The system SHALL accept Lunii.QT-compatible `.plain.pk`, `.v1.pk`, `.v2.pk`, `.pk`, `.zip`, and `.7z` candidates, inspect their actual container structure, and create a story record only when it can identify a valid story UUID and supported format.

#### Scenario: Valid archive is imported
- **WHEN** the user imports a candidate whose structure, required entries, UUID, and safety limits are valid
- **THEN** the system records the detected format and UUID and presents the story in the collection

#### Scenario: Extension does not match content
- **WHEN** the user imports a file with a supported extension whose container structure is not a supported story format
- **THEN** the system rejects it with a format-validation reason and creates no story or archive record

#### Scenario: Archive exceeds safety limits
- **WHEN** a candidate contains unsafe paths or exceeds configured entry-count, expanded-size, compression-ratio, or metadata limits
- **THEN** the system rejects it without extracting entries outside its unique staging area

### Requirement: Immutable and atomic archive custody
The system SHALL calculate a SHA-256 digest, copy a valid input into content-addressed managed storage without modifying the source file, and publish its database record only after the managed copy is complete.

#### Scenario: Import succeeds atomically
- **WHEN** validation and the managed copy complete successfully
- **THEN** the stored bytes have the recorded checksum and the story becomes visible only after both the archive and database record are durable

#### Scenario: Import fails or is cancelled
- **WHEN** an import fails or is cancelled before publication
- **THEN** the system removes its incomplete staging bytes, leaves the source unchanged, and exposes no partial story

### Requirement: Deterministic duplicate handling
The system SHALL make an exact archive checksum idempotent and SHALL not silently replace an existing story when a different archive claims the same UUID.

#### Scenario: Exact archive is imported again
- **WHEN** the user imports bytes whose SHA-256 already belongs to a managed archive
- **THEN** the system keeps one managed copy and directs the user to the existing story

#### Scenario: UUID conflicts with different bytes
- **WHEN** a new valid archive has an existing story UUID but a different SHA-256
- **THEN** the system reports a UUID conflict and leaves the existing archive and story unchanged

### Requirement: Embedded metadata fallback
The system SHALL capture permitted embedded title, description, artwork, and format details when available and SHALL still make a valid story browsable when optional metadata is absent.

#### Scenario: Embedded metadata is present
- **WHEN** a valid archive contains supported embedded metadata and artwork
- **THEN** the system associates the validated fields and artwork with the story for offline display

#### Scenario: Optional metadata is absent
- **WHEN** a valid archive has no supported embedded description or artwork
- **THEN** the system displays a UUID-based fallback and artwork placeholder rather than failing the import

### Requirement: Recoverable story removal
The system SHALL require confirmation before removing a story from the active collection and SHALL move its managed archive to application trash before deleting the active database associations.

#### Scenario: User confirms removal
- **WHEN** the user confirms removal of a story
- **THEN** the story disappears from the active collection, its user tag assignments are removed, and its archive bytes remain in application trash for recovery or later purge

#### Scenario: User cancels removal
- **WHEN** the user cancels the confirmation
- **THEN** the story, archive, and assignments remain unchanged
