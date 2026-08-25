# GHOSTRACE

[![CI](https://github.com/AlisinaDevelo/GHOSTRACE/actions/workflows/ci.yml/badge.svg)](https://github.com/AlisinaDevelo/GHOSTRACE/actions/workflows/ci.yml)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![MSRV: 1.88.0](https://img.shields.io/badge/MSRV-1.88.0-informational.svg)](rust-toolchain.toml)
[![Status: fixture-only](https://img.shields.io/badge/status-fixture--only-orange.svg)](docs/ROADMAP.md)

GHOSTRACE is a local macOS causal event journal. It records bounded, user-authorized
evidence about changes—not everything a person does—and explains which observations
support each causal link.

> **Status:** M0, fixture-only developer headstart (0.0.1). Live capture is
> intentionally disabled until consent and policy enforcement, cursor recovery, a
> bounded writer, and production Keychain encryption are complete. This repository
> makes no legal chain-of-custody claim.

## Ten-minute demo

The current vertical slice is offline and fixture-driven. It does not ask for macOS
permissions, start a collector, contact a service, or upload data.

~~~sh
git clone https://github.com/AlisinaDevelo/GHOSTRACE.git
cd GHOSTRACE
rustup toolchain install 1.88.0
cargo +1.88.0 build

# Print the versioned event contract.
cargo +1.88.0 run -- schema

# Select the terminal event so the explanation includes the full chain and its gap.
EVENT_ID="00000000-0000-4000-8000-000000000008"

# Initialize a private, durable fixture journal. The parent directory must be
# private (mktemp -d creates one with mode 0700).
JOURNAL_DIR="$(mktemp -d)"
JOURNAL="$JOURNAL_DIR/journal.sqlite3"
cargo +1.88.0 run -- init --journal "$JOURNAL"

# Ingest the checked-in synthetic fixture into SQLite, then explain it after
# reopening the journal in a separate process.
cargo +1.88.0 run -- ingest \
  --journal "$JOURNAL" \
  --fixture fixtures/causal-chain.jsonl
cargo +1.88.0 run -- explain \
  --journal "$JOURNAL" \
  --event "$EVENT_ID"

# Export a user-requested, local JSONL view. Existing files are protected.
cargo +1.88.0 run -- export \
  --journal "$JOURNAL" \
  --output /tmp/ghostrace-export.jsonl

# The baseline refuses ambient capture by design.
if cargo +1.88.0 run -- capture; then
  echo "capture unexpectedly succeeded" >&2
  exit 1
else
  echo "capture refused as expected"
fi
~~~

The demo output labels evidence as direct, contextual, inferred, or unknown, and
surfaces gaps instead of filling them with a guess. The same fixture and event ID
produce the same explanation after a process restart. `demo --fixture ...` remains
available as an in-memory shortcut. The durable CLI path uses a deterministic
synthetic key only for this fixture-only headstart; it is not a production
encryption or key-management claim.

## What is shipped now

| Surface | M0 status |
| --- | --- |
| Fixture JSONL ingestion and validation | Available for the developer headstart |
| ghostrace init --journal <path> | Available; creates an idempotent durable fixture journal |
| ghostrace ingest --journal ... --fixture ... | Available; persists a checked-in fixture batch |
| ghostrace explain --journal ... --event <uuid> | Available; deterministic after reopen |
| ghostrace demo --fixture ... --event <uuid> | Available |
| ghostrace export --journal ... --output ... [--force] | Available |
| ghostrace export --fixture ... --output ... [--force] | Available in-memory shortcut |
| ghostrace schema | Available |
| ghostrace capture | Refuses by design |
| Local journal and bounded durable writer | Scaffolded for the fixture path; live ingestion is gated |
| FSEvents, shell, Git, frontmost-app, or browser collectors | Not shipped |
| macOS Keychain-backed production encryption | Not shipped |
| Signed/notarized release artifacts | Not shipped |

The roadmap is a plan, not a promise. A capability is shipped only when its privacy,
failure, and coverage tests are present.

## Architecture

The causal path is deliberately small:

~~~text
fixture JSONL (now) ─┐
                     ├─> normalization ─> deny-by-default policy
opt-in source (later)┘                         │
                                               v
                                  versioned event + provenance
                                               │
                                               v
                                  bounded SQLite WAL writer
                                               │
                                               v
                           deterministic explain / explicit export
~~~

Each accepted event carries source facts, timestamps, policy context, and evidence
quality. A gap is a first-class record. Explanations cite the observations they use
and state when the source cannot establish completeness. See
[Architecture](docs/ARCHITECTURE.md) and [Event model](docs/EVENT_MODEL.md).

Journal ingestion also requires a typed adapter-origin capability. Fixture, live,
import, and repair paths own separate provenance namespaces and allowed event classes;
deserializing a fixture cannot grant a live-collector capability.

## Trust contract

GHOSTRACE is designed around a narrow local boundary:

- **Local-only:** source inspection of current product and runtime paths finds no
  network client, telemetry, cloud sync, URL fetching, or silent upload path. Task
  0044 must make that boundary independently enforceable in CI before it becomes
  release evidence. The separate maintainer-only roadmap synchronizer invokes `gh`
  only when an operator explicitly runs its GitHub commands.
- **User-authorized:** future collectors require explicit consent, selected scope,
  and a versioned policy. No event is retained before policy evaluation.
- **Minimized:** the baseline records bounded metadata about changes. It does not
  read file contents as part of the filesystem design.
- **Honest evidence:** direct, contextual, inferred, and unknown evidence levels are
  distinct. Missing coverage is visible.
- **Fail closed:** ghostrace capture refuses while the gates for consent, cursor
  recovery, bounded writing, and Keychain protection are incomplete.
- **Inspectable:** exports are explicit commands. Existing destinations are not
  overwritten unless --force is supplied.

The initial product does **not** use keylogging, microphones, screen recording,
clipboard capture, window titles, page contents, or private-browsing data by default.
It does not require root, Full Disk Access, Accessibility, or Automation permissions.
No silent upload mechanism exists in the current source; the planned network-denial
CI lane will continuously verify that boundary.

FSEvents is a change-notification source, not a complete process-attributed causal
trace. It can omit, coalesce, reorder, or delay observations. Endpoint Security is
optional and deferred; it is entitlement-gated and will not silently become part of
the baseline.

## Non-goals

GHOSTRACE is not an employee-monitoring product, keylogger, screen recorder, content
indexer, browser-history default, malware detector, remote agent, cloud analytics
service, or legal evidence-preservation system. It does not infer intent from absent
events and does not claim that a recorded sequence proves causality.

## Project layout

~~~text
src/                 Rust library and CLI
fixtures/            Synthetic, non-user event fixtures
docs/                Architecture, privacy, threat, platform, and research notes
docs/adr/            Immutable architecture decisions
.github/             CI, dependency checks, issue forms, and contribution templates
~~~

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — data path, boundaries, and failure behavior
- [Privacy](docs/PRIVACY.md) — data inventory, defaults, consent, and export rules
- [Threat model](docs/THREAT_MODEL.md) — assets, STRIDE analysis, and residual risk
- [Event model](docs/EVENT_MODEL.md) — evidence levels, provenance, and gaps
- [Evaluation](docs/EVALUATION.md) — correctness, privacy, and performance gates
- [Reproducibility](docs/REPRODUCIBILITY.md) — pinned toolchain, fixture provenance, and clean-machine smoke
- [Research](docs/RESEARCH.md) — landscape, differentiation, and primary sources
- [Identity gate](docs/IDENTITY.md) — qualified descriptor, release identifiers, and legal-review boundary
- [Platform](docs/PLATFORM.md) — macOS boundary and permission policy
- [Roadmap](docs/ROADMAP.md) — 160 tasks across M0 through M11, August 2026–December 2031
- [ADR 0001](docs/adr/0001-local-only-minimized-capture.md) — local-only minimized capture
- [ADR 0002](docs/adr/0002-fsevents-before-endpoint-security.md) — FSEvents before Endpoint Security
- [ADR 0003](docs/adr/0003-sqlite-wal-active-journal.md) — SQLite WAL active journal

For how to change the project, see [CONTRIBUTING.md](CONTRIBUTING.md). For a suspected
vulnerability, see [SECURITY.md](SECURITY.md). Questions and feature proposals belong
in the issue forms, not in a journal attachment.

## Development checks

Use the pinned toolchain and run the core checks exercised by CI:

~~~sh
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.88.0 test --locked --all-targets --all-features
python3 scripts/roadmap.py check
python3 scripts/reproducibility.py check
python3 scripts/fixture-manifest.py check
python3 -m unittest discover -s tests -p 'test_roadmap.py' -v
python3 scripts/roadmap.py index > /tmp/ghostrace-roadmap-index.md
diff -u .forge/tasks/README.md /tmp/ghostrace-roadmap-index.md
scripts/reproducibility-test.sh
~~~

The fixture-only path should remain offline. Do not add a network dependency, a
permission request, a new sensitive field, or a collector without updating the
privacy and threat documentation and adding regression coverage.

## License

GHOSTRACE is licensed under the [Mozilla Public License 2.0](LICENSE). Third-party
dependencies retain their own licenses; dependency policy is checked in
[deny.toml](deny.toml).
