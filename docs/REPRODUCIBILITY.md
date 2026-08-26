# Reproducibility contract

GHOSTRACE's M0 path is reproducible from versioned inputs. The Rust compiler and
Cargo components are pinned in [`rust-toolchain.toml`](../rust-toolchain.toml), the
dependency graph is frozen by [`Cargo.lock`](../Cargo.lock), and the exact input
digests and install commands are recorded in [`toolchain/manifest.json`](../toolchain/manifest.json).
The Python planning and fixture checks use only the Python standard library and
require Python 3.9 or newer.

The filesystem benchmark contract is checked offline by
`python3 scripts/filesystem-benchmark.py check`. A native measurement is a separate
macOS-only run (`python3 scripts/filesystem-benchmark.py run --profile release`);
its receipt must retain the source revision, model, OS, architecture, toolchain,
workload repetition count, latency distribution, coverage classes, duplicate/gap
counts, CPU, memory, energy status, and journal disk growth. Non-macOS builds are an
explicit no-go for the native lane, never a CI substitution.

The backpressure lane is a separate macOS-only native-safe stress test. It uses
synthetic path metadata, proves the copied callback queue never exceeds 4096
events, fills the normal writer queue without losing the emergency status slot,
and checks that an induced overflow becomes a durable `callback_queue_overflow`
gap plus `recovery_required` status. It is run with:

```sh
cargo +1.88.0 test --locked --lib \
  fsevents_collector::tests::synthetic_event_storm_backpressure_stays_bounded_and_emits_durable_gap \
  -- --exact --nocapture
```

Non-macOS builds retain an explicit no-go for this native lane.

## Snapshot pagination reproduction

The query contract is platform-neutral and runs against an in-memory or private
mode-0700 file-backed journal. Run the focused matrix with the pinned toolchain:

```sh
cargo +1.88.0 test --locked --test query_pagination
cargo +1.88.0 test --locked --lib query::tests

# Authenticated journal-state happy and negative paths (offline/private journals).
cargo +1.88.0 test --locked --test authenticated_state -- --nocapture
cargo +1.88.0 run -- authenticated-check --journal "$JOURNAL"
```

The tests use only the checked-in synthetic fixture and bounded local SQLite
operations. They retain no paths or contents. The file-backed cases verify that
new ingest is excluded by the snapshot upper bound, rows deleted after page one
are not resurrected, and a changed storage schema invalidates a token. Token
root-filter cases add matching and non-matching opaque roots, exercise page-token
continuation across filtered rows, and confirm policy-blocked summaries are
reported through coverage rather than returned as query events. The ordering and
clock matrix uses the same stable key as export.
forgery, cross-profile reuse, changed filters/page size, and expiry are negative
cases; no hosted runner is needed to substitute for this local contract.

## Install the pinned inputs

On a clean machine, install Rust through the official rustup channel manifest:

```sh
rustup toolchain install --profile minimal \
  --component clippy --component rustfmt 1.88.0
rustc +1.88.0 --version --verbose
cargo +1.88.0 --version
```

The repository does not silently update a toolchain or dependency graph. Cargo
commands that resolve dependencies use `--locked`; the smoke lane sets
`CARGO_NET_OFFLINE=true` after the pinned toolchain and lockfile have been
installed. The lockfile's SHA-256 is checked by `scripts/reproducibility.py`.

## Synthetic fixture provenance

[`fixtures/manifest.json`](../fixtures/manifest.json) is the fixture provenance
record. Every checked-in fixture is bound to:

- generator metadata `ghostrace-fixture-manifest-v1`;
- deterministic seed `ghostrace-fixture-seed-v1`;
- a format, byte length, and SHA-256 digest; and
- an explicit synthetic-only, no-user-data, offline privacy declaration.

The manifest is metadata about the canonical synthetic corpus, not a journal
export. It contains no account names, machine paths, credentials, or real event
payloads. Validate it without writing anything:

```sh
python3 scripts/fixture-manifest.py check
```

The checker rejects a missing fixture, byte drift, digest drift, changed generator
version or seed, unsafe path, schema drift, or a privacy declaration that permits
user data or network access.

The export contract registry is checked in at
[`schemas/export-registry-v1.json`](../schemas/export-registry-v1.json). Its six
golden examples are included in the same fixture manifest. The focused registry
test compiles each JSON Schema, validates its golden, rejects an injected unknown
field, and validates the export body digest without network access:

```sh
cargo +1.88.0 test --locked --test export_schema
```

The explanation counterexample fixture is included in the same manifest. Run its
focused deterministic matrix with the pinned toolchain:

```sh
cargo +1.88.0 test --locked --test explanation_determinism
```

The matrix is intentionally offline and synthetic. It records no machine paths,
accounts, or user payloads, and it does not substitute for native macOS collector
or lifecycle evidence.

The filesystem lifecycle corpus has a separate validator because its contract also
binds scenario ground truth and reporting semantics:

```sh
python3 scripts/fsevents-lifecycle-corpus.py check
```

That command is offline and deterministic. It checks all nine rows, including the
three guarded no-go rows, then replays the pinned operation projection 32 times and
reports omission, duplicate, ordering, recovery, and resource distributions. The
replay is a fixture-contract check, not a claim of native coverage. Native macOS
evidence must come from the selected-root integration test and must state the exact
device, run count, counters, retained receipt, and any no-go transition.

## Clean-machine smoke

After installing the pinned inputs, run the complete local reproduction lane:

```sh
scripts/reproducibility-test.sh
```

The lane validates the toolchain and fixture manifests, checks Rust formatting,
compares the emitted schema to the checked-in schema, runs the demo twice and
requires byte-identical explanations, initializes the durable fixture journal
twice, ingests the fixture, reopens it for two byte-identical explanations, and
checks the durable JSONL export manifest and record count. Every export first
renders a declassification preview and then passes both the plan and journal
snapshot digests back as explicit confirmation; the lane also exports the
in-memory fixture twice and requires byte-identical JSONL. It verifies that
ambient capture refuses, runs the retention dry-run twice with an explicit UTC
cutoff and requires byte-identical plans, validates its snapshot/candidate/gap
counts. It then runs the path-free residue report twice and requires byte-identical
mode/artifact inventories, validates the roadmap, runs all Python tests, and runs
locked Clippy and Rust tests. Temporary outputs are created outside the repository and
removed on exit. The durable journal path is created below a mode-0700 temporary
directory; the CLI refuses broader parent directories rather than weakening the
path boundary.

The same lane confirms an empty transactional retention deletion with all three
plan values, compares two integrity-check receipts byte-for-byte, and requires
the bounded recovery guidance to remain present. The deletion receipt is
logical-only: compaction and external-copy handling are asserted false/untouched.

The export step is complete only after `validate_export` confirms the manifest
before consuming body records; a green build alone is not sufficient evidence of
schema or digest integrity.

This procedure is local acceptance evidence. Hosted GitHub Actions may confirm
the same pinned commands, but they are not substituted for the clean-machine
reproduction record.
