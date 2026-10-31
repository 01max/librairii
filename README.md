<p align="center">
  <img src="build/appicon.svg" width="96" height="96" alt="Librairii's three colorful books">
</p>

<h1 align="center">Librairii</h1>

<p align="center">
  <strong>Your stories, beautifully shelved.</strong><br>
  A local-first desktop library for your Lunii story archives.
</p>

<p align="center">
  <a href="https://github.com/01max/librairii/releases/latest"><img src="https://img.shields.io/github/v/release/01max/librairii?style=flat-square&amp;color=405cf5" alt="Latest release"></a>
  <a href="https://github.com/01max/librairii/actions/workflows/rust-quality.yml"><img src="https://img.shields.io/github/actions/workflow/status/01max/librairii/rust-quality.yml?branch=rewrite-in-rust-native-desktop&amp;style=flat-square&amp;label=Native%20CI" alt="Native CI status on the Rust rewrite branch"></a>
  <a href="docs/release-platforms.md"><img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-15213c?style=flat-square" alt="Platforms: macOS and Linux"></a>
</p>

<p align="center">
  <a href="https://github.com/01max/librairii/releases/latest"><strong>Published releases</strong></a> ·
  <a href="#features">Explore features</a> ·
  <a href="docs/user-guide.md">User guide</a> ·
  <a href="#development">Build from source</a>
</p>

Bring your story collection together in one place. Inspect archives, find the
right story, save dynamic Collections, and curate Shelves whose contents you
approve. When you're ready, export your stories for use with Lunii.QT.

On `rewrite-in-rust-native-desktop`, the candidate uses Rust, a directly rendered
Slint/Winit/Skia interface and bundled SQLite. All three native targets have
installed-package CI coverage. Public Mac signing and release promotion remain
open gates; see the [platform matrix](docs/release-platforms.md). Published-release
links do not imply that the Rust candidate has been promoted.

<p align="center">
  <a href="docs/assets/library.png"><img src="docs/assets/library.png" width="1000" alt="Librairii library with colorful story covers, separate Collections and Shelves sections, age filters, and selected-story details"></a>
  <br>
  <sub>Earlier Go/Wails interface with demo stories and generated placeholder covers. Click to enlarge.</sub>
</p>

> [!IMPORTANT]
> While I'm a professional software engineer, this project is an experiment in
> vibe coding with 6.1 Sol and should be taken as such.

## Features

| Feature | What you can do |
| :--- | :--- |
| **Browse your collection** | Explore story covers and open details for descriptions, tags, and archive information. |
| **Find the right story** | Search titles without worrying about accents or capitalization. Refine by age, language, compatibility, and tags. |
| **Make it your own** | Create boolean or choice tags, assign them to stories, and choose their colors. |
| **Save a Collection** | Keep a search and its filters as a dynamic Collection. Matches update as your library changes. |
| **Curate a Shelf** | Choose which stories belong. Link Collections for suggestions, then explicitly approve additions and removals in **Review updates**. |
| **Bring stories to life** | Choose **Sync with Lunii** to fetch official descriptions and cover images for matching stories. |
| **Export what you need** | Copy the current view, Collections, approved Shelf members, or selected stories. Unions deduplicate stories; Broken and unavailable stories are skipped with a reason. |
| **Choose your setup** | Switch the interface between English and French, choose your library folder, and move the complete library from Settings. |

<details>
<summary><strong>Take a closer look: curated Shelves, story details, tags, and settings</strong></summary>

### Shelves you control

Collections find stories automatically; Shelves keep the stories you approve.
Review linked Collection suggestions when you choose. Every decision starts at
**Not now**, and pending suggestions stay out of exports.

<p align="center">
  <img src="docs/assets/curated-shelf.png" width="1000" alt="A curated Shelf in List view showing approved, Broken, and unavailable members with separate export actions">
</p>

<p align="center">
  <img src="docs/assets/shelf-review.png" width="520" alt="Shelf source-mismatch review with Not now selected and zero approved changes">
</p>

### Everything about a story, in one place

See its cover, description, and assigned tags. Open technical details when you
need the UUID, archive filename, verification status, or checksum.

<p align="center">
  <img src="docs/assets/story-details.png" width="720" alt="Story details dialog showing a demo cover, assigned tags, and the option to reveal technical details">
</p>

### A library organized your way

Define reusable tags and colors. Derived tags keep their protected names and
values while letting you choose their colors. Use the **Broken** switch in the
selected-story panel or details to flag troublesome stories, with bulk actions
and Undo. Flagged stories stay visible and are skipped by every export.

<p align="center">
  <img src="docs/assets/tag-manager.png" width="850" alt="Tag manager with controls for creating a tag and changing the derived Age tag's color">
</p>

### A local library that moves with you

Choose English or French and manage the complete library location in Settings.
Moving copies and verifies the library before switching to the new folder.

<p align="center">
  <img src="docs/assets/settings.png" width="720" alt="Settings showing interface language and the stories library location with a Change location action">
</p>

These retained screenshots show the earlier Go/Wails interface using deterministic
frontend demo data and generated covers. They are visual references rather than
captures of the Rust native candidate. The [capture notes](docs/assets/README.md)
describe that earlier UI workflow.

</details>

## Get started

The native candidate targets macOS 15+ on Apple silicon and Intel, and Ubuntu
24.04 x86-64. Choose the package matching your actual architecture. Native
build/install instructions and the distinction between internal candidates and
signed public distributions are in the [platform matrix](docs/release-platforms.md).

| Platform | Native candidate |
| :--- | :--- |
| **macOS · Apple silicon** | Separate ARM application/DMG; matching-host builds and installed GUI pass in CI. Public signing/notarization/stapling remains gated. |
| **macOS · Intel** | Separate Intel application/DMG, exercised on native Intel hardware. Public signed-byte verification remains gated. |
| **Linux · x64** | Ubuntu 24.04/glibc 2.39 portable archive with executable, desktop entry and icons; X11/Wayland and GPU/software routes are exercised in CI. |

Installed native packages need no Rust, Go, Node or browser runtime. Linux still
needs its declared graphics/font/portal/file-manager services. To build the
candidate, follow [native development](docs/development.md). For an existing Go
library, use the [transition and recovery guide](docs/native-transition.md).

### Your first shelf

1. **Choose your library folder.** Confirm the proposed location on first launch,
   or choose another empty folder. Settings lets you change it later.
2. **Import stories.** Choose your local archives. Librairii inspects each file
   and adds a managed copy to your library.
3. **Explore and organize.** Search, filter, and tag your stories. Optionally
   sync with Lunii for official covers and descriptions.
4. **Save a Collection or create a Shelf.** Collections keep a useful query;
   Shelves keep approved stories. Use **Add to Shelf…** for selected stories,
   or **Create Shelf** to preview initial membership.
5. **Export when ready.** Choose the current view, a Collection, or a Shelf,
   then a destination folder. Review any skipped stories and import the exported
   archives through Lunii.QT's supported workflow.

**Accepted file extensions:** `.plain.pk`, `.v1.pk`, `.v2.pk`, `.pk`, `.zip`, `.7z`.
Librairii inspects archive contents; a supported extension alone does not make a
file valid. Plain packs, v1/v2 packs, generic archives, and STUdio archives
containing `story.json` are recognized. See [supported imports](docs/user-guide.md#supported-imports)
for details.

> [!NOTE]
> Librairii organizes and exports archives. It does not decrypt, repair, or
> convert packs, or transfer stories directly to a device. Lunii.QT and device
> compatibility depend on the archive and firmware.

## Your library stays yours

- **Originals stay untouched.** Imports create managed copies. Removing a story
  moves that copy to application trash and leaves your source file in place.
- **Everyday use works offline.** Import, inspection, search, tags, shelves,
  export, and recovery run on your device, without a personal account.
- **Metadata sync is optional.** An explicit sync contacts the official Lunii
  API; cover images are fetched as needed and cached. Story archives are never
  uploaded, and a failed sync keeps the last working catalog.
- **Exports preserve your archives.** Original filenames, extensions, and bytes
  stay intact. Existing destination files are never overwritten.

For backups, data locations, and recovering removed stories, read the
[user and recovery guide](docs/user-guide.md#application-data-trash-and-recovery).

## Development

The Rust workspace separates pure domain/application policies, SQLite and
filesystem custody, external/native adapters, Slint presentation and desktop
composition. See the [architecture](docs/architecture/0005-rust-native-desktop.md)
and [development guide](docs/development.md) for the actual boundaries, native
compiler/runtime prerequisites and Rust Analyzer setup.

Install rustup and the declared native prerequisites first. The checked-in
toolchain selects Rust **1.99.1** with rustfmt, Clippy and llvm-tools-preview.
Python 3 with `tomllib`/`sqlite3` is needed for checks and packaging.
On macOS, set `MACOSX_DEPLOYMENT_TARGET=15.0` as described in the guide.

```sh
git clone https://github.com/01max/librairii.git
cd librairii
git switch rewrite-in-rust-native-desktop
cargo xtask tools
cargo build --locked --package librairii-desktop
cargo xtask check
```

`cargo xtask tools` installs the pinned nextest/deny/audit helpers.
`cargo xtask check` checks the host's locked graph, boundaries, preservation
corpus, source/dependency policy, formatting, Clippy, nextest and separate
doctests. Use the three declared native default configurations rather than
`--all-features`.

| Command / guide | Purpose |
| :--- | :--- |
| `cargo nextest run --workspace --locked --profile ci` | Run workspace behavior tests with the pinned runner. |
| `cargo test --workspace --doc --locked` | Run doctests separately. |
| [Disposable development library](docs/development.md#run-a-disposable-development-library) | Launch the GUI using a fresh explicit root, without normal profile discovery. |
| [Distribution commands](docs/development.md#build-distribution-candidates) | Plan/build a sealed native payload and assemble the matching DMG or Linux archive. |
| [Platform verification](docs/release-platforms.md#evidence-for-each-candidate) | Inspect exact package bytes and run actual installed workflows in disposable CI. |

Full native CI also rehearses the Go/native handover with Go **1.26.5**. Native
packages do not require Go at runtime. Optional coverage/fuzz/benchmark commands
and their extra pinned tools are listed in the development guide.

<details>
<summary><strong>Retained Go/Wails compatibility workflow</strong></summary>

The Go/Wails/React implementation, oracle fixtures and release commands remain
available until the separate release/cutover review. `make check`, `make build`,
`make dmg` and `make linux` still operate on that implementation; they do not
build the native Rust application. Node/Wails/browser prerequisites and the
applicable legacy checks are documented in
[the development guide](docs/development.md#retained-go-compatibility-checks).

</details>

## Documentation

| Looking for… | Start here |
| :--- | :--- |
| Supported formats, workflows, privacy, or recovery | [User and recovery guide](docs/user-guide.md) |
| A safe demo without a device or Lunii account | [First-story walkthrough](docs/demos/first-story-vertical-slice.md) |
| Packaging and platform qualification | [Release platform matrix](docs/release-platforms.md) |
| The current application architecture | [Rust native desktop](docs/architecture/0005-rust-native-desktop.md) |
| Collections, curated membership, and reviewed updates | [Collections and Shelves](docs/architecture/0004-collections-and-shelves.md) · [Verification](docs/verification/collections-and-shelves.md) |
| Language settings and moving a library | [Settings and library location](docs/user-guide.md#settings-interface-language-and-library-location) · [Architecture](docs/architecture/0003-settings-and-library-location.md) |
| Native build, checks and editor setup | [Development guide](docs/development.md) |
| Earlier UI screenshot references | [Capture notes](docs/assets/README.md) |
| Large-library performance and budgets | [Performance guide](docs/performance/large-library.md) |
| The Lunii catalog integration boundary | [Interoperability decision](docs/architecture/0002-lunii-catalog-interoperability.md) |

## Help improve Librairii

Found a bug or have an idea? [Open an issue](https://github.com/01max/librairii/issues)
with the behavior you expected, what happened, and your platform. Documentation
improvements and focused pull requests are welcome; start with the
[development setup](#development) and run `cargo xtask check` plus applicable
[retained Go checks](docs/development.md#retained-go-compatibility-checks) before
submitting code changes.

---

Librairii is an independent project, not affiliated with or endorsed by Lunii
SAS or the Lunii.QT maintainers. Names describe archive and workflow
interoperability. See [attribution and independence](docs/user-guide.md#attribution-and-independence).
