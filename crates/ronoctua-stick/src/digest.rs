//! SHA-256, as this crate says it: lower-case hex, the way `SHA256SUMS`
//! writes it.
//!
//! One wrapper over the tree's one implementation of the digest, so the
//! queue's verification, the root writer's and the read-back all hash the
//! same way — and so the day the provider changes, one file does.

/// A running SHA-256.
pub struct Sha256(aws_lc_rs::digest::Context);

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// An empty one.
    #[must_use]
    pub fn new() -> Self {
        Self(aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256))
    }

    /// More bytes.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// The digest, in lower-case hex.
    #[must_use]
    pub fn hex(self) -> String {
        hex_of(self.0.finish().as_ref())
    }
}

/// Bytes as lower-case hex.
#[must_use]
pub fn hex_of(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

/// Whether `text` is a SHA-256 as `SHA256SUMS` writes one: 64 hex digits.
/// Upper case is read too; [`same`] compares without case.
#[must_use]
pub fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether two digests in hex are the same digest.
#[must_use]
pub fn same(a: &str, b: &str) -> bool {
    is_sha256(a) && is_sha256(b) && a.eq_ignore_ascii_case(b)
}

/// The SHA-256 of everything `from` reads, and how many bytes that was;
/// `seen` hears the running count after each read.
///
/// # Errors
///
/// What reading said.
pub fn of_reader(
    from: &mut dyn std::io::Read,
    seen: &mut dyn FnMut(u64),
) -> std::io::Result<(String, u64)> {
    let mut sha = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let n = match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        sha.update(&buffer[..n]);
        total += n as u64;
        seen(total);
    }
    Ok((sha.hex(), total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_string_and_abc_hash_as_the_standard_says() {
        // FIPS 180-2's own vectors.
        assert_eq!(
            Sha256::new().hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let mut s = Sha256::new();
        s.update(b"a");
        s.update(b"bc");
        assert_eq!(
            s.hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let (hex, n) = of_reader(&mut &b"abc"[..], &mut |_| {}).unwrap();
        assert_eq!(n, 3);
        assert!(same(
            &hex,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        ));
        assert!(!same(&hex, "ba78"));
        assert!(!is_sha256(&"g".repeat(64)));
    }
}
