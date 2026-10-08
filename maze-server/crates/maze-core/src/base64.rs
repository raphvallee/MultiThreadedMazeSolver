//! Zero-allocation base64 codec (RFC 4648, standard alphabet).
//!
//! Both directions write into caller-provided buffers, so cookie handling on
//! the hot path never allocates. Decoding matches what the Daedalus site
//! does (Python `base64.b64decode(s, validate=False)` semantics), so
//! hand-crafted cookies behave identically against both servers:
//!
//! - Bytes outside the standard alphabet (`+ / A-Za-z0-9`) are discarded
//!   before decoding. This includes the URL-safe `-` and `_`, whitespace,
//!   and any punctuation.
//! - `=` counts as padding: the remaining stream is processed in 4-char
//!   groups, and a group is only valid as `xxxx`, `xxx=` or `xx==`. A group
//!   like `x=xx`, `x=`, or a trailing partial group is a structure error
//!   (the site answers those with a 500).
//!
//! Encoding always emits the canonical standard alphabet with `=` padding.
//! All of `+`, `/`, `=` are legal cookie-octets per RFC 6265, so standard
//! base64 is safe to carry in the `path` cookie without quoting.

/// Decode/encode failure with enough detail to log or to reject precisely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base64Error {
    /// The filtered input cannot decode: a partial trailing group, or an `=`
    /// where padding cannot occur. The server maps this to a 500, matching
    /// the site's behavior for structurally broken cookies.
    InvalidLength,
    /// The output buffer is smaller than [`decoded_len`] / [`encoded_len`].
    BufferTooSmall {
        /// Number of bytes the operation would have needed.
        needed: usize,
    },
}

const PAD: u8 = b'=';
const ENC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Decode value for a base64 char; `0xff` marks "discard" (not in the
/// standard alphabet). Note the URL-safe `-` and `_` are deliberately *not*
/// accepted: the site's decoder discards them too.
const fn dec_val(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 0xff,
    }
}

/// Map a byte to its filter class: `Some(v)` for alphabet chars, `None` for
/// `=` (padding, kept but not a value), and a discard marker otherwise.
/// Internally: `0xfe` = padding, `0xff` = discard.
static DEC: [u8; 256] = {
    let mut t = [0xffu8; 256];
    let mut i = 0usize;
    while i < 256 {
        t[i] = dec_val(i as u8);
        i += 1;
    }
    t[b'=' as usize] = 0xfe;
    t
};

/// Byte classification used by both the length probe and the decoder.
/// `0xff` = discard, `0xfe` = padding, anything else is a 6-bit value.
#[inline]
const fn class(c: u8) -> u8 {
    DEC[c as usize]
}

/// Walks the kept (non-discarded) stream in 4-char groups. `f(group, kind)`
/// is called for every complete group, where `kind` is the number of data
/// chars in the group (2, 3, or 4). Returns `Err(())` on structure errors:
/// a partial trailing group, a pad in a position that cannot pad, or a
/// final group with exactly one pad char.
fn for_each_group(input: &[u8], mut f: impl FnMut([u8; 4], usize)) -> Result<(), ()> {
    let mut group = [0u8; 4];
    let mut gi = 0usize;
    for &c in input {
        let k = class(c);
        if k == 0xff {
            continue; // discarded, exactly like the site's decoder
        }
        if gi == 4 {
            return Err(()); // a previous group was left partial
        }
        group[gi] = c;
        gi += 1;
        if gi == 4 {
            // classify the group
            let kind = match (class(group[2]), class(group[3])) {
                (0xfe, 0xfe) => {
                    if class(group[0]) == 0xfe || class(group[1]) == 0xfe {
                        return Err(());
                    }
                    2
                }
                (0xfe, _) | (_, 0xfe) => {
                    if class(group[3]) != 0xfe
                        || class(group[0]) == 0xfe
                        || class(group[1]) == 0xfe
                        || class(group[2]) == 0xfe
                    {
                        return Err(());
                    }
                    3
                }
                _ => {
                    if class(group[0]) == 0xfe || class(group[1]) == 0xfe {
                        return Err(());
                    }
                    4
                }
            };
            f(group, kind);
            gi = 0;
        }
    }
    if gi != 0 {
        return Err(()); // trailing partial group (1..3 kept chars)
    }
    Ok(())
}

/// Number of bytes [`decode_into`] will write for `input`.
///
/// Structurally invalid input also yields a number here (the valid prefix's
/// size); [`decode_into`] is what rejects it.
#[must_use]
pub const fn decoded_len(input: &[u8]) -> usize {
    let mut out = 0usize;
    let mut gi = 0usize;
    let mut group = [0u8; 4];
    let mut i = 0usize;
    while i < input.len() {
        let c = input[i];
        i += 1;
        if class(c) == 0xff {
            continue;
        }
        group[gi] = c;
        gi += 1;
        if gi == 4 {
            out += match (class(group[2]), class(group[3])) {
                (0xfe, 0xfe) => 1,
                (0xfe, _) => 2,
                (_, 0xfe) => 2,
                _ => 3,
            };
            gi = 0;
        }
    }
    out
}

/// Number of bytes [`encode_into`] will write for `input_len` raw bytes.
#[must_use]
pub const fn encoded_len(input_len: usize) -> usize {
    (input_len.div_ceil(3)) * 4
}

/// Decodes `input` into `out`, returning the number of bytes written.
///
/// The input is first filtered exactly like the site's decoder: bytes outside
/// the standard alphabet are discarded. The kept stream is then processed in
/// 4-char groups, where padding may only appear as a trailing `=` (2 bytes)
/// or `==` (1 byte) of a group; any other arrangement, or a partial trailing
/// group, is [`Base64Error::InvalidLength`] (the site answers these with a
/// 500). `out` must be at least [`decoded_len(input)`] long.
pub fn decode_into(input: &[u8], out: &mut [u8]) -> Result<usize, Base64Error> {
    let needed = decoded_len(input);
    if out.len() < needed {
        return Err(Base64Error::BufferTooSmall { needed });
    }
    let mut oi = 0usize;
    let res = for_each_group(input, |group, kind| {
        let v = |c: u8| DEC[c as usize];
        match kind {
            4 => {
                let n = (v(group[0]) as u32) << 18
                    | (v(group[1]) as u32) << 12
                    | (v(group[2]) as u32) << 6
                    | v(group[3]) as u32;
                out[oi] = (n >> 16) as u8;
                out[oi + 1] = (n >> 8) as u8;
                out[oi + 2] = n as u8;
                oi += 3;
            }
            3 => {
                // 16 significant bits: two bytes
                let n = (v(group[0]) as u32) << 10
                    | (v(group[1]) as u32) << 4
                    | (v(group[2]) as u32) >> 2;
                out[oi] = (n >> 8) as u8;
                out[oi + 1] = n as u8;
                oi += 2;
            }
            _ => {
                // 8 significant bits: one byte
                let n = (v(group[0]) as u32) << 2 | (v(group[1]) as u32) >> 4;
                out[oi] = n as u8;
                oi += 1;
            }
        }
    });
    if res.is_err() {
        return Err(Base64Error::InvalidLength);
    }
    Ok(oi)
}

/// Encodes `input` into `out` (standard alphabet, `=` padding), returning the
/// number of bytes written.
///
/// `out` must be at least [`encoded_len(input.len())`] long.
pub fn encode_into(input: &[u8], out: &mut [u8]) -> Result<usize, Base64Error> {
    let needed = encoded_len(input.len());
    if out.len() < needed {
        return Err(Base64Error::BufferTooSmall { needed });
    }
    let mut oi = 0usize;
    let mut i = 0usize;
    while i + 3 <= input.len() {
        let n = (input[i] as u32) << 16 | (input[i + 1] as u32) << 8 | input[i + 2] as u32;
        out[oi] = ENC[(n >> 18) as usize & 63];
        out[oi + 1] = ENC[(n >> 12) as usize & 63];
        out[oi + 2] = ENC[(n >> 6) as usize & 63];
        out[oi + 3] = ENC[n as usize & 63];
        oi += 4;
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = (input[i] as u32) << 16;
        out[oi] = ENC[(n >> 18) as usize & 63];
        out[oi + 1] = ENC[(n >> 12) as usize & 63];
        out[oi + 2] = PAD;
        out[oi + 3] = PAD;
        oi += 4;
    } else if rem == 2 {
        let n = (input[i] as u32) << 16 | (input[i + 1] as u32) << 8;
        out[oi] = ENC[(n >> 18) as usize & 63];
        out[oi + 1] = ENC[(n >> 12) as usize & 63];
        out[oi + 2] = ENC[(n >> 6) as usize & 63];
        out[oi + 3] = PAD;
        oi += 4;
    }
    Ok(oi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_vectors() {
        let cases: &[(&str, &str)] = &[
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for &(raw, enc) in cases {
            // encode
            let mut buf = [0u8; 64];
            let n = encode_into(raw.as_bytes(), &mut buf).unwrap();
            assert_eq!(&buf[..n], enc.as_bytes(), "encoding {raw:?}");
            assert_eq!(n, encoded_len(raw.len()));
            assert_eq!(decoded_len(enc.as_bytes()), raw.len());
            // decode
            let mut out = [0u8; 64];
            let m = decode_into(enc.as_bytes(), &mut out).unwrap();
            assert_eq!(&out[..m], raw.as_bytes(), "decoding {enc:?}");
        }
    }

    #[test]
    fn decode_discards_non_alphabet_bytes() {
        // exactly the site's behavior: invalid chars are dropped before the
        // structure check, so "!!!", "*" or whitespace decode to nothing
        let mut out = [0u8; 8];
        assert_eq!(decode_into(b"!!!", &mut out).unwrap(), 0);
        assert_eq!(decode_into(b"*@@@", &mut out).unwrap(), 0);
        assert_eq!(decode_into(b" Zg== ", &mut out).unwrap(), 1);
        assert_eq!(out[0], b'f');
        // URL-safe alphabet bytes are NOT accepted: '-', '_' are discarded
        assert_eq!(decode_into(b"-_", &mut out).unwrap(), 0);
    }

    #[test]
    fn decode_structure_errors() {
        // the site 500s on these; the router maps InvalidLength to a 500
        let mut out = [0u8; 8];
        assert_eq!(
            decode_into(b"cg", &mut out),
            Err(Base64Error::InvalidLength)
        );
        assert_eq!(
            decode_into(b"cg=", &mut out),
            Err(Base64Error::InvalidLength)
        );
        assert_eq!(
            decode_into(b"cg==x", &mut out),
            Err(Base64Error::InvalidLength)
        );
        assert_eq!(
            decode_into(b"Y=g=", &mut out),
            Err(Base64Error::InvalidLength)
        );
        assert_eq!(
            decode_into(b"cg======", &mut out),
            Err(Base64Error::InvalidLength)
        );
        assert_eq!(decode_into(b"Z", &mut out), Err(Base64Error::InvalidLength));
        // but "Zg==YQ==" is two complete padded groups: "fa"
        assert_eq!(decode_into(b"Zg==YQ==", &mut out).unwrap(), 2);
        assert_eq!(&out[..2], b"fa");
    }

    #[test]
    fn buffer_size_checks() {
        let mut tiny = [0u8; 0];
        assert_eq!(
            decode_into(b"Zg==", &mut tiny),
            Err(Base64Error::BufferTooSmall { needed: 1 })
        );
        let mut small = [0u8; 7];
        assert_eq!(
            encode_into(b"foobar", &mut small),
            Err(Base64Error::BufferTooSmall { needed: 8 })
        );
        // exact sizes succeed
        let mut exact_enc = [0u8; encoded_len(6)];
        assert_eq!(encode_into(b"foobar", &mut exact_enc).unwrap(), 8);
        let mut exact_dec = [0u8; decoded_len(b"Zg==")];
        assert_eq!(decode_into(b"Zg==", &mut exact_dec).unwrap(), 1);
    }

    #[test]
    fn round_trip_all_lengths() {
        for n in 0..=64usize {
            let raw: Vec<u8> = (0..n).map(|i| (i * 37 % 251) as u8).collect();
            let mut enc = [0u8; 512];
            let en = encode_into(&raw, &mut enc).unwrap();
            assert_eq!(en, encoded_len(n));
            let mut dec = [0u8; 512];
            let dn = decode_into(&enc[..en], &mut dec).unwrap();
            assert_eq!(&dec[..dn], raw.as_slice(), "length {n}");
        }
    }

    #[test]
    fn round_trip_all_byte_values() {
        let raw: Vec<u8> = (0..=255u8).collect();
        let mut enc = [0u8; 1024];
        let en = encode_into(&raw, &mut enc).unwrap();
        let mut dec = [0u8; 1024];
        let dn = decode_into(&enc[..en], &mut dec).unwrap();
        assert_eq!(&dec[..dn], raw.as_slice());
    }

    #[test]
    fn empty_io() {
        let mut out = [0u8; 0];
        assert_eq!(decode_into(b"", &mut out).unwrap(), 0);
        assert_eq!(encode_into(b"", &mut out).unwrap(), 0);
    }
}
