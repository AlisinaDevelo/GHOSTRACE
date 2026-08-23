## Summary

Describe the change and the user-visible contract it preserves or adds.

## Privacy and trust boundary

- [ ] This change does not add a network client, telemetry, cloud sync, URL
      fetching, silent upload, or broad permission.
- [ ] Any new field, source, permission, or export behavior is documented.
- [ ] Rejected or unavailable data remains bounded and does not enter diagnostics.
- [ ] Gaps and source limitations remain visible.
- [ ] No real journal, export, path, URL, account name, or secret is included.

## Verification

- [ ] cargo fmt --all -- --check
- [ ] cargo clippy --all-targets --all-features -- -D warnings
- [ ] cargo test --all-targets --all-features
- [ ] Documentation and ADRs updated where the boundary or contract changed.

Checks not run and the reason:

## Release and compatibility

Describe schema, fixture, CLI, or migration compatibility impact. If this is a
roadmap-only or documentation change, say so explicitly.
