//! What `nix build --dry-run` says it would do, read off its standard error.
//!
//! Nix prints two headings and nothing else of the kind:
//!
//! ```text
//! these 3 derivations will be built:          (or "this derivation will be built:")
//! these 12 paths will be fetched (34.56 MiB download, 120.00 MiB unpacked):
//! ```
//!
//! — the size in the unit Nix chose (`KiB`, `MiB`, `GiB`…). A heading that is
//! absent is nothing to do: no build, no download.

use crate::Rehearsal;

/// The rehearsal `text` describes.
#[must_use]
pub fn rehearsal(text: &str) -> Rehearsal {
    let mut r = Rehearsal {
        download_bytes: 0,
        builds: 0,
    };
    for line in text.lines().map(str::trim) {
        if line.ends_with("will be built:") {
            r.builds += count(line);
        } else if line.contains("will be fetched (") {
            r.download_bytes += download(line).unwrap_or(0);
        }
    }
    r
}

/// The N of "these N …"; 1 for "this …".
fn count(line: &str) -> u64 {
    let mut words = line.split_whitespace();
    match words.next() {
        Some("these") => words.next().and_then(|n| n.parse().ok()).unwrap_or(0),
        Some("this") => 1,
        _ => 0,
    }
}

/// The bytes of "(<size> <unit> download, …)".
fn download(line: &str) -> Option<u64> {
    let inside = line.split_once('(')?.1;
    let before = inside.split_once(" download")?.0;
    let (number, unit) = before.trim().rsplit_once(' ')?;
    let number: f64 = number.trim().parse().ok()?;
    let scale: f64 = match unit {
        "B" | "bytes" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let bytes = (number * scale).round();
    if bytes.is_finite() && bytes >= 0.0 {
        // In range by the guard: a dry run never fetches 2^64 bytes.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(bytes as u64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_headings_are_read_in_every_unit() {
        let text = "warning: Git tree is dirty\n\
                    these 3 derivations will be built:\n  /nix/store/a.drv\n  /nix/store/b.drv\n  /nix/store/c.drv\n\
                    these 12 paths will be fetched (34.50 MiB download, 120.00 MiB unpacked):\n  /nix/store/x\n";
        let r = rehearsal(text);
        assert_eq!(r.builds, 3);
        assert_eq!(r.download_bytes, 36_175_872);
        let one = rehearsal(
            "this derivation will be built:\n  /nix/store/a.drv\nthis path will be fetched (1.5 KiB download, 3.0 KiB unpacked):\n",
        );
        assert_eq!((one.builds, one.download_bytes), (1, 1536));
        let big = rehearsal("these 2 paths will be fetched (1.25 GiB download, 4 GiB unpacked):");
        assert_eq!(big.download_bytes, 1_342_177_280);
    }

    #[test]
    fn nothing_said_is_nothing_to_do() {
        let r = rehearsal("");
        assert_eq!((r.builds, r.download_bytes), (0, 0));
        let odd = rehearsal("these many paths will be fetched (lots download):");
        assert_eq!(
            odd.download_bytes, 0,
            "a size it cannot read is not invented"
        );
    }
}
