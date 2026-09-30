//! An album's cover from the Cover Art Archive, keyless.
//!
//! `GET https://coverartarchive.org/release/<mbid>` answers `307` to
//! archive.org's `index.json`, which answers `302` to a storage host: three
//! requests across three hosts, which the web walks by its rule
//! ([`ronoctua_http`]'s `MOST_HOPS` is asserted to allow it). The list names
//! each image and its thumbnails, as `http://coverartarchive.org/…`
//! addresses — asked over `https` ([`crate::picture::address_on`]).

use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, object};
use crate::query::is_mbid;
use crate::{Host, Refusal, Service, Why};

/// Which size of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Size {
    /// 250 px.
    Small,
    /// 500 px — a door's picture.
    Large,
    /// 1200 px — a tag's cover.
    Huge,
    /// As uploaded, which may be tens of megabytes.
    Original,
}

/// One image of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Art {
    /// Its id.
    pub id: String,
    /// Whether it is the front cover.
    pub front: bool,
    /// Whether it is the back cover.
    pub back: bool,
    /// Whether an editor approved it.
    pub approved: bool,
    /// Its types (`Front`, `Back`, `Booklet`, `Medium`…).
    pub types: Vec<String>,
    /// The image as uploaded.
    pub image: String,
    /// 250 px.
    pub small: Option<String>,
    /// 500 px.
    pub large: Option<String>,
    /// 1200 px.
    pub huge: Option<String>,
}

impl Art {
    /// Its address at `size`, or the nearest larger one there is, or the
    /// image as uploaded.
    #[must_use]
    pub fn address(&self, size: Size) -> &str {
        let order: &[&Option<String>] = match size {
            Size::Small => &[&self.small, &self.large, &self.huge],
            Size::Large => &[&self.large, &self.huge],
            Size::Huge => &[&self.huge],
            Size::Original => &[],
        };
        order
            .iter()
            .find_map(|a| a.as_deref())
            .unwrap_or(&self.image)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawImage {
    id: serde_json::Value,
    front: bool,
    back: bool,
    approved: bool,
    types: Vec<String>,
    image: String,
    thumbnails: std::collections::HashMap<String, String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawList {
    images: Vec<RawImage>,
}

/// A release's list's address.
///
/// # Errors
///
/// [`Why::Malformed`] for an id that is not one.
pub fn list_url(release: &str) -> Result<String, Why> {
    if !is_mbid(release) {
        return Err(Why::Malformed);
    }
    Ok(format!(
        "{}/release/{release}",
        Host::CoverArtArchive.origin()
    ))
}

/// The images of a list answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_list(body: &[u8]) -> Result<Vec<Art>, Refusal> {
    let raw: RawList = object(Service::CoverArtArchive, body)?;
    Ok(raw
        .images
        .into_iter()
        .filter(|i| !i.image.is_empty())
        .map(|i| {
            let thumb = |keys: &[&str]| {
                keys.iter()
                    .find_map(|k| i.thumbnails.get(*k))
                    .filter(|a| !a.is_empty())
                    .cloned()
            };
            Art {
                id: match &i.id {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                },
                front: i.front,
                back: i.back,
                approved: i.approved,
                small: thumb(&["250", "small"]),
                large: thumb(&["500", "large"]),
                huge: thumb(&["1200"]),
                types: i.types,
                image: i.image,
            }
        })
        .collect())
}

/// The front cover: an approved front image first, then any front image.
#[must_use]
pub fn front(arts: &[Art]) -> Option<&Art> {
    arts.iter()
        .find(|a| a.front && a.approved)
        .or_else(|| arts.iter().find(|a| a.front))
}

/// A release's images.
///
/// # Errors
///
/// [`Why::NothingFound`] (a release with no art is the service's 404),
/// [`Why::Malformed`] for an id that is not one (nothing is sent), and what
/// the network answers.
pub fn list(net: &dyn Net, release: &str) -> Result<Vec<Art>, Refusal> {
    let url = list_url(release).map_err(|why| Refusal::new(Service::CoverArtArchive, why))?;
    parse_list(&answered(net, &Request::get(Host::CoverArtArchive, url))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    const REL: &str = "22222222-2222-4222-8222-222222222222";

    #[test]
    fn the_front_is_the_approved_front_and_its_size_falls_back() {
        let body = br#"{"images":[
            {"id":"9","front":true,"approved":false,"types":["Front"],
             "image":"http://coverartarchive.org/release/r/9.jpg","thumbnails":{}},
            {"id":12345678901,"front":true,"approved":true,"types":["Front"],
             "image":"http://coverartarchive.org/release/r/1.jpg",
             "thumbnails":{"250":"http://c/250.jpg","500":"http://c/500.jpg","small":"x","large":"y"}},
            {"id":3,"front":false,"back":true,"approved":true,"types":["Back"],"image":""}
        ],"release":"https://musicbrainz.org/release/r"}"#;
        let arts = parse_list(body).unwrap();
        assert_eq!(arts.len(), 2, "an image with no address is dropped");
        let f = front(&arts).unwrap();
        assert_eq!(f.id, "12345678901");
        assert_eq!(f.address(Size::Large), "http://c/500.jpg");
        assert_eq!(f.address(Size::Small), "http://c/250.jpg");
        assert_eq!(
            f.address(Size::Huge),
            "http://coverartarchive.org/release/r/1.jpg",
            "no 1200: the original"
        );
        assert_eq!(front(&[]), None);
    }

    #[test]
    fn a_release_with_no_art_is_nothing_found() {
        let url = list_url(REL).unwrap();
        assert_eq!(url, format!("https://coverartarchive.org/release/{REL}"));
        let net = Canned::new(&[(url.as_str(), 404, b"")]);
        assert_eq!(
            list(&net, REL).unwrap_err(),
            Refusal::new(Service::CoverArtArchive, Why::NothingFound)
        );
        assert_eq!(list_url("x/../y"), Err(Why::Malformed));
    }
}
