// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Minimal std-only PNG writer (W21-04) for headless one-dot frames.
//!
//! RGB8, no interlace, zlib stream made of *stored* (uncompressed) deflate blocks,
//! so there is no compression dependency. Files are ~3 bytes/pixel (a 320×180 frame
//! is ≈ 173 KB) — fine for a demo PNG sequence or a single snapshot. Input is the same
//! `0x00RRGGBB` framebuffer that [`crate::dot::rasterize`] fills.

use std::io::Write;
use std::path::Path;

const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for d in chunks {
        for &b in d.iter() {
            c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    !c
}

fn adler32(d: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in d.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[ty, data]).to_be_bytes());
}

/// Encode a `w × h` `0x00RRGGBB` buffer as an RGB8 PNG. Panics if `buf.len() != w*h`.
pub fn encode_rgb(w: u32, h: u32, buf: &[u32]) -> Vec<u8> {
    assert_eq!(buf.len(), (w as usize) * (h as usize), "framebuffer size mismatch");
    // Raw scanlines: filter byte 0 + RGB.
    let mut raw = Vec::with_capacity((w as usize * 3 + 1) * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        for &p in &buf[y * w as usize..(y + 1) * w as usize] {
            raw.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
    }
    let mut z = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    z.extend_from_slice(&[0x78, 0x01]);
    let blocks: Vec<&[u8]> = if raw.is_empty() { vec![&[][..]] } else { raw.chunks(65535).collect() };
    let n = blocks.len();
    for (i, b) in blocks.into_iter().enumerate() {
        z.push(if i + 1 == n { 1 } else { 0 });
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = Vec::with_capacity(z.len() + 64);
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour, deflate, filter 0, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Encode and write atomically-ish (write `*.tmp`, then rename) so a viewer polling
/// the file never sees a half-written frame.
pub fn write_rgb(path: impl AsRef<Path>, w: u32, h: u32, buf: &[u32]) -> std::io::Result<()> {
    let path = path.as_ref();
    let tmp = path.with_extension("png.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&encode_rgb(w, h, buf))?;
    }
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_and_adler_known_values() {
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }
    #[test]
    fn png_structure() {
        let buf = vec![0x00F5F5F7u32; 4 * 3];
        let p = encode_rgb(4, 3, &buf);
        assert_eq!(&p[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&p[12..16], b"IHDR");
        assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
        // stored data length = h * (1 + 3w) = 39
        assert!(p.len() > 39 + 8 + 25 + 12);
    }
    #[test]
    fn large_frame_multi_block() {
        let (w, h) = (320u32, 180u32);
        let p = encode_rgb(w, h, &vec![0x000A0A0C; (w * h) as usize]);
        assert!(p.len() > (w * h * 3) as usize);
    }
}
