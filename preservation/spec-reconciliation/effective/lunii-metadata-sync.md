# Lunii Metadata Sync

## Purpose

Define explicit official catalog refresh, UUID-based metadata matching, protected derived facets, and reliable offline access to the last valid metadata snapshot.

## Requirements

### Requirement: Explicit official catalog refresh
The system SHALL refresh official metadata only when explicitly requested by the user; no scheduled catalog refresh is provided, using an isolated client with bounded timeouts and no personal Lunii credentials.

#### Scenario: User refreshes successfully
- **WHEN** the user requests a refresh and the guest-token and pack-catalog responses are valid
- **THEN** the system activates a timestamped normalized catalog snapshot and reports the refresh time and matched-story count

#### Scenario: Upstream request times out
- **WHEN** a guest-token or catalog request exceeds its timeout
- **THEN** the refresh ends with a visible failure state without blocking local collection use

### Requirement: Match official metadata by story UUID
The system SHALL associate normalized official metadata with local stories by complete UUID and SHALL expose the provenance and freshness of the displayed official data.

#### Scenario: Imported story has a catalog match
- **WHEN** an active catalog snapshot contains the complete UUID of a local story
- **THEN** the story displays the available localized title, description, author, artwork, and supported metadata fields with official provenance

#### Scenario: Imported story has no catalog match
- **WHEN** no active official row matches a local story UUID
- **THEN** the story retains its embedded or fallback display metadata and remains fully taggable and exportable

### Requirement: Last-known-good offline behavior
The system SHALL validate a downloaded catalog before activation and SHALL retain the last known good snapshot when a later refresh fails or returns invalid data.

#### Scenario: Refresh payload is invalid
- **WHEN** a refresh downloads a response that fails schema or integrity validation
- **THEN** the system rejects that snapshot, keeps the prior active snapshot, and reports that its displayed data may be stale

#### Scenario: No catalog has ever been downloaded
- **WHEN** the application is offline before its first successful refresh
- **THEN** the library operates from embedded and fallback metadata and clearly reports that official metadata is unavailable

### Requirement: Derived system facets
The system SHALL project configured system facets from sufficiently precise official metadata and SHALL not guess a derived value when the source data is absent or ambiguous.

#### Scenario: Age metadata maps to a configured band
- **WHEN** a matched official story has age information that satisfies a configured age-band mapping such as `3-5`
- **THEN** the system assigns the corresponding read-only derived age value to that story

#### Scenario: Age metadata is insufficient
- **WHEN** a matched story lacks sufficient age information for an unambiguous configured band
- **THEN** the system leaves the derived age facet unassigned and exposes no guessed value

#### Scenario: Catalog refresh changes a derived value
- **WHEN** a later valid snapshot changes source metadata used by a system-facet mapping
- **THEN** the system rebuilds the affected derived assignment without changing any user-defined tag, the independent manual broken flag on any story, or any manual assignment

#### Scenario: Flagged story gains official metadata
- **WHEN** an unflagged or flagged story receives new official metadata, artwork, or rebuilt derived facets
- **THEN** its manual broken flag is unchanged and refresh neither creates nor restores a built-in Broken tag

### Requirement: Metadata-only network scope
The system SHALL limit Lunii synchronization to catalog metadata and referenced artwork and SHALL not download story archives, firmware, device keys, or user-account data.

#### Scenario: Refresh completes
- **WHEN** an official metadata refresh succeeds
- **THEN** all new network-derived local data consists only of catalog responses and artwork required for library display
