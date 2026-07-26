# Canonical visual parity

The normative visual source is
`openspec/ui-prototypes/05-archive-shelves.html` inside its `.app` element.
The `.back` gallery link is removed before comparison. Because OpenSpec
working files are not published with the application repository, CI renders
the checksum-locked mirror at
`testdata/ui-prototypes/05-archive-shelves.html`.

`npm --prefix frontend run test:visual` builds the production frontend with
the deterministic `?fixture=parity` data, renders both the canonical HTML and
the application in the same system Chromium process, and compares viewport
screenshots at 1181×900, 1180×900, 850×900, and 560×900.

Pixel comparison uses Pixelmatch with a per-pixel threshold of `0.1` and
accepts at most a `0.001` mismatch ratio (0.1% of viewport pixels). A changed
tolerance in `frontend/scripts/frontend-visual-parity.mjs` is a visual contract
change and must be reviewed with the prototype and its checksum.

## Release gate

Run `make test-frontend-release`. It verifies the prototype checksum, component
interactions and focus behavior, exact responsive contracts, screenshot parity,
WCAG 2.1 A/AA semantics and contrast in preference-aware high-contrast mode,
reduced motion, logical tab order, and non-color-only labels. `make check`
includes this release gate.

An approved visual change must update all of these in one reviewed change:

1. the normative prototype `.app` subtree;
2. the tracked test mirror and its SHA-256 in `prototype_contract_test.go`,
   `parity-fixture.ts`, and the accepted architecture baseline;
3. the applicable OpenSpec design, requirements, and task artifacts;
4. the deterministic parity fixture and any intentionally changed comparison
   tolerance or responsive acceptance values.

The prototype rendered at test time is the screenshot baseline. There are no
detached golden images that can silently diverge from it. Changing only the
application, fixture, checksum, or tolerance leaves at least one release gate
red.
