//! Client identity for the per-client upstream budget.
//!
//! The host sits behind Cloudflare, which names the viewer in
//! `CF-Connecting-IP`; `X-Forwarded-For` is the fallback. Only the budget on
//! upstream loads reads this key (cached answers never do), and IPv6 clients
//! share one budget per /64 so a rotating suffix does not evade it.

use std::net::IpAddr;

use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;

use crate::bootstrap::AppState;

/// Charge the upstream loads of this request to its client.
pub(super) async fn scope(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let key = client_key(req.headers());
    st.live.scope_client(&key, next.run(req)).await
}

fn first_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    headers
        .get(name)?
        .to_str()
        .ok()?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// The budget key: the IPv4 address, the IPv6 /64, or `unknown`.
pub(super) fn client_key(headers: &HeaderMap) -> String {
    match first_ip(headers, "cf-connecting-ip").or_else(|| first_ip(headers, "x-forwarded-for")) {
        Some(IpAddr::V4(v4)) => v4.to_string(),
        Some(IpAddr::V6(v6)) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
        None => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, v.parse().unwrap());
        }
        h
    }

    #[test]
    fn the_edge_header_wins_and_v6_shares_a_64() {
        let both = headers(&[
            ("cf-connecting-ip", "203.0.113.7"),
            ("x-forwarded-for", "198.51.100.1"),
        ]);
        assert_eq!(client_key(&both), "203.0.113.7");
        let xff = headers(&[("x-forwarded-for", "198.51.100.1, 10.0.0.1")]);
        assert_eq!(client_key(&xff), "198.51.100.1");
        let a = headers(&[("cf-connecting-ip", "2001:db8:1:2:aaaa::1")]);
        let b = headers(&[("cf-connecting-ip", "2001:db8:1:2:bbbb::9")]);
        assert_eq!(client_key(&a), client_key(&b));
        assert_eq!(
            client_key(&headers(&[("cf-connecting-ip", "junk")])),
            "unknown"
        );
        assert_eq!(client_key(&HeaderMap::new()), "unknown");
    }
}
