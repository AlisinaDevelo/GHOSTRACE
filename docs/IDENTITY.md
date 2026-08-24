# Project identity gate

Status: decision recorded on 2026-08-24. This document is a naming and
namespace decision record, not trademark, domain, corporate, USPTO, EUIPO, or
legal clearance. The machine-checkable source is
[`planning/identity-gate.json`](../planning/identity-gate.json), and the retained
acceptance evidence is
[`docs/evidence/0041-identity-gate.md`](evidence/0041-identity-gate.md).

## Decision

Retain the qualified public descriptor:

> **GHOSTRACE — local macOS causal event journal**

The bare name has a material collision with the VUSec GhostRace speculative
race-condition research project. This repository is unrelated and must not imply
affiliation. The descriptor is kept for continuity, while all broadly distributed
identifiers are selected with a journal qualifier:

| Surface | Selected release identifier |
| --- | --- |
| Binary | `ghostrace-journal` |
| Crate | `ghostrace-journal` |
| Homebrew formula | `ghostrace-journal` |
| Application bundle | `GHOSTRACE Journal.app` |
| Bundle / reverse-DNS identifier | `com.alisinadevelo.ghostrace.journal` |

The current fixture-only development package remains `ghostrace` with publication
disabled. Renaming the package, binary, formula, and bundle identifiers is a
release-boundary task; it must happen before the first broadly distributed
artifact, not after a registry or marketplace listing exists.

## What was observed

- VUSec uses **GhostRace** for speculative race-condition research and publishes
  the [public implementation](https://github.com/vusec/ghostrace) plus a
  [project page](https://www.vusec.net/projects/ghostrace/). The distinction is
  documented here and in the manifest; no VUSec branding is used.
- GitHub repository search for `ghostrace` returned 73 repositories, including
  `vusec/ghostrace`, `lunixbochs/ghostrace`, `chriz-3656/GHOSTRACE`, and this
  repository. The repository name is therefore not unique.
- The crates.io exact requests returned HTTP 403 under the provider's API/search
  policy. This is recorded as unavailable evidence, not as “no crate.”
- The exact Homebrew formula URL returned HTTP 404. npm exact search returned zero
  objects; the exact PyPI project endpoint returned HTTP 404. None of those results
  reserves a name or establishes legal availability.
- RDAP returned registered records for `ghostrace.com` and `ghostrace.net`.
  `ghostrace.org` had no RDAP record observed; `.dev`, `.app`, and `.io` lookups
  were unavailable from the audit environment. No registrant identity is retained
  and no domain availability claim is made.
- Major web results include [GhostTrace forensic tools](https://github.com/Devzinh/GhostTrace),
  [GhostTrace AI pentesting](https://ghosttrace.ai/), a
  [reverse-engineering workbench](https://github.com/numaera/GhostTrace),
  [GhostTrace LLC](https://ghosttrace.net/press), and a
  [GhostRace game](https://lifealchemist.itch.io/ghostrace).

## Legal review boundary

The official [USPTO trademark search](https://tmsearch.uspto.gov/search/) and
[TSDR](https://tsdr.uspto.gov/) tools were queried for the exact `GHOSTRACE`
wordmark; the observed result count was zero. The official
[EUIPO search tools](https://www.euipo.europa.eu/en/search-ip) and
[eSearch plus](https://euipo.europa.eu/eSearch/) were queried for the exact
verbal element; the observed result count was zero. These are dated search
observations, not opinions about confusing similarity, common-law rights,
classification overlap, national records, or future filings.

The legal state remains **not cleared**. Qualified trademark counsel must review
the descriptor and identifiers before a public release, registry publication,
domain purchase, or paid naming commitment. The exact searches must be rerun at
each of those boundaries.

## Usage rules

- Keep the qualified descriptor attached in public documentation and release copy.
- Never describe this project as VUSec GhostRace or suggest a technical relationship.
- Do not claim trademark, domain, USPTO, EUIPO, package, or registry clearance.
- Keep Cargo publication disabled until the release identifiers and legal review
  pass the release gate.
- Rerun GitHub, registry, domain, USPTO/TSDR, EUIPO/TMview, and major web searches
  immediately before distribution.
