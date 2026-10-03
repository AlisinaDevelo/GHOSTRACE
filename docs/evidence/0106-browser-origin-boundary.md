# Browser origin digest boundary

Scope: the canonical navigation library and native-host session, not a shipped
browser collector or a migration of v1 event payloads. Issue #110 remains open.

## Contract and regression

`CanonicalNavigation::from_url` admits only bounded HTTP/HTTPS URLs through the
WHATWG parser, with explicit refusal classes for other schemes and private
contexts. The default keeps only a minimized origin. The optional path policy
keeps one short plain word or an opaque digest.

The previous path digest omitted the scheme. The initial form of
`opaque_path_digests_bind_the_complete_retained_origin` fails against production
source e506507f2067f70eecc154d82b67cff8ac1b8509: HTTP and HTTPS produce the same
digest. The correction uses the `ghostrace-navigation-path-segment-v2` domain,
a stable host-class tag, and length-framed retained-origin and segment bytes.
This changes pre-collector opaque path digests; it does not change an event,
journal, export schema or legacy `SanitizedUrl` value.

| Evidence | Test and observable outcome |
| --- | --- |
| B-origin-01 | `opaque_path_digests_bind_the_complete_retained_origin`: scheme, host, port, segment and host-class changes separate digests; case, default port, trailing-dot spelling and removed credential/query/fragment fields do not. A golden pins the v2 byte contract. |
| B-origin-02 | `every_url_class_has_an_explicit_outcome`: IDN, IPv4/IPv6, covered local/private names and addresses, schemes, invalid/opaque URLs and byte overflow have explicit outcomes. |
| B-origin-03 | `credentials_queries_fragments_and_private_markers_never_serialize`: 2,000 deterministic synthetic cases; private contexts are refused and sentinel fields are absent from JSON and origin rendering. |
| B-origin-04 | `maximum_size_navigation_has_a_bounded_shape_and_refusals_are_path_free`: 8,192-byte input yields a sub-256-byte shape for the tested maximum segment; 8,193 bytes are refused; private refusal precedes parsing, and errors contain no rejected sentinel. This is a bound test, not an allocator or performance benchmark. |
| B-origin-05 | `tests/native_host_session.rs`: pairing/MAC/protocol handling precedes canonicalization; accepted navigation is minimized, private navigation refused, and forged/replayed messages rejected. |
| B-origin-06 | `authenticated_path_policy_preserves_the_origin_digest_boundary`: authenticated optional-path messages preserve the v2 golden, separate HTTP from HTTPS and omit credential/query/fragment sentinels. |

## Retained reference-device verification

Reference environment: MacBookPro17,1 / Apple M1, 8 logical CPUs, macOS 26.6.2
(25G83), arm64, rustc 1.88.0 (6b00bc388), cargo 1.88.0 (873a06493), Python 3.9.6.
The initial candidate c2422c357b74e901fe334b7afa32ae3aad63461d passed all 19
then-present focused tests, the serial network-denied product pipeline, Clippy,
formatting, all 73 Python tests and roadmap/index/fixture checks. Subsequent
review adds the authenticated path-policy test and private-host port/IPv6 cases;
their final candidate and merged-main receipts accompany the pull request.

| Raw artifact | Result | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| `browser-origin-red.log` | Initial regression against unchanged e506507 source: expected scheme-boundary failure, exit 101 | `4acdc69ba7ba7289d458e72ddf4ec169589bb00ac162933195836eda7e448241` | 5750 |
| `browser-origin-candidate-focused.log` | Initial c2422c3 focused tests: 19 passes, exit 0 | `93f0e94968a110f3ac2d77f5c88250ff689cd59fe5cd8f0e7fa71ba7bedbc97e` | 2346 |

Raw logs are retained outside the repository. The initial concurrent broad run
failed because a default-feature build replaced the CLI binary used by the
all-feature archive test. Default/all-feature controls reproduce that collision;
serial rebuilding passes all four archive tests and the complete denied runner.
The failing run is retained, not counted as a green pipeline.

Focused reproduction:

```sh
cargo +1.88.0 test --locked --test browser_origin --test native_host_session \
  --test browser_threat_corpus --test browser_pairing
```

## Limits

- Private-network hosts are withheld and intentionally coalesce. Their digests
  cannot establish that two observations reached the same internal service.
- A deterministic digest is not encryption and does not protect guessable
  segments from dictionary attacks. The opt-in plain-word policy can retain a
  sensitive word; origin-only is the default.
- Legacy v1 fixture/event URLs still retain paths, private hosts and trailing-dot
  spelling. Their threat-corpus findings are not closed by this correction.
- Chromium/Safari collection, permission/state handling, durable projection and
  end-to-end browser privacy acceptance remain separate open work. No browser,
  network channel or additional macOS permission is enabled here.
- The authenticated optional-path test configures the host policy explicitly;
  it does not prove a browser approval flow enforces the retained-fields list.

The pull request receipt retains exact candidate and merged source, reference
device/toolchain, red/green and full local-suite results, raw log digests and
any failing or unavailable checks. Passing host tests are not browser/device
integration proof.
