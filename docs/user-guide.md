# Librairii user and recovery guide

Librairii keeps a private, local library of story archives. Importing creates a
managed copy; it never edits, renames, moves, or deletes the source file you
selected. This guide describes the behavior of release `0.1.x`.

## Supported imports

The native import picker accepts these filename extensions:

- `.plain.pk`
- `.v1.pk`
- `.v2.pk`
- `.pk`
- `.zip`
- `.7z`

Librairii recognizes plain packs, v1/v2 packs, generic PK/ZIP archives, and
STUdio archives containing `story.json`. Equivalent STUdio and generic content
is supported in ZIP or 7z containers. An extension alone is not enough:
Librairii inspects required entries, paths, UUIDs, embedded metadata, and
artwork, and applies bounded entry-count, expanded-size, compression-ratio,
metadata, and image limits before publishing a managed copy.

An accepted archive is streamed into isolated staging, hashed with SHA-256,
and published under the application data root. The managed bytes are identical
to the selected source bytes. Importing the same checksum is idempotent. If
different bytes claim a UUID that is already active, the existing story and
archive stay authoritative and the conflicting source is not published.

Encrypted, malformed, unsafe, incomplete, or unsupported archives are rejected.
Librairii does not decrypt, repair, or convert story packs.

## Export passthrough and Lunii.QT

An export can snapshot any of these scopes:

- the explicit story selection;
- every result in the current query, not only the visible page;
- one saved shelf; or
- the deduplicated union of several shelves.

Before copying, Librairii reevaluates the scope and checks each managed archive,
its checksum and validation state, its extension, the destination, and filename
conflicts. Exports preserve the original filename, supported extension, and
archive bytes. They do not add a manifest, decrypt, rewrite, transcode, or
otherwise mutate a pack. Destination-local temporary files are published
atomically and existing files are never overwritten. Cancellation removes
unfinished temporary files but retains exports that were already completed.

The resulting files are intended to remain selectable or draggable into
Lunii.QT's supported import flow. This is archive passthrough interoperability,
not a promise that Lunii.QT will accept every structurally valid archive or
that every archive works with every device or firmware version. Librairii does
not discover, pair with, update, or transfer files directly to a storyteller
device.

## Search and saved shelves

Name search is a literal, case-insensitive, accent-insensitive substring match
against the currently displayed title. Characters such as `%` and `_` have no
wildcard meaning. Name, language, archive compatibility, built-in/user boolean
tags, choice tags, and supported derived metadata filters combine as follows:

- selected values inside one choice definition use OR;
- separate filter groups use AND;
- a boolean filter can require assigned, require unassigned, or be ignored.

The collection URL hash records the live query, sort, and page so reload,
back, and forward restore navigation. A saved shelf stores only membership:
name search and membership-affecting filters. It does not store pagination,
sort, current selection, dialog state, or operation progress.

Shelves are dynamic saved queries rather than folders of copied stories. Their
membership and count are reevaluated after imports, removals, metadata refreshes,
and tag changes. An empty saved query intentionally means all active stories.
When a referenced tag or value is deleted, the shelf is marked as needing
attention and evaluation/export is blocked; Librairii never silently broadens
the query. Multi-shelf views and exports count each story once and report
overlap separately.

## Application data, trash, and recovery

The default application data root is:

| Platform | Location |
| --- | --- |
| macOS | `~/Library/Application Support/Librairii` |
| Windows | `%LOCALAPPDATA%\Librairii` |
| Linux | `$XDG_DATA_HOME/librairii`, or `~/.local/share/librairii` when `XDG_DATA_HOME` is unset |

Development and test commands may select a different absolute root with
`LIBRAIRII_DATA_ROOT`. A production library contains:

| Path | Purpose |
| --- | --- |
| `db/librairii.sqlite3` | stories, archive custody, tags, shelves, metadata state, and operation history |
| `archives/` | content-addressed managed archive copies |
| `catalog/` | validated official-metadata snapshots and cached artwork |
| `staging/` | incomplete local operations; abandoned entries are cleaned during startup recovery |
| `trash/` | managed archives removed from the active library |
| `logs/` | bounded local diagnostic logs |

Removing a story first moves its managed archive to
`trash/removals/<operation-id>/`, then removes the active database record.
Librairii trash is not the operating system trash, and this release has no
in-app restore or empty-trash command. To recover an archive, quit Librairii,
copy the archive from `trash/` to a normal folder outside the application data
root, restart Librairii, and import that copy. Reimporting recreates the story
from archive/official metadata; manual tag assignments that were deleted with
the old story record are not restored.

For a complete backup, quit Librairii and copy the entire application data root.
Keep the database, `archives/`, and `catalog/` together. Do not edit the SQLite
database or move files directly into `archives/`.

Before applying a pending database migration, Librairii creates a protected
SQLite snapshot beside the live database named like
`librairii.pre-migration-vNNN-….sqlite3`. A failed migration automatically
restores that snapshot and never deletes managed archive bytes. If the expected
database path contains an unrelated or legacy schema, mutations remain disabled
until the recovery screen's **Preserve database and create new library** action
moves the database and any WAL/SHM sidecars into a unique
`db/schema-conflict-recovery-*` directory. Nothing in that directory is
overwritten. If preservation cannot be completed, Librairii keeps mutations
disabled and does not silently initialize over the conflicting files.

Uninstalling the application bundle does not delete the application data root.
Delete that root separately only when you intend to permanently erase the
library, managed archives, catalog cache, trash, logs, and recovery files.

## Official metadata and provenance

An explicit metadata refresh obtains a bounded guest token and downloads the
official pack catalog. Librairii validates and stages the complete response
before atomically activating it. A failed, cancelled, corrupt, timed-out, or
offline refresh leaves the last-known-good snapshot active.

Only complete archive UUIDs are matched. For display fields, active official
metadata in the configured locale takes precedence, then permitted embedded
metadata, then a deterministic UUID fallback. The detail UI exposes source and
freshness information such as locale, publisher, source record, source update,
fetch, and activation timestamps when the catalog provides them. Missing or
ambiguous age data is left unassigned; unambiguous ages produce read-only
derived facets.

Refreshing official metadata can change display text, artwork, derived facets,
search results, and dynamic shelf membership. It never changes user-created
tag definitions, manual tag assignments, saved shelf definitions, archive
bytes, or the protected built-in `broken` tag.

## Privacy and network behavior

Import, inspection, organization, search, shelves, export, removal, and local
recovery run entirely on the device. Librairii has no personal sign-in path and
does not request or store a username, password, account cookie, personal token,
device key, firmware, story archive, or user-library data from Lunii services.

The official metadata adapter is the only product path that contacts the
network. An explicit refresh uses HTTPS (TLS 1.2 minimum) to request a guest
token and the public pack catalog from the configured Lunii endpoints.
Referenced official artwork is fetched lazily from its allowlisted HTTPS origin
when the UI needs it, then validated and cached. Requests have time and byte
limits and do not follow redirects. There is no scheduled background catalog
refresh and no archive upload.

Diagnostic exports exclude archive bytes, guest tokens, absolute private paths,
catalog artwork, and other sensitive local data.

## Attribution and independence

Librairii is an independent project and is not affiliated with or endorsed by
Lunii SAS or the Lunii.QT maintainers. Product and project names are used only
to describe archive and workflow interoperability.

The guest-token envelope, catalog protocol details, and supported passthrough
workflow were independently reimplemented from observable behavior in
[Lunii.QT](https://github.com/o-daneel/Lunii.QT) at tree
`c8afe43dde21c2be33c0667ced962dc023eb948a`. Lunii.QT is licensed under
GPL-3.0. Librairii contains no copied Lunii.QT source code or copyrighted
catalog dataset. The detailed technical boundary is recorded in
[the Lunii catalog interoperability decision](architecture/0002-lunii-catalog-interoperability.md).
