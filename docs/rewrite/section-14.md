# Native presentation foundations

Integration base: `08cdae9fc9bd690d2927731252f40b5bc48953e5`. Section branch:
`rust/rewrite-14-native-foundations`; every subtask has its own commit.

The production composition promotes the selected Slint 1.18.1, Winit 0.30.13,
Skia 0.153.3 graph from section 2. It uses the approved royalty-free license
route, with the actual AboutSlint widget accessible through the application
menu. The qualified AccessKit 0.19.1 AT-SPI Enabled/Sensitive backport is copied
byte for byte to `third_party/accesskit-atspi-common`; its upstream, backport,
license and Chromium notices remain attached. `.config/native-toolkit.json`
seals that source tree. This does not claim product package qualification.

The process main thread owns the native event loop, window, menu and view
projection. One joined controller thread owns startup inspection and storage.
There are at most 16 pending presentation commands and one coalesced snapshot.
Shutdown has a separate atomic path so a full command queue cannot prevent
closing. The event loop remains alive until the worker finishes; storage owners
and their journal/worker cleanup complete before process exit. Constructor or
renderer failure also drops and joins the controller. No browser, HTTP bridge,
HTML renderer, detached storage task or production network call is introduced.

Embedded catalogs retain the product message identities and synthetic assets
are compiled into the native UI. Developer overrides bypass normal profiles;
existing unconfirmed overrides remain refused before window construction. The
historically admitted skeleton refusal identity remains as a regression of
that boundary, rather than a claim that the new native runtime is absent.

Section 14 supplies shared presentation behavior; complete domain workflow
views and final installed-package acceptance remain sections 15 and 18.

## Native layout adaptations

The OS owns title bars and window buttons on macOS/Linux; no simulated macOS
chrome is painted. Slint's system palette and standard controls supply native
contrast, font fallback and widget metrics. The approved labeled action rail,
separate Collections/Shelves navigation, catalog panel, central stories and
right inspector retain their hierarchy. Wide starts at 1180 logical pixels;
850–1179 retains navigation and opens details separately; below 850 navigation
is a dismissible overlay. The supported minimum is 560×400 logical pixels.
Navigation, results and inspector scroll independently without horizontal
scrolling. A toolbar disclosure remains available when navigation collapses;
Escape/backdrop restore its focus. Layout thresholds use observed window size,
not self-dependent layout constraints. Selection and query belong to the Rust
presentation owner, independently of panel visibility. Full workflow-specific
controls remain section 15.

## Bounded browsing and selection

The native list and both cover densities use the toolkit's virtualized ListView.
The model is grouped into responsive cover rows; list uses one story per row.
Only the current SQL page is projected (existing default 24; request ceiling
100, Shelf participant ceiling 25). No all-library artwork/model allocation is
introduced. Counts and page commands retain full result totals. Artwork is read
by opaque story/artwork IDs and decoded through the existing per-library bounded
thumbnail cache on the controller worker; the UI receives at most one bounded
page of RGBA previews. Images use contain fitting and captions remain outside.
Unavailable approved UUID members retain their cached title and explicit warning.
Mode changes keep selection and criteria independent and persist to the existing
library-bound native navigation file. The real-library controller test exercises
all modes, independently reads the durable preference and reopens after shutdown.
