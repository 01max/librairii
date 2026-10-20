# Frozen Go product contract

This directory is section 1 of `rewrite-in-rust-native-desktop`. It preserves the working product as an oracle; it does not qualify a Rust implementation or any new platform. The separate OpenSpec repository remains separate and is never vendored as a nested Git repository.

Start with [evidence-index.json](evidence-index.json), [oracle.json](oracle.json), and the contract inventories. Every P01–P60 row remains unverified in Rust on macOS ARM, real Intel macOS, Linux x64 and Windows x64 until its own implementation evidence passes. Baseline observations are scoped to their actual host and command. Missing GUI/hardware resources remain open gates.

The immutable shared oracle is an independent clone at the exact SHA in `oracle.json`, outside AO task worktrees. Never develop, install dependencies, build, generate or run tests there. Use a disposable independent clone; verify tracked source hashes before and after execution. The fixture exporter must link to that frozen source, not a task branch containing later behavior changes. Keep the oracle until cutover and preserve the commit and source manifest afterward.

All fixture content is synthetic. Generation and smoke checks use temporary explicit library/config roots and local fixtures. Never supply a user library/profile or production catalog endpoint. Existing production sync configuration is preserved as source evidence; generation does not invoke it.

Subtask commits are recorded in the evidence index. A commit cannot contain its own hash, so each ledger update records preceding commits and a final evidence-only commit closes the ledger. Review fixes get separate commits and do not rewrite subtask history. The [independent 1.9 audit](independent-review.md) passed after its findings were addressed. External AO/GitHub review and provider checks on the resulting final head remain the admission authority. No merge is performed by this worker.
