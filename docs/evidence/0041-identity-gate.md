# Task 0041 evidence: project identity and package namespaces

Status: complete.

This gate records the keep-or-rename decision for the GHOSTRACE public identity
before any broadly distributed artifact. It is a research and decision artifact,
not trademark or legal clearance. The structured source is
[`planning/identity-gate.json`](../../planning/identity-gate.json); the human
summary is [`docs/IDENTITY.md`](../IDENTITY.md).

## Retained artifacts

| Artifact | Value |
| --- | --- |
| Implementation commit before merge | `GHOSTRACE-0041-CODE-4ED86A5` — `4ed86a5` |
| Implementation pull request | [#181](https://github.com/AlisinaDevelo/GHOSTRACE/pull/181) |
| Protected-main merge | `GHOSTRACE-0041-MERGED-FFA90EF` — `ffa90efc431ba08f1a942c7e302944b968e626ba` |
| Post-merge local pipeline log | `GHOSTRACE-0041-POSTMERGE-LOG-20260824` — SHA-256 `23533221d57347e54d9daf0e4b0d0b9634067c40b45ab6501cabf6413a7c0b23` |

The raw log is retained locally only. It contains platform and tool versions,
test output, and no registrant identity, serial number, UUID, username, computer
name, journal, export, account, credential, or network service data.

## Research method and observations

The audit date is 2026-08-24. Searches were exact where the provider supported an
exact endpoint, and all provider failures or zero-result responses are retained as
observations rather than interpreted as availability or ownership.

| Surface | Observation | Boundary |
| --- | --- | --- |
| GitHub | Repository search for `ghostrace` returned 73 results, including VUSec, syscall-tracing, OSINT, game, and this repository. | The repository namespace is not unique. |
| crates.io | Exact search/API requests returned HTTP 403 under the provider policy. | No absence or reservation claim. Publication remains disabled. |
| Homebrew | Exact formula URL returned HTTP 404. | Not a reservation or legal clearance. Proposed formula: `ghostrace-journal`. |
| npm | Exact registry search returned zero objects. | Not a reservation; no npm package is planned. |
| PyPI | Exact project endpoint returned HTTP 404. | Not a reservation; no Python package is planned. |
| Domains | RDAP returned registered records for `ghostrace.com` and `ghostrace.net`; no RDAP record was observed for `.org`; `.dev`, `.app`, and `.io` lookups were unavailable. | No registrant identity is retained and no domain availability claim is made. |
| Major web search | Results include GhostTrace forensic scanners, AI pentesting, a reverse-engineering workbench, an LLC, and GhostRace/GhostRacer games. | Search-facing copy uses the qualified descriptor. |

Primary collision references are the [VUSec GhostRace repository](https://github.com/vusec/ghostrace),
the [VUSec project page](https://www.vusec.net/projects/ghostrace/),
[GhostTrace](https://github.com/Devzinh/GhostTrace),
[GhostTrace AI](https://ghosttrace.ai/),
[the GhostTrace workbench](https://github.com/numaera/GhostTrace),
[GhostTrace LLC](https://ghosttrace.net/press), and the
[GhostRace game](https://lifealchemist.itch.io/ghostrace). All source URLs and
result states are machine-checked in the manifest.

## USPTO and EUIPO review

The official [USPTO Trademark Search](https://tmsearch.uspto.gov/search/) service
and [TSDR](https://tsdr.uspto.gov/) entry point were reviewed. An exact
`GHOSTRACE` wordmark request through the public TMSearch endpoint returned zero
records. The official [EUIPO search tools](https://www.euipo.europa.eu/en/search-ip),
[eSearch plus](https://euipo.europa.eu/eSearch/), and its documented basic-search
workflow were reviewed. An exact `MarkVerbalElementText=ghostrace` request
returned zero records.

Those are dated search results, not clearance opinions. They do not resolve
similar marks, goods/services overlap, common-law rights, national records,
unpublished applications, or future filings. The manifest therefore keeps both
jurisdictions at `unresolved_manual_review`, sets `legal_status` to `not_cleared`,
and requires qualified trademark counsel before release or paid naming
commitments. Exact searches must be rerun at each distribution boundary.

## Decision and identifiers

The project keeps **GHOSTRACE — local macOS causal event journal** as its qualified
descriptor, explicitly unrelated to VUSec. Distribution identifiers are selected
as follows:

| Surface | Identifier |
| --- | --- |
| Binary | `ghostrace-journal` |
| Crate | `ghostrace-journal` |
| Homebrew formula | `ghostrace-journal` |
| Bundle display | `GHOSTRACE Journal.app` |
| Bundle / reverse DNS | `com.alisinadevelo.ghostrace.journal` |

The current fixture-only development crate remains `ghostrace` with
`publish = false`. The rename is a pre-distribution migration boundary, so no
registry or marketplace identifier has been committed prematurely.

## Local verification on merged main

Source: protected-main merge `ffa90efc431ba08f1a942c7e302944b968e626ba`.

Target: MacBook Pro `MacBookPro17,1`, Apple M1, 8 cores, 8 GB; macOS `26.6.2`,
Darwin `25.6.0`, `arm64`; `rustc 1.88.0 (6b00bc388 2025-06-23)`, Cargo
`1.88.0 (873a06493 2025-05-10)`, Python `3.9.6`.

- `python3 scripts/identity-audit.py check` — pass: 8 collision sources, 2 legal jurisdictions, release identifier `ghostrace-journal`;
- `python3 scripts/reproducibility.py check` and `python3 scripts/fixture-manifest.py check` — pass;
- `cargo +1.88.0 fmt --all -- --check` — pass;
- deterministic schema, demo, and export checks — pass; repeated outputs were byte-identical;
- capture refusal — pass with the intentional live-capture gate;
- `python3 scripts/roadmap.py check` — pass: 160 tasks, 12 milestones, 488 dependency edges;
- `python3 -m unittest discover -s tests -p 'test_*.py'` — pass: 38 tests;
- `cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings` — pass;
- `cargo +1.88.0 test --locked --all-targets --all-features` — pass: 20 vertical-slice tests, 1 privacy regression, 5 support-matrix tests, and the ignored canary outside its denial lane;
- `scripts/offline-network-test.sh` — pass under macOS `sandbox-exec` for the canary, privacy fixture, and full product suite;
- `git diff --check` — pass.

The hosted checks on PR #181 were green and served only as the protected-branch
merge gate. The target-local reproduction above is the acceptance evidence.
