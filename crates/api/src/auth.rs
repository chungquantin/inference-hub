//! Bearer-token protection for the admin routes, with an optional allow-list
//! of client networks on top.

use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use crate::{AppState, error::ApiError};

/// Rejects requests without `Authorization: Bearer <ADMIN_TOKEN>`.
///
/// When no admin token is configured the admin API does not exist: every
/// request gets 404, so an unconfigured deployment exposes nothing.
pub async fn require_admin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.admin_token.as_deref() else {
        return ApiError::NotFound("route").into_response();
    };
    if let Some(allowed) = &state.admin_allowed {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        match allowed.client_ip(request.headers(), peer) {
            Some(ip) if allowed.contains(ip) => {}
            ip => {
                tracing::warn!(client = ?ip, "admin request from a network not in ADMIN_ALLOWED_IPS");
                // Same answer as a disabled admin API: nothing to probe.
                return ApiError::NotFound("route").into_response();
            }
        }
    }
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if bool::from(presented.as_bytes().ct_eq(expected.as_bytes())) {
        next.run(request).await
    } else {
        ApiError::Unauthorized.into_response()
    }
}

/// Networks admin requests may come from (`ADMIN_ALLOWED_IPS`), such as
/// `203.0.113.7, 2001:db8::/32`.
///
/// The client address is the TCP peer, unless `CLIENT_IP_HEADER` names a
/// header that a trusted proxy in front of the API sets, such as Railway's
/// `X-Real-IP`. Only name a header the proxy overwrites: clients can send any
/// header they like.
#[derive(Debug, Clone)]
pub struct AllowList {
    networks: Vec<(IpAddr, u8)>,
    header: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("`{0}` is not an IP address or CIDR network")]
pub struct BadNetwork(String);

impl AllowList {
    pub fn parse(list: &str, header: Option<String>) -> Result<Self, BadNetwork> {
        let networks = list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                let (addr, bits) = s.split_once('/').unwrap_or((s, ""));
                let ip: IpAddr = addr.parse().map_err(|_| BadNetwork(s.into()))?;
                let max = if ip.is_ipv4() { 32 } else { 128 };
                let bits = if bits.is_empty() {
                    max
                } else {
                    bits.parse::<u8>()
                        .ok()
                        .filter(|b| *b <= max)
                        .ok_or_else(|| BadNetwork(s.into()))?
                };
                Ok((ip, bits))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            networks,
            header: header
                .map(|h| h.trim().to_ascii_lowercase())
                .filter(|h| !h.is_empty()),
        })
    }

    /// The client's address: from the trusted header when one is configured,
    /// otherwise the TCP peer.
    pub fn client_ip(&self, headers: &HeaderMap, peer: Option<IpAddr>) -> Option<IpAddr> {
        match &self.header {
            Some(name) => headers
                .get(name.as_str())
                .and_then(|v| v.to_str().ok())
                // X-Forwarded-For style lists: the proxy appends the client last.
                .and_then(|v| v.rsplit(',').next())
                .and_then(|v| v.trim().parse().ok()),
            None => peer,
        }
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let ip = match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
            v4 => v4,
        };
        self.networks.iter().any(|(net, bits)| match (net, ip) {
            (IpAddr::V4(n), IpAddr::V4(a)) => prefix_eq(&n.octets(), &a.octets(), *bits),
            (IpAddr::V6(n), IpAddr::V6(a)) => prefix_eq(&n.octets(), &a.octets(), *bits),
            _ => false,
        })
    }
}

fn prefix_eq(a: &[u8], b: &[u8], bits: u8) -> bool {
    let full = usize::from(bits / 8);
    let rest = bits % 8;
    a[..full] == b[..full]
        && (rest == 0 || {
            let mask = 0xffu8 << (8 - rest);
            a[full] & mask == b[full] & mask
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn matches_addresses_and_networks() {
        let list = AllowList::parse("203.0.113.7, 10.0.0.0/8, 2001:db8::/32", None).unwrap();
        assert!(list.contains(ip("203.0.113.7")));
        assert!(!list.contains(ip("203.0.113.8")));
        assert!(list.contains(ip("10.200.1.1")));
        assert!(list.contains(ip("2001:db8:abcd::1")));
        assert!(!list.contains(ip("2001:db9::1")));
        assert!(list.contains(ip("::ffff:10.1.2.3")), "IPv4-mapped IPv6");
        let odd = AllowList::parse("192.168.4.0/22", None).unwrap();
        assert!(odd.contains(ip("192.168.7.255")));
        assert!(!odd.contains(ip("192.168.8.0")));
    }

    #[test]
    fn rejects_bad_entries() {
        assert!(AllowList::parse("not-an-ip", None).is_err());
        assert!(AllowList::parse("10.0.0.0/33", None).is_err());
    }

    #[test]
    fn reads_the_client_from_the_trusted_header_only_when_configured() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.4".parse().unwrap());
        let peer = Some(ip("10.0.0.1"));
        let direct = AllowList::parse("10.0.0.1", None).unwrap();
        assert_eq!(
            direct.client_ip(&headers, peer),
            peer,
            "headers are ignored without CLIENT_IP_HEADER"
        );
        let proxied = AllowList::parse("198.51.100.4", Some("X-Real-IP".into())).unwrap();
        assert_eq!(proxied.client_ip(&headers, peer), Some(ip("198.51.100.4")));
        headers.insert("x-forwarded-for", "1.2.3.4, 198.51.100.9".parse().unwrap());
        let xff = AllowList::parse("0.0.0.0/0", Some("x-forwarded-for".into())).unwrap();
        assert_eq!(xff.client_ip(&headers, peer), Some(ip("198.51.100.9")));
    }
}
