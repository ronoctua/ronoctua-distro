//! Where a download lands, and which engine takes it.
//!
//! Two questions every door into the queue asks — the `.desktop` that catches a
//! magnet, the browser's native-messaging host, `LEADER t a` — and neither of
//! them belongs in three places. A caller may always name a directory instead;
//! this is only the answer when nobody did.

use crate::job::{Identity, Want, Work};
use std::path::PathBuf;

/// Where downloads go: the user's download directory, by the one reader of
/// `user-dirs.dirs` the tree has (`ronoctua_xdg::user_dir`).
#[must_use]
pub fn landing() -> PathBuf {
    ronoctua_xdg::user_dir("XDG_DOWNLOAD_DIR", "Downloads")
}

/// What was asked for, beyond the address itself.
///
/// A struct and not two more positional arguments: they are both small enums,
/// and two of those in a row is a call whose arguments can be swapped without
/// the compiler noticing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Wish {
    /// Which part of a page's media, or `None` to fetch the address itself.
    pub want: Option<Want>,
    /// Whose session the request is made with.
    pub identity: Identity,
}

impl Wish {
    /// The address itself, as nobody. What a `.desktop` handler asks for.
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            want: None,
            identity: Identity::Anonymous,
        }
    }

    /// A page's media, as nobody.
    #[must_use]
    pub const fn media(want: Want) -> Self {
        Self {
            want: Some(want),
            identity: Identity::Anonymous,
        }
    }

    /// The same, but as the person.
    #[must_use]
    pub const fn as_me(mut self) -> Self {
        self.identity = Identity::Mine;
        self
    }
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
pub fn work_for(address: &str, into: PathBuf, wish: Wish) -> Work {
    let address = address.trim();
    if crate::torrent::takes(address) {
        // A torrent carries no identity: a swarm has no account, and the
        // request that fetches the metadata is the same for everybody.
        return Work::Torrent {
            source: address.to_owned(),
            into,
        };
    }
    if let Some(want) = wish.want {
        return Work::Media {
            url: address.to_owned(),
            into,
            want,
            identity: wish.identity,
        };
    }
    Work::Fetch {
        url: address.to_owned(),
        into,
        identity: wish.identity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_magnet_never_reaches_the_link_engine() {
        let into = PathBuf::from("/where/it/goes");
        assert!(matches!(
            work_for("magnet:?xt=urn:btih:abc", into.clone(), Wish::plain()),
            Work::Torrent { .. }
        ));
        // Even when the caller asked for media: a magnet is not a page.
        assert!(matches!(
            work_for(
                "magnet:?xt=urn:btmh:1220ab",
                into.clone(),
                Wish::media(Want::Video)
            ),
            Work::Torrent { .. }
        ));
        assert!(matches!(
            work_for("https://exemplo/x.iso", into.clone(), Wish::plain()),
            Work::Fetch { .. }
        ));
        assert!(matches!(
            work_for(
                "https://exemplo/assistir",
                into.clone(),
                Wish::media(Want::Audio)
            ),
            Work::Media {
                want: Want::Audio,
                identity: Identity::Anonymous,
                ..
            }
        ));
        // And asking to be oneself reaches the work, rather than a setting
        // somewhere else deciding it later.
        assert!(matches!(
            work_for("https://exemplo/v", into, Wish::media(Want::Video).as_me()),
            Work::Media {
                identity: Identity::Mine,
                ..
            }
        ));
    }

    #[test]
    fn the_user_dirs_file_is_read_the_way_the_file_dialog_reads_it() {
        let text = "# comentario\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/main/temp\"\n";
        assert_eq!(
            ronoctua_xdg::user_dir_value(text, "XDG_DOWNLOAD_DIR").as_deref(),
            Some("$HOME/main/temp")
        );
        assert_eq!(
            ronoctua_xdg::user_dir_expand(std::path::Path::new("/home/someone"), "$HOME/main/temp"),
            PathBuf::from("/home/someone/main/temp")
        );
        // A commented-out line is not a value, and a key that is a prefix of
        // another one must not match it.
        assert_eq!(
            ronoctua_xdg::user_dir_value("#XDG_DOWNLOAD_DIR=\"/x\"\n", "XDG_DOWNLOAD_DIR"),
            None
        );
        // And a key that is a PREFIX of another one must not match it: without
        // the `=` after the name, `XDG_DOWNLOAD` would answer with the
        // directory of `XDG_DOWNLOAD_DIR`.
        assert_eq!(ronoctua_xdg::user_dir_value(text, "XDG_DOWNLOAD"), None);
    }
}
