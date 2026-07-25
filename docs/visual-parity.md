# Canonical visual parity

The normative visual source is
`openspec/ui-prototypes/05-archive-shelves.html` inside its `.app` element.
The `.back` gallery link is removed before comparison.

`npm --prefix frontend run test:visual` builds the production frontend with
the deterministic `?fixture=parity` data, renders both the canonical HTML and
the application in the same system Chromium process, and compares viewport
screenshots at 1181×900, 1180×900, 850×900, and 560×900.

Pixel comparison uses Pixelmatch with a per-pixel threshold of `0.1` and
accepts at most a `0.001` mismatch ratio (0.1% of viewport pixels). A changed
tolerance in `frontend/scripts/frontend-visual-parity.mjs` is a visual contract
change and must be reviewed with the prototype and its checksum.
