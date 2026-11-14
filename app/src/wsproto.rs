//! Minimal WebSocket pieces (RFC 6455) shared by the match server and the native game client:
//! SHA-1 + base64 for the handshake, and frame encoding/decoding. No external crates.

#![allow(dead_code)]

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for i in 0..5 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

pub fn accept_key(key: &str) -> String {
    let mut k = key.trim().to_string();
    k.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64(&sha1(k.as_bytes()))
}

pub const OP_TEXT: u8 = 1;
pub const OP_BINARY: u8 = 2;
pub const OP_CLOSE: u8 = 8;
pub const OP_PING: u8 = 9;
pub const OP_PONG: u8 = 10;

/// Encodes one final frame. Clients must mask; servers must not.
pub fn encode_frame(opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mut f = Vec::with_capacity(payload.len() + 14);
    f.push(0x80 | opcode);
    let m = if mask.is_some() { 0x80 } else { 0 };
    let n = payload.len();
    if n < 126 {
        f.push(m | n as u8);
    } else if n < 65536 {
        f.push(m | 126);
        f.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        f.push(m | 127);
        f.extend_from_slice(&(n as u64).to_be_bytes());
    }
    match mask {
        Some(k) => {
            f.extend_from_slice(&k);
            f.extend(payload.iter().enumerate().map(|(i, b)| b ^ k[i % 4]));
        }
        None => f.extend_from_slice(payload),
    }
    f
}

/// Collects bytes from the socket and yields complete messages (handles fragmentation).
#[derive(Default)]
pub struct FrameReader {
    buf: Vec<u8>,
    partial: Vec<u8>,
    partial_op: u8,
}

pub const MAX_MESSAGE: usize = 1 << 20;

impl FrameReader {
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Next complete message as (opcode, payload); Err on a protocol violation.
    pub fn next(&mut self) -> Option<Result<(u8, Vec<u8>), ()>> {
        loop {
            if self.buf.len() < 2 {
                return None;
            }
            let fin = self.buf[0] & 0x80 != 0;
            let op = self.buf[0] & 0x0f;
            let masked = self.buf[1] & 0x80 != 0;
            let mut len = (self.buf[1] & 0x7f) as usize;
            let mut pos = 2;
            if len == 126 {
                if self.buf.len() < 4 {
                    return None;
                }
                len = u16::from_be_bytes([self.buf[2], self.buf[3]]) as usize;
                pos = 4;
            } else if len == 127 {
                if self.buf.len() < 10 {
                    return None;
                }
                let mut b = [0u8; 8];
                b.copy_from_slice(&self.buf[2..10]);
                let l = u64::from_be_bytes(b);
                if l > MAX_MESSAGE as u64 {
                    return Some(Err(()));
                }
                len = l as usize;
                pos = 10;
            }
            if len > MAX_MESSAGE {
                return Some(Err(()));
            }
            let mut key = [0u8; 4];
            if masked {
                if self.buf.len() < pos + 4 {
                    return None;
                }
                key.copy_from_slice(&self.buf[pos..pos + 4]);
                pos += 4;
            }
            if self.buf.len() < pos + len {
                return None;
            }
            let mut payload: Vec<u8> = self.buf[pos..pos + len].to_vec();
            if masked {
                for (i, b) in payload.iter_mut().enumerate() {
                    *b ^= key[i % 4];
                }
            }
            self.buf.drain(0..pos + len);
            if op >= 8 {
                return Some(Ok((op, payload))); // control frames are never fragmented
            }
            if op != 0 {
                self.partial_op = op;
                self.partial.clear();
            }
            self.partial.extend_from_slice(&payload);
            if self.partial.len() > MAX_MESSAGE {
                return Some(Err(()));
            }
            if fin {
                let msg = std::mem::take(&mut self.partial);
                return Some(Ok((self.partial_op, msg)));
            }
        }
    }
}

/// Removes characters that the line protocol uses as separators.
pub fn clean_field(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| !matches!(c, '|' | ';' | '~' | ',' | '\n' | '\r'))
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handshake_key() {
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
