# Platform policy

GHOSTRACE is macOS-first because its first live source and its future protected key
store are macOS facilities. The current fixture core is intentionally portable
enough to build and test on Linux; portability must not weaken the macOS privacy
boundary.

## Support posture

| Area | Current statement |
| --- | --- |
| Operating system | macOS 15.0 (Sequoia) is the planned development floor for live capture; the release gate will revalidate it and may raise it |
| Architectures | Apple silicon and Intel are design targets; each must be tested against the supported macOS floor before release |
| Fixture core | Linux and macOS CI exercise parsing, policy, storage, explain, and export without ambient capture |
| Distribution | No signed, notarized, or bundled release artifact is shipped in the headstart |
| Permissions | The fixture path requests none; live permissions are a future, explicit decision |

The complete, machine-readable support and permission contract is in the
[support matrix](SUPPORT_MATRIX.md). It records target versus verified macOS
major-version/architecture rows, explicit unavailable-hardware no-go rows, and
the required/optional/prohibited permissions plus observable refusal for every
planned collector.

## Permission boundary

The baseline does not require root, Full Disk Access, Accessibility, or Automation.
It does not install a privileged helper. A future collector may request only the
minimum permission required for the selected source, explain why it is needed, and
remain disabled when consent is absent or revoked.

Endpoint Security is optional and entitlement-gated. It is not a hidden fallback for
FSEvents and will require its own threat model, attribution tests, user-facing
consent, and release evidence.

## FSEvents boundary

The planned first live source observes bounded filesystem metadata below explicitly
selected roots. It must canonicalize roots, reject symlink escapes, apply exclusions
before persistence, and retain no file contents. FSEvents does not guarantee
process attribution, event completeness, or one notification per change. Source
flags, cursor state, and gaps must remain visible.

## Private contexts

Private browsing and private application contexts are excluded by default. A future
browser or frontmost adapter must define how it detects private context and must not
turn it on merely because a user selected a filesystem root.

## CI and cross-platform work

Linux CI provides a fast, deterministic test environment for the fixture contract.
macOS CI is required for platform APIs, permissions, Keychain integration, and
FSEvents behavior. Platform-specific code belongs behind an explicit adapter boundary
so compiling the fixture core cannot accidentally enable live capture.

The [roadmap](ROADMAP.md) and [evaluation plan](EVALUATION.md) define the evidence
required before the support matrix becomes a release promise.
