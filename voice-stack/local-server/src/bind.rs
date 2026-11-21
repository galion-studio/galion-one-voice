// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback bind for `GALION_SAMANTHA_WS`.
//! `127.0.0.1` and `::1` are accepted. `0.0.0.0` and every other address are refused.

use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotLoopback(pub IpAddr);

impl std::fmt::Display for NotLoopback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "refusing bind on {}; samantha-local listens on 127.0.0.1 or ::1 only",
            self.0
        )
    }
}

/// `127.0.0.0/8` and `::1`. Nothing else.
pub fn require_loopback(addr: SocketAddr) -> Result<(), NotLoopback> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(NotLoopback(addr.ip()))
    }
}

/// An `Origin` header, when the browser sends one, must be this process or the
/// existing one-dot page on port 7878. A missing header is allowed for a
/// same-machine client. `https` and any other host are refused.
pub fn origin_allowed(origin: Option<&str>, bound: SocketAddr) -> bool {
    let Some(origin) = origin.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if origin == crate::protocol::ONE_DOT_ORIGIN
        || origin == format!("http://127.0.0.1:{}", crate::protocol::DEFAULT_PORT)
    {
        return true;
    }
    let Some(rest) = origin.strip_prefix("http://") else {
        return false;
    };
    let hostport = rest.split('/').next().unwrap_or("");
    match bound.ip() {
        IpAddr::V4(ip) => hostport == format!("{ip}:{}", bound.port()),
        IpAddr::V6(ip) if ip.is_loopback() => hostport == format!("[::1]:{}", bound.port()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn loopback_v4_and_v6_only() {
        assert!(require_loopback(addr("127.0.0.1:7881")).is_ok());
        assert!(require_loopback(addr("127.0.0.9:1")).is_ok());
        assert!(require_loopback(addr("[::1]:7881")).is_ok());
        assert!(require_loopback(addr("0.0.0.0:7881")).is_err());
        assert!(require_loopback(addr("192.168.1.10:7881")).is_err());
        assert!(require_loopback(addr("[::]:7881")).is_err());
    }

    #[test]
    fn origin_is_this_socket_or_the_one_dot_page() {
        let bound = addr("127.0.0.1:7881");
        assert!(origin_allowed(None, bound));
        assert!(origin_allowed(Some("http://127.0.0.1:7881"), bound));
        assert!(origin_allowed(Some("http://127.0.0.1:7878"), bound));
        assert!(!origin_allowed(Some("https://127.0.0.1:7881"), bound));
        assert!(!origin_allowed(Some("http://evil.example"), bound));
        assert!(!origin_allowed(Some("null"), bound));
        let moved = addr("127.0.0.1:9000");
        assert!(origin_allowed(Some("http://127.0.0.1:9000"), moved));
        assert!(origin_allowed(Some("http://127.0.0.1:7878"), moved));
        let v6 = addr("[::1]:7881");
        assert!(origin_allowed(Some("http://[::1]:7881"), v6));
        assert!(origin_allowed(Some("http://127.0.0.1:7881"), v6));
        assert!(origin_allowed(Some("http://127.0.0.1:7878"), v6));
    }
}
