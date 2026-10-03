//! Canonical browser navigation evidence.
//!
//! A navigation is reduced to a bounded origin, or to an origin plus one
//! policy-selected path class, before it can be retained. Parsing uses the
//! WHATWG URL standard (`url` crate), so the host the browser would contact is
//! the host GHOSTRACE records; prefix or regex validation is never used. Only
//! `http` and `https` navigations are accepted; every other scheme is refused
//! with an explicit reason. Userinfo, query, fragment, and any private-context
//! marker never reach the output type, which has no field that could hold them.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::{Host, Url};

use crate::model::MAX_BROWSER_URL_BYTES;

/// Longest path segment kept verbatim by [`UrlShapePolicy::FirstPathSegment`].
pub const MAX_RETAINED_PATH_SEGMENT: usize = 16;

/// Why a navigation cannot become evidence. Values never include the URL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Error)]
#[serde(rename_all = "snake_case")]
pub enum NavigationRefusal {
    #[error("the URL exceeds its byte bound")]
    TooLong,
    #[error("the URL is not a valid absolute URL")]
    Invalid,
    #[error("file URLs are local paths, not navigations")]
    FileScheme,
    #[error("blob URLs name in-page objects, not navigations")]
    BlobScheme,
    #[error("data URLs carry page content")]
    DataScheme,
    #[error("about and browser-internal pages are not navigations")]
    InternalPage,
    #[error("extension pages are not web navigations")]
    ExtensionScheme,
    #[error("script URLs are code, not navigations")]
    ScriptScheme,
    #[error("the URL has an opaque scheme without a host")]
    Opaque,
    #[error("the navigation came from a private or incognito context")]
    PrivateContext,
}

/// The class of host a navigation reached.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationHostClass {
    /// A registrable DNS name, stored in ASCII (punycode) form.
    Domain,
    /// A public IPv4 or IPv6 address.
    PublicAddress,
    /// Loopback, private, link-local, or unique-local address; the address
    /// itself is withheld because it can reveal internal services.
    PrivateNetwork,
}

/// How much of the URL beyond the origin a policy retains.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlShapePolicy {
    /// Scheme, host, and non-default port only.
    #[default]
    OriginOnly,
    /// The origin plus the first path segment when it is a short, plain
    /// word; anything longer or token-like is replaced by a digest class.
    FirstPathSegment,
}

/// The only retainable form of a navigation.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalNavigation {
    pub scheme: String,
    pub host_class: NavigationHostClass,
    /// ASCII host for domains and public addresses; absent for private-network hosts.
    pub host: Option<String>,
    /// Explicit non-default port.
    pub port: Option<u16>,
    /// Present only under [`UrlShapePolicy::FirstPathSegment`].
    pub path_segment: Option<PathSegmentClass>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case", deny_unknown_fields)]
pub enum PathSegmentClass {
    Root,
    Word {
        value: String,
    },
    /// A segment that could be an identifier or secret; only a digest,
    /// domain-separated by host class and the complete retained origin, is
    /// kept. Withheld private hosts intentionally share a minimized origin.
    Opaque {
        digest: String,
    },
}

impl CanonicalNavigation {
    /// Canonicalize a navigation. `private_context` is the browser's incognito
    /// or private-window flag; such navigations are refused before parsing.
    pub fn from_url(
        raw: &str,
        private_context: bool,
        policy: UrlShapePolicy,
    ) -> Result<Self, NavigationRefusal> {
        if private_context {
            return Err(NavigationRefusal::PrivateContext);
        }
        if raw.len() > MAX_BROWSER_URL_BYTES {
            return Err(NavigationRefusal::TooLong);
        }
        let url = Url::parse(raw).map_err(|_| NavigationRefusal::Invalid)?;
        match url.scheme() {
            "http" | "https" => {}
            "file" => return Err(NavigationRefusal::FileScheme),
            "blob" => return Err(NavigationRefusal::BlobScheme),
            "data" => return Err(NavigationRefusal::DataScheme),
            "about" | "chrome" | "edge" | "brave" | "opera" | "vivaldi" | "view-source" => {
                return Err(NavigationRefusal::InternalPage)
            }
            "chrome-extension" | "moz-extension" | "safari-web-extension" | "extension" => {
                return Err(NavigationRefusal::ExtensionScheme)
            }
            "javascript" | "vbscript" => return Err(NavigationRefusal::ScriptScheme),
            _ => return Err(NavigationRefusal::Opaque),
        }
        let (host_class, host) = match url.host() {
            Some(Host::Domain(domain)) => {
                let domain = domain.trim_end_matches('.');
                if domain.is_empty() {
                    return Err(NavigationRefusal::Invalid);
                }
                let domain = domain.to_ascii_lowercase();
                if is_private_name(&domain) {
                    (NavigationHostClass::PrivateNetwork, None)
                } else {
                    (NavigationHostClass::Domain, Some(domain))
                }
            }
            Some(Host::Ipv4(address)) => classify_ip(IpAddr::V4(address)),
            Some(Host::Ipv6(address)) => classify_ip(IpAddr::V6(address)),
            None => return Err(NavigationRefusal::Opaque),
        };
        let port = url.port();
        let mut navigation =
            Self { scheme: url.scheme().to_owned(), host_class, host, port, path_segment: None };
        if policy == UrlShapePolicy::FirstPathSegment {
            navigation.path_segment = Some(path_class(&url, &navigation));
        }
        Ok(navigation)
    }

    /// The canonical origin string, e.g. `https://example.com:8443`. A
    /// private-network host renders as `private-network`.
    pub fn origin(&self) -> String {
        let host = self.host.as_deref().unwrap_or("private-network");
        match self.port {
            Some(port) => format!("{}://{host}:{port}", self.scheme),
            None => format!("{}://{host}", self.scheme),
        }
    }
}

/// Names that only resolve on the local machine or network. Like private
/// addresses, they can reveal internal services, so they are withheld.
fn is_private_name(domain: &str) -> bool {
    const PRIVATE_SUFFIXES: [&str; 5] = [".localhost", ".local", ".internal", ".home.arpa", ".lan"];
    domain == "localhost" || PRIVATE_SUFFIXES.iter().any(|suffix| domain.ends_with(suffix))
}

fn classify_ip(address: IpAddr) -> (NavigationHostClass, Option<String>) {
    if is_private(address) {
        (NavigationHostClass::PrivateNetwork, None)
    } else {
        let text = match address {
            IpAddr::V4(address) => address.to_string(),
            IpAddr::V6(address) => format!("[{address}]"),
        };
        (NavigationHostClass::PublicAddress, Some(text))
    }
}

fn is_private(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_private_v4(address),
        IpAddr::V6(address) => {
            if let Some(mapped) = address.to_ipv4_mapped() {
                return is_private_v4(mapped);
            }
            is_private_v6(address)
        }
    }
}

fn is_private_v4(address: Ipv4Addr) -> bool {
    let [a, b, ..] = address.octets();
    address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_broadcast()
        // Carrier-grade NAT 100.64.0.0/10.
        || (a == 100 && (64..=127).contains(&b))
}

fn is_private_v6(address: Ipv6Addr) -> bool {
    let first = address.segments()[0];
    address.is_loopback()
        || address.is_unspecified()
        // Unique local fc00::/7 and link-local fe80::/10.
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
}

fn path_class(url: &Url, navigation: &CanonicalNavigation) -> PathSegmentClass {
    let Some(segment) = url.path_segments().and_then(|mut segments| segments.next()) else {
        return PathSegmentClass::Root;
    };
    if segment.is_empty() {
        return PathSegmentClass::Root;
    }
    let plain_word = segment.len() <= MAX_RETAINED_PATH_SEGMENT
        && segment.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'-' || byte == b'_')
        && segment.bytes().filter(|byte| *byte == b'-' || *byte == b'_').count() <= 1;
    if plain_word {
        return PathSegmentClass::Word { value: segment.to_owned() };
    }
    let mut hasher = Sha256::new();
    // This pre-collector digest domain replaces v1, which omitted the scheme.
    // Hash only the retained origin: never reintroduce a withheld private host,
    // userinfo, query or fragment through the digest input. Length framing and
    // a stable host-class tag keep component and placeholder boundaries distinct.
    hasher.update(b"ghostrace-navigation-path-segment-v2\0");
    hasher.update([match navigation.host_class {
        NavigationHostClass::Domain => 0,
        NavigationHostClass::PublicAddress => 1,
        NavigationHostClass::PrivateNetwork => 2,
    }]);
    let origin = navigation.origin();
    hasher.update((origin.len() as u64).to_le_bytes());
    hasher.update(origin.as_bytes());
    hasher.update((segment.len() as u64).to_le_bytes());
    hasher.update(segment.as_bytes());
    let digest = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    PathSegmentClass::Opaque { digest: format!("sha256:{digest}") }
}
