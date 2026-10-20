# Story Tagging

## Purpose

Define user-managed story tag definitions, allowed values, and assignments, including protected built-in tags and confirmation for destructive changes.

## Requirements

### Requirement: User-defined tag definition CRUD
The system SHALL let the user create, view, rename, recolor, and delete user-defined tag definitions with a stable normalized key, a display label, an accessible color, and either a boolean or choice kind.

#### Scenario: Valid choice definition is created
- **WHEN** the user supplies a unique key, nonblank label, valid color, and the choice kind
- **THEN** the system creates the definition and makes it available for allowed values, assignment, display, and filtering

#### Scenario: Valid boolean definition is created
- **WHEN** the user supplies a unique key, nonblank label, valid color, and the boolean kind
- **THEN** the system creates the definition with an assignable true/false state and no allowed values

#### Scenario: Definition key is not unique
- **WHEN** the user submits a key that normalizes to an existing user, built-in, or derived definition key
- **THEN** the system rejects the change with an inline validation error and preserves the submitted label and color for correction

#### Scenario: Definition is renamed or recolored
- **WHEN** the user changes the label or color of a user-defined definition
- **THEN** all existing story assignments remain attached and immediately use the updated presentation

#### Scenario: User creates an ordinary tag named broken
- **WHEN** the user creates a tag with normalized key `broken` and no current definition owns that key
- **THEN** the tag is created under ordinary user-tag rules without changing or controlling any story's broken flag

### Requirement: Allowed value CRUD
The system SHALL let the user create, rename, reorder, and delete allowed values within a user-defined choice tag, with value uniqueness enforced inside that definition, and SHALL reject value management for boolean tags.

#### Scenario: Values are added to an age definition
- **WHEN** the user adds distinct values such as `3-5` and `6-8` to a user-defined definition
- **THEN** both values become available for story assignment in the configured order

#### Scenario: Duplicate value is submitted
- **WHEN** the user submits a value that normalizes to another value in the same definition
- **THEN** the system rejects it without changing existing values

#### Scenario: Value is renamed
- **WHEN** the user renames an existing allowed value to a valid unique value
- **THEN** existing story assignments remain attached and display the new label

#### Scenario: Value is added to a boolean definition
- **WHEN** the user attempts to add an allowed value to a boolean tag definition
- **THEN** the system rejects the value because the assignment's presence already represents true

### Requirement: Manual story tag assignment
The system SHALL let the user toggle boolean definitions and assign or remove one or more allowed values from choice definitions on one story or a selected group of stories.

#### Scenario: Values are assigned to one story
- **WHEN** the user applies allowed values to a story and saves
- **THEN** the story displays those values and becomes discoverable through their filters

#### Scenario: Value is assigned in bulk
- **WHEN** the user applies an allowed value to multiple selected stories
- **THEN** the system idempotently adds the value to every selected story and reports the affected count

#### Scenario: Manual value is removed
- **WHEN** the user removes a manually assigned value from a story
- **THEN** only that story-value association is removed

#### Scenario: User-defined boolean is toggled
- **WHEN** the user turns a user-defined boolean tag on or off for a story
- **THEN** the system respectively creates or removes the valueless manual assignment for that definition

### Requirement: Derived system facets are protected
The system SHALL display derived tag definitions and values alongside other tags while preventing identity/value edits, deletion, or manual assignment of derived data. Every definition, including protected derived definitions, SHALL permit color changes without changing protected identity or assignment rules. Age SHALL default to green and customized colors SHALL survive seeding and metadata refresh.

#### Scenario: User inspects a derived age value
- **WHEN** a story has a metadata-derived age value
- **THEN** the UI identifies it as system-derived and allows it to be used as a filter

#### Scenario: User attempts to manage a derived definition
- **WHEN** the user opens tag management for a derived definition
- **THEN** create, rename, delete, and manual assignment controls are unavailable while recoloring remains available for that definition

### Requirement: Destructive tag changes require informed confirmation
The system SHALL show the number of affected assignments and saved Collections before deleting a user-defined definition or allowed value, SHALL perform the cascading deletion only after confirmation, and SHALL never offer deletion for a protected derived definition.

#### Scenario: User confirms definition deletion
- **WHEN** the user confirms deletion after seeing the affected assignment count
- **THEN** the system deletes the user-defined definition, its values, and its manual assignments without affecting stories or other definitions

#### Scenario: User cancels value deletion
- **WHEN** the user cancels deletion of an allowed value
- **THEN** the value and every story assignment remain unchanged

#### Scenario: Deleted value is referenced by a Collection
- **WHEN** the user confirms deletion of a value after being shown that a saved Collection references it
- **THEN** the value and assignments are removed and the affected Collection is marked as needing attention rather than silently ignoring the missing criterion

#### Scenario: Invalidated Collection supplies a curated Shelf
- **WHEN** deleting a confirmed tag/value invalidates a linked Collection
- **THEN** approved Shelf membership, internal manual-retention state, and exclusions remain unchanged, and subsequent source review follows incomplete-source safeguards
