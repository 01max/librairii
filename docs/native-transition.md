# Starting fresh with the Rust application

The Rust application starts with its own profile and a new library. It does not
migrate an existing Go library, settings, recovery journals or browser preferences.
Old files are left untouched, even when old settings are invalid or a Go move
was interrupted. There is no Go bridge release or compatible rollback requirement.

On first launch, choose English or French, then confirm a new or empty library
folder. Import supported story archives through the normal **Import** workflow.
Do not select a nonempty old library as the new destination. Subsequent launches
reopen the Rust library saved in the native profile.

## Back up the Rust library

Quit Librairii, then copy the complete library folder and native configuration
directory. Keep the database, archives, catalog/artwork, staging, trash, logs,
identity marker and any recovery records together. Configuration locations are:

| Platform | Native configuration |
| --- | --- |
| macOS | `~/Library/Preferences/io.github.librairii.native/` |
| Linux | `$XDG_CONFIG_HOME/librairii-native/`, or `~/.config/librairii-native/` |

Moving a Rust library uses **Settings → Change library folder…**. Wait for
active imports, exports and syncs to finish. The app copies and verifies the
complete library before committing the new location. Keep retained source or
unfinished-copy files until the app's recovery status is understood.

## Recover an existing Rust library

A missing drive, corrupt profile or interrupted operation enters recovery; it
never silently creates an empty replacement. Reconnect the drive and choose
**Retry recovery**, or use **Locate the same library…** after a move. The selected
library must have the same identity. A different library is rejected.

Keep profiles, journals, SQLite sidecars and unfinished files when recovery is
refused. Do not delete locks or edit control records to force opening. Follow the
specific recovery message and retain a complete copy before investigation.
Uninstalling replaces/removes the application, not the library or profile.

The [user guide](user-guide.md#settings-interface-language-and-library-location)
describes current settings and recovery. Earlier migration/Go rollback reports
under `docs/rewrite/` are historical evidence, superseded by the
[fresh Rust scope](rewrite/fresh-rust-contract.md).
