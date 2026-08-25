# Product boundaries

This note makes the GHOSTRACE contract explicit and prevents broad terms such as
“local”, “private”, and “evidence” from being mistaken for a shared product.

## GHOSTRACE's contract

GHOSTRACE is a local macOS event journal for evidence-linked change explanation.
Its unit of record is a bounded observation: what a source reported, when it
reported it, which policy allowed or denied it, how strong the evidence is, and
where coverage is missing. Its explanation layer links observations without
turning temporal order into proof of intent or complete causality.

The current release includes the fixture CLI and an explicitly consent-gated,
selected-root FSEvents collector API. Ambient CLI capture remains disabled until
the remaining path-policy, recovery, writer, encryption, and release gates pass.

## Portfolio comparison

| Project | Primary object | Time axis | Primary question | GHOSTRACE boundary |
| --- | --- | --- | --- | --- |
| **GHOSTRACE** | Event observations and provenance | Across an observation window | What happened, and which observations support the sequence? | This project owns the bounded journal and explanation contract. |
| **LOOM** | User-selected source artifacts, versions, and passages | At and across source versions | Where is the exact source evidence? | GHOSTRACE does not index documents, run OCR, or provide passage retrieval. |
| **STRATA** | TypeScript source and committed revisions | Between code revisions | What architectural behavior changed? | GHOSTRACE does not statically analyze source or produce architecture diffs. |
| **CARTOGRAPH** | TypeScript graph snapshots and revision diffs | Between code revisions | Which graph nodes and relationships changed? | GHOSTRACE does not build source graphs, enforce architecture policy, or render code-change reports. |

The public descriptions of STRATA and CARTOGRAPH may evolve independently. That is
a sibling-project identity decision, not a reason to merge either analyzer into
GHOSTRACE. This repository refers to both as source-code architecture analysis and
does not present them as event-journal components.

## Allowed relationship

An explicit adapter may later record that a user-requested analysis or retrieval
operation occurred. Such a record is an event about the operation; it is not the
operation's source corpus, architecture graph, or search index. Any adapter must
retain GHOSTRACE's consent, minimization, provenance, gap, and offline rules.

The reverse direction is also explicit: another tool may consume a user-exported
GHOSTRACE artifact, but it must not assume that the journal is complete, infer
intent from event order, or treat an export as legal chain-of-custody evidence.

## Non-overlap checklist

- If the task is **reconstructing observed changes over time**, it belongs here.
- If the task is **finding text or image evidence in selected files**, it belongs to
  a retrieval/indexing tool such as LOOM.
- If the task is **comparing TypeScript architecture across Git revisions**, it
  belongs to an architecture analyzer such as STRATA or CARTOGRAPH.
- If the task requires ambient capture, content indexing, or source-code execution,
  it is outside the current GHOSTRACE contract.
