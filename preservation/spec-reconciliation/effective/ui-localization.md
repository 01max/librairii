# UI Localization

## Purpose

Define complete English and French interface presentation, persistent language selection, immediate language changes, and localized outcomes while preserving content and metadata policy.

## Requirements

### Requirement: Complete English and French UI presentation
The system SHALL provide English and French translations for all application-owned user-facing interface text, including collection navigation, filters, tag and shelf controls, story dialogs, setup, settings, recovery, confirmations, operation progress and history summaries, validation and errors, native menu labels and picker titles, empty states, tooltips, and accessibility labels. Counts SHALL use the selected language's plural forms, and dates and numbers SHALL use its formatting. The document language SHALL reflect the active UI language. OS-owned native controls SHALL retain the operating system's presentation.

#### Scenario: User browses the French interface
- **WHEN** French is the active UI language
- **THEN** application-owned interface text throughout existing and new screens is French, the document language is French, and dynamic counts, dates, and accessible names use French presentation

#### Scenario: User browses the English interface
- **WHEN** English is the active UI language
- **THEN** the same screens and interactions use complete English presentation with English formatting

#### Scenario: Application opens a native picker
- **WHEN** a French-speaking user opens an import, export, diagnostics, or library-folder picker
- **THEN** its application-supplied title and filter labels are French while OS-owned buttons follow the operating system

### Requirement: Initial UI language uses supported system preferences
In normal production mode without a saved explicit language, the system SHALL select French when the system's preferred language resolves to French and English otherwise. A saved explicit choice SHALL take precedence on subsequent launches.

#### Scenario: French system language
- **WHEN** the application first launches with a French preferred system language and no saved choice
- **THEN** setup and application-owned native commands use French

#### Scenario: Unsupported system language
- **WHEN** the application first launches with a preferred language other than French and no saved choice
- **THEN** setup and application-owned native commands use English

#### Scenario: Saved choice overrides system language
- **WHEN** the user has saved English and later launches on a system whose preferred language is French
- **THEN** the application continues to use English

### Requirement: Language changes apply immediately without losing work
The system SHALL offer exactly the explicit UI-language choices Français and English. A successfully persisted choice SHALL immediately update the currently rendered UI and application-owned native menu text without restarting the library, clearing navigation or unsaved input, or cancelling running operations. Unsupported values submitted at the application boundary SHALL be rejected without changing settings.

#### Scenario: Language switches during ordinary use
- **WHEN** the user changes from English to French while a collection query, selection, and unsaved shelf input are present
- **THEN** their presentation changes to French while the query, selected stories, and entered input remain intact

#### Scenario: Operation is active during a language change
- **WHEN** the user changes language while an import is running
- **THEN** the import continues and its progress and eventual result render in the newly selected language

#### Scenario: Unsupported language is submitted
- **WHEN** a client requests a language other than `en` or `fr`
- **THEN** the application returns a localized validation error and retains the current language

### Requirement: UI language does not change content or metadata policy
The system SHALL keep UI-language preferences independent of official metadata locale configuration, fetch requests, catalog activation, and metadata selection. Switching UI language SHALL preserve displayed story titles and descriptions, user-authored tag labels and values, shelf names, filenames, stored identifiers, and saved query semantics. Protected built-in and derived labels SHALL be localized for display using stable identities without rewriting domain data.

#### Scenario: User switches to French with official metadata present
- **WHEN** a library displays official story content and the user switches the UI to French
- **THEN** interface labels change while official story titles and descriptions remain the same, no metadata refresh is triggered, and metadata locale configuration and fetch behavior remain unchanged

#### Scenario: User-authored organization is preserved
- **WHEN** UI language changes in a library containing custom tags, values, and shelves
- **THEN** their authored text, identities, assignments, and query membership remain unchanged while protected system labels use the selected UI language

### Requirement: Backend outcomes render in the current UI language
The system SHALL present backend errors and operation outcomes using stable message identities and parameters in the currently selected UI language. Stored results SHALL remain understandable after language changes and restarts. Unknown historical messages SHALL receive a localized summary with original diagnostic text available as technical detail, without guessing a translation of arbitrary content.

#### Scenario: User reads previous operation history in French
- **WHEN** an operation completed while English was active and the user later opens its history entry in French
- **THEN** application-owned outcome summaries and supported reason messages render in French while stored content and technical identifiers remain unchanged

#### Scenario: Historical outcome lacks a supported message identity
- **WHEN** an old history entry contains an unknown English technical message
- **THEN** the UI presents a localized fallback summary and makes the original message available as technical detail
