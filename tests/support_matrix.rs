use serde_json::Value;

const MATRIX: &str = include_str!("fixtures/support-matrix-v1.json");

fn matrix() -> Value {
    serde_json::from_str(MATRIX).expect("support matrix must be valid JSON")
}

#[test]
fn support_matrix_has_an_explicit_floor_and_architecture_policy() {
    let value = matrix();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["contract_id"], "ghostrace-support-matrix-v1");
    assert_eq!(value["product"], "GHOSTRACE");
    assert_eq!(value["platform"], "macOS");
    assert_eq!(value["support_floor"]["major"], 15);
    assert_eq!(value["support_floor"]["codename"], "Sequoia");

    let target = value["architectures"]["target"].as_array().expect("target architectures");
    assert!(target.iter().any(|item| item == "arm64"));
    assert!(target.iter().any(|item| item == "x86_64"));
    assert!(value["architectures"]["verified"]
        .as_array()
        .expect("verified architectures")
        .iter()
        .any(|item| item == "arm64"));
    assert!(value["architectures"]["unavailable_for_current_run"]
        .as_array()
        .expect("unavailable architectures")
        .iter()
        .any(|item| item == "x86_64"));
}

#[test]
fn every_target_architecture_has_a_non_ambiguous_platform_row() {
    let value = matrix();
    let rows = value["platform_matrix"].as_array().expect("platform rows");
    for architecture in ["arm64", "x86_64"] {
        let matching =
            rows.iter().filter(|row| row["architecture"] == architecture).collect::<Vec<_>>();
        assert!(!matching.is_empty(), "missing row for {architecture}");
        for row in matching {
            assert!(row["os_major"].as_u64().is_some());
            assert!(row["codename"].as_str().is_some());
            assert!(row["status"].as_str().is_some());
            assert!(row["evidence"].as_str().is_some());
            assert!(row["limitation"].as_str().is_some());
        }
    }
    assert!(rows.iter().any(|row| row["status"] == "verified"));
    assert!(rows.iter().any(|row| row["status"] == "no-go-unavailable-hardware"));
}

#[test]
fn each_collector_has_permissions_and_observable_refusal() {
    let value = matrix();
    let collectors = value["collectors"].as_array().expect("collectors");
    assert!(collectors.len() >= 8);

    for collector in collectors {
        assert!(collector["id"].as_str().is_some());
        assert!(collector["status"].as_str().is_some());
        let permissions = &collector["permissions"];
        for key in ["required", "optional", "prohibited"] {
            let entries = permissions[key]
                .as_array()
                .unwrap_or_else(|| panic!("{key} permission list missing"));
            assert!(entries.iter().all(Value::is_string));
        }
        let refusal = &permissions["refusal"];
        assert!(refusal["code"].as_str().is_some());
        assert!(!refusal["when"].as_str().unwrap_or_default().is_empty());
        assert!(!refusal["observable"].as_str().unwrap_or_default().is_empty());
    }
}

#[test]
fn annual_validation_is_reproducible_and_has_a_retirement_rule() {
    let value = matrix();
    let annual = &value["annual_validation"];
    assert_eq!(annual["owner"], "maintainer");
    let cadence = annual["cadence"].as_array().expect("validation cadence");
    assert!(cadence.iter().any(|item| item == "each macOS beta cycle"));
    assert!(cadence.iter().any(|item| item == "each macOS release candidate"));
    let evidence = &annual["evidence"];
    assert!(evidence["format"].as_str().is_some());
    assert!(evidence["required_fields"].as_array().is_some());
    assert!(evidence["retention"].as_str().is_some());
    assert!(annual["retirement_rule"].as_str().is_some());
}

#[cfg(target_os = "macos")]
#[test]
fn running_macos_architecture_is_explicitly_accounted_for() {
    let value = matrix();
    let architecture = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => panic!("unsupported host architecture: {other}"),
    };
    let rows = value["platform_matrix"].as_array().expect("platform rows");
    assert!(rows.iter().any(|row| {
        row["architecture"] == architecture
            && matches!(
                row["status"].as_str(),
                Some("verified") | Some("no-go-unavailable-hardware")
            )
    }));
}
