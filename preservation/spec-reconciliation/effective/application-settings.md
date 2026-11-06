# Application Settings

## Purpose

Define accessible application settings, independently persisted preferences, explicit first-launch setup, adoption of existing libraries, and isolated development configuration.

## Requirements

### Requirement: Settings are accessible through native and in-app commands
The system SHALL provide one accessible settings panel containing interface-language and library-location controls. On macOS it SHALL expose a localized Settings command in the Librairii application menu with the Command-comma shortcut, while preserving standard application and editing commands. On Linux it SHALL expose a localized application-menu settings command and an in-app entry. An in-app entry SHALL remain accessible during library recovery on both platforms.

#### Scenario: macOS user opens settings
- **WHEN** the user chooses the Settings command in the Librairii application menu or presses Command-comma
- **THEN** the application opens or focuses the settings panel displaying the current language and full library path

#### Scenario: Linux user opens settings
- **WHEN** the user invokes either the application-menu settings command or the in-app settings entry
- **THEN** the same settings panel opens with the current preferences

#### Scenario: Settings command arrives during frontend initialization
- **WHEN** a native settings command is invoked before the frontend subscribes to application events
- **THEN** the settings panel opens once the frontend is ready without requiring another command

#### Scenario: User navigates settings with the keyboard
- **WHEN** the user opens and closes the ordinary settings panel using the keyboard
- **THEN** focus enters the panel, remains within it while open, and returns to the invoking control when closed, and all controls have localized accessible names

### Requirement: Preferences persist independently of library storage
The system SHALL persist a versioned configuration containing the explicit interface language, selected library root, library identity, and setup completion independently of the movable library. Updates SHALL be atomic and serialized, and failed persistence SHALL leave the previously effective settings intact and report a localized error.

#### Scenario: Settings survive relocation and restart
- **WHEN** the user changes interface language, moves the library successfully, and restarts the application
- **THEN** the saved language is restored and the selected library is opened at its new root using configuration outside both library locations

#### Scenario: Preference persistence fails
- **WHEN** an update cannot be durably written
- **THEN** the system reports failure, retains the previously effective settings, and does not claim the change succeeded

#### Scenario: Updates overlap
- **WHEN** a language update and a relocation root commit are requested concurrently
- **THEN** the persisted result preserves both successful updates without replacing either with an older configuration snapshot

#### Scenario: Configuration is invalid
- **WHEN** an existing configuration is unreadable, malformed, or uses an unsupported version
- **THEN** the system presents configuration recovery without overwriting it, treating the installation as new, or initializing a replacement library

### Requirement: First launch requires explicit library setup
For a new installation without an existing identifiable library, the system SHALL present setup before enabling collection operations. Setup SHALL offer French and English language selection, a proposed platform default library root, a native folder picker, and explicit confirmation of the selected root. The system SHALL initialize library data only after confirmation and mark setup complete only after initialization and configuration persistence succeed.

#### Scenario: New user confirms the suggested location
- **WHEN** a new user confirms the proposed writable default library root
- **THEN** the application initializes the complete library there, persists the location and language, and opens the empty collection

#### Scenario: New user chooses another folder
- **WHEN** a new user selects an acceptable folder through the native picker and confirms setup
- **THEN** that exact folder becomes the initialized library root without a silently appended subfolder

#### Scenario: User cancels the folder picker
- **WHEN** the user cancels folder selection during setup
- **THEN** setup remains available with its previous selection and no library is initialized by the cancelled action

#### Scenario: Setup cannot complete
- **WHEN** storage initialization or configuration persistence fails during setup
- **THEN** the system keeps collection mutations disabled, reports the failure, and supports retry without discarding data already initialized by that attempt

### Requirement: Existing installations retain their library
When no settings profile exists, the system SHALL detect an identifiable existing Librairii library at the prior platform default root, retain that location, assign or retain its stable identity, and persist settings without requiring first-launch folder selection. An ambiguous or incompatible existing root SHALL retain existing recovery protections.

#### Scenario: User upgrades an existing installation
- **WHEN** the application starts without a settings profile and finds a valid existing library at its prior default root
- **THEN** it opens that library in place with its archives, tags, shelves, and history intact and records the current root in settings

#### Scenario: Default root contains a foreign database
- **WHEN** an installation without settings finds an incompatible database at the prior default root
- **THEN** the application preserves the database and enters storage recovery without silently replacing it or opening a new library

### Requirement: Explicit development roots remain isolated
When `LIBRAIRII_DATA_ROOT` is explicitly supplied, the system SHALL use that root without first-launch folder selection, SHALL avoid reading or writing the production settings profile, SHALL default the isolated UI to English, and SHALL disable library relocation with a localized explanation.

#### Scenario: Packaged acceptance supplies an isolated root
- **WHEN** an acceptance or development launch supplies an absolute `LIBRAIRII_DATA_ROOT`
- **THEN** existing initialization and acceptance flows operate at that root without production preferences or onboarding affecting the result

#### Scenario: User attempts relocation in override mode
- **WHEN** the settings panel is opened under an explicit root override
- **THEN** it shows the override location and explains why change-location is unavailable without modifying the root or production profile
