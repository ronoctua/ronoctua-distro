//! A cover or a poster, fetched: the bytes, and what kind of image they ARE.
//!
//! The kind is read from the bytes and never from a header: a server that
//! says `image/jpeg` over an HTML error page would otherwise put a page into
//! a tag. Only the four kinds a tag can carry are taken.

use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered};
use crate::suggestion::Cover;
use crate::{Host, Refusal, Why};

/// The most a picture may be: a cover's ceiling on the sheet.
pub const MOST_BYTES: u64 = ronoctua_tags::COVER_MOST_BYTES;

/// What kind of image some bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// WebP.
    Webp,
    /// GIF.
    Gif,
}

impl Kind {
    /// Its media type.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }

    /// Its usual extension, for `cover.<ext>`.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }
}

/// A picture, fetched.
#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    /// The bytes.
    pub bytes: Vec<u8>,
    /// What they are.
    pub kind: Kind,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("bytes", &self.bytes.len())
            .field("kind", &self.kind)
            .finish()
    }
}

/// What kind of image `bytes` are, by their first bytes; `None` for anything
/// else. Pure.
#[must_use]
pub fn kind_of(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Kind::Jpeg)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some(Kind::Webp)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else {
        None
    }
}

/// The image at `url`, which must be on `host`.
///
/// # Errors
///
/// [`Why::Malformed`] for an address that is not on `host` ([`address_on`])
/// or bytes that are not an image, and what the network or the status
/// answers.
pub fn fetch(net: &dyn Net, host: Host, url: &str) -> Result<Picture, Refusal> {
    let service = host.service();
    let url = address_on(host, url).ok_or(Refusal::new(service, Why::Malformed))?;
    let request = Request::get(host, url)
        .accepting("image/*")
        .identity()
        .most(MOST_BYTES);
    let bytes = answered(net, &request)?;
    let kind = kind_of(&bytes).ok_or(Refusal::new(service, Why::Malformed))?;
    Ok(Picture { bytes, kind })
}

/// The picture a suggestion's cover names: the Cover Art Archive's front image
/// at 1200 px, a tag's size (its list first), or the address.
///
/// # Errors
///
/// As [`fetch`] and [`crate::coverart::list`]; a release with no front image
/// is [`Why::NothingFound`].
pub fn cover(net: &dyn Net, cover: &Cover) -> Result<Picture, Refusal> {
    match cover {
        Cover::Address { host, url } => fetch(net, *host, url),
        Cover::CoverArt { release } => {
            let arts = crate::coverart::list(net, release)?;
            let front = crate::coverart::front(&arts).ok_or(Refusal::new(
                crate::Service::CoverArtArchive,
                Why::NothingFound,
            ))?;
            fetch(
                net,
                Host::CoverArtArchive,
                front.address(crate::coverart::Size::Huge),
            )
        }
    }
}

/// The picture a suggestion's cover names, AT A DOOR'S SIZE — to be seen
/// beside the file's own before anything is written: the Cover Art Archive's
/// front image at 500 px (its list first), or the address (a poster is
/// already a door's size). Taking the candidate fetches [`cover`]'s, a tag's
/// size.
///
/// # Errors
///
/// As [`cover`].
pub fn preview(net: &dyn Net, cover: &Cover) -> Result<Picture, Refusal> {
    match cover {
        Cover::Address { host, url } => fetch(net, *host, url),
        Cover::CoverArt { release } => {
            let arts = crate::coverart::list(net, release)?;
            let front = crate::coverart::front(&arts).ok_or(Refusal::new(
                crate::Service::CoverArtArchive,
                Why::NothingFound,
            ))?;
            fetch(
                net,
                Host::CoverArtArchive,
                front.address(crate::coverart::Size::Large),
            )
        }
    }
}

/// `url` as the `https` address on `host` it names, or `None` when it names
/// another host.
///
/// The Cover Art Archive's list and Wikidata's posters hand out `http`
/// addresses on hosts that answer `http` with a redirect to `https`; those two
/// are asked over `https` at once (as [`ronoctua_http`]'s web does). Any
/// other `http` address is not asked. Pure.
#[must_use]
pub fn address_on(host: Host, url: &str) -> Option<String> {
    let url = url.trim();
    let (secure, rest) = match url.strip_prefix("https://") {
        Some(rest) => (true, rest),
        None => (false, url.strip_prefix("http://")?),
    };
    let named = rest.split(['/', '?', '#']).next()?;
    if !named.eq_ignore_ascii_case(host.name()) {
        return None;
    }
    let upgraded = matches!(host, Host::CoverArtArchive | Host::Commons);
    (secure || upgraded).then(|| format!("https://{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F'];

    #[test]
    fn the_kind_is_read_from_the_bytes() {
        assert_eq!(kind_of(JPEG), Some(Kind::Jpeg));
        assert_eq!(kind_of(b"\x89PNG\r\n\x1a\n...."), Some(Kind::Png));
        assert_eq!(kind_of(b"RIFF\x10\0\0\0WEBPVP8 "), Some(Kind::Webp));
        assert_eq!(kind_of(b"GIF89a.."), Some(Kind::Gif));
        assert_eq!(kind_of(b"<!DOCTYPE html>"), None);
        assert_eq!(kind_of(b""), None);
    }

    #[test]
    fn a_picture_comes_only_from_its_host_and_only_as_an_image() {
        let url = "https://image.tmdb.org/t/p/w500/abc.jpg";
        let net = Canned::new(&[(url, 200, JPEG)]);
        let p = fetch(&net, Host::TmdbImages, url).unwrap();
        assert_eq!(p.kind, Kind::Jpeg);
        let asked = net.only();
        assert!(!asked.gzip, "an image is asked as it is");
        assert_eq!(asked.most_bytes, MOST_BYTES);

        // Another host's address is never asked with this host's pace.
        let net = Canned::new(&[]);
        assert_eq!(
            fetch(&net, Host::TmdbImages, "https://evil.example/x.jpg")
                .unwrap_err()
                .why,
            Why::Malformed
        );
        assert!(net.asked().is_empty());

        // An http address is asked over https on the two hosts that redirect
        // so, and not at all anywhere else.
        assert_eq!(
            address_on(
                Host::Commons,
                "http://commons.wikimedia.org/wiki/Special:FilePath/A.jpg"
            )
            .as_deref(),
            Some("https://commons.wikimedia.org/wiki/Special:FilePath/A.jpg")
        );
        assert_eq!(
            address_on(Host::TmdbImages, "http://image.tmdb.org/t/p/w500/a.jpg"),
            None
        );
        assert_eq!(
            address_on(
                Host::Commons,
                "https://commons.wikimedia.org.evil.example/x"
            ),
            None
        );

        // A page where an image should be is not an image.
        let net = Canned::new(&[(url, 200, b"<html>")]);
        assert_eq!(
            fetch(&net, Host::TmdbImages, url).unwrap_err(),
            Refusal::new(crate::Service::Tmdb, Why::Malformed)
        );
    }

    #[test]
    fn a_releases_front_cover_is_its_list_then_its_image() {
        let release = "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d";
        let list = format!("https://coverartarchive.org/release/{release}");
        let body = br#"{"images":[
            {"id":1,"front":false,"back":true,"approved":true,"types":["Back"],
             "image":"http://coverartarchive.org/release/r/1.jpg","thumbnails":{}},
            {"id":2,"front":true,"back":false,"approved":true,"types":["Front"],
             "image":"http://coverartarchive.org/release/r/2.jpg",
             "thumbnails":{"500":"http://coverartarchive.org/release/r/2-500.jpg",
                           "1200":"http://coverartarchive.org/release/r/2-1200.jpg"}}]}"#;
        let net = Canned::new(&[
            (list.as_str(), 200, body),
            (
                "https://coverartarchive.org/release/r/2-1200.jpg",
                200,
                JPEG,
            ),
        ]);
        let p = cover(
            &net,
            &Cover::CoverArt {
                release: release.into(),
            },
        )
        .unwrap();
        assert_eq!(p.kind, Kind::Jpeg);
        assert_eq!(net.asked().len(), 2);
        // TO BE SEEN, a door's size: the 500 px thumbnail, never the 1200.
        let net = Canned::new(&[
            (list.as_str(), 200, body),
            ("https://coverartarchive.org/release/r/2-500.jpg", 200, JPEG),
        ]);
        let p = preview(
            &net,
            &Cover::CoverArt {
                release: release.into(),
            },
        )
        .unwrap();
        assert_eq!(p.kind, Kind::Jpeg);
        assert_eq!(net.asked().len(), 2);
    }
}
