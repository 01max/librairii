<p align="center">
  <img src="build/appicon.svg" width="96" height="96" alt="Librairii's three colorful books">
</p>

<h1 align="center">Librairii</h1>

> [!IMPORTANT]
> While I'm a professional software engineer, this project is an experiment in
> vibe coding with 6.1 Sol and should be taken as such.

<p align="center">
  <strong>Your stories, beautifully shelved.</strong><br>
  A local-first desktop library for your Lunii story archives.
</p>

<p align="center">
  <a href="https://github.com/01max/librairii/releases/latest"><img src="https://img.shields.io/github/v/release/01max/librairii?style=flat-square&amp;color=405cf5" alt="Latest release"></a>
  <a href="https://github.com/01max/librairii/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/01max/librairii/ci.yml?branch=main&amp;style=flat-square&amp;label=CI" alt="CI status on main"></a>
  <a href="docs/release-platforms.md"><img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-15213c?style=flat-square" alt="Platforms: macOS and Linux"></a>
</p>

<p align="center">
  <a href="https://github.com/01max/librairii/releases/latest"><strong>Download for macOS</strong></a> ·
  <a href="#features">Explore features</a> ·
  <a href="docs/user-guide.md">User guide</a> ·
  <a href="#development">Build from source</a>
</p>

Bring your story collection together in one place. Inspect archives, find the
right story, save dynamic Collections, and curate Shelves whose contents you
approve. When you're ready, export your stories for use with Lunii.QT.

<p align="center">
  <a href="docs/assets/library.png"><img src="docs/assets/library.png" width="1000" alt="Librairii library with colorful story covers, separate Collections and Shelves sections, age filters, and selected-story details"></a>
  <br>
  <sub>The Librairii interface with demo stories and generated placeholder covers. Click to enlarge.</sub>
</p>

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

These screenshots use deterministic frontend demo data, including generated
placeholder covers. See the [screenshot capture instructions](docs/assets/README.md)
to refresh them from the current application.

</details>

## Get started

| Platform | How to get Librairii |
| :--- | :--- |
| **macOS · Apple silicon** | Download the `.dmg` from the [latest release](https://github.com/01max/librairii/releases/latest), open it, and drag Librairii into Applications. |
| **Linux · x64** | [Build from source](#development). The desktop runtime requires GTK 3 and WebKitGTK 4.1; the platform workflow produces a portable `tar.gz`. |

See the [release platform matrix](docs/release-platforms.md) for host requirements,
artifact details, and platform verification.

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

Librairii runs as a single-process **Wails + Go + React + SQLite** desktop
application. There is no sidecar service, loopback web server, or external
database to configure.

### Prerequisites

| Tool | Version / setup |
| :--- | :--- |
| Go | **1.26 or newer** |
| Node.js | Use the version in [`.nvmrc`](.nvmrc), matching CI. The frontend requires `^24.15.0 \|\| >=26.0.0`. |
| Wails CLI | Installed by `make setup`, pinned in [`.wails-version`](.wails-version). |
| Native platform dependencies | Follow the [Wails platform setup](https://wails.io/docs/gettingstarted/installation/#platform-specific-dependencies). Linux uses GTK 3 and WebKitGTK 4.1. |
| Browser checks | Google Chrome or Chromium for `make check`. Set `LIBRAIRII_CHROME_PATH` if installed at a nonstandard location. |

If you use nvm, run `nvm install` and `nvm use` in the repository before setup.

On macOS:

```sh
git clone https://github.com/01max/librairii.git
cd librairii
make setup
make check
make build
```

The built application is placed in `build/bin/`. If the Wails pin changes,
run `make setup-wails` to update your local CLI.

<details>
<summary><strong>Build on Linux with WebKitGTK 4.1</strong></summary>

Clone the repository and run `make setup` as above. Use the WebKitGTK 4.1 tag
for Go checks and the Wails build:

```sh
GOFLAGS="-tags=webkit2_41" make check
"$(go env GOPATH)/bin/wails" build -m -nocolour -tags webkit2_41
```

For a verified portable archive, follow the host setup in
[the platform workflow](.github/workflows/platform-release.yml) and run
`make verify-platform-linux`.

</details>

### Useful commands

| Command | Purpose |
| :--- | :--- |
| `make check` | Check Go formatting, vet and test Go, type-check and lint TypeScript, run frontend tests and browser acceptance checks, and build the production frontend. |
| `make build` | Build the desktop application with the pinned Wails CLI. |
| `make smoke-first-story` | Exercise import, inspection, and removal with a synthetic, copyright-free archive. |
| `make smoke-foundation` | Check a clean-environment macOS launch, SQLite reopen, and shutdown. |
| `npm --prefix frontend run docs:screenshots` | Refresh documentation images from isolated frontend fixtures using Chrome or Chromium. |

The [first-story walkthrough](docs/demos/first-story-vertical-slice.md) also
shows how to try the synthetic archive in the packaged app with an isolated
data root.

<details>
<summary><strong>Release verification</strong></summary>

On the current macOS host, build and independently verify the installer with:

```sh
make dmg
make smoke-release
make verify-packaged-acceptance
```

`make dmg` runs `build-current-installer` followed by `verify-current-installer`.
Both targets can also be run individually.

The packaged acceptance gate launches the installed application twice against
an isolated data root. It covers the release UI, SQLite persistence, native
adapters, the complete local story workflow, and clean shutdown.

On a Linux verification host, run `make verify-platform-linux`. See the
[release platform matrix](docs/release-platforms.md) for the complete platform
requirements and evidence contract.

</details>

## Documentation

| Looking for… | Start here |
| :--- | :--- |
| Supported formats, workflows, privacy, or recovery | [User and recovery guide](docs/user-guide.md) |
| A safe demo without a device or Lunii account | [First-story walkthrough](docs/demos/first-story-vertical-slice.md) |
| Packaging and platform qualification | [Release platform matrix](docs/release-platforms.md) |
| The application architecture | [Wails rebuild baseline](docs/architecture/0001-wails-rebuild-baseline.md) |
| Collections, curated membership, and reviewed updates | [Collections and Shelves](docs/architecture/0004-collections-and-shelves.md) · [Verification](docs/verification/collections-and-shelves.md) |
| Language settings and moving a library | [Settings and library location](docs/user-guide.md#settings-interface-language-and-library-location) · [Architecture](docs/architecture/0003-settings-and-library-location.md) |
| Refreshing documentation screenshots | [Capture instructions](docs/assets/README.md) |
| Large-library performance and budgets | [Performance guide](docs/performance/large-library.md) |
| The Lunii catalog integration boundary | [Interoperability decision](docs/architecture/0002-lunii-catalog-interoperability.md) |

## Help improve Librairii

Found a bug or have an idea? [Open an issue](https://github.com/01max/librairii/issues)
with the behavior you expected, what happened, and your platform. Documentation
improvements and focused pull requests are welcome; start with the
[development setup](#development) and run `make check` before submitting code
changes.

---

Librairii is an independent project, not affiliated with or endorsed by Lunii
SAS or the Lunii.QT maintainers. Names describe archive and workflow
interoperability. See [attribution and independence](docs/user-guide.md#attribution-and-independence).
