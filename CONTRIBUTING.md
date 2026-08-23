# Contributing to GHOSTRACE

GHOSTRACE is a privacy-sensitive, local-first project. Contributions are welcome
when they preserve the product contract and make evidence quality more honest, not
when they broaden collection by default.

## Before you start

1. Read the [README](README.md), [privacy model](docs/PRIVACY.md), and
   [threat model](docs/THREAT_MODEL.md).
2. Check open issues and the [roadmap](docs/ROADMAP.md).
3. For a material design change, open a proposal issue before implementing it.

The initial code path is fixture-only. Do not enable ambient capture, request a new
macOS permission, add a network client, or add a sensitive field as an incidental
part of another change.

## Local setup

Install Rust 1.88 or newer:

~~~sh
rustup toolchain install 1.88.0
rustup default 1.88.0
~~~

Run the checks locally:

~~~sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
~~~

The fixture demo must not require a network connection or macOS privacy permission.
Use synthetic fixtures only; never commit a real journal, export, path, account name,
URL, or event payload.

## Making a change

- Keep the smallest coherent change in one pull request.
- Preserve deterministic output for the same fixture and event ID.
- Treat missing or unavailable source coverage as a visible gap.
- Add or update tests for parsing, policy decisions, error behavior, and privacy
  boundaries.
- Update the relevant documentation and an ADR when the data path or trust boundary
  changes.
- Keep public text factual. Omit generated-by notices, prompt traces, and automated
  co-author trailers.

For a new collector, the pull request must describe its scope, consent state,
permissions, fields retained, exclusion behavior, backpressure, restart behavior,
and failure gaps. A collector is not ready merely because it can observe an event.

## Pull requests

Use the pull request template. Explain what changed, what is deliberately not
changed, and which checks you ran. Do not attach journal databases or exports. Redact
paths and identifiers from logs before posting them.

CI runs formatting, Clippy, tests on macOS and Linux, the MSRV check, dependency
review, advisory checks, and license/source policy checks. A maintainer may request
additional platform or privacy evidence.

## Reporting security issues

Do not open a public issue for a vulnerability. Follow [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contribution is provided under the Mozilla
Public License 2.0, as described in [LICENSE](LICENSE).
