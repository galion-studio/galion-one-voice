// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! A small WebSocket (RFC 6455) for loopback. No fragments, no extensions.
//! The accept value is checked against the RFC example.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const MAX_FRAME: usize = 256 * 1024;

#[derive(Debug)]
pub enum WsError {
    Io(std::io::Error),
    Handshake,
    Origin,
    Protocol,
    TooLarge,
    Closed,
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::Io(e) => write!(f, "{e}"),
            WsError::Handshake => write!(f, "handshake"),
            WsError::Origin => write!(f, "origin"),
            WsError::Protocol => write!(f, "protocol"),
            WsError::TooLarge => write!(f, "frame too large"),
            WsError::Closed => write!(f, "closed"),
        }
    }
}

impl std::error::Error for WsError {}

impl From<std::io::Error> for WsError {
    fn from(e: std::io::Error) -> Self {
        WsError::Io(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsFrame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

pub const OP_TEXT: u8 = 0x1;
pub const OP_BIN: u8 = 0x2;
pub const OP_CLOSE: u8 = 0x8;
pub const OP_PING: u8 = 0x9;
pub const OP_PONG: u8 = 0xA;

pub fn accept_key(client_key: &str) -> String {
    let mut data = client_key.as_bytes().to_vec();
    data.extend_from_slice(GUID.as_bytes());
    b64_encode(&sha1(&data))
}

pub fn origin_allowed(origin: &str, port: u16) -> bool {
    origin == format!("http://127.0.0.1:{port}") || origin == format!("http://localhost:{port}")
}

/// `None` when the buffer is still short a frame.
pub fn decode_frame(buf: &[u8]) -> Result<Option<(WsFrame, usize)>, WsError> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let b0 = buf[0];
    let b1 = buf[1];
    if b0 & 0x70 != 0 {
        return Err(WsError::Protocol);
    }
    let fin = b0 & 0x80 != 0;
    let opcode = b0 & 0x0f;
    let masked = b1 & 0x80 != 0;
    let mut len = (b1 & 0x7f) as u64;
    let mut off = 2usize;
    if len == 126 {
        if buf.len() < 4 {
            return Ok(None);
        }
        len = u16::from_be_bytes([buf[2], buf[3]]) as u64;
        off = 4;
    } else if len == 127 {
        if buf.len() < 10 {
            return Ok(None);
        }
        len = u64::from_be_bytes(buf[2..10].try_into().unwrap());
        off = 10;
    }
    if len > MAX_FRAME as u64 {
        return Err(WsError::TooLarge);
    }
    let mask_len = if masked { 4 } else { 0 };
    let total = off + mask_len + len as usize;
    if buf.len() < total {
        return Ok(None);
    }
    if !fin {
        return Err(WsError::Protocol);
    }
    let mut payload = buf[off + mask_len..total].to_vec();
    if masked {
        let mask = &buf[off..off + 4];
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
    }
    if !matches!(opcode, OP_TEXT | OP_BIN | OP_CLOSE | OP_PING | OP_PONG) {
        return Err(WsError::Protocol);
    }
    Ok(Some((WsFrame { opcode, payload }, total)))
}

pub fn encode_frame(opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 14);
    out.push(0x80 | opcode);
    let mask_bit = if mask.is_some() { 0x80 } else { 0 };
    let n = payload.len();
    if n < 126 {
        out.push(mask_bit | n as u8);
    } else if n <= 65535 {
        out.push(mask_bit | 126);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push(mask_bit | 127);
        out.extend_from_slice(&(n as u64).to_be_bytes());
    }
    if let Some(key) = mask {
        out.extend_from_slice(&key);
        for (i, byte) in payload.iter().enumerate() {
            out.push(byte ^ key[i % 4]);
        }
    } else {
        out.extend_from_slice(payload);
    }
    out
}

pub struct WsConn {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl WsConn {
    pub fn connect(addr: std::net::SocketAddr) -> Result<Self, WsError> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let req = format!(
            "GET /v1 HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nOrigin: http://127.0.0.1:{}\r\n\r\n",
            addr.ip(),
            addr.port(),
            addr.port()
        );
        stream.write_all(req.as_bytes())?;
        let resp = read_headers(&mut stream)?;
        if !resp.starts_with("HTTP/1.1 101") {
            return Err(WsError::Handshake);
        }
        let expect = accept_key(key);
        if !resp.contains(&expect) {
            return Err(WsError::Handshake);
        }
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        Ok(Self {
            stream,
            buf: Vec::new(),
        })
    }

    pub fn set_read_timeout(&mut self, t: Option<Duration>) -> Result<(), WsError> {
        self.stream.set_read_timeout(t)?;
        Ok(())
    }

    pub fn send(&mut self, opcode: u8, payload: &[u8]) -> Result<(), WsError> {
        // Client frames are masked. The mask is not a secret.
        let frame = encode_frame(opcode, payload, Some([1, 2, 3, 4]));
        self.stream.write_all(&frame)?;
        Ok(())
    }

    pub fn next_frame(&mut self) -> Result<WsFrame, WsError> {
        loop {
            if let Some(frame) = self.pop()? {
                return Ok(frame);
            }
            let mut tmp = [0u8; 8192];
            let n = self.stream.read(&mut tmp)?;
            if n == 0 {
                return Err(WsError::Closed);
            }
            self.buf.extend_from_slice(&tmp[..n]);
        }
    }

    fn pop(&mut self) -> Result<Option<WsFrame>, WsError> {
        match decode_frame(&self.buf)? {
            Some((frame, used)) => {
                self.buf.drain(..used);
                Ok(Some(frame))
            }
            None => Ok(None),
        }
    }
}

pub fn read_headers(stream: &mut TcpStream) -> Result<String, WsError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while buf.len() < 8192 {
        let n = stream.read(&mut byte)?;
        if n == 0 {
            break;
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            return String::from_utf8(buf).map_err(|_| WsError::Handshake);
        }
    }
    Err(WsError::Handshake)
}

pub fn header<'a>(req: &'a str, name: &str) -> Option<&'a str> {
    for line in req.split("\r\n") {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case(name) {
                return Some(v.trim());
            }
        }
    }
    None
}

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xEFCD_AB89;
    let mut h2: u32 = 0x98BA_DCFE;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xC3D2_E1F0;
    let bit_len = (data.len() as u64).saturating_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for (i, word) in w.iter().copied().enumerate() {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5A82_7999)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ED9_EBA1)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC)
            } else {
                (b ^ c ^ d, 0xCA62_C1D6)
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }
    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

pub fn b64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if i + 1 == data.len() {
        let n = (data[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if i + 2 == data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_and_websocket_accept_match_the_rfc() {
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn masked_frame_roundtrip() {
        let raw = encode_frame(OP_TEXT, b"{\"a\":1}", Some([9, 8, 7, 6]));
        let (frame, used) = decode_frame(&raw).unwrap().unwrap();
        assert_eq!(used, raw.len());
        assert_eq!(frame.opcode, OP_TEXT);
        assert_eq!(frame.payload, b"{\"a\":1}");
        assert!(decode_frame(&raw[..2]).unwrap().is_none());
    }

    #[test]
    fn origin_is_only_this_loopback_port() {
        assert!(origin_allowed("http://127.0.0.1:8788", 8788));
        assert!(origin_allowed("http://localhost:8788", 8788));
        assert!(!origin_allowed("http://127.0.0.1:8788", 8789));
        assert!(!origin_allowed("https://example.com", 8788));
        assert!(!origin_allowed("null", 8788));
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(DIGITS[(byte >> 4) as usize] as char);
            out.push(DIGITS[(byte & 0xf) as usize] as char);
        }
        out
    }
}
