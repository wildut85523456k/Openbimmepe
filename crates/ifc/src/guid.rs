//! IFC GlobalIds that do not change between exports.
//!
//! A GlobalId is 128 bits written in 22 characters. Here the bits are laid out, most significant first, as
//! 48 bits of project key, 16 bits saying which part of the element it is (the element itself, its n-th
//! opening, its n-th port), and the 64-bit element number from the operation log (author in the high half).
//! Two exports of an unchanged model therefore carry the same ids, an edited element keeps its id, and ids
//! of different elements, parts or projects cannot collide.

const CHARS: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// Largest project key (48 bits).
pub const MAX_PROJECT_KEY: u64 = (1 << 48) - 1;

pub fn bits(project_key: u64, part: u16, element: u64) -> u128 {
    (u128::from(project_key & MAX_PROJECT_KEY) << 80) | (u128::from(part) << 64) | u128::from(element)
}

/// The 22-character form: the first character carries the top 2 bits, the others 6 bits each.
pub fn compress(v: u128) -> String {
    let mut out = [0u8; 22];
    out[0] = CHARS[(v >> 126) as usize];
    for (k, o) in out.iter_mut().enumerate().skip(1) {
        let shift = 126 - 6 * k as u32;
        *o = CHARS[((v >> shift) & 63) as usize];
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn expand(s: &str) -> Option<u128> {
    let b = s.as_bytes();
    if b.len() != 22 {
        return None;
    }
    let mut v: u128 = 0;
    for (k, c) in b.iter().enumerate() {
        let d = CHARS.iter().position(|x| x == c)? as u128;
        if k == 0 && d > 3 {
            return None;
        }
        v = (v << if k == 0 { 2 } else { 6 }) | d;
    }
    Some(v)
}

pub fn global_id(project_key: u64, part: u16, element: u64) -> String {
    compress(bits(project_key, part, element))
}
