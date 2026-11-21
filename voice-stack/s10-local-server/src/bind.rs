// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Loopback policy. Same idea as `deploy/systemd/templates/galion-voice-stage.service`:
//! the socket is `127.0.0.1`, and a page from anywhere else is refused.
//! This module does not install a unit and does not open a public port.

use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotLoopback(pub IpAddr);

impl std::fmt::Display for NotLoopback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "refusing bind on {}; samantha-local listens on 127.0.0.1 only",
            self.0
        )
    }
}

/// Accept `127.0.0.0/8`. Refuse `0.0.0.0`, every other IPv4 address, and IPv6
/// (including `::1`). The default the binary uses is `127.0.0.1`.
pub fn require_ipv4_loopback(addr: SocketAddr) -> Result<(), NotLoopback> {
    match addr.ip() {
        IpAddr::V4(ip) if ip.octets()[0] == 127 => Ok(()),
        other => Err(NotLoopback(other)),
    }
}

/// Browser `Origin` values this server will upgrade.
/// Missing or empty is allowed so a same-machine test client can connect.
/// `http://127.0.0.1` and `http://localhost` (optional port) are allowed.
/// Anything else, including `https` and `null`, is refused.
pub fn origin_is_local(origin: Option<&str>) -> bool {
    let Some(origin) = origin.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    let Some(rest) = origin.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or("");
    let host_only = host.split(':').next().unwrap_or("");
    host_only == "127.0.0.1" || host_only == "localhost"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn loopback_range_only() {
        assert!(require_ipv4_loopback(addr("127.0.0.1:8790")).is_ok());
        assert!(require_ipv4_loopback(addr("127.0.0.1:0")).is_ok());
        assert!(require_ipv4_loopback(addr("127.0.0.9:1")).is_ok());
        assert!(require_ipv4_loopback(addr("0.0.0.0:8790")).is_err());
        assert!(require_ipv4_loopback(addr("192.168.1.10:8790")).is_err());
        assert!(require_ipv4_loopback(addr("[::1]:8790")).is_err());
    }

    #[test]
    fn origin_policy() {
        assert!(origin_is_local(None));
        assert!(origin_is_local(Some("")));
        assert!(origin_is_local(Some("http://127.0.0.1:8790")));
        assert!(origin_is_local(Some("http://localhost:8790")));
        assert!(origin_is_local(Some("http://127.0.0.1")));
        assert!(!origin_is_local(Some("https://127.0.0.1:8790")));
        assert!(!origin_is_local(Some("http://evil.example")));
        assert!(!origin_is_local(Some("http://127.0.0.1.evil.example")));
        assert!(!origin_is_local(Some("null")));
    }
}
