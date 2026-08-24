use std::{env, io, net::TcpStream, time::Duration};

const SANDBOX_PROBE_ADDRESS: &str = "127.0.0.1:9";
const NAMESPACE_PROBE_ADDRESS: &str = "198.51.100.1:80";

/// This test is intentionally ignored in the ordinary suite. It must run only
/// inside the explicit network-denial wrapper, where the runner identifies the
/// mechanism and the expected kernel error. A normal connection refusal is not
/// accepted: that would prove only that the port is closed, not that networking
/// is denied.
#[test]
#[ignore = "run through scripts/offline-network-test.sh"]
fn network_denial_canary_proves_the_runner_is_enforced() {
    assert_eq!(
        env::var("GHOSTRACE_OFFLINE_ENFORCED").as_deref(),
        Ok("1"),
        "the canary must run inside the checked-in denial wrapper"
    );

    let mode = env::var("GHOSTRACE_OFFLINE_MODE").expect("offline runner mode");
    let probe_address = match mode.as_str() {
        "sandbox-exec" => SANDBOX_PROBE_ADDRESS,
        "docker-network-none" | "linux-network-namespace" => NAMESPACE_PROBE_ADDRESS,
        other => panic!("unknown offline runner mode: {other}"),
    };
    let error = TcpStream::connect_timeout(
        &probe_address.parse().expect("probe address"),
        Duration::from_millis(250),
    )
    .expect_err("the canary connection unexpectedly succeeded");

    match mode.as_str() {
        "sandbox-exec" => assert_eq!(
            error.kind(),
            io::ErrorKind::PermissionDenied,
            "sandbox-exec must return permission denied, got {error:?}"
        ),
        "docker-network-none" | "linux-network-namespace" => assert!(
            matches!(
                error.kind(),
                io::ErrorKind::NetworkUnreachable
                    | io::ErrorKind::HostUnreachable
                    | io::ErrorKind::PermissionDenied
            ),
            "an isolated network namespace must have no reachable route, got {error:?}"
        ),
        other => panic!("unknown offline runner mode: {other}"),
    }
}
