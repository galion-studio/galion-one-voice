// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    let rest = data.len() - i;
    if rest == 1 {
        let n = (data[i] as u32) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

pub fn b64_decode(text: &str) -> Result<Vec<u8>, String> {
    let bytes = text.as_bytes();
    let mut filtered = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if b.is_ascii_whitespace() {
            continue;
        }
        filtered.push(b);
    }
    if filtered.len() % 4 != 0 {
        return Err("base64 length is not a multiple of 4".into());
    }
    let mut out = Vec::with_capacity(filtered.len() / 4 * 3);
    for chunk in filtered.chunks_exact(4) {
        let (a, b, c, d) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let av = dec_byte(a)?;
        let bv = dec_byte(b)?;
        let cv = if c == b'=' { 0 } else { dec_byte(c)? };
        let dv = if d == b'=' { 0 } else { dec_byte(d)? };
        out.push(((av << 2) | (bv >> 4)) as u8);
        if c != b'=' {
            out.push((((bv & 0x0f) << 4) | (cv >> 2)) as u8);
        }
        if d != b'=' {
            out.push((((cv & 0x03) << 6) | dv) as u8);
        }
    }
    Ok(out)
}

fn dec_byte(b: u8) -> Result<u8, String> {
    let v = match b {
        b'A'..=b'Z' => b - b'A',
        b'a'..=b'z' => b - b'a' + 26,
        b'0'..=b'9' => b - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return Err(format!("bad base64 byte {b}")),
    };
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_lengths() {
        for n in 0..16 {
            let data: Vec<u8> = (0..n)
                .map(|i| (i as u8).wrapping_mul(17).wrapping_add(3))
                .collect();
            let enc = b64_encode(&data);
            let dec = b64_decode(&enc).unwrap();
            assert_eq!(dec, data, "{enc}");
        }
        assert_eq!(b64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(b64_decode("aGVs bG8=").unwrap(), b"hello");
    }
}
