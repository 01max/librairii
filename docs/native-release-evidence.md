# Assemble native release evidence for review

Use this after collecting the three targets' package and CI archives. The
offline helper writes a manifest keyed by the embedded source commit, target
and **inner distribution SHA-256**. It checks selected archive hashes, source
bindings, production build records and retained installed payload bytes.

The manifest is a review index. Provider authentication, raw behavior and
numerical acceptance, parity reconciliation and release approval require
independent review. Its `provider_authenticated`, `acceptance_verified` and
`public_release_qualified` fields always remain false, including when signed
packages are selected. The helper performs no network access, extraction,
package execution, signing, credential access, publication or cutover.

## Collect and select the evidence

Keep the original provider ZIPs, their provider metadata and all failed attempts.
For each target collect:

| Input | Retained workflow archive |
| --- | --- |
| `candidate` | Native package build: `stage/` compiler/source records and `package/` distribution plus `artifact.json`. |
| `reports.quality` | Full native quality archive, including `quality/source.txt` and native UI manifest. Preserve its complete recovery, handover and Linux spoken records. |
| `reports.scale` | Full scale archive with backend, desktop and resource manifests. |
| `reports.installed` | Installation inspection, actual installed payload and `gui/qualification.json` for the selected distribution. |

Required targets are ARM macOS, real Intel macOS and Linux x86-64. A complete
selection includes all three. Use the **embedded build source**, which can
differ from a PR head or later merge commit. The source, source tree, product
version, toolchain, lockfile, toolkit pin and shipping features must agree.

Select exact provider artifact IDs. Retries can retain multiple archives with
the same name; names alone do not select an attempt. Independently obtain each
ZIP's SHA-256 and byte count from its provider metadata. These differ from the
inner DMG/tar.gz checksum. The helper checks the supplied values against local
bytes; it does not authenticate the provider or fetch missing evidence.

## Fill the input plan

Copy [the input template](examples/native-evidence-input.json) into the directory
holding your downloaded ZIPs. Replace every placeholder and every zero value:

- `source_commit` and `source_tree`: exact lowercase 40-character Git identities
  from the selected package records.
- Each archive record: its relative `path`, positive provider `artifact_id` and
  `run_id`, ZIP `bytes` and lowercase 64-character `sha256`.
- `limitations`: the actual scope decisions and remaining release gaps.
- `retained_records`: checksummed local reviews, parity/recovery reports,
  workflow metadata, failures and diagnostics, including prior attempts.

Paths are relative to the input plan's directory and must remain inside it.
The manifest is written beside the plan so the bundle stays portable. Keep the
plan, manifest and all referenced files together.

For example, attach an original failed run without changing its verdict:

```json
{
  "kind": "failure",
  "conclusion": "failed",
  "file": {
    "path": "original-failure.log",
    "bytes": 1234,
    "sha256": "REPLACE_WITH_EXACT_FILE_SHA256"
  }
}
```

Record kinds are `failure`, `diagnostic`, `review`, `parity`, `recovery` or
`workflow`. Conclusions are `passed`, `failed`, `cancelled`, `incomplete` or
`diagnostic`. These classifications are retained as supplied; attaching a
review does not make the helper approve its conclusions.

## Write a new manifest

From a checkout containing the helper, run:

```bash
python3 -I -B scripts/assemble-native-evidence.py \
  --input target/native-release/review/input.json \
  --output target/native-release/review/manifest.json
```

The helper streams checksums without unpacking or launching a package. It
refuses missing target/archive selections, changed input files, duplicate
JSON/ZIP identities, mixed sources/graphs, incomplete GUI/scale status records
and mismatched distribution or retained installed bytes. Quality checking is
limited to the source record and native UI manifest's source/target binding;
full quality completion and raw assertions require independent review.
Existing output files are refused;
choose a new manifest name for another selection. Validation errors create no
manifest. An I/O failure while writing can leave a partial new output, which
should be retained as a failed attempt and never used for acceptance.

The output includes exact provider IDs, ZIP hashes, metadata member hashes,
inner distribution hashes, source/configuration and target baselines, recorded
signing state, observed host records, attached evidence and the assembler's
own source-file hash. Read the indexed raw assertions, logs and screenshots;
the helper does not recompute behavioral/numerical gates, authenticate a
signature or replace an independent release rehearsal.

## Add final signed Mac bytes when ready

First follow [the manual build/signing guide](native-build-and-sign.md). Keep
each original unsigned candidate. After you have signed and obtained the
separate matching-host installation/GUI evidence, add a `signed_candidate`
archive record to that Mac target, using the same five fields as `candidate`.
It selects the protected workflow's ZIP containing root `artifact.json` and
the signed DMG. Point `reports.installed` at that signed package's final-byte
installation archive.

The signed record must name the selected unsigned DMG hash and retain the same
source/configuration. Its actual retained installed payload must match the
signed inventory. Earlier unsigned installation evidence is refused for the
changed distribution. Recorded signing fields are indexed for review; the
manifest still does not grant signature, notarization or release acceptance.

Owner review of the final signatures/tickets, exact final-byte host evidence,
every mandatory parity row and the complete rehearsal remains required by
[Task 19](rewrite/section-19.md). Publication and runtime cutover are separate
decisions. The Go runtime and frozen evidence remain available throughout.
