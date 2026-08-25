# Task 0080 evidence: bounded evidence-claim grammar

Status: implementation, review, merge, and protected-main device reproduction
complete.

Implementation PR [#283](https://github.com/AlisinaDevelo/GHOSTRACE/pull/283)
was merged to protected `main` at
`aa01b23985a7a47da86cd1d2464cc35cac2a4c29` on 2026-08-25. The reviewed
implementation commit before squash was
`192781f6dea7edfe8297a9a5c49aa0083a580380`; the protected-main commit is the
acceptance source for this record. The issue is closed only after this evidence
change is merged and linked.

## Contract and acceptance mapping

Claim grammar version `1` is defined in `src/claims.rs`. A static descriptor is
present for each supported event kind. Every descriptor declares its required
facts, prohibited implications, evidence requirement, and gap behavior. The
renderer carries the grammar version, template identifier, locale, evidence
level, gap behavior, and cited event UUIDs in structured output as well as in
the human-readable statement.

| Acceptance criterion | Implementation and retained proof | Result |
| --- | --- | --- |
| Each template declares required facts, prohibited implications, evidence level, and gap behavior. | `ClaimTemplateId` maps all 12 supported event kinds to descriptors containing `RequiredFact`, `ProhibitedImplication`, `EvidenceRequirement`, and `GapBehavior`. `tests/claim_grammar.rs::every_template_declares_required_facts_prohibitions_and_gap_behavior` enumerates the catalog and rejects incomplete descriptors. | PASS |
| No template asserts intent, completeness, process attribution, or old-to-new rename identity without the required direct source. | `render_claim` applies a fail-closed lexical validation for intent, completeness, process attribution, unsupported causality, and rename-identity language. The rename fixture explicitly renders that an old-to-new identity is not established. `tests/claim_grammar.rs::rename_never_invents_old_to_new_identity` and `::localized_rendering_preserves_evidence_and_citations` cover the negative cases. | PASS |
| Localization and rendering tests preserve claim meaning and cited event identifiers. | `ClaimLocale::En` and `ClaimLocale::EnGb` share the same descriptor, evidence label, gap behavior, template, and citations. `tests/claim_grammar.rs::localized_rendering_preserves_evidence_and_citations` compares both locales and checks the event UUID and evidence level; `::gap_behavior_and_explanation_integration` verifies the explanation API carries grammar metadata and citations. | PASS |

The tests use synthetic, offline events and a sensitive fixture string to prove
that the claim renderer does not disclose unsupported secret content. They do
not claim causal completeness, intent, process attribution, or rename identity.

## Protected-main device reproduction

The focused claim-grammar matrix and the full product suites were rerun from
protected `main` `aa01b23985a7a47da86cd1d2464cc35cac2a4c29` on the named device.
Every command exited 0.

| Fact | Recorded value |
| --- | --- |
| OS | macOS 26.6.2 (25G83), Darwin 25.6.0 |
| Hardware | MacBookPro17,1, Apple arm64 (M1), `aarch64-apple-darwin` |
| Rust/Cargo | rustc/cargo 1.88.0 |
| Python | 3.9.6 |
| Source | protected `main` `aa01b23985a7a47da86cd1d2464cc35cac2a4c29` |
| Date | 2026-08-25 |

Commands:

```text
cargo +1.88.0 test --locked --test claim_grammar --test vertical_slice -- --nocapture
cargo +1.88.0 test --locked --release --test claim_grammar --test vertical_slice -- --nocapture
cargo +1.88.0 test --locked --all-targets --all-features
cargo +1.88.0 test --locked --all-targets --all-features --release
```

| Lane | Result | Log SHA-256 |
| --- | --- | --- |
| protected-main focused debug claim grammar + vertical slice | 4 + 28 passed, exit 0 | `fd5dfb9621db0a9a2d92f6d4f8d3d50681b9bef53edb6a2b71b4202caec0d3fa` |
| protected-main focused release claim grammar + vertical slice | 4 + 28 passed, exit 0 | `8e95029de40c6a77cbf772df5b7e645751a5c92625a73c6d0b690db7347a6a65` |
| protected-main all-target/all-feature debug | all targets passed, exit 0 | `d56095678e75abc883d74ef8e135bf6071cef0023e4c9164af86d50190d00b2c` |
| protected-main all-target/all-feature release | all targets passed, exit 0 | `32d242f688c8944f2b01fa1c93188855d0e95775650c9b789fa6a6e696d55001` |

The suites use private temporary journals and checked-in fixtures. They retain
no paths, file contents, account names, token plaintext, network payloads, or
user timing data.

## Local implementation pipe

Before merge, the implementation branch ran the local pipe on the same device.
Every required lane exited 0. The focused tests cover the new grammar and the
existing vertical slice; no expected-red result is claimed in this record.

| Lane | Log SHA-256 |
| --- | --- |
| all-target/all-feature debug Rust tests | `bf5f1156a6e623b18737dac209415995af3bcb6b2fa7f2d6514bbe7649b6d083` |
| all-target/all-feature release Rust tests | `9ee21e6ad9766a047d41ba8448e7e952db90fe4fce549d54300d3d6514d4fafa` |
| `cargo +1.88.0 fmt --all -- --check` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| all-target/all-feature Clippy with warnings denied | `baa2bff20ddd38181555898bbca40c18681ef116d25ac915973a4e28c76df7d7` |
| Rustdoc with warnings denied | `d63b7707a5b4efd525ac5934b0388227ffa3411321767f0681a8ba58765c7aa8` |
| Python unit suite (46 passed) | `b4dec70450367ca25b67e4e76643eed6c140fa62d2e0beb2267f35cadedf51a2` |
| fixture, roadmap, and release-evidence validators | `c1eae3e2ce088b109a984a989f91c135da1867de80e79380fcaa3dee32629028` |
| reproducibility contract check | `bdc6d6d5b06ec05059da93dc472c92384372f84fcfd0dca7e1476250a16b02c4` |
| offline network-denied product pipe | `df0362fd9008df6c23bba7841ab0bb73a516772a1b96ba915e94c7047495d16b` |
| FSEvents sanitizer lane | `2340c0dcdd4c90b3cbb8984a102e79483bcc923cdf898fedf588ac2ddc686ba3` |

The reproducibility pipe reran deterministic demo, durable reopen/export,
schema, capture-refusal, roadmap, privacy, and all Rust target lanes. The
offline lane enforced network denial before the privacy and product suites. The
sanitizer lane completed with its documented suppressions and no sanitizer
finding.

## Final protected-main validators

The merged-main checkout also passed the non-Cargo contracts and documentation
lanes:

| Lane | Log SHA-256 |
| --- | --- |
| protected-main `cargo +1.88.0 fmt --all -- --check` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| protected-main Clippy | `1a587aecfe04173026cbb6482a163337c770fe4b13679d599781d0abc37a6046` |
| protected-main Rustdoc | `50fbeca9cfdfdaf4787879e31d0689866745e1f0c45e2f5dae9e03739e41cfb3` |
| protected-main Python suite (46 passed) | `259ec25ff297b1d24d16f27090c6a80fc6ef9dac2fce5c06de39d4b98d0f6481` |
| protected-main fixture, roadmap, release-evidence, and contracts | `c1eae3e2ce088b109a984a989f91c135da1867de80e79380fcaa3dee32629028` |
| protected-main offline-network product pipe | `4675cd19d1c9a330abc43128e44d86a0ab051b44cfd5c7440b097b2d5a04215c` |
| protected-main sanitizer | `34ce167cf0b22475c7535401eb85cac4fc67fb8e1bd8e462df139f1e50269585` |
| protected-main reproducibility pipe | `b2cbdef7b269771efa9099a14fd9170117e9b646b2116b3446bf95179ef2df1e` |

The final reproducibility run reported `reproducibility: all checks passed`.

## Hosted review and merge

PR #283 passed duplicate hosted runs for rustfmt, Clippy, Linux stable, Linux
MSRV, macOS stable, offline fixture, Cargo policy, advisories, dependency
review, and roadmap before the protected-main merge. Hosted checks were review
gates only; the device pipe and post-merge device matrix above are the
acceptance evidence.

## Boundaries and limitations

- Grammar version `1` supports the bounded `en` and `en-GB` locales; adding a
  locale requires a descriptor/rendering test that preserves evidence labels,
  template identity, gap behavior, and cited event IDs.
- Claim text is intentionally conservative and fail-closed. It does not infer
  intent, completeness, process attribution, unsupported causality, or an
  old-to-new rename identity from correlation or absence.
- The tests are offline and synthetic. No interactive sleep, logout, volume
  detach, or live Keychain lifecycle was performed; the Keychain lifecycle
  tests remain explicitly ignored because they require separate authorization.
- This task does not close the M3 aggregate gate or claim native collector
  completeness, causal reconstruction, or release readiness.
