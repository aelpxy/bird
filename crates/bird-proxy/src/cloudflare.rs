use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use hyper::HeaderMap;
use hyper::header::HeaderName;

const CF_CONNECTING_IP: HeaderName = HeaderName::from_static("cf-connecting-ip");

// https://www.cloudflare.com/ips-v4/ and /ips-v6/
const V4: [(Ipv4Addr, u32); 15] = [
    (Ipv4Addr::new(173, 245, 48, 0), 20),
    (Ipv4Addr::new(103, 21, 244, 0), 22),
    (Ipv4Addr::new(103, 22, 200, 0), 22),
    (Ipv4Addr::new(103, 31, 4, 0), 22),
    (Ipv4Addr::new(141, 101, 64, 0), 18),
    (Ipv4Addr::new(108, 162, 192, 0), 18),
    (Ipv4Addr::new(190, 93, 240, 0), 20),
    (Ipv4Addr::new(188, 114, 96, 0), 20),
    (Ipv4Addr::new(197, 234, 240, 0), 22),
    (Ipv4Addr::new(198, 41, 128, 0), 17),
    (Ipv4Addr::new(162, 158, 0, 0), 15),
    (Ipv4Addr::new(104, 16, 0, 0), 13),
    (Ipv4Addr::new(104, 24, 0, 0), 14),
    (Ipv4Addr::new(172, 64, 0, 0), 13),
    (Ipv4Addr::new(131, 0, 72, 0), 22),
];
const V6: [(Ipv6Addr, u32); 7] = [
    (Ipv6Addr::new(0x2400, 0xcb00, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2803, 0xf800, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2405, 0xb500, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2405, 0x8100, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2a06, 0x98c0, 0, 0, 0, 0, 0, 0), 29),
    (Ipv6Addr::new(0x2c0f, 0xf248, 0, 0, 0, 0, 0, 0), 32),
];

/// The visitor Cloudflare names, only when the request really came from Cloudflare, since
/// anyone reaching the server directly could send the header
pub(crate) fn visitor(peer: IpAddr, headers: &HeaderMap) -> Option<IpAddr> {
    if !is_cloudflare(peer.to_canonical()) {
        return None;
    }
    headers.get(CF_CONNECTING_IP)?.to_str().ok()?.parse().ok()
}

fn is_cloudflare(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => V4.iter().any(|&(network, prefix)| {
            (u32::from(ip) ^ u32::from(network)).leading_zeros() >= prefix
        }),
        IpAddr::V6(ip) => V6.iter().any(|&(network, prefix)| {
            (u128::from(ip) ^ u128::from(network)).leading_zeros() >= prefix
        }),
    }
}

#[cfg(test)]
mod tests {
    use hyper::header::HeaderValue;

    use super::*;

    fn headers(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CF_CONNECTING_IP, HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn trusts_the_header_only_from_cloudflare() {
        let headers = headers("203.0.113.7");
        let visitor = Some("203.0.113.7".parse().unwrap());
        for edge in [
            "104.16.1.1",
            "172.71.255.255",
            "2606:4700::1",
            "::ffff:162.158.3.4",
        ] {
            assert_eq!(
                super::visitor(edge.parse().unwrap(), &headers),
                visitor,
                "{edge}"
            );
        }
        for other in ["104.32.0.1", "8.8.8.8", "2606:4701::1", "127.0.0.1"] {
            assert_eq!(
                super::visitor(other.parse().unwrap(), &headers),
                None,
                "{other}"
            );
        }
    }

    #[test]
    fn ignores_a_missing_or_malformed_header() {
        let edge = "104.16.1.1".parse().unwrap();
        assert_eq!(visitor(edge, &HeaderMap::new()), None);
        assert_eq!(visitor(edge, &headers("not an ip")), None);
        assert_eq!(
            visitor(edge, &headers("2001:db8::1")),
            Some("2001:db8::1".parse().unwrap())
        );
    }
}
