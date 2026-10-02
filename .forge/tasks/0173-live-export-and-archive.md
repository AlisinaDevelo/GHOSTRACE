---
id: 0173
title: Export and archive a live journal
status: done
agent: maintainer
model: human
release: M4
depends_on: [0165, 0020, 0022]
change: null
workstream: explain-export
type: feature
priority: p2
risks: [privacy]
platform: macos
---

## Goal
Let a person take their live journal out as a validated JSONL export and, optionally, a Parquet archive.

## Acceptance criteria
- [x] `ghostrace live export` uses the same preview, plan and snapshot confirmation, and 0600 atomic publish as the fixture export, unlocked with the home's key custody.
- [x] The export passes `ghostrace validate`, and with the `parquet` feature an archive written from it passes `verify-archive`.
- [x] The export destination is refused inside the GHOSTRACE home and is never observed by a running watch.

## Context
This task closes the live-home export gap by reusing the fixture export and archive contracts.

## Notes
Completion requires the acceptance evidence above; issue closure alone is not evidence.

Implemented in PR #406 and acceptance-tested in PR #409. The exact merged-main
device rerun and retained artifact digests are recorded in
[docs/evidence/0173-live-export-and-archive.md](../../docs/evidence/0173-live-export-and-archive.md).

A follow-up native regression identified internal-denial summaries changing an
active export preview. Those summaries now persist only after observation stops;
watch acceptance uses actual startup and completed-journal checks. The retained
failure and final protected-main reproduction are linked from issue #394.
