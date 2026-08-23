# Security policy

GHOSTRACE is a local, privacy-sensitive application. A security issue may expose
journal metadata, plaintext fixture or export data, policy state, encryption
material, or a path by which collection exceeds the published contract.

## Reporting a vulnerability

Please use a [private GitHub Security Advisory](https://github.com/AlisinaDevelo/GHOSTRACE/security/advisories/new).
Include:

- the affected commit, version, or platform;
- a concise description of the impact and trust boundary;
- reproduction steps using synthetic data where possible;
- a minimal proof of concept, if needed.

Do not include real journal databases, exports, file contents, browser data, account
names, URLs, or secrets. If the advisory form is unavailable, do not disclose the
details in a public issue; contact the repository maintainers through GitHub and
request a private reporting path.

We will acknowledge a report when practical, confirm the affected scope, and publish
the minimum necessary correction and credit. Timelines depend on reproducibility and
severity.

## Supported versions

Only the latest main revision and the latest tagged release receive security
attention while the project is pre-1.0. Old fixture formats may be rejected rather
than silently interpreted.

## Security boundaries

The baseline intentionally has no network client, telemetry, cloud sync, URL
fetching, silent upload, root access, Full Disk Access, Accessibility, Automation,
keylogging, microphone capture, screen recording, clipboard capture, window-title
capture, or page-content capture. capture refuses until the documented gates land.

Read [PRIVACY.md](docs/PRIVACY.md) and [THREAT_MODEL.md](docs/THREAT_MODEL.md) before
testing a suspected issue. GHOSTRACE does not provide legal chain of custody.
