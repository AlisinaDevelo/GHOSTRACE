use chrono::{TimeZone, Utc};
use ghostrace::{
    ConsentState, ConsentStateMachine, EventSource, PolicyChange, PolicyDecision, PolicyDocument,
    PolicyOutcome, PolicyProfile, PolicyReason,
};

// This is deliberately dependency-free. A fixed generator gives us a
// reproducible property corpus without adding a network-fetched test crate to
// the privacy-sensitive baseline.
#[derive(Clone, Copy)]
struct Lcg(u64);

impl Lcg {
    fn next(mut self) -> (Self, u64) {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self, self.0)
    }
}

const SOURCES: [EventSource; 7] = [
    EventSource::Filesystem,
    EventSource::FrontmostApp,
    EventSource::Shell,
    EventSource::Git,
    EventSource::Browser,
    EventSource::Lifecycle,
    EventSource::Fixture,
];

fn generated_profile(seed: u64) -> (PolicyProfile, Lcg) {
    let mut profile = PolicyProfile::deny_by_default(format!("property-{seed:x}"));
    profile.version = (seed as u32 % 31) + 1;
    let mut generator = Lcg(seed ^ 0x504f4c494359);
    for source in SOURCES {
        let (next, value) = generator.next();
        generator = next;
        if value & 1 == 1 {
            profile.enable_source(source);
        }
    }
    for root in ["root-a", "root-b", "root-c", "root-d"] {
        let (next, value) = generator.next();
        generator = next;
        if value & 1 == 1 {
            profile.select_root(root);
        }
        let (next, value) = generator.next();
        generator = next;
        if value & 1 == 1 {
            profile.exclude_root(root);
        }
    }
    let (next, value) = generator.next();
    generator = next;
    if value & 1 == 1 {
        profile.allow_private_context();
    }
    (profile, generator)
}

#[test]
fn policy_properties_hold_for_reproducible_scope_matrix() {
    let mut exercised_exclusion = false;
    let mut exercised_redaction = false;

    for seed in 0..512_u64 {
        let (mut profile, mut generator) = generated_profile(seed);
        if seed == 0 {
            profile.enable_source(EventSource::Filesystem);
            profile.select_root("root-a");
            profile.exclude_root("root-a");
        }
        let document = profile.to_document().expect("generated profile is valid");
        let round_trip = PolicyDocument::from_json(&document.to_json().expect("policy JSON"))
            .expect("serialized policy is valid")
            .to_profile()
            .expect("policy round-trip");
        assert_eq!(round_trip, profile, "seed={seed}");
        assert_eq!(
            document.scope_digest().expect("scope digest"),
            round_trip.to_document().unwrap().scope_digest().unwrap()
        );

        for source in SOURCES {
            for root in [None, Some("root-a"), Some("root-b"), Some("../private")] {
                for private_context in [false, true] {
                    let decision = profile.decide(source, root, private_context);
                    let expected = if !profile.enabled_sources.contains(&source) {
                        PolicyReason::SourceNotEnabled
                    } else if root.is_some_and(|value| !profile.selected_roots.contains(value)) {
                        PolicyReason::RootNotSelected
                    } else if root.is_some_and(|value| profile.excluded_roots.contains(value)) {
                        exercised_exclusion = true;
                        PolicyReason::RootExcluded
                    } else if private_context && !profile.allow_private_context {
                        PolicyReason::PrivateContext
                    } else {
                        PolicyReason::PolicyAllowed
                    };

                    match expected {
                        PolicyReason::PolicyAllowed => {
                            assert!(decision.is_allowed(), "seed={seed}")
                        }
                        reason => {
                            assert_eq!(decision.reason_code(), Some(reason.code()), "seed={seed}")
                        }
                    }

                    let record = profile.decide_record(source, root, private_context);
                    assert_eq!(record.root_present, root.is_some(), "seed={seed}");
                    let serialized = serde_json::to_string(&record).expect("decision JSON");
                    assert!(!serialized.contains("../private"), "seed={seed}");
                    if matches!(decision, PolicyDecision::Denied { .. }) {
                        let redacted = record.clone().redact();
                        assert_eq!(redacted.outcome, PolicyOutcome::Redact);
                        assert_eq!(redacted.reason, PolicyReason::RedactionRequired);
                        assert!(!serde_json::to_string(&redacted).unwrap().contains("root-a"));
                        let summarized = record.clone().summarize();
                        assert_eq!(summarized.outcome, PolicyOutcome::Summarize);
                        exercised_redaction = true;
                    }
                }
            }
        }

        let (next, value) = generator.next();
        if value & 1 == 1 {
            let mut changed = profile.clone();
            changed.version = profile.version + 1;
            changed.exclude_root("root-z");
            let migration = changed
                .to_document()
                .expect("changed profile")
                .migration_from(&document)
                .expect("forward migration");
            assert!(
                matches!(migration, ghostrace::PolicyMigration::RequiresReconfirmation { changed, .. } if changed.contains(&PolicyChange::ExcludedRoots)),
                "seed={seed}"
            );
        }
        generator = next;
        let _ = generator;
    }

    assert!(exercised_exclusion, "generator never exercised an excluded root");
    assert!(exercised_redaction, "generator never exercised a redaction outcome");
}

#[derive(Clone, Copy)]
enum Command {
    Grant,
    ChangeScope,
    Suspend,
    Revoke,
    Delete,
}

fn property_document(version: u32, variant: u64) -> PolicyDocument {
    let root = format!("root-{}", variant % 4);
    let exclusions = if variant & 1 == 1 { vec!["root-3"] } else { Vec::new() };
    PolicyDocument::new(
        "consent-property-v1",
        version,
        [EventSource::Filesystem, EventSource::Fixture],
        [root],
        variant & 2 == 2,
    )
    .expect("property policy")
    .with_excluded_roots(exclusions)
    .expect("property exclusions")
}

fn assert_consent_invariants(machine: &ConsentStateMachine, seed: u64, step: usize) {
    assert_eq!(
        machine.is_capture_allowed(),
        machine.state() == ConsentState::Active,
        "seed={seed} step={step}"
    );
    for (index, receipt) in machine.receipts().iter().enumerate() {
        assert_eq!(receipt.sequence, index as u64 + 1, "seed={seed} step={step}");
        assert!(receipt.policy_version > 0, "seed={seed} step={step}");
        if let Some(previous) = index.checked_sub(1).and_then(|i| machine.receipts().get(i)) {
            assert!(receipt.occurred_at >= previous.occurred_at, "seed={seed} step={step}");
        }
    }
    let replayed = ConsentStateMachine::replay(machine.receipts()).expect("receipt replay");
    assert_eq!(&replayed, machine, "seed={seed} step={step}");
}

#[test]
fn consent_properties_preserve_replay_and_deny_silent_reenable() {
    for seed in 0..256_u64 {
        let mut machine = ConsentStateMachine::new();
        let mut generator = Lcg(seed ^ 0x434f4e53454e54);
        for step in 0..96_usize {
            let (next, value) = generator.next();
            generator = next;
            let command = match value % 5 {
                0 => Command::Grant,
                1 => Command::ChangeScope,
                2 => Command::Suspend,
                3 => Command::Revoke,
                _ => Command::Delete,
            };
            let version = ((step as u32 + value as u32) % 16) + 1;
            let document = property_document(version, value);
            let occurred_at = Utc.timestamp_opt(1_700_000_000 + step as i64, 0).single().unwrap();
            let before = machine.clone();
            let result = match command {
                Command::Grant => {
                    machine.grant(&document, occurred_at, "property-actor", "property-grant")
                }
                Command::ChangeScope => {
                    machine.change_scope(&document, occurred_at, "property-actor", "property-scope")
                }
                Command::Suspend => {
                    machine.suspend(occurred_at, "property-actor", "property-suspend")
                }
                Command::Revoke => machine.revoke(occurred_at, "property-actor", "property-revoke"),
                Command::Delete => {
                    machine.request_deletion(occurred_at, "property-actor", "property-delete")
                }
            };
            if result.is_err() {
                assert_eq!(
                    machine, before,
                    "failed command mutated state; seed={seed} step={step}"
                );
            }
            assert_consent_invariants(&machine, seed, step);
        }

        if machine.state() != ConsentState::Active && !machine.receipts().is_empty() {
            let mut forged = machine.receipts().to_vec();
            let last = forged.last_mut().unwrap();
            last.sequence += 1;
            last.transition = ghostrace::ConsentTransitionKind::ScopeChanged;
            last.state = ConsentState::Active;
            assert!(
                ConsentStateMachine::replay(&forged).is_err(),
                "silent re-enable accepted; seed={seed}"
            );
        }
    }
}
