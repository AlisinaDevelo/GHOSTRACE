# GHOSTRACE task ledger

This board tracks the dependency-ordered roadmap from the initial public contract through v1.0 readiness. A task becomes ready only after every listed dependency is done, and becomes done only after its acceptance criteria have been verified.

| id | title | release | status | agent | model | depends_on |
|----|-------|---------|--------|-------|-------|------------|
| 0001 | Publish GHOSTRACE product contract and non-goals | M0 | done | maintainer | human | — |
| 0002 | Record macOS support, permission, and collector boundaries | M0 | done | maintainer | human | 0001 |
| 0003 | Scaffold the MPL-2.0 Rust core and CI | M0 | done | maintainer | human | 0001 |
| 0004 | Publish the threat model and data inventory | M0 | done | maintainer | human | 0001, 0002 |
| 0005 | Add privacy regression and network-surface checks | M0 | ready | maintainer | human | 0003, 0004 |
| 0006 | Define the versioned canonical event envelope | M1 | done | maintainer | human | 0003 |
| 0007 | Implement the consent and capture-policy engine | M1 | ready | maintainer | human | 0004, 0006 |
| 0008 | Implement Keychain-backed DEK and AEAD envelopes | M1 | ready | maintainer | human | 0003, 0004 |
| 0009 | Create SQLite WAL schema and migration runner | M1 | backlog | maintainer | human | 0006, 0008 |
| 0010 | Build bounded ingest writer with atomic cursor commit | M1 | backlog | maintainer | human | 0006, 0007, 0008, 0009 |
| 0011 | Build fixture replay and crash-injection harness | M1 | backlog | maintainer | human | 0006, 0010 |
| 0012 | Ship fixture ingest, explain, and JSONL export CLI slice | M1 | backlog | maintainer | human | 0007, 0009, 0010, 0011 |
| 0013 | Implement selected-root macOS FSEvents collector | M2 | backlog | maintainer | human | 0002, 0007, 0010, 0012 |
| 0014 | Enforce root canonicalization, symlink, and exclusion rules | M2 | backlog | maintainer | human | 0007, 0013 |
| 0015 | Persist FSEvents cursors and recover after restart | M2 | backlog | maintainer | human | 0010, 0013 |
| 0016 | Add event-storm backpressure and loss accounting | M2 | backlog | maintainer | human | 0010, 0013, 0015 |
| 0017 | Publish filesystem correctness and latency benchmarks | M2 | backlog | maintainer | human | 0013, 0014, 0015, 0016 |
| 0018 | Implement time-window queries and stable ordering | M3 | backlog | maintainer | human | 0010, 0012 |
| 0019 | Implement deterministic evidence-backed explain | M3 | backlog | maintainer | human | 0017, 0018 |
| 0020 | Define and ship JSONL export v1 with manifest | M3 | backlog | maintainer | human | 0018, 0019 |
| 0021 | Add retention, deletion, and integrity-check commands | M3 | backlog | maintainer | human | 0009, 0018, 0020 |
| 0022 | Add optional Parquet cold-archive export | M3 | backlog | maintainer | human | 0020, 0021 |
| 0023 | Add tamper-evident event chain and verifier | M3 | backlog | maintainer | human | 0008, 0009, 0020 |
| 0024 | Add explicit shell-wrapper metadata capture | M4 | backlog | maintainer | human | 0007, 0018 |
| 0025 | Add explicit Git snapshot integration | M4 | backlog | maintainer | human | 0007, 0018 |
| 0026 | Add opt-in Git hook install and uninstall | M4 | backlog | maintainer | human | 0025 |
| 0027 | Implement NSWorkspace frontmost-app collector | M4 | backlog | maintainer | human | 0006, 0007, 0010, 0012 |
| 0028 | Add frontmost attribution and privacy tests | M4 | backlog | maintainer | human | 0027, 0018 |
| 0029 | Decide browser transport and permissions in security ADR | M5 | backlog | maintainer | human | 0002, 0004, 0005, 0012 |
| 0030 | Implement Native Messaging host and explicit pairing | M5 | backlog | maintainer | human | 0008, 0010, 0029 |
| 0031 | Implement Chromium top-level navigation collector | M5 | backlog | maintainer | human | 0007, 0029, 0030 |
| 0032 | Implement browser bookmark event and snapshot collector | M5 | backlog | maintainer | human | 0007, 0030, 0031 |
| 0033 | Add browser privacy matrix and private-mode regression suite | M5 | backlog | maintainer | human | 0031, 0032 |
| 0034 | Add Safari WebExtension adapter | M5 | backlog | maintainer | human | 0029, 0030, 0031, 0032, 0033 |
| 0035 | Expose versioned Unix-domain local service API | M5 | backlog | maintainer | human | 0010, 0018, 0020, 0029 |
| 0036 | Build read-only Tauri timeline and explain UI | M5 | backlog | maintainer | human | 0019, 0020, 0035 |
| 0037 | Add launchd user-agent lifecycle and permission UX | M5 | backlog | maintainer | human | 0013, 0027, 0030, 0035 |
| 0038 | Harden release signing, notarization, SBOM, and dependencies | M6 | backlog | maintainer | human | 0005, 0008, 0023, 0031, 0037 |
| 0039 | Establish performance and resource benchmark gates | M6 | backlog | maintainer | human | 0013, 0018, 0035, 0037 |
| 0040 | Complete v1.0 compatibility, privacy, and incident readiness | M6 | backlog | maintainer | human | 0020, 0021, 0022, 0023, 0024, 0025, 0026, 0027, 0028, 0029, 0030, 0031, 0032, 0033, 0034, 0035, 0036, 0037, 0038, 0039, 0041, 0042 |
| 0041 | Validate the project identity and package namespaces | M0 | ready | maintainer | human | 0001 |
| 0042 | Evaluate optional Endpoint Security actor attribution | M6 | backlog | maintainer | human | 0017, 0019, 0037 |
