//! The shields `endpoint` badge: the URL guard and the schema-v1 reader.
//!
//! `/endpoint?url=` makes the server fetch a URL a stranger chose, so the
//! guard here is the first of two layers (the second is the connector's DNS
//! filter in `application::upstream`): https only, a public host, the default
//! port, no credentials. Everything is a pure function of its input.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use reqwest::Url;
use serde_json::Value;

/// Longest endpoint URL accepted.
const MAX_URL: usize = 2048;
/// Longest label or message kept (shields truncates nothing; a README badge
/// that wide is a mistake, and the cap bounds the render).
const MAX_TEXT: usize = 120;

/// What an endpoint's JSON asks the badge to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EndpointBadge {
    pub label: String,
    pub message: String,
    pub color: Option<String>,
    pub label_color: Option<String>,
    pub is_error: bool,
}

/// Why an endpoint URL was refused (the badge message, never an error page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UrlRefusal {
    Malformed,
    NotHttps,
    Credentials,
    PrivateHost,
    Port,
}

impl UrlRefusal {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Malformed => "invalid url",
            Self::NotHttps => "https only",
            Self::Credentials => "invalid url",
            Self::PrivateHost => "host not allowed",
            Self::Port => "port not allowed",
        }
    }
}

/// Whether `ip` is a globally routable unicast address. Private, loopback,
/// link-local, CGNAT, documentation, benchmarking, multicast and reserved
/// ranges are refused, including their IPv4-mapped and NAT64 forms.
pub(crate) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => public_v6(v6),
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..=127).contains(&b)) // CGNAT 100.64.0.0/10
        || (a == 192 && b == 0 && c == 0) // IETF protocol assignments
        || (a == 198 && (b == 18 || b == 19)) // benchmarking
        || a >= 240) // reserved
}

fn public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return public_v4(v4);
    }
    let s = ip.segments();
    // 64:ff9b::/96 (NAT64) embeds an IPv4 address in the last 32 bits.
    if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        let [a, b] = s[6].to_be_bytes();
        let [c, d] = s[7].to_be_bytes();
        return public_v4(Ipv4Addr::new(a, b, c, d));
    }
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || s[..6] == [0; 6] // ::/96 IPv4-compatible
        || s[0] == 0x2002 // 6to4
        || (s[0] == 0x2001 && s[1] == 0) // Teredo 2001::/32
        || (s[0] & 0xffc0) == 0xfec0 // site-local fec0::/10
        || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (s[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
        || (s[0] == 0x0100 && s[1..4] == [0, 0, 0])) // discard-only
}

/// Parse and vet an endpoint URL. `reqwest::Url` normalizes the odd IPv4
/// spellings (decimal, hex, octal) to a dotted address before it is judged.
pub(crate) fn check_url(raw: &str) -> Result<Url, UrlRefusal> {
    if raw.len() > MAX_URL {
        return Err(UrlRefusal::Malformed);
    }
    let url = Url::parse(raw.trim()).map_err(|_| UrlRefusal::Malformed)?;
    if url.scheme() != "https" {
        return Err(UrlRefusal::NotHttps);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlRefusal::Credentials);
    }
    if url.port().is_some_and(|p| p != 443) {
        return Err(UrlRefusal::Port);
    }
    let host = url.host_str().ok_or(UrlRefusal::Malformed)?;
    match host.trim_matches(['[', ']']).parse::<IpAddr>() {
        Ok(ip) => public_host(is_public_ip(ip)),
        Err(_) => {
            let d = host.trim_end_matches('.').to_ascii_lowercase();
            let ours = d == "sylphx.com" || d.ends_with(".sylphx.com");
            let internal = ours
                || !d.contains('.')
                || d == "localhost"
                || [".localhost", ".local", ".internal", ".home.arpa", ".lan"]
                    .iter()
                    .any(|s| d.ends_with(s));
            public_host(!internal)
        }
    }
    .map(|()| url)
}

fn public_host(ok: bool) -> Result<(), UrlRefusal> {
    if ok {
        Ok(())
    } else {
        Err(UrlRefusal::PrivateHost)
    }
}

fn text(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?;
    Some(
        s.chars()
            .filter(|c| !c.is_control())
            .take(MAX_TEXT)
            .collect(),
    )
}

/// Read a shields endpoint document (`schemaVersion` 1: `label`, `message`,
/// optional `color`, `labelColor`, `isError`). `None` is "invalid
/// properties". Logo fields are ignored: a remote document never injects an
/// image into our SVG.
pub(crate) fn parse(body: &str) -> Option<EndpointBadge> {
    let v: Value = serde_json::from_str(body).ok()?;
    if v.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let label = text(v.get("label"))?;
    let message = text(v.get("message"))?;
    Some(EndpointBadge {
        label,
        message,
        color: text(v.get("color")).filter(|c| !c.is_empty()),
        label_color: text(v.get("labelColor")).filter(|c| !c.is_empty()),
        is_error: v.get("isError").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_https_hosts_pass() {
        for ok in [
            "https://example.com/badge.json",
            "https://example.com:443/x?y=1",
            "https://8.8.8.8/x",
            "https://[2606:4700:4700::1111]/x",
        ] {
            assert!(check_url(ok).is_ok(), "{ok}");
        }
        let refused = [
            ("http://example.com/x", UrlRefusal::NotHttps),
            ("ftp://example.com/x", UrlRefusal::NotHttps),
            ("https://localhost/x", UrlRefusal::PrivateHost),
            ("https://LOCALHOST./x", UrlRefusal::PrivateHost),
            ("https://app.localhost/x", UrlRefusal::PrivateHost),
            ("https://intranet/x", UrlRefusal::PrivateHost),
            ("https://db.internal/x", UrlRefusal::PrivateHost),
            ("https://127.0.0.1/x", UrlRefusal::PrivateHost),
            (
                "https://mark.sylphx.com/badge/a-b-c",
                UrlRefusal::PrivateHost,
            ),
            ("https://sylphx.com/x", UrlRefusal::PrivateHost),
            ("https://[::7f00:1]/x", UrlRefusal::PrivateHost),
            ("https://[2002:7f00:1::]/x", UrlRefusal::PrivateHost),
            ("https://[2001:0:4136:e378::1]/x", UrlRefusal::PrivateHost),
            ("https://[fec0::1]/x", UrlRefusal::PrivateHost),
            ("https://2130706433/x", UrlRefusal::PrivateHost),
            ("https://0x7f.1/x", UrlRefusal::PrivateHost),
            ("https://10.0.0.5/x", UrlRefusal::PrivateHost),
            ("https://172.16.0.1/x", UrlRefusal::PrivateHost),
            ("https://192.168.1.1/x", UrlRefusal::PrivateHost),
            (
                "https://169.254.169.254/latest/meta-data",
                UrlRefusal::PrivateHost,
            ),
            ("https://100.64.0.1/x", UrlRefusal::PrivateHost),
            ("https://0.0.0.0/x", UrlRefusal::PrivateHost),
            ("https://[::1]/x", UrlRefusal::PrivateHost),
            ("https://[::ffff:127.0.0.1]/x", UrlRefusal::PrivateHost),
            ("https://[fd00::1]/x", UrlRefusal::PrivateHost),
            ("https://[fe80::1]/x", UrlRefusal::PrivateHost),
            ("https://[64:ff9b::a00:1]/x", UrlRefusal::PrivateHost),
            ("https://example.com:8443/x", UrlRefusal::Port),
            ("https://user:pw@example.com/x", UrlRefusal::Credentials),
            ("https://user@example.com/x", UrlRefusal::Credentials),
            ("not a url", UrlRefusal::Malformed),
            ("", UrlRefusal::Malformed),
        ];
        for (raw, why) in refused {
            assert_eq!(check_url(raw).err(), Some(why), "{raw}");
        }
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL));
        assert_eq!(check_url(&long).err(), Some(UrlRefusal::Malformed));
    }

    #[test]
    fn schema_v1_is_read_and_anything_else_is_invalid() {
        let b = parse(
            r#"{"schemaVersion":1,"label":"build","message":"passing","color":"brightgreen","labelColor":"333","isError":false,"namedLogo":"x","logoSvg":"<svg onload=1>"}"#,
        )
        .expect("valid");
        assert_eq!(b.label, "build");
        assert_eq!(b.color.as_deref(), Some("brightgreen"));
        assert_eq!(b.label_color.as_deref(), Some("333"));
        assert!(!b.is_error);
        assert!(parse(r#"{"schemaVersion":2,"label":"a","message":"b"}"#).is_none());
        assert!(parse(r#"{"label":"a","message":"b"}"#).is_none());
        assert!(parse(r#"{"schemaVersion":1,"label":"a"}"#).is_none());
        assert!(parse("<html>").is_none());
        let long = format!(
            r#"{{"schemaVersion":1,"label":"a","message":"{}"}}"#,
            "x".repeat(500)
        );
        assert_eq!(parse(&long).unwrap().message.len(), MAX_TEXT);
    }
}
