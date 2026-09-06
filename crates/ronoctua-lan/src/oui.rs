//! Who MADE the thing at a hardware address.
//!
//! # Where the table comes from
//!
//! `nmap`'s own `nmap-mac-prefixes`, which is already in this system's
//! closure (`nix/modules/cli.nix`), so the table costs nothing new. Its shape
//! is one line per prefix: six hexadecimal digits, a space, and the name.
//!
//! # Why a vendor is worth a column
//!
//! Because a list of addresses says nothing about what the machines ARE. "a
//! phone, a printer, three of somebody's light bulbs" is what a person is
//! looking for, and the manufacturer is the only part of a hardware address
//! that carries it.

use std::collections::HashMap;
use std::path::Path;

/// The variable that names the table.
///
/// FROM THE ENVIRONMENT AND NOT `option_env!`, which is the trap
/// `RONOCTUA_ICONS` and `RONOCTUA_SWAPPY` both carry a comment about: a store
/// path compiled in is a reference the BINARY carries, so a nmap release
/// would rebuild the whole workspace for a table nothing else reads. Unset,
/// the table is empty and the column stays blank — which is what an
/// unregistered prefix looks like anyway.
pub const VARIABLE: &str = "RONOCTUA_NMAP_MAC_PREFIXES";

/// The table's path, from the environment.
#[must_use]
pub fn table() -> Option<std::path::PathBuf> {
    std::env::var_os(VARIABLE)
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
}

/// The table, read once.
#[derive(Debug, Default, Clone)]
pub struct Vendors(HashMap<[u8; 3], String>);

impl Vendors {
    /// Reads the table. An absent one is an empty table, not an error: the
    /// column simply stays blank, which is what it looks like for a prefix
    /// nobody registered anyway.
    #[must_use]
    pub fn read(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|t| Self::of(&t))
            .unwrap_or_default()
    }

    /// PURE, and that is what makes the grammar testable: the file is 40 000
    /// lines and its shape is two fields.
    #[must_use]
    pub fn of(content: &str) -> Self {
        let mut out = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((prefix, name)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            let Some(bytes) = triple(prefix) else {
                continue;
            };
            out.insert(bytes, name.trim().to_owned());
        }
        Self(out)
    }

    /// The maker of a hardware address, when the table knows it.
    #[must_use]
    pub fn of_address(&self, hardware: &str) -> Option<&str> {
        let flat: String = hardware.chars().filter(char::is_ascii_hexdigit).collect();
        self.0.get(&triple(&flat)?).map(String::as_str)
    }

    /// How many prefixes it knows — what says whether the table was there.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is the table empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The first three bytes of a hexadecimal string.
fn triple(hex: &str) -> Option<[u8; 3]> {
    if hex.len() < 6 {
        return None;
    }
    let mut out = [0u8; 3];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file's shape, with invented names — what is proved here is the
    /// grammar, not somebody's registration (invariant 13).
    const TABLE_TEXT: &str = "\
# a comment line
000000 A Maker
AABBCC Another Maker Ltd
";

    #[test]
    fn a_prefix_finds_its_maker_however_the_address_is_written() {
        let v = Vendors::of(TABLE_TEXT);
        assert_eq!(v.len(), 2);
        // THE THREE SPELLINGS a hardware address arrives in: the kernel's
        // colons, a browser's dashes, and the table's own bare digits.
        assert_eq!(v.of_address("aa:bb:cc:dd:ee:ff"), Some("Another Maker Ltd"));
        assert_eq!(v.of_address("AA-BB-CC-DD-EE-FF"), Some("Another Maker Ltd"));
        assert_eq!(v.of_address("AABBCCDDEEFF"), Some("Another Maker Ltd"));
        assert_eq!(v.of_address("00:00:00:11:22:33"), Some("A Maker"));
        // AND AN UNKNOWN PREFIX IS A BLANK COLUMN, never a guess.
        assert_eq!(v.of_address("ff:ff:ff:00:00:00"), None);
        assert_eq!(v.of_address("nonsense"), None);
    }

    #[test]
    fn an_absent_table_is_an_empty_one_and_not_a_failure() {
        let v = Vendors::read(Path::new("/nonexistent/nmap-mac-prefixes"));
        assert!(v.is_empty());
        assert_eq!(v.of_address("aa:bb:cc:dd:ee:ff"), None);
    }
}
