# Product-contract inventory

The pinned source has 436 tracked files. `source-inventory.json` accounts for every source, test, document, fixture, screenshot and build input with exact hashes. `parity-register.json` carries all 60 initial behavior rows, source evidence, retained behavior, proposed Rust check IDs, responsible workstream, accountable owner and separate unverified A/I/L/W states. Section owners must replace the routing owner with their concrete implementation checks before claiming a pass.

`command-inventory.json` records every exported App method (79) with its full signature and source location. `ui-inventory.json` records all 256 JSX interaction bindings, 34 bridge imports and 7 browser-state calls through the TypeScript parser. The callback expressions are preserved, including delegated component actions. Keyboard listeners, CSS-only states, help text, frontend test assertions and markup without callbacks remain in the complete file inventory; JSX counts alone are not a coverage claim. `test-inventory.json` indexes 388 Go test entrypoints; execution is recorded separately in `baseline.json`.

`payload-inventory.json` enumerates 482 JSON-tagged Go fields, including transient responses and diagnostic/operation models. `schema-inventory.json` inventories the actual schema-018 tables, columns, indexes and triggers. `state-and-adapters.json` explains the independent settings and relocation authority, localStorage keys, typed browser navigation state, stable operation/export discriminators, runtime events and OS adapters. Persisted schema/profile/query state remains authoritative; transient DTO fields do not automatically become new persistent preferences.

Inventory links can cover a complete module and multiple parity IDs. They are discovery routes, not proof that every case has executable evidence. Follow the precise source/test references and the retained behavior row before porting a case. No row in this inventory marks Rust complete.

Current behavior overrides stale wording in historical feature records: Broken is visible/counted everywhere and always skipped on export; cards show warnings while details/bulk controls mutate the independent flag; protection fields remain internal compatibility data; ordinary removal and explicit exclusion differ. Historical prototype checksums/screenshots remain preserved. The separate spec reconciliation record identifies superseded wording and intentional native-rendering differences.

Rebuild syntax inventories with:

```
go run preservation/tools/inventory.go <readonly-oracle> > <scratch-go.json>
node preservation/tools/ui-inventory.cjs <disposable-oracle-with-locked-frontend-dependencies> > <scratch-ui.json>
python3 preservation/tools/assemble-inventory.py <separate-spec-root> <scratch-go.json> <scratch-ui.json>
```

The assembler writes only inventory JSON beside this document. Use the exact oracle SHA and the recorded spec digests; do not replace evidence with a moving task tree. The full source digest manifest must match before capture. These parser commands do not execute the project or contact production services.
