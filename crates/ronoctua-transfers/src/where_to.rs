//! Where a download lands, and which engine takes it.
//!
//! Two questions every door into the queue asks — the `.desktop` that catches a
//! magnet, the browser's native-messaging host, `LEADER t a` — and neither of
//! them belongs in three places. A caller may always name a directory instead;
//! this is only the answer when nobody did.

use crate::job::{Want, Work};
use std::path::PathBuf;

/// Where downloads go.
///
/// `~/.config/user-dirs.dirs` and not the environment: the variable is only
/// exported inside a graphical session, and this is read by a `.desktop`
/// handler, a systemd unit and a browser's child process alike. The file is
/// what a file dialog, a browser download and `xdg-open` actually read — which
/// is exactly the reason `nix/modules/home.nix` writes it.
#[must_use]
pub fn landing() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(from_file) = home.as_ref().and_then(|h| download_in(h)) {
        return from_file;
    }
    // The environment second, for a session that exported it without the file.
    if let Some(dir) = std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from) {
        return dir;
    }
    home.map_or_else(|| PathBuf::from("."), |h| h.join("Downloads"))
}

/// Reads `XDG_DOWNLOAD_DIR` out of the user-dirs file.
fn download_in(home: &std::path::Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(home.join(".config").join("user-dirs.dirs")).ok()?;
    let value = value_of(&text, "XDG_DOWNLOAD_DIR")?;
    Some(expand(home, &value))
}

/// The value of one key in the user-dirs file, unquoted.
fn value_of(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        return Some(rest.trim().trim_matches('"').to_owned());
    }
    None
}

/// `$HOME/...` as written in that file, made absolute.
fn expand(home: &std::path::Path, value: &str) -> PathBuf {
    for prefix in ["$HOME/", "${HOME}/"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

/// Which engine an address belongs to, as a [`Work`].
///
/// ONE PLACE, and every door uses it: a magnet handed to aria2 comes back with
/// a message about an unknown scheme, and a YouTube page handed to aria2 comes
/// back as a few kilobytes of HTML named after the video. Guessing in three
/// places is guessing differently in three places.
///
/// `want` only matters when the address turns out to be a page with media on
/// it; for everything else it is ignored.
#[must_use]
pub fn work_for(address: &str, into: PathBuf, want: Option<Want>) -> Work {
    let address = address.trim();
    if crate::torrent::takes(address) {
        return Work::Torrent {
            source: address.to_owned(),
            into,
        };
    }
    if let Some(want) = want {
        return Work::Media {
            url: address.to_owned(),
            into,
            want,
        };
    }
    Work::Fetch {
        url: address.to_owned(),
        into,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_magnet_never_reaches_the_link_engine() {
        let into = PathBuf::from("/onde/vai");
        assert!(matches!(
            work_for("magnet:?xt=urn:btih:abc", into.clone(), None),
            Work::Torrent { .. }
        ));
        // Even when the caller asked for media: a magnet is not a page.
        assert!(matches!(
            work_for(
                "magnet:?xt=urn:btmh:1220ab",
                into.clone(),
                Some(Want::Video)
            ),
            Work::Torrent { .. }
        ));
        assert!(matches!(
            work_for("https://exemplo/x.iso", into.clone(), None),
            Work::Fetch { .. }
        ));
        assert!(matches!(
            work_for("https://exemplo/assistir", into, Some(Want::Audio)),
            Work::Media {
                want: Want::Audio,
                ..
            }
        ));
    }

    #[test]
    fn the_user_dirs_file_is_read_the_way_the_file_dialog_reads_it() {
        let text = "# comentario\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/main/temp\"\n";
        assert_eq!(
            value_of(text, "XDG_DOWNLOAD_DIR").as_deref(),
            Some("$HOME/main/temp")
        );
        assert_eq!(
            expand(std::path::Path::new("/casa/alguem"), "$HOME/main/temp"),
            PathBuf::from("/casa/alguem/main/temp")
        );
        // A commented-out line is not a value, and a key that is a prefix of
        // another one must not match it.
        assert_eq!(
            value_of("#XDG_DOWNLOAD_DIR=\"/x\"\n", "XDG_DOWNLOAD_DIR"),
            None
        );
        // And a key that is a PREFIX of another one must not match it: without
        // the `=` after the name, `XDG_DOWNLOAD` would answer with the
        // directory of `XDG_DOWNLOAD_DIR`.
        assert_eq!(value_of(text, "XDG_DOWNLOAD"), None);
    }
}
