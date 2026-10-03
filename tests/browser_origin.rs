//! Canonical browser navigation evidence: explicit outcomes for every URL
//! class, and property tests that credentials, queries, fragments, and
//! private-context markers never serialize.

use ghostrace::{
    CanonicalNavigation, NavigationHostClass, NavigationRefusal, PathSegmentClass, UrlShapePolicy,
};
use sha2::{Digest, Sha256};

fn origin(raw: &str) -> Result<String, NavigationRefusal> {
    CanonicalNavigation::from_url(raw, false, UrlShapePolicy::OriginOnly)
        .map(|navigation| navigation.origin())
}

#[test]
fn every_url_class_has_an_explicit_outcome() {
    let accepted = [
        ("https://user:secret@example.com/a?token=1#frag", "https://example.com"),
        ("HTTPS://EXAMPLE.COM:443/Path", "https://example.com"),
        ("http://example.com:8080/", "http://example.com:8080"),
        ("https://example.com./", "https://example.com"),
        ("https://\u{430}pple.com/", "https://xn--pple-43d.com"),
        (
            "https://\u{ff45}\u{ff58}\u{ff41}\u{ff4d}\u{ff50}\u{ff4c}\u{ff45}.com/",
            "https://example.com",
        ),
        ("https://b\u{fc}cher.example/", "https://xn--bcher-kva.example"),
        ("https://example.com\\@evil.invalid/", "https://example.com"),
        ("https://evil.invalid#@example.com", "https://evil.invalid"),
        ("https://93.184.215.14/", "https://93.184.215.14"),
        (
            "https://[2606:2800:21f:cb07:6820:80da:af6b:8b2c]/",
            "https://[2606:2800:21f:cb07:6820:80da:af6b:8b2c]",
        ),
        ("https://0x7f.1/", "https://private-network"),
        ("http://192.168.1.10:3000/admin", "http://private-network:3000"),
        ("http://10.0.0.1/", "http://private-network"),
        ("http://169.254.169.254/latest/meta-data", "http://private-network"),
        ("http://100.64.1.2/", "http://private-network"),
        ("http://[::1]/", "http://private-network"),
        ("http://[fd00::1]/", "http://private-network"),
        ("http://[fe80::1]/", "http://private-network"),
        ("http://[::ffff:127.0.0.1]/", "http://private-network"),
        ("http://localhost:3000/admin", "http://private-network:3000"),
        ("http://app.localhost/", "http://private-network"),
        ("http://nas.local/", "http://private-network"),
        ("https://grafana.corp.internal/", "https://private-network"),
        ("http://router.home.arpa/", "http://private-network"),
        ("http://printer.lan/", "http://private-network"),
        ("https://localhost.example.com/", "https://localhost.example.com"),
    ];
    for (raw, expected) in accepted {
        assert_eq!(origin(raw).as_deref(), Ok(expected), "{raw:?}");
    }
    let refused = [
        ("file:///etc/passwd", NavigationRefusal::FileScheme),
        (
            "blob:https://example.com/550e8400-e29b-41d4-a716-446655440000",
            NavigationRefusal::BlobScheme,
        ),
        ("data:text/html,<p>hi</p>", NavigationRefusal::DataScheme),
        ("about:blank", NavigationRefusal::InternalPage),
        ("chrome://settings/passwords", NavigationRefusal::InternalPage),
        ("view-source:https://example.com/", NavigationRefusal::InternalPage),
        (
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/popup.html",
            NavigationRefusal::ExtensionScheme,
        ),
        ("moz-extension://1234/page.html", NavigationRefusal::ExtensionScheme),
        ("javascript:alert(1)", NavigationRefusal::ScriptScheme),
        ("mailto:person@example.com", NavigationRefusal::Opaque),
        ("urn:isbn:0451450523", NavigationRefusal::Opaque),
        ("not a url", NavigationRefusal::Invalid),
        ("https://", NavigationRefusal::Invalid),
        ("https://./", NavigationRefusal::Invalid),
    ];
    for (raw, expected) in refused {
        assert_eq!(origin(raw), Err(expected), "{raw:?}");
    }
    let long = format!("https://example.com/{}", "a".repeat(9000));
    assert_eq!(origin(&long), Err(NavigationRefusal::TooLong));
    assert_eq!(
        CanonicalNavigation::from_url("https://example.com/", true, UrlShapePolicy::OriginOnly),
        Err(NavigationRefusal::PrivateContext)
    );
}

#[test]
fn private_network_hosts_are_withheld_not_just_relabelled() {
    let navigation = CanonicalNavigation::from_url(
        "http://192.168.1.10:3000/",
        false,
        UrlShapePolicy::OriginOnly,
    )
    .expect("navigation");
    assert_eq!(navigation.host_class, NavigationHostClass::PrivateNetwork);
    assert_eq!(navigation.host, None);
    let json = serde_json::to_string(&navigation).expect("JSON");
    assert!(!json.contains("192.168"));
}

#[test]
fn first_path_segment_keeps_only_plain_words_and_digests_everything_else() {
    let shape = |raw: &str| {
        CanonicalNavigation::from_url(raw, false, UrlShapePolicy::FirstPathSegment)
            .expect("navigation")
            .path_segment
            .expect("segment")
    };
    assert_eq!(shape("https://example.com/"), PathSegmentClass::Root);
    assert_eq!(shape("https://example.com"), PathSegmentClass::Root);
    assert_eq!(
        shape("https://example.com/docs/x"),
        PathSegmentClass::Word { value: "docs".into() }
    );
    assert_eq!(
        shape("https://example.com/pull-requests"),
        PathSegmentClass::Word { value: "pull-requests".into() }
    );
    for secret in [
        "https://example.com/Zt7xQ-secret-token",
        "https://example.com/reset-password-now",
        "https://example.com/abcdefghijklmnopq",
        "https://example.com/550e8400",
        "https://example.com/%E2%80%AEfdp",
    ] {
        let class = shape(secret);
        let PathSegmentClass::Opaque { digest } = &class else {
            panic!("{secret} was kept verbatim: {class:?}");
        };
        assert!(digest.starts_with("sha256:"));
        let json = serde_json::to_string(&class).expect("JSON");
        let segment = secret.rsplit('/').next().expect("segment");
        assert!(!json.contains(segment), "{secret}");
    }
    // The same segment on two origins does not produce a linkable digest.
    assert_ne!(shape("https://a.example/Zt7xQ"), shape("https://b.example/Zt7xQ"));
}

#[test]
fn opaque_path_digests_bind_the_complete_retained_origin() {
    let shape = |raw: &str| {
        CanonicalNavigation::from_url(raw, false, UrlShapePolicy::FirstPathSegment)
            .expect("navigation")
            .path_segment
            .expect("segment")
    };
    let baseline = shape("https://example.com/Zt7xQ");
    assert_eq!(
        baseline,
        PathSegmentClass::Opaque {
            digest: "sha256:7101d302ea56404605c4f874aa18d234fa50a1bd13255fdad1253365b13411b1"
                .into(),
        },
        "v2 digest domain and framing golden"
    );
    assert_ne!(baseline, shape("http://example.com/Zt7xQ"), "scheme boundary");
    assert_ne!(baseline, shape("https://example.com:8443/Zt7xQ"), "port boundary");
    assert_ne!(baseline, shape("https://other.example/Zt7xQ"), "host boundary");
    assert_ne!(baseline, shape("https://example.com/Ab4Cd9"), "segment boundary");
    for equivalent in [
        "HTTPS://EXAMPLE.COM:443/Zt7xQ",
        "https://example.com./Zt7xQ",
        "https://user:password@example.com/Zt7xQ?token=secret#fragment",
        "https://example.com/Zt7xQ/other-path",
    ] {
        assert_eq!(baseline, shape(equivalent), "canonical equivalence");
    }
    // Withheld private hosts intentionally share the same minimized origin.
    // A digest is not evidence that two internal services are the same host.
    assert_eq!(shape("https://10.1.2.3/Zt7xQ"), shape("https://nas.local/Zt7xQ"));
    assert_ne!(
        shape("https://10.1.2.3/Zt7xQ"),
        shape("http://10.1.2.3/Zt7xQ"),
        "private-network scheme boundary"
    );
    assert_ne!(
        shape("https://10.1.2.3/Zt7xQ"),
        shape("https://private-network/Zt7xQ"),
        "host class boundary"
    );
}

#[test]
fn maximum_size_navigation_has_a_bounded_shape_and_refusals_are_path_free() {
    let prefix = "https://example.com/";
    let maximum =
        format!("{prefix}{}", "A".repeat(ghostrace::MAX_BROWSER_URL_BYTES - prefix.len()));
    let navigation =
        CanonicalNavigation::from_url(&maximum, false, UrlShapePolicy::FirstPathSegment)
            .expect("maximum-size navigation");
    let PathSegmentClass::Opaque { digest } = navigation.path_segment.as_ref().expect("segment")
    else {
        panic!("maximum segment was retained");
    };
    assert_eq!(digest.len(), "sha256:".len() + 64);
    assert!(serde_json::to_vec(&navigation).expect("JSON").len() < 256);
    let oversized = format!("{maximum}A");
    assert_eq!(
        CanonicalNavigation::from_url(&oversized, false, UrlShapePolicy::FirstPathSegment),
        Err(NavigationRefusal::TooLong)
    );
    // Private-context refusal precedes even URL validation and byte admission.
    assert_eq!(
        CanonicalNavigation::from_url(&oversized, true, UrlShapePolicy::FirstPathSegment),
        Err(NavigationRefusal::PrivateContext)
    );
    for raw in ["file:///SENTINELPATH", "invalid:SENTINELSECRET", "https://[SENTINELHOST"] {
        let refusal = CanonicalNavigation::from_url(raw, false, UrlShapePolicy::OriginOnly)
            .expect_err("refused URL");
        let diagnostic =
            format!("{refusal:?} {refusal} {}", serde_json::to_string(&refusal).unwrap());
        assert!(!diagnostic.contains("SENTINEL"));
        assert!(!diagnostic.contains(raw));
    }
}

/// A deterministic generator so the property test needs no extra dependency.
struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        let digest = Sha256::digest(self.0.to_le_bytes());
        u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

#[test]
fn credentials_queries_fragments_and_private_markers_never_serialize() {
    let mut generator = Generator(0x6768_6f73);
    let hosts = [
        "example.com",
        "sub.example.org",
        "b\u{fc}cher.example",
        "93.184.215.14",
        "10.1.2.3",
        "[fd00::7]",
    ];
    let schemes = ["http", "https", "HTTPS"];
    let paths = ["", "/", "/docs", "/a/b/c", "/Zt7xQ", "/%2e%2e/x"];
    for case in 0..2_000u32 {
        let user = format!("SENTINELUSER{case}");
        let password = format!("SENTINELPASS{case}");
        let query = format!("SENTINELQUERY{case}");
        let fragment = format!("SENTINELFRAG{case}");
        let raw = format!(
            "{}://{user}:{password}@{}{}?token={query}&incognito=1#{fragment}",
            generator.pick(&schemes),
            generator.pick(&hosts),
            generator.pick(&paths),
        );
        let private = generator.next() % 5 == 0;
        let policy = if generator.next() % 2 == 0 {
            UrlShapePolicy::OriginOnly
        } else {
            UrlShapePolicy::FirstPathSegment
        };
        match CanonicalNavigation::from_url(&raw, private, policy) {
            Ok(navigation) => {
                assert!(!private, "a private-context navigation was accepted");
                let json = serde_json::to_string(&navigation).expect("JSON");
                let rendered = format!("{json}{}", navigation.origin());
                for sentinel in [&user, &password, &query, &fragment] {
                    assert!(!rendered.contains(sentinel.as_str()), "{raw} leaked {sentinel}");
                }
                assert!(!rendered.contains("incognito"), "{raw}");
                assert!(
                    !rendered.contains('?') && !rendered.contains('#') && !rendered.contains('@')
                );
            }
            Err(refusal) => {
                assert!(private, "{raw} was refused: {refusal:?}");
                assert_eq!(refusal, NavigationRefusal::PrivateContext);
                assert!(!refusal.to_string().contains("SENTINEL"));
            }
        }
    }
}
