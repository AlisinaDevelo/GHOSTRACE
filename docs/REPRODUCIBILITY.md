# Reproducibility contract

GHOSTRACE's M0 path is reproducible from versioned inputs. The Rust compiler and
Cargo components are pinned in [`rust-toolchain.toml`](../rust-toolchain.toml), the
dependency graph is frozen by [`Cargo.lock`](../Cargo.lock), and the exact input
digests and install commands are recorded in [`toolchain/manifest.json`](../toolchain/manifest.json).
The Python planning and fixture checks use only the Python standard library and
require Python 3.9 or newer.

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

## Clean-machine smoke

After installing the pinned inputs, run the complete local reproduction lane:

```sh
scripts/reproducibility-test.sh
```

The lane validates the toolchain and fixture manifests, checks Rust formatting,
compares the emitted schema to the checked-in schema, runs the demo twice and
requires byte-identical explanations, initializes the durable fixture journal
twice, ingests the fixture, reopens it for two byte-identical explanations, and
checks the durable JSONL export manifest and record count. It also exports the
in-memory fixture twice and requires byte-identical JSONL, verifies that ambient
capture refuses, validates the roadmap, runs all Python tests, and runs locked
Clippy and Rust tests. Temporary outputs are created outside the repository and
removed on exit. The durable journal path is created below a mode-0700 temporary
directory; the CLI refuses broader parent directories rather than weakening the
path boundary.

This procedure is local acceptance evidence. Hosted GitHub Actions may confirm
the same pinned commands, but they are not substituted for the clean-machine
reproduction record.
