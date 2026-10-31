# Moving from Go Librairii to the native application

These instructions apply to the Rust candidate on
`rewrite-in-rust-native-desktop`. Public Mac signing and final release promotion
remain gated; see [platform support](release-platforms.md). The retained
[handover rehearsal](rewrite/section-17.md) describes the tested bridge and
isolated-copy evidence. A previous published Go release does not necessarily
include that bridge or its preference-export command.

## Upgrade using an independent library

1. Finish or cancel active imports, exports and metadata sync in Go. In Settings,
   select **Export preferences…** (**Exporter les préférences…**) and choose a
   folder without an existing transfer. Keep `librairii-preferences.json`.
   If that Go version has no export action, preserve its settings backup and use
   the documented missing-transfer defaults; do not fabricate a transfer file.
2. Quit Go completely. Copy the **entire library folder** and its separate Go
   settings folder to a backup location. Include the database, archives, artwork,
   catalog, history, staging, trash, logs, additional root files, settings and
   any recovery journal. Keep the original folder paths with this backup.
   A database file copied while an application is running is insufficient.
3. If a move or recovery is pending, reconnect the expected drive and finish
   recovery in the owning application first. Keep unfinished copies and journals.
   Do not delete SQLite sidecars, lock files or recovery records to force opening.
4. Start native Librairii. In the adoption screen choose **Choose library
   folder…**, review a **different empty folder**, then **Create an independent
   copy**. Native verifies the copy before binding it. The existing Go settings and
   default location continue to select the original Go source. Native profiles,
   recovery records and copied files belong to the separate candidate.
5. In native Settings, select **Import preferences…** (**Importer les préférences…**)
   and choose the exported JSON file. It must belong to this library. Import
   once, before customizing native preferences, to transfer all available choices.
   Already saved native choices take precedence. Repeating an import preserves
   current choices; another export cannot reset them.
6. Check your stories, Collections, Shelves, tags, Broken flags, History and
   presentation after closing and reopening native Librairii. Keep the source
   and backup until you have checked the result.

The transfer contains view mode, dismissed notice identifiers, sync-panel
visibility and bounded semantic navigation. It excludes browser storage, tokens,
private paths, archive data and the database. Navigation includes only browser
history known to that Go session; earlier history cannot be enumerated after a
reload. Re-export in a new folder if you want a fresh transfer. Copying the JSON
alone does not back up a library.

Without a transfer, library data still moves through the verified copy. Native
presentation starts in List view at Library, with default query settings, no
imported dismissed notices and the sync panel visible when applicable. Language
comes from the independent saved profile. An invalid or future transfer remains
untouched and leaves existing choices intact.

## Backup locations and checks

Settings shows the actual library root. A custom root takes precedence over the
defaults below. Linux honors absolute `XDG_CONFIG_HOME`/`XDG_DATA_HOME`, otherwise
uses the home-directory defaults shown. macOS uses its platform paths.

| Application | macOS default library | macOS separate settings/profile |
| --- | --- | --- |
| Go | `~/Library/Application Support/Librairii` | `~/Library/Preferences/io.github.librairii.app/` |
| Native | `~/Library/Application Support/LibrairiiNative` | `~/Library/Preferences/io.github.librairii.native/` |

| Application | Linux default library | Linux separate settings/profile |
| --- | --- | --- |
| Go | `$XDG_DATA_HOME/librairii` or `~/.local/share/librairii` | `$XDG_CONFIG_HOME/librairii/` or `~/.config/librairii/` |
| Native | `$XDG_DATA_HOME/librairii-native` or `~/.local/share/librairii-native` | `$XDG_CONFIG_HOME/librairii-native/` or `~/.config/librairii-native/` |

Quit both applications before backing up. Copy each complete library and its
matching settings/profile directory, preserving hidden files, exact names,
archives, database and any SQLite sidecars, root markers, native ownership
records and recovery journals. Label the application version, date, original
paths and whether recovery was pending. Keep the preference transfer beside the
backup, not instead of it. Check file counts/sizes/checksums against the closed
source and retain the original until the independent copy reopens successfully.
Copying only `db/librairii.sqlite3` or only the transfer JSON loses other state.

A pre-migration SQLite snapshot does not contain archives, profiles or every
post-backup edit. Restoring it alone is not a complete rollback. Do not merge a
restored database with another library's archive/profile/journal set. If a
backup is uncertain or has unfinished work, preserve it and recover in the
owning application before attempting adoption. Application uninstall/replacement
does not erase these data directories.

## Why the Go source stays separate

Safe in-place adoption would require evidence that **every unmodified older Go
executable** is excluded for the entire native library lifetime, including
restarts, relocation and interrupted recovery. A lock used by the new Go bridge
does not make older binaries obey it. That lifetime exclusion has not been
established, so native adoption refuses the in-place route and creates an
independent library with its own native profile and recovery authority.

The source and candidate must not alias through symlinks, hard links, overlapping
directories or shared profile/journal paths. The verified copy creates fresh
target file identities; pointing two applications at the same folder is not
isolation. Keep unmodified Go's default and saved settings pointed at its
original source. Close it during the initial verified copy; later Go work belongs
only to that source and does not synchronize with native changes.

**Unsupported manual routing:** do not edit old Go settings, set a developer
override, copy a native profile into the Go settings directory, remove ownership
markers/locks, or redirect a legacy path to the candidate. An older executable
may ignore the shared lock and native ownership marker. Such routing bypasses
the supported adoption boundary and can mutate native files or live journals;
the bridge's refusal is not protection from every historical binary.

## Moving between operating systems

The qualified native targets are macOS Apple silicon, macOS Intel and Ubuntu
24.04 x64. Windows naming diagnostics in retained code do not qualify a Windows
application or a Windows filesystem as a supported destination.

Before moving between macOS/Linux, finish recovery on the original host, quit
and preserve a complete closed backup. Use a destination that can preserve
**every exact file and directory name and byte**. Case-sensitive names may
collide on a case-insensitive volume; canonically equivalent Unicode spellings,
long components/paths, reserved characters or trailing spaces/dots may be refused
by another filesystem or copying tool. Do not accept silent renaming, Unicode
normalization, case folding, truncation, replacement or skipped files. Keep the
source intact and use another suitable destination if verification fails.
External export destinations have the same filename/no-overwrite constraints;
Librairii does not rename packs to force an export through.

Profiles contain OS-specific absolute paths and local binding identity; they
are not portable launch instructions. Preserve foreign profiles unchanged as
backup evidence instead of copying them over the destination's native profile
or editing their JSON. Copied export-history destinations remain historical
strings and must pass fresh local validation before Reveal destination can act.
They never become permission to open, overwrite or clean a foreign path.

There is no general cross-OS migration wizard in this candidate. The storage
layer's foreign-profile rebinding requires an actual verified independent copy,
matching library UUID, fresh local binding and no conflicting/recovering profile;
decoding a foreign path alone grants no authority. The ordinary **Locate the
same library…** recovery action is for the already bound identity, not permission
to bypass a refused foreign copy. If the application cannot verify/bind a copy,
retain source and destination and stop that migration rather than manually
repointing profiles. For archive-only transfer, export to an independent folder
and import on the other host; this preserves pack bytes but does not transfer
all tags, Collections, Shelves, preferences or operation history.

Finish interrupted operations on the owning OS before moving. Keep foreign Go
or native relocation/removal/export journals with the backup. A journal's path
strings, or a manually edited prefix, cannot authorize native replay or cleanup
on another host. Copying a pending journal does not make its file identities,
leases or recorded source/destination valid there. Unsupported, corrupt,
mismatched or uncertain records remain recovery cases with their bytes retained.

## Returning to Go

Use the original Go source or a **separate restored pre-native backup**. Never
restore an older database over the newer native candidate. The bridge requires
this separate rollback route and refuses a native-owned library. The disposable
reopening rehearsal preserves the tested Collections, Shelves and native edits,
but Go startup refreshes derived search names and may add derived Age tags.
That bounded experiment does not qualify arbitrary native changes for direct
rollback. Keep the native recovery marker intact. Older Go executables can ignore the shared lock;
keep their settings pointed at the original source.

Changes made after the backup remain in the native candidate. Before switching,
keep a full closed copy of that candidate and its native settings. Export newly
imported archives to a separate destination and re-import them into the restored
Go copy if needed. Review Collection criteria, Shelf order/membership, exclusions,
source decisions, tags, Broken flags and notice preferences explicitly. Those
organizational edits are not merged automatically by archive export. Record which
application contains each newer change before resuming work.

## If opening or recovery is refused

Close the other application and retry. Reconnect the expected drive or locate the
same library UUID through native recovery. A different library cannot replace it.
Keep saved settings, journals, source and unfinished destination together. Native
recovery follows its own verified record; an imported Go journal never authorizes
native cleanup. Changed or unrelated files are retained. You can change language
while storage is unavailable. Keep the retained paths shown by recovery; do not
manually remove control files or overwrite uncertain data.
