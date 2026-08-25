# Task 0063 evidence: memory-safe FSEvents stream lifecycle adapter

Status: complete for the bounded native stream lifecycle contract. This work
does not enable live collection, consent, selected-root policy, attribution,
cursor persistence, or journal writes.

## Scope and implementation

Implementation [PR #209](https://github.com/AlisinaDevelo/GHOSTRACE/pull/209)
was authored on `feature/ghostrace-0063-fsevents-lifecycle` and merged to
protected `main` with the repository's allowed squash strategy at
`14846b78076e56ab4e4489d73edd39eeb5fe302c` on 2026-08-25 at 02:53:53Z. The
final PR head was `55bc3ad077505209e9f5744f5f0f613d5f69f1f2`; the branch also
contains the portability fixes that made Linux warnings fail closed rather than
being suppressed.

`src/fsevents.rs` owns one `FSEventStreamRef` and one boxed callback context on
macOS. `FseventsStream` is deliberately `!Send`/`!Sync`, records its owner
thread, requires explicit run-loop scheduling before start, and exposes typed
created/scheduled/running/stopped/invalidated states. Drop performs the native
shutdown fence in order: stop if running, invalidate if scheduled, release once,
then reclaim the callback context. The raw callback copies bounded C-string
paths, rejects null/oversized batches and paths, contains user panics, and
counts malformed batches and panics. CFType, extended-data, full-history, and
document-ID callback modes are rejected before native creation because they need
a different pointer representation.

The non-macOS backend is an explicit `UnsupportedPlatform` no-go. The Linux
configuration is still compiled warning-free and is covered by the hosted Linux
MSRV/stable checks; no Linux native watcher is claimed.

## Acceptance mapping

| Acceptance criterion | Evidence |
| --- | --- |
| Callback pointers, context ownership, panic containment, and shutdown ordering are documented and tested | `src/fsevents.rs`, `docs/ARCHITECTURE.md`, `docs/PLATFORM.md`, ADR 0004, the callback copy/panic/malformed-batch test, and the merged native integration test |
| Start, stop, restart, and flush follow an explicit state machine | `LifecycleController` mock tests plus `tests/fsevents_lifecycle.rs` assert not-scheduled, duplicate schedule/start/stop, restart, flush, invalidation, and terminal-state errors |
| Partial initialization releases exactly once | Mock schedule-failure and start-failure tests assert exact native call sequences; idempotent invalidation asserts one invalidate/release pair |
| Thread/run-loop assumptions are asserted | `!Send`/`!Sync`, owner-thread checks, current-run-loop scheduling and driving in `native_stream_observes_metadata_and_restarts_on_one_run_loop_thread` |
| Unsafe callback representations are fail-closed | `UnsupportedCallbackFlags` validation tests cover CFType, extended-data, full-history, and document-ID modes; bounded null/oversized callback tests cover malformed input |
| Sanitizer coverage exists for the native callback path | Merged-main AddressSanitizer receipt below; no ASan finding, one native integration test passed |

## Merged-main device verification

The authoritative reproduction ran from clean protected main at
`14846b78076e56ab4e4489d73edd39eeb5fe302c` on the target device:

| Field | Value |
| --- | --- |
| Device | MacBook Pro 17,1; Apple M1; 8 GB; arm64 |
| OS | macOS 26.6.2 (25G83); Darwin 25.6.0 |
| Stable toolchain | Rust/Cargo 1.88.0; `aarch64-apple-darwin`; LLVM 20.1.5 |
| Full merged-main pipe | `/private/tmp/ghostrace-0063-merged-full-v1.log` |
| Full merged-main SHA-256 | `b318d37147acc70afe8ceae92f5597c0c255d8beee08d4cb8a4af056122f9f7c` |
| Focused post-merge receipt | `/private/tmp/ghostrace-0063-postmerge-device-v1.log` |
| Focused receipt SHA-256 | `0359beb0bea720211977cba3933e39ae2162d7cd2a5adf974d055559ac27f0d5` |

The full pipe used one build job, locked offline dependencies, format checking,
all-target/all-feature check and Clippy with `-D warnings`, all debug tests,
doctests, the real macOS FSEvents integration test, fixture/identity/release
evidence/roadmap checks, 38 Python tests, deterministic reproducibility,
network-denial sandbox, ShellCheck, actionlint, and final diff checks. It ended
`GHOSTRACE_0063_MERGED_FULL_V1_PASS` with exit 0. The focused receipt independently
reran the native integration and seven FSEvents lifecycle unit tests at the same
merge SHA.

## Merged-main sanitizer verification

| Field | Value |
| --- | --- |
| Sanitizer toolchain | Rust/Cargo 1.100.0-nightly (`e7769602a`, 2026-08-24); LLVM 23.1.0 |
| Sanitizer receipt | `/private/tmp/ghostrace-0063-merged-sanitizer-v1.log` |
| Sanitizer SHA-256 | `d4cc1bdaad16d2518720ba06d94849b21f0cd4374d298c0c704f42ce451299e5` |
| Result | `GHOSTRACE_0063_MERGED_SANITIZER_V1_PASS`, exit 0; native integration passed |

The only ASan suppressions are the macOS runtime's
`*_fetchInitializingClassList*` initialization interceptors; no adapter
allocation, callback, or shutdown finding was reported.

## Additional receipts and limits

The implementation source pipe at `bbaf2bc1c25941516d39ad142b3dc0515907a000`
(`/private/tmp/ghostrace-0063-source-pipe-v4.log`, SHA-256
`28f658f16b44e19b48a6cd21bb4c88e9fc9d089bbfa808f06c6c185e0d7db47e`) and its
focused release receipt (`/private/tmp/ghostrace-0063-release-focused-v2.log`,
SHA-256 `b7c7f7410a82dfe032be77c0ecbe856dec119eee3a252e6916249f74c73d2bc9`)
were superseded as authoritative by the merged-main pipe above. They remain
useful implementation receipts; the merge receipt is the acceptance anchor.

A local x86_64 Linux cross-target attempt is retained at
`/private/tmp/ghostrace-0063-linux-check-v1.log` (SHA-256
`0f19b9deb548e12150ea20a2795988a225112e0f893fa994b6fbf197990f77ce`) and is
not a pass: this Mac has no `x86_64-linux-gnu-gcc`. Hosted Linux stable and MSRV
checks for the final PR head were green, but hosted checks are supplementary to
the device receipts above.

Intel macOS, the macOS 15 floor, permissions/consent, root canonicalization and
symlink policy, event attribution/completeness, cursor recovery, persistence,
backpressure, signed/notarized distribution, and production throughput remain
unverified or explicit no-go gates for later collector tasks.
