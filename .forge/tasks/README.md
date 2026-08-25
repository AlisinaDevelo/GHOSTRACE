# GHOSTRACE task ledger

This is the versioned source of truth for the 2026–2031 program. A task is done only when its acceptance criteria have verified evidence; GitHub mirrors this ledger with milestones, labels, native sub-issues, and blocked-by relationships.

| id | title | release | status | workstream | owner | parent | depends_on |
|---|---|---|---|---|---|---|---|
| 0001 | Publish GHOSTRACE product contract and non-goals | M0 | done | foundation | maintainer | — | — |
| 0002 | Record macOS support, permission, and collector boundaries | M0 | done | foundation | maintainer | — | 0001 |
| 0003 | Scaffold the MPL-2.0 Rust core and CI | M0 | done | foundation | maintainer | — | 0001 |
| 0004 | Publish the threat model and data inventory | M0 | done | foundation | maintainer | — | 0001, 0002 |
| 0005 | Add privacy regression and network-surface checks | M0 | done | privacy | maintainer | — | 0003, 0004, 0043, 0044 |
| 0006 | Define the versioned canonical event envelope | M0 | done | foundation | maintainer | — | 0003 |
| 0007 | Implement the consent and capture-policy engine | M1 | done | privacy | maintainer | — | 0004, 0006, 0051, 0052, 0053 |
| 0008 | Implement Keychain-backed DEK and AEAD envelopes | M1 | done | storage | maintainer | — | 0003, 0004, 0054, 0055, 0056 |
| 0009 | Create SQLite WAL schema and migration runner | M1 | done | storage | maintainer | — | 0006, 0008, 0057, 0058, 0059 |
| 0010 | Build bounded ingest writer with atomic cursor commit | M1 | done | storage | maintainer | — | 0006, 0007, 0008, 0009, 0049, 0060, 0061 |
| 0011 | Build fixture replay and crash-injection harness | M1 | done | storage | maintainer | — | 0006, 0010, 0062 |
| 0012 | Ship fixture ingest, explain, and JSONL export CLI slice | M1 | done | foundation | maintainer | — | 0007, 0009, 0010, 0011 |
| 0013 | Implement selected-root macOS FSEvents collector | M2 | done | filesystem | maintainer | — | 0002, 0007, 0010, 0012, 0063, 0064, 0065 |
| 0014 | Enforce root canonicalization, symlink, and exclusion rules | M2 | done | filesystem | maintainer | — | 0007, 0013, 0067, 0068, 0069 |
| 0015 | Persist FSEvents cursors and recover after restart | M2 | done | filesystem | maintainer | — | 0010, 0013, 0066, 0070, 0071, 0072 |
| 0016 | Add event-storm backpressure and loss accounting | M2 | backlog | filesystem | maintainer | — | 0010, 0013, 0015, 0073, 0074 |
| 0017 | Publish filesystem correctness and latency benchmarks | M2 | backlog | filesystem | maintainer | — | 0013, 0014, 0015, 0016, 0075, 0076 |
| 0018 | Implement time-window queries and stable ordering | M3 | backlog | explain-export | maintainer | — | 0010, 0012, 0077, 0078, 0079 |
| 0019 | Implement deterministic evidence-backed explain | M3 | backlog | explain-export | maintainer | — | 0017, 0018, 0080, 0081, 0082 |
| 0020 | Define and ship JSONL export v1 with manifest | M3 | backlog | explain-export | maintainer | — | 0018, 0019, 0083, 0084, 0085 |
| 0021 | Add retention, deletion, and integrity-check commands | M3 | backlog | explain-export | maintainer | — | 0009, 0018, 0020, 0086, 0087 |
| 0022 | Add optional Parquet cold-archive export | M3 | backlog | explain-export | maintainer | — | 0020, 0021, 0090 |
| 0023 | Add tamper-evident event chain and verifier | M3 | backlog | explain-export | maintainer | — | 0008, 0009, 0020, 0088, 0089 |
| 0024 | Add explicit shell-wrapper metadata capture | M4 | backlog | shell-git | maintainer | — | 0007, 0018, 0091, 0092, 0093 |
| 0025 | Add explicit Git snapshot integration | M4 | backlog | shell-git | maintainer | — | 0007, 0018, 0094, 0095, 0096 |
| 0026 | Add opt-in Git hook install and uninstall | M4 | backlog | shell-git | maintainer | — | 0025, 0097 |
| 0027 | Implement NSWorkspace frontmost-app collector | M4 | backlog | frontmost | maintainer | — | 0006, 0007, 0010, 0012, 0098 |
| 0028 | Add frontmost attribution and privacy tests | M4 | backlog | frontmost | maintainer | — | 0027, 0018, 0099, 0100 |
| 0029 | Decide browser transport and permissions in security ADR | M5 | backlog | browser | maintainer | — | 0002, 0004, 0005, 0012, 0101 |
| 0030 | Implement Native Messaging host and explicit pairing | M5 | backlog | browser | maintainer | — | 0008, 0010, 0029, 0102, 0103, 0104 |
| 0031 | Implement Chromium top-level navigation collector | M5 | backlog | browser | maintainer | — | 0007, 0029, 0030, 0105, 0106 |
| 0032 | Implement browser bookmark event and snapshot collector | M5 | backlog | browser | maintainer | — | 0007, 0030, 0031, 0107 |
| 0033 | Add browser privacy matrix and private-mode regression suite | M5 | backlog | browser | maintainer | — | 0031, 0032, 0108, 0109 |
| 0034 | Decide Safari WebExtension parity and ship only if viable | M5 | backlog | browser | maintainer | — | 0029, 0030, 0031, 0032, 0033, 0110 |
| 0035 | Expose versioned Unix-domain local service API | M5 | backlog | service-ui | maintainer | — | 0010, 0018, 0020, 0029, 0111, 0112 |
| 0036 | Build read-only Tauri timeline and explain UI | M5 | backlog | service-ui | maintainer | — | 0019, 0020, 0035, 0113 |
| 0037 | Add launchd user-agent lifecycle and permission UX | M5 | backlog | service-ui | maintainer | — | 0013, 0027, 0030, 0035, 0114 |
| 0038 | Harden release signing, notarization, SBOM, and dependencies | M6 | backlog | release-scale | maintainer | — | 0005, 0008, 0023, 0031, 0037, 0047, 0048, 0115, 0116, 0117, 0118 |
| 0039 | Establish performance and resource benchmark gates | M6 | backlog | release-scale | maintainer | — | 0013, 0018, 0035, 0037, 0119, 0120 |
| 0040 | Complete v1.0 compatibility, privacy, and incident readiness | M6 | backlog | release-scale | maintainer | — | 0020, 0021, 0022, 0023, 0024, 0025, 0026, 0027, 0028, 0029, 0030, 0031, 0032, 0033, 0034, 0035, 0036, 0037, 0038, 0039, 0041, 0042, 0121, 0122, 0123 |
| 0041 | Validate the project identity and package namespaces | M0 | done | foundation | maintainer | — | 0001 |
| 0042 | Evaluate optional Endpoint Security actor attribution | M6 | backlog | release-scale | maintainer | — | 0017, 0019, 0037, 0124 |
| 0043 | Build the prohibited-data privacy regression corpus | M0 | done | privacy | test-engineer | 0005 | 0004, 0006 |
| 0044 | Enforce an offline network-denial CI lane | M0 | done | privacy | security-auditor | 0005 | 0003, 0004 |
| 0045 | Publish the supported macOS and permission test matrix | M0 | done | foundation | platform-engineer | — | 0002 |
| 0046 | Freeze semantic identifier and digest contracts | M0 | done | foundation | security-auditor | — | 0004, 0006 |
| 0047 | Define program outcomes and the release evidence register | M0 | done | foundation | tech-lead | — | 0001, 0004 |
| 0048 | Pin the reproducible developer and fixture toolchain | M0 | done | foundation | devops-engineer | — | 0003, 0006 |
| 0049 | Make ingestion origin an explicit capability | M1 | done | storage | security-auditor | 0010 | 0004, 0006 |
| 0050 | Introduce semantic wrappers for retained fields | M1 | done | foundation | implementation-engineer | — | 0004, 0006 |
| 0051 | Version and migrate capture-policy documents | M1 | done | privacy | architect | 0007 | 0004, 0006 |
| 0052 | Model consent as a revocable state machine | M1 | done | privacy | privacy-engineer | 0007 | 0004, 0006 |
| 0053 | Expose bounded policy decisions and refusal reasons | M1 | done | privacy | security-auditor | 0007 | 0004, 0006 |
| 0054 | Implement the macOS data-protection Keychain backend | M1 | done | storage | macos-engineer | 0008 | 0003, 0004 |
| 0055 | Specify key rotation, recovery, and destruction | M1 | done | storage | security-auditor | 0008 | 0054 |
| 0056 | Test locked-session and background key behavior | M1 | done | storage | test-engineer | 0008 | 0054 |
| 0057 | Harden persistent journal path creation | M1 | done | storage | security-auditor | 0009 | 0003, 0004 |
| 0058 | Define WAL, SHM, checkpoint, and reader policy | M1 | done | storage | database-expert | 0009 | 0057 |
| 0059 | Checksum migrations and refuse unsafe downgrade | M1 | done | storage | database-expert | 0009 | 0006, 0057 |
| 0060 | Specify bounded writer queue and acknowledgement semantics | M1 | done | storage | concurrency-specialist | 0010 | 0049, 0051, 0057 |
| 0061 | Enforce cursor monotonicity and idempotent replay | M1 | done | storage | concurrency-specialist | 0010 | 0059, 0060 |
| 0062 | Build the storage crash and fault-injection matrix | M1 | done | storage | test-engineer | 0011 | 0055, 0058, 0061 |
| 0063 | Build a memory-safe FSEvents stream lifecycle adapter | M2 | done | filesystem | macos-engineer | 0013 | 0010, 0012 |
| 0064 | Implement selected-root consent and lifecycle receipts | M2 | done | filesystem | privacy-engineer | 0013 | 0007, 0012 |
| 0065 | Normalize every FSEvents flag into evidence status | M2 | done | filesystem | macos-engineer | 0013 | 0006, 0063 |
| 0066 | Track volume identity and mount transitions | M2 | done | filesystem | macos-engineer | 0015 | 0013, 0014 |
| 0067 | Test APFS case, Unicode, and root-containment behavior | M2 | done | filesystem | test-engineer | 0014 | 0013, 0045 |
| 0068 | Harden symlink, hard-link, and open-race containment | M2 | done | filesystem | security-auditor | 0014 | 0013, 0057 |
| 0069 | Version exclusion precedence and matching rules | M2 | done | filesystem | privacy-engineer | 0014 | 0007, 0050 |
| 0070 | Persist one replay boundary per source and volume | M2 | done | filesystem | database-expert | 0015 | 0061, 0063, 0066 |
| 0071 | Emit gaps for dropped, wrapped, and root-changed history | M2 | done | filesystem | macos-engineer | 0015 | 0065, 0070 |
| 0072 | Define startup history-done and invalid-cursor behavior | M2 | done | filesystem | macos-engineer | 0015 | 0065, 0070 |
| 0073 | Specify coalescing, deduplication, and rename limits | M2 | done | filesystem | architect | 0016 | 0065, 0070, 0071 |
| 0074 | Prevent collector feedback loops and own-event suppression errors | M2 | done | filesystem | security-auditor | 0016 | 0065, 0069, 0070 |
| 0075 | Exercise storms, sleep, wake, detach, and restart | M2 | done | filesystem | test-engineer | 0017 | 0014, 0015, 0016 |
| 0076 | Publish a reproducible filesystem benchmark corpus | M2 | done | filesystem | performance-engineer | 0017 | 0075 |
| 0077 | Define snapshot-consistent query pagination | M3 | backlog | explain-export | database-expert | 0018 | 0010, 0012 |
| 0078 | Model clock skew and deterministic total ordering | M3 | backlog | explain-export | architect | 0018 | 0010, 0012 |
| 0079 | Make query windows gap-aware | M3 | backlog | explain-export | implementation-engineer | 0018 | 0071, 0077, 0078 |
| 0080 | Define a bounded evidence-claim grammar | M3 | backlog | explain-export | researcher | 0019 | 0017, 0018, 0046 |
| 0081 | Version the cross-source correlation rule registry | M3 | backlog | explain-export | architect | 0019 | 0079, 0080 |
| 0082 | Build explanation determinism and counterexample tests | M3 | backlog | explain-export | test-engineer | 0019 | 0080, 0081 |
| 0083 | Create the export schema and manifest registry | M3 | backlog | explain-export | api-designer | 0020 | 0018, 0019, 0046 |
| 0084 | Stream exports through an atomic bounded writer | M3 | backlog | explain-export | implementation-engineer | 0020 | 0083 |
| 0085 | Add export redaction preview and policy receipts | M3 | backlog | explain-export | privacy-engineer | 0020 | 0007, 0083 |
| 0086 | Implement retention planning and dry-run | M3 | backlog | explain-export | database-expert | 0021 | 0009, 0018, 0020 |
| 0087 | Document and test deletion residue limits | M3 | backlog | explain-export | security-auditor | 0021 | 0086 |
| 0088 | Authenticate sequence, cursor, policy, and diagnostic state | M3 | backlog | explain-export | security-auditor | 0023 | 0008, 0009, 0020 |
| 0089 | Add signed checkpoints and bounded repair workflows | M3 | backlog | explain-export | incident-responder | 0023 | 0088 |
| 0090 | Specify and validate the Parquet archive profile | M3 | backlog | explain-export | data-engineer | 0022 | 0084, 0087 |
| 0091 | Freeze the explicit shell metadata schema | M4 | backlog | shell-git | privacy-engineer | 0024 | 0007, 0018 |
| 0092 | Test shell wrapper lifecycle and exit semantics | M4 | backlog | shell-git | test-engineer | 0024 | 0091 |
| 0093 | Red-team shell secret leakage | M4 | backlog | shell-git | security-auditor | 0024 | 0091 |
| 0094 | Define stable Git repository and worktree identity | M4 | backlog | shell-git | git-specialist | 0025 | 0007, 0018 |
| 0095 | Minimize Git refs, object IDs, and snapshot fields | M4 | backlog | shell-git | privacy-engineer | 0025 | 0094 |
| 0096 | Represent Git rewrites and unavailable history as gaps | M4 | backlog | shell-git | git-specialist | 0025 | 0094, 0095 |
| 0097 | Make Git hook installation verifiable and reversible | M4 | backlog | shell-git | git-specialist | 0026 | 0094, 0095 |
| 0098 | Define frontmost application identity and session semantics | M4 | backlog | frontmost | macos-engineer | 0027 | 0006, 0007, 0010, 0012 |
| 0099 | Test frontmost sleep, wake, and privacy transitions | M4 | backlog | frontmost | test-engineer | 0028 | 0018, 0098 |
| 0100 | Evaluate developer-workflow cross-source explanations | M4 | backlog | frontmost | researcher | 0028 | 0092, 0096, 0099 |
| 0101 | Build the browser integration threat corpus | M5 | backlog | browser | security-auditor | 0029 | 0002, 0004, 0005, 0012 |
| 0102 | Install and remove the native-host manifest safely | M5 | backlog | browser | macos-engineer | 0030 | 0008, 0010, 0029 |
| 0103 | Version and bound the native-messaging protocol | M5 | backlog | browser | api-designer | 0030 | 0102 |
| 0104 | Implement explicit browser pairing and replay protection | M5 | backlog | browser | security-auditor | 0030 | 0102 |
| 0105 | Define Chromium navigation permission and state handling | M5 | backlog | browser | browser-engineer | 0031 | 0007, 0029, 0103, 0104 |
| 0106 | Canonicalize browser origins without retaining secrets | M5 | backlog | browser | security-auditor | 0031 | 0050, 0105 |
| 0107 | Model bookmark snapshots as bounded diffs | M5 | backlog | browser | browser-engineer | 0032 | 0103, 0105, 0106 |
| 0108 | Enforce private and incognito context refusal | M5 | backlog | browser | privacy-engineer | 0033 | 0105, 0107 |
| 0109 | Fuzz hostile extension and native-host messages | M5 | backlog | browser | test-engineer | 0033 | 0103, 0104 |
| 0110 | Run a Safari WebExtension parity gate | M5 | backlog | browser | macos-engineer | 0034 | 0029, 0108, 0109 |
| 0111 | Authenticate the local Unix-socket protocol | M5 | backlog | service-ui | security-auditor | 0035 | 0010, 0018, 0020, 0029 |
| 0112 | Bound and fuzz every local-service capability | M5 | backlog | service-ui | test-engineer | 0035 | 0111 |
| 0113 | Design an accessible evidence and gap interface | M5 | backlog | service-ui | accessibility-specialist | 0036 | 0019, 0020, 0035 |
| 0114 | Make launchd install, upgrade, and permission recovery reversible | M5 | backlog | service-ui | macos-engineer | 0037 | 0013, 0027, 0030, 0035 |
| 0115 | Freeze release entitlements and permission drift | M6 | backlog | release-scale | security-auditor | 0038 | 0005, 0008, 0023 |
| 0116 | Produce reproducible universal macOS artifacts | M6 | backlog | release-scale | devops-engineer | 0038 | 0115 |
| 0117 | Publish SBOM and SLSA build provenance | M6 | backlog | release-scale | supply-chain-specialist | 0038 | 0116 |
| 0118 | Automate notarization, stapling, and Gatekeeper verification | M6 | backlog | release-scale | release-engineer | 0038 | 0116, 0117 |
| 0119 | Version the end-to-end performance methodology | M6 | backlog | release-scale | performance-engineer | 0039 | 0017, 0018, 0035 |
| 0120 | Gate regressions with soak and resource-limit tests | M6 | backlog | release-scale | performance-engineer | 0039 | 0119 |
| 0121 | Build the schema and export compatibility matrix | M6 | backlog | release-scale | test-engineer | 0040 | 0020, 0021, 0083, 0084 |
| 0122 | Run the pre-v1 privacy and permission red-team | M6 | backlog | release-scale | security-auditor | 0040 | 0033, 0037, 0115 |
| 0123 | Practice disaster recovery and incident response | M6 | backlog | release-scale | incident-responder | 0040 | 0023, 0037, 0089 |
| 0124 | Build the Endpoint Security attribution evaluation harness | M6 | backlog | release-scale | security-researcher | 0042 | 0017, 0019, 0037 |
| 0125 | Pass the v1.1 operational resilience gate | M7 | backlog | operations | tech-lead | — | 0040, 0126, 0127, 0128, 0129, 0130, 0131 |
| 0126 | Ship a redacted self-diagnostic health report | M7 | backlog | operations | sre | 0125 | 0040 |
| 0127 | Build guided backup, restore, and upgrade recovery | M7 | backlog | operations | database-expert | 0125 | 0040 |
| 0128 | Operate an annual macOS compatibility lab | M7 | backlog | operations | macos-engineer | 0125 | 0040, 0045 |
| 0129 | Complete accessibility and localization certification | M7 | backlog | operations | accessibility-specialist | 0125 | 0036, 0040 |
| 0130 | Decide direct and Homebrew distribution and updates | M7 | backlog | operations | release-engineer | 0125 | 0038, 0040 |
| 0131 | Export a consented telemetry-free support bundle | M7 | backlog | operations | privacy-engineer | 0125 | 0040, 0126 |
| 0132 | Pass the interoperability and adapter conformance gate | M8 | backlog | interoperability | tech-lead | — | 0125, 0133, 0134, 0135, 0136, 0137, 0138 |
| 0133 | Define an imported-evidence trust boundary | M8 | backlog | interoperability | security-auditor | 0132 | 0125 |
| 0134 | Publish a conservative W3C PROV mapping | M8 | backlog | interoperability | researcher | 0132 | 0125, 0133 |
| 0135 | Specify an offline OpenTelemetry import profile | M8 | backlog | interoperability | observability-specialist | 0132 | 0125, 0133 |
| 0136 | Version the adapter capability manifest | M8 | backlog | interoperability | api-designer | 0132 | 0125 |
| 0137 | Build the adapter conformance and fault suite | M8 | backlog | interoperability | test-engineer | 0132 | 0136 |
| 0138 | Design encrypted evidence bundle transfer | M8 | backlog | interoperability | security-auditor | 0132 | 0133, 0134 |
| 0139 | Pass the research-grade evaluation gate | M9 | backlog | research | research-lead | — | 0132, 0140, 0141, 0142, 0143, 0144, 0145 |
| 0140 | Publish a synthetic causal ground-truth corpus | M9 | backlog | research | researcher | 0139 | 0132 |
| 0141 | Measure claim precision, coverage, abstention, and calibration | M9 | backlog | research | researcher | 0139 | 0140 |
| 0142 | Evaluate whether people understand gaps and uncertainty | M9 | backlog | research | ux-researcher | 0139 | 0113, 0140 |
| 0143 | Benchmark privacy leakage across every artifact | M9 | backlog | research | privacy-engineer | 0139 | 0132, 0140 |
| 0144 | Run a longitudinal energy and storage study | M9 | backlog | research | performance-engineer | 0139 | 0120, 0140 |
| 0145 | Publish a reproducible research artifact | M9 | backlog | research | research-lead | 0139 | 0140, 0141, 0142, 0143, 0144 |
| 0146 | Pass the governed ecosystem extensibility gate | M10 | backlog | ecosystem | tech-lead | — | 0139, 0147, 0148, 0149, 0150, 0151, 0152 |
| 0147 | Stabilize the adapter ABI and compatibility policy | M10 | backlog | ecosystem | api-designer | 0146 | 0136, 0137, 0139 |
| 0148 | Decide on an isolated plug-in execution model | M10 | backlog | ecosystem | security-researcher | 0146 | 0137, 0147 |
| 0149 | Sign, admit, quarantine, and revoke adapters | M10 | backlog | ecosystem | security-auditor | 0146 | 0147, 0148 |
| 0150 | Publish reproducible adapter conformance results | M10 | backlog | ecosystem | test-engineer | 0146 | 0137, 0149 |
| 0151 | Scale contributor governance and security response | M10 | backlog | ecosystem | maintainer | 0146 | 0139 |
| 0152 | Run a cross-platform feasibility study | M10 | backlog | ecosystem | architect | 0146 | 0139, 0147 |
| 0153 | Pass the v2 and long-term support readiness gate | M11 | backlog | long-term | tech-lead | — | 0146, 0154, 0155, 0156, 0157, 0158, 0159, 0160 |
| 0154 | Specify event, claim, and export format v2 | M11 | backlog | long-term | architect | 0153 | 0145, 0147 |
| 0155 | Implement cryptographic agility and key migration | M11 | backlog | long-term | cryptography-specialist | 0153 | 0055, 0089, 0145 |
| 0156 | Compact storage without breaking verification | M11 | backlog | long-term | database-expert | 0153 | 0087, 0089, 0144 |
| 0157 | Publish the five-year deprecation and LTS policy | M11 | backlog | long-term | maintainer | 0153 | 0146 |
| 0158 | Commission an independent privacy and security audit | M11 | backlog | long-term | security-auditor | 0153 | 0146, 0154, 0155, 0156 |
| 0159 | Run the v2 migration and rollback release candidate | M11 | backlog | long-term | release-engineer | 0153 | 0154, 0155, 0156, 0158 |
| 0160 | Publish the 2032 research and sustainability decision | M11 | backlog | long-term | maintainer | 0153 | 0145, 0151, 0157 |
