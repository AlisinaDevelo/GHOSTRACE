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

/// The product's private local-service transport is Unix IPC, not TCP. The
/// denied runner must keep that transport available while still rejecting IP.
#[cfg(unix)]
#[test]
#[ignore = "run through scripts/offline-network-test.sh"]
fn local_unix_ipc_remains_available_in_the_denied_runner() {
    use std::{
        io::{Read, Write},
        os::unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    };

    assert_eq!(env::var("GHOSTRACE_OFFLINE_ENFORCED").as_deref(), Ok("1"));
    let mode = env::var("GHOSTRACE_OFFLINE_MODE").expect("offline runner mode");
    assert!(
        matches!(mode.as_str(), "sandbox-exec" | "docker-network-none" | "linux-network-namespace"),
        "unknown offline runner mode: {mode}"
    );
    let directory = tempfile::tempdir().expect("private IPC directory");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private IPC permissions");
    let path = directory.path().join("canary.sock");
    let listener = UnixListener::bind(&path).expect("local Unix bind must remain available");
    let mut client = UnixStream::connect(&path).expect("local Unix connect");
    let (mut server, _) = listener.accept().expect("local Unix accept");
    client.set_write_timeout(Some(Duration::from_secs(1))).expect("write timeout");
    server.set_read_timeout(Some(Duration::from_secs(1))).expect("read timeout");
    client.write_all(b"local-ipc").expect("local Unix write");
    let mut message = [0; 9];
    server.read_exact(&mut message).expect("local Unix read");
    assert_eq!(&message, b"local-ipc");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "run through scripts/offline-network-test.sh"]
fn macos_ip_bind_and_ipv6_connect_remain_denied() {
    use std::net::{TcpListener, UdpSocket};

    assert_eq!(env::var("GHOSTRACE_OFFLINE_MODE").as_deref(), Ok("sandbox-exec"));
    assert_eq!(env::var("GHOSTRACE_OFFLINE_ENFORCED").as_deref(), Ok("1"));
    for address in ["127.0.0.1:0", "[::1]:0"] {
        let tcp = TcpListener::bind(address).expect_err("IP TCP bind unexpectedly allowed");
        assert_eq!(tcp.kind(), io::ErrorKind::PermissionDenied, "{address}: {tcp:?}");
        let udp = UdpSocket::bind(address).expect_err("IP UDP bind unexpectedly allowed");
        assert_eq!(udp.kind(), io::ErrorKind::PermissionDenied, "{address}: {udp:?}");
    }
    let ipv6 = TcpStream::connect_timeout(
        &"[::1]:9".parse().expect("IPv6 loopback"),
        Duration::from_millis(250),
    )
    .expect_err("IPv6 TCP connect unexpectedly allowed");
    assert_eq!(ipv6.kind(), io::ErrorKind::PermissionDenied, "{ipv6:?}");
}
