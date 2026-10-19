# Settings and movable libraries

Application configuration lives outside library storage. The versioned profile
records UI language, root, setup completion, and stable library identity.
`internal/preferences` validates and atomically publishes the profile and owns
relocation journals, manifests, verification, and cleanup. Native dialog and
storage adapters remain in `internal/platform`; the Wails facade coordinates
setup, recovery, and moves. Root paths selected by the native picker stay in Go,
with explicit display-only path fields in settings snapshots and opaque move
preparation IDs.

Startup distinguishes a new installation, an existing default-root library, a
configured library, and recovery. A production library is created only after
setup confirmation. Existing libraries are adopted in place. Missing configured
libraries are never implicitly recreated. Explicit developer roots remain
isolated from production profiles and onboarding.

A facade lifecycle gate covers library bindings and artwork requests. A move
requires an idle operation manager and exclusive access, closes runtime/logging
resources, and checkpoints/closes SQLite before recording the source manifest.
Copying always streams to a destination-volume workspace. Checksums, database
integrity, schema, and identity are verified before publishing and committing
the new root. Runtime services are reconstructed afterward, clearing prepared
exports and ensuring root-bound dependencies use one storage generation.

The durable profile root is the single source of authority. Before its commit,
recovery retains the source; after commitment it retains the destination. The
independent journal permits restart reconciliation while SQLite is closed.
Cleanup only removes manifest files with matching hashes and preserves changed
or newly introduced content. Inconsistent journals or missing authoritative
libraries require explicit recovery.

Shared English/French catalogs serve React presentation and Go-owned menu and
picker text. UI language is independent of metadata locale, story content,
custom labels, and saved query identities. Stable outcome identities and a
localized fallback keep historical technical messages inspectable. macOS uses
a small native adapter to relocate the Wails callback item into the standard
application menu while retaining native application/Edit roles.
