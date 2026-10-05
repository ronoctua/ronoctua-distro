//! Icons: found, decoded, squared and written as the PNGs a launcher reads.
//!
//! # Four shelves, one way out
//!
//! A web app's icon comes from its SITE (the manifest, the touch icon, the
//! favicon), from Dashboard Icons, from the distro's own COLLECTION (the icon
//! superpack) or from a FILE the person gave. Whatever the shelf, [`prepare`]
//! turns the choice into the same thing: a square, transparent-padded PNG of
//! at most [`SIDE`] pixels, named by the hash of its bytes. The hash in the
//! name is what makes a changed icon a changed `Icon=` line — the launcher's
//! pixmap cache is keyed by that string, and an icon rewritten under the same
//! name would stay stale on screen for the rest of the session.
//!
//! # Every byte is suspect
//!
//! The bytes come from somebody else's server or from a file nobody vouched
//! for, so: a ceiling on every read ([`ICON_MOST`], [`FILE_MOST`]), the format
//! decided by MAGIC BYTES and never by a name or a `Content-Type` ([`sniff`]),
//! decoder limits (4096 px a side, 64 MiB), and an SVG renderer that refuses
//! every external reference — `usvg`'s default resolver reads any path an
//! `<image href>` names, which would put a file of the person's into an icon
//! because a web page asked. What does not decode is
//! [`Refusal::IconUnusable`], a value with a sentence.
//!
//! # Blocking
//!
//! [`from_site`], [`typed_pass`], [`from_dashboard`] and [`prepare`] reach
//! the network or the disk and decode images: call them on a THREAD.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::Refusal;
use crate::dashboard;
use crate::html::{Declared, Head, Offered};
use crate::order::IconChoice;

/// One icon a person can pick for a web app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Where it comes from (and what the order will carry).
    pub choice: IconChoice,
    /// A line on what it is (`manifest · 512 px · SVG`), as data.
    pub about: String,
}

/// The side of the PNG [`prepare`] writes.
pub const SIDE: u32 = 512;

/// The smallest side a raster image may have to become an icon: below it,
/// even doubled, it is a blur on a launcher row.
pub const SMALLEST: u32 = 32;

/// How much a raster may be enlarged: twice, and never to more than [`SIDE`].
pub const MOST_UPSCALE: u32 = 2;

/// Ceilings, in bytes: a page read for its head…
pub const PAGE_MOST: u64 = 2 << 20;
/// …a web manifest…
pub const MANIFEST_MOST: u64 = 256 << 10;
/// …an icon fetched from the network…
pub const ICON_MOST: u64 = 4 << 20;
/// …and a file the person gave (or one read from the collection).
pub const FILE_MOST: u64 = 8 << 20;

/// The decoder's limits: no side above this…
pub const DECODE_SIDE: u32 = 4096;
/// …and no more than this allocated while decoding.
pub const DECODE_ALLOC: u64 = 64 << 20;

/// Where the Dashboard Icons metadata is kept, under the cache directory
/// [`from_dashboard`] is given.
pub const METADATA_FILE: &str = "dashboard-icons.json";

/// How long that copy is believed before it is fetched again: a week, which
/// is the CDN's own `max-age` for it.
pub const METADATA_FRESH: Duration = Duration::from_secs(7 * 24 * 3600);

/// The site's own icons, best first (manifest `any` → apple-touch → icon →
/// favicon), and the page's title.
///
/// The title is the manifest's name when there is one, else the head's
/// (`application-name`, then `<title>`). A page that answers nothing still
/// gets its two well-known paths probed — `/apple-touch-icon.png` when no
/// touch icon is declared, `/favicon.ico` when no icon is — and a probe counts
/// only if what came back IS an image: a single-page app answers every path
/// with its HTML and a 200.
///
/// An `http` address is looked at over `https`: the framer carries TLS only
/// (see `net`), and nearly every site answers both. The web app itself still
/// opens the address as typed. Reaches the network: call it on a thread.
///
/// # On the person's pass
///
/// The page, and every probe and manifest on its host, are asked on `pass`
/// — the person's consent to the host they typed ([`typed_pass`]). A
/// manifest or a redirect the PAGE names on another host is the page's
/// answer, as a redirect is, and is asked BARE on the page's pass — no
/// header and no body of ours (`web::request_named`): nobody could let it
/// before the page named it.
///
/// # Errors
///
/// [`Refusal::NotLet`] when the address is not on the pass's host, and the
/// network's refusal, when the page could not be read and no probe found
/// anything.
pub fn from_site(
    pass: &ronoctua_consent::Pass,
    page_url: &str,
) -> Result<(Option<String>, Vec<Candidate>), crate::Refusal> {
    #[cfg(feature = "net")]
    {
        site(pass, page_url)
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = (pass, page_url);
        Err(Refusal::Offline)
    }
}

/// The pass for the host of a web app's address the person TYPED: typing it
/// is the consent to ask it, and it is RECORDED in `consent.json` like any
/// other — the person's newest answer for that host, so a host they refused
/// once and now typed is let — and then minted from there.
///
/// # Errors
///
/// [`Refusal::UrlInvalid`] for an address with no host, and
/// [`Refusal::NotLet`] when the consent could not be kept or does not let it.
pub fn typed_pass(page_url: &str) -> Result<ronoctua_consent::Pass, Refusal> {
    let host = host_typed(page_url).ok_or(Refusal::UrlInvalid)?;
    ronoctua_consent::let_hosts(&[host.as_str()], true).map_err(|_| Refusal::NotLet)?;
    ronoctua_consent::pass(&host).map_err(|_| Refusal::NotLet)
}

/// The host of a typed address, as it is asked. Pure.
fn host_typed(page_url: &str) -> Option<String> {
    url::Url::parse(page_url.trim())
        .ok()?
        .host_str()
        .filter(|h| !h.is_empty())
        .map(str::to_owned)
}

#[cfg(feature = "net")]
fn site(
    pass: &ronoctua_consent::Pass,
    page_url: &str,
) -> Result<(Option<String>, Vec<Candidate>), Refusal> {
    let mut at = url::Url::parse(page_url.trim()).map_err(|_| Refusal::UrlInvalid)?;
    if at.scheme() == "http" {
        at.set_scheme("https").map_err(|()| Refusal::UrlScheme)?;
    }
    let (head, page, refused) = match crate::net::get_let(pass, at.as_str(), PAGE_MOST) {
        Ok(got) => (
            crate::html::head_of(&got.reply.body, &got.url),
            got.url,
            None,
        ),
        // No network: every probe would wait for the same nothing. Not
        // let: nothing may be asked of it at all.
        Err(r @ (Refusal::Offline | Refusal::NotLet)) => return Err(r),
        Err(r) => (Head::default(), at.to_string(), Some(r)),
    };
    let manifest =
        head.manifest
            .as_deref()
            .and_then(|m| match named(pass, at.as_str(), m, MANIFEST_MOST) {
                Ok(got) => Some(crate::html::manifest_icons(&got.reply.body, &got.url)),
                Err(r) => {
                    tracing::debug!(
                        manifest = m,
                        refusal = r.word(),
                        "the manifest could not be read"
                    );
                    None
                }
            });
    let probe = |path: &str| {
        let url = url::Url::parse(&page).ok()?.join(path).ok()?.to_string();
        let got = named(pass, at.as_str(), &url, ICON_MOST).ok()?;
        sniff(&got.reply.body).map(|_| got.url)
    };
    let apple = (!head.icons.iter().any(|o| o.from == Declared::AppleTouch))
        .then(|| probe("/apple-touch-icon.png"))
        .flatten();
    let favicon = (!head.icons.iter().any(|o| o.from == Declared::Icon))
        .then(|| probe("/favicon.ico"))
        .flatten();
    let (name, mut found) = ranked(&head, manifest, apple, favicon);
    // EACH ONE KNOWS THE PAGE THAT NAMED IT, for the fetch that prepares it
    // later (`prepare`): an icon on a CDN is the page's answer.
    for c in &mut found {
        if let IconChoice::Site { page, .. } = &mut c.choice {
            *page = Some(at.to_string());
        }
    }
    match (found.is_empty(), refused) {
        (true, Some(r)) => Err(r),
        _ => Ok((name, found)),
    }
}

/// A file the page NAMED: on the pass when it is on the typed host, and as
/// the page's answer — bare, on the page's pass (`net::get_named`) — when
/// the page put it on another (a manifest on a CDN, a page that
/// redirected). See [`from_site`].
#[cfg(feature = "net")]
fn named(
    pass: &ronoctua_consent::Pass,
    page: &str,
    url: &str,
    most: u64,
) -> Result<crate::net::Got, Refusal> {
    if crate::net::first_host_is(pass, url) {
        crate::net::get_let(pass, url, most)
    } else {
        crate::net::get_named(pass, page, url, most)
    }
}

/// The candidates of a page, in the order they are offered, and its name.
/// Pure: the head, the manifest's `(name, icons)` when it was read, and the
/// two probes that answered with an image.
///
/// The order: the manifest's `any` icons (SVG first, then the largest), the
/// declared touch icons (largest first), the probed touch icon, the declared
/// icons (SVG first, then the largest), the probed favicon, and the maskable
/// ones last — drawn to be cropped into a circle, they look padded as they
/// are. An address offered twice is offered once, in its best place, and an
/// icon that SAYS it is under [`SMALLEST`] is left out: it would be refused.
#[must_use]
pub fn ranked(
    head: &Head,
    manifest: Option<(Option<String>, Vec<Offered>)>,
    apple_probe: Option<String>,
    favicon_probe: Option<String>,
) -> (Option<String>, Vec<Candidate>) {
    let (manifest_name, manifest_icons) = manifest.unwrap_or_default();
    let mut all: Vec<Offered> = manifest_icons;
    all.extend(head.icons.iter().cloned());
    if let Some(url) = apple_probe {
        all.push(Offered {
            url,
            from: Declared::AppleTouch,
            side: 0,
            mime: None,
        });
    }
    if let Some(url) = favicon_probe {
        all.push(Offered {
            url,
            from: Declared::Favicon,
            side: 0,
            mime: None,
        });
    }
    all.retain(|o| o.side == 0 || o.side >= SMALLEST);
    // Stable: equal keys keep the page's order, which is the author's.
    all.sort_by_key(|o| (o.from, !o.is_svg(), std::cmp::Reverse(o.side)));
    let mut seen = std::collections::HashSet::new();
    let found = all
        .into_iter()
        .filter(|o| seen.insert(o.url.clone()))
        .map(|o| Candidate {
            about: about_of(&o),
            choice: IconChoice::Site {
                url: o.url,
                page: None,
            },
        })
        .collect();
    (manifest_name.or_else(|| head.title.clone()), found)
}

/// `manifest · 512 px · PNG`: where, how big, what — each part only when it
/// is known.
fn about_of(o: &Offered) -> String {
    let size = match o.side {
        0 | u32::MAX => String::new(),
        n => format!("{n} px"),
    };
    let lower = o.url.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or_default();
    let mime = o.mime.as_deref().unwrap_or_default();
    let format = if o.is_svg() {
        "SVG"
    } else if mime.contains("png") || path.ends_with(".png") {
        "PNG"
    } else if mime.contains("icon") || path.ends_with(".ico") {
        "ICO"
    } else if mime.contains("webp") || path.ends_with(".webp") {
        "WebP"
    } else if mime.contains("jpeg") || path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "JPEG"
    } else if mime.contains("gif") || path.ends_with(".gif") {
        "GIF"
    } else {
        ""
    };
    [o.from.term(), size.as_str(), format]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Dashboard Icons matching a query (the metadata is fetched once a week and
/// cached under `cache`).
///
/// Every drawing of an icon is a candidate — the base, and the variants for a
/// dark and a light background — in the order of [`dashboard::Icon::drawings`];
/// [`from_dashboard_in`] puts the one for the theme first. `limit` counts
/// candidates. Reaches the network when the cached metadata is older than
/// [`METADATA_FRESH`]: call it on a thread.
///
/// # Errors
///
/// The network's refusal when there is no cache.
pub fn from_dashboard(
    cache: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<Candidate>, crate::Refusal> {
    from_dashboard_in(cache, query, limit, None)
}

/// [`from_dashboard`], with the drawing made for the theme's background first:
/// `Some(true)` for a dark theme (the metadata's LIGHT-coloured variant),
/// `Some(false)` for a light one.
///
/// # Errors
///
/// As [`from_dashboard`].
pub fn from_dashboard_in(
    cache: &Path,
    query: &str,
    limit: usize,
    dark_theme: Option<bool>,
) -> Result<Vec<Candidate>, Refusal> {
    let all = dashboard::parse(&metadata(cache)?);
    let mut out = Vec::new();
    for icon in dashboard::search(&all, query, limit) {
        let mut drawings = icon.drawings();
        if let Some(dark) = dark_theme {
            let first = if dark { "light" } else { "dark" };
            drawings.sort_by_key(|(_, word)| *word != first);
        }
        for (file, variant) in drawings {
            out.push(Candidate {
                about: format!(
                    "dashboardicons · {file} · {}",
                    icon.base.to_ascii_uppercase()
                ),
                choice: IconChoice::Dashboard {
                    name: file.to_owned(),
                    variant: variant.to_owned(),
                },
            });
        }
    }
    out.truncate(limit);
    Ok(out)
}

/// The metadata's bytes: the cached copy while it is fresh, a new one when it
/// is not, and the stale copy when a new one cannot be had.
fn metadata(cache: &Path) -> Result<Vec<u8>, Refusal> {
    let path = cache.join(METADATA_FILE);
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age < METADATA_FRESH));
    let kept = std::fs::read(&path)
        .ok()
        .filter(|b| !dashboard::parse(b).is_empty());
    if fresh && let Some(bytes) = kept.as_ref() {
        return Ok(bytes.clone());
    }
    #[cfg(feature = "net")]
    match crate::net::get(dashboard::METADATA, dashboard::METADATA_MOST) {
        Ok(got) if !dashboard::parse(&got.reply.body).is_empty() => {
            if let Err(e) = ronoctua_xdg::write_atomically(cache, METADATA_FILE, &got.reply.body) {
                tracing::warn!(error = %e, "the Dashboard Icons metadata could not be cached");
            }
            return Ok(got.reply.body);
        }
        Ok(_) => tracing::warn!("the Dashboard Icons metadata did not parse"),
        Err(r) => {
            if kept.is_none() {
                return Err(r);
            }
        }
    }
    kept.ok_or(Refusal::Offline)
}

/// The distro's collection (the icon superpack's names) matching a query.
///
/// `names` is the file the build writes with every name the superpack
/// answers to, one per line. The tiers: the name is the query, starts with
/// it, contains it; shorter first inside a tier. Reads a file: call it on a
/// thread.
#[must_use]
pub fn from_collection(names: &Path, query: &str, limit: usize) -> Vec<Candidate> {
    let squash = |s: &str| -> String {
        ronoctua_apps::fold::fold(s)
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect()
    };
    let q = squash(query);
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(names) else {
        return Vec::new();
    };
    let mut hits: Vec<(u8, &str)> = text
        .lines()
        .map(str::trim)
        .filter(|n| is_icon_name(n))
        .filter_map(|n| {
            let s = squash(n);
            let tier = if s == q {
                0
            } else if s.starts_with(&q) {
                1
            } else if s.contains(&q) {
                2
            } else {
                return None;
            };
            Some((tier, n))
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| ta.cmp(tb).then(a.len().cmp(&b.len())).then(a.cmp(b)));
    hits.dedup_by(|a, b| a.1 == b.1);
    hits.into_iter()
        .take(limit)
        .map(|(_, n)| Candidate {
            choice: IconChoice::Collection { name: n.to_owned() },
            about: format!("collection · {n}"),
        })
        .collect()
}

/// A name that can only name a file inside the theme: no separator, no
/// parent, nothing hidden.
fn is_icon_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
        && !name.chars().any(char::is_control)
}

/// Turn a choice into a square PNG (512 px) under `out`, named by its content
/// hash, and return its path. Used for the preview and by the order.
///
/// - `Site`: the address, over the network (at most [`ICON_MOST`]).
/// - `Dashboard`: the drawing's SVG, and its PNG when there is no SVG (the
///   714 icons whose original is a PNG answer 404 for it) or it does not
///   render — which is `url_of(name, "svg")` exactly when the metadata's
///   `base` is `svg`, without having to read the metadata here.
/// - `Collection`: looked up in the theme at `icons_root` — `scalable/**`
///   first, then the size directories from the LARGEST, by number
///   (`84x84` before `8x8`, `48x48@2` as 96).
/// - `File`: the person's file, a regular one of at most [`FILE_MOST`].
///
/// Reaches the network or the disk and decodes: call it on a thread.
///
/// # Errors
///
/// `IconUnusable` for an image that cannot be read or is smaller than 32 px;
/// the network's refusal for a remote one.
pub fn prepare(
    choice: &IconChoice,
    icons_root: Option<&Path>,
    out: &Path,
) -> Result<PathBuf, crate::Refusal> {
    let png = match choice {
        IconChoice::Site { url, page } => normalised(&site_icon(url, page.as_deref())?)?,
        IconChoice::Dashboard { name, .. } => {
            if !dashboard::is_file_name(name) {
                return Err(Refusal::UnknownItem);
            }
            let mut last = Refusal::IconUnusable;
            let mut done = None;
            for format in ["svg", "png"] {
                match fetched(&dashboard::url_of(name, format)).and_then(|b| normalised(&b)) {
                    Ok(png) => {
                        done = Some(png);
                        break;
                    }
                    Err(r) => last = r,
                }
            }
            done.ok_or(last)?
        }
        IconChoice::Collection { name } => {
            let root = icons_root.ok_or(Refusal::SourceAbsent)?;
            if !is_icon_name(name) {
                return Err(Refusal::UnknownItem);
            }
            let file = look_up(root, name).ok_or(Refusal::UnknownItem)?;
            normalised(&read_file(&file)?)?
        }
        IconChoice::File { path } => normalised(&read_file(path)?)?,
    };
    let name = format!("icon-{}.png", &crate::run::sha256(&png)[..8]);
    ronoctua_xdg::write_atomically(out, &name, &png).map_err(|e| {
        tracing::warn!(error = %e, dir = %out.display(), "the icon could not be written");
        Refusal::ToolFailed
    })?;
    Ok(out.join(name))
}

/// A site's icon: on a pass of its own host when the person let it (the
/// host they typed), else bare on the pass of the page that named it.
fn site_icon(url: &str, page: Option<&str>) -> Result<Vec<u8>, Refusal> {
    #[cfg(feature = "net")]
    {
        match (crate::net::pass_for(url), page) {
            (Ok(_), _) | (Err(_), None) => fetched(url),
            (Err(_), Some(page)) => {
                let pass =
                    crate::net::pass_for(page).map_err(|f| f.refusal.unwrap_or(Refusal::NotLet))?;
                crate::net::get_named(&pass, page, url, ICON_MOST).map(|g| g.reply.body)
            }
        }
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = page;
        fetched(url)
    }
}

/// An icon's bytes from the network, on a pass of its host.
fn fetched(url: &str) -> Result<Vec<u8>, Refusal> {
    #[cfg(feature = "net")]
    {
        crate::net::get(url, ICON_MOST).map(|g| g.reply.body)
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = url;
        Err(Refusal::Offline)
    }
}

/// A file's bytes, when it is a regular file of at most [`FILE_MOST`].
fn read_file(path: &Path) -> Result<Vec<u8>, Refusal> {
    let meta = std::fs::metadata(path).map_err(|_| Refusal::IconUnusable)?;
    if !meta.is_file() {
        return Err(Refusal::IconUnusable);
    }
    if meta.len() > FILE_MOST {
        return Err(Refusal::TooLarge);
    }
    let bytes = crate::run::head_of(path, usize::try_from(FILE_MOST).unwrap_or(usize::MAX))
        .map_err(|_| Refusal::IconUnusable)?;
    Ok(bytes)
}

/// An icon name's file in a theme: `scalable/**/<name>.svg` first, then each
/// size directory from the largest — its SVG, then its PNG. Reads the disk.
#[must_use]
pub fn look_up(root: &Path, name: &str) -> Option<PathBuf> {
    if let Some(found) = in_dir(&root.join("scalable"), name, "svg") {
        return Some(found);
    }
    let mut sized: Vec<(u32, PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| Some((size_of_dir(e.file_name().to_str()?)?, e.path())))
        .filter(|(_, p)| p.is_dir())
        .collect();
    // NUMERIC, and decreasing: sorted as text, `8x8` comes after `84x84` and a
    // reversed list picks the eight-pixel drawing over the eighty-four.
    sized.sort_by(|(a, pa), (b, pb)| b.cmp(a).then_with(|| pa.cmp(pb)));
    sized
        .into_iter()
        .find_map(|(_, dir)| in_dir(&dir, name, "svg").or_else(|| in_dir(&dir, name, "png")))
}

/// The pixels a size directory holds: `48x48` is 48, `48x48@2` is 96;
/// anything else (`scalable`, `symbolic`) is not a size. Pure.
#[must_use]
pub fn size_of_dir(name: &str) -> Option<u32> {
    let (dims, scale) = match name.split_once('@') {
        Some((d, s)) => (d, s.trim_end_matches('x').parse::<u32>().ok()?),
        None => (name, 1),
    };
    let (w, h) = dims.split_once('x')?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    w.max(h).checked_mul(scale)
}

/// `dir/<name>.<ext>`, else `dir/<context>/<name>.<ext>` for each context
/// directory in name order (`apps`, `places`, … — how freedesktop themes lay
/// their files out).
fn in_dir(dir: &Path, name: &str, ext: &str) -> Option<PathBuf> {
    let file = format!("{name}.{ext}");
    let direct = dir.join(&file);
    if direct.is_file() {
        return Some(direct);
    }
    let mut contexts: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    contexts.sort();
    contexts
        .into_iter()
        .map(|c| c.join(&file))
        .find(|p| p.is_file())
}

/// What a file's first bytes say it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `\x89PNG\r\n\x1a\n`.
    Png,
    /// `00 00 01 00`: a Windows icon, one or more frames.
    Ico,
    /// `FF D8 FF`.
    Jpeg,
    /// `GIF87a` / `GIF89a` (the first frame is used).
    Gif,
    /// `RIFF` … `WEBP`.
    WebP,
    /// `<svg`, after an optional BOM, XML prolog, comments and doctype.
    Svg,
}

/// The format by its magic bytes, never by its name. Pure.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if bytes.starts_with(&[0, 0, 1, 0]) && bytes.len() >= 6 {
        Some(Kind::Ico)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Kind::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some(Kind::WebP)
    } else if is_svg(bytes) {
        Some(Kind::Svg)
    } else {
        None
    }
}

/// Whether the text starts, past its preamble, with an `<svg` element.
///
/// A DOCTYPE with an internal subset (`[ … ]`, where XML declares entities)
/// is NOT walked past: that is where an entity expansion lives, and an icon
/// has no business carrying one.
fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(64 << 10)];
    let body = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    // A multi-byte character cut by the 64 KiB window is not a reason.
    let Ok(text) =
        std::str::from_utf8(body).or_else(|e| std::str::from_utf8(&body[..e.valid_up_to()]))
    else {
        return false;
    };
    let mut rest = text.trim_start();
    loop {
        if let Some(r) = rest.strip_prefix("<?") {
            let Some(end) = r.find("?>") else {
                return false;
            };
            rest = r[end + 2..].trim_start();
        } else if let Some(r) = rest.strip_prefix("<!--") {
            let Some(end) = r.find("-->") else {
                return false;
            };
            rest = r[end + 3..].trim_start();
        } else if rest.len() >= 9 && rest[..9].eq_ignore_ascii_case("<!doctype") {
            let Some(end) = rest.find('>') else {
                return false;
            };
            if rest[..end].contains('[') {
                return false;
            }
            rest = rest[end + 1..].trim_start();
        } else {
            return rest.starts_with("<svg")
                && rest[4..].starts_with(|c: char| c.is_whitespace() || c == '>');
        }
    }
}

/// Any image bytes to the square PNG [`prepare`] writes.
///
/// A raster under [`SMALLEST`] on its larger side is refused; one smaller than
/// [`SIDE`] is enlarged at most [`MOST_UPSCALE`] times, so a 100 px favicon
/// becomes a 200 px icon and not a 512 px blur. It is padded to a square,
/// centred, with TRANSPARENT sides — never stretched, which would make
/// whoever looks at it doubt what they see. An SVG is drawn at [`SIDE`]. Pure.
///
/// # Errors
///
/// [`Refusal::IconUnusable`].
pub fn normalised(bytes: &[u8]) -> Result<Vec<u8>, Refusal> {
    let kind = sniff(bytes).ok_or(Refusal::IconUnusable)?;
    let image = match kind {
        Kind::Svg => return encoded(&drawn(bytes)?),
        Kind::Ico => {
            let frame = largest_frame(bytes)?;
            let format = if sniff(&frame) == Some(Kind::Png) {
                image::ImageFormat::Png
            } else {
                image::ImageFormat::Ico
            };
            decoded(&frame, format)?
        }
        Kind::Png => decoded(bytes, image::ImageFormat::Png)?,
        Kind::Jpeg => decoded(bytes, image::ImageFormat::Jpeg)?,
        Kind::Gif => decoded(bytes, image::ImageFormat::Gif)?,
        Kind::WebP => decoded(bytes, image::ImageFormat::WebP)?,
    };
    let (w, h) = image.dimensions();
    let side = w.max(h);
    if side < SMALLEST {
        return Err(Refusal::IconUnusable);
    }
    let target = SIDE.min(side.saturating_mul(MOST_UPSCALE));
    // THE PICTURE is scaled and THEN padded: padding first would scale the
    // empty bands too, and a 4096 px banner would cost a 4096 px square.
    let scale = |n: u32| {
        u32::try_from(u64::from(n) * u64::from(target) / u64::from(side))
            .unwrap_or(target)
            .max(1)
    };
    let picture = if target == side {
        image
    } else {
        resized(image, scale(w), scale(h))
    };
    let (pw, ph) = picture.dimensions();
    let mut square = image::RgbaImage::new(target, target);
    image::imageops::overlay(
        &mut square,
        &picture,
        i64::from((target - pw) / 2),
        i64::from((target - ph) / 2),
    );
    encoded(&square)
}

/// Decoded with the limits, whatever the format.
fn decoded(bytes: &[u8], format: image::ImageFormat) -> Result<image::RgbaImage, Refusal> {
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(DECODE_SIDE);
    limits.max_image_height = Some(DECODE_SIDE);
    limits.max_alloc = Some(DECODE_ALLOC);
    reader.limits(limits);
    reader.decode().map(|i| i.to_rgba8()).map_err(|e| {
        tracing::debug!(error = %e, "an icon did not decode");
        Refusal::IconUnusable
    })
}

/// An ICO reduced to its LARGEST frame, as an ICO of one frame (or the
/// frame's own PNG).
///
/// The decoder picks a frame by colour depth first and size second, so a
/// file with a 32-bit 16 px frame and a 24-bit 256 px one gives the sixteen.
/// An icon wants pixels. Pure.
fn largest_frame(bytes: &[u8]) -> Result<Vec<u8>, Refusal> {
    let word = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let long = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let count = usize::from(word(4).ok_or(Refusal::IconUnusable)?);
    let side = |b: u8| if b == 0 { 256 } else { u32::from(b) };
    let best = (0..count)
        .filter_map(|i| {
            let at = 6 + 16 * i;
            let entry = bytes.get(at..at + 16)?;
            let (size, offset) = (long(at + 8)?, long(at + 12)?);
            let (start, len) = (usize::try_from(offset).ok()?, usize::try_from(size).ok()?);
            let data = bytes.get(start..start.checked_add(len)?)?;
            let area = side(entry[0]) * side(entry[1]);
            Some(((area, word(at + 6)?), entry, data))
        })
        .max_by_key(|(score, _, _)| *score)
        .ok_or(Refusal::IconUnusable)?;
    let (_, entry, data) = best;
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        // A PNG frame is a PNG, and the ICO decoder would only unwrap it.
        return Ok(data.to_vec());
    }
    let mut one = Vec::with_capacity(22 + data.len());
    one.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    one.extend_from_slice(&entry[..12]);
    one.extend_from_slice(&22u32.to_le_bytes());
    one.extend_from_slice(data);
    Ok(one)
}

/// An SVG drawn at [`SIDE`], centred, with no external reference followed.
fn drawn(bytes: &[u8]) -> Result<image::RgbaImage, Refusal> {
    use resvg::{tiny_skia, usvg};
    let options = usvg::Options {
        // No directory to resolve a relative path against…
        resources_dir: None,
        // …and no path, file or address resolved at all. Embedded `data:`
        // images keep the default (they are inside the bytes already).
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|_| Refusal::IconUnusable)?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(Refusal::IconUnusable);
    }
    let side = SIDE as f32;
    let scale = side / w.max(h);
    let mut pixmap = tiny_skia::Pixmap::new(SIDE, SIDE).ok_or(Refusal::IconUnusable)?;
    let transform =
        tiny_skia::Transform::from_translate((side - w * scale) / 2.0, (side - h * scale) / 2.0)
            .pre_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // `tiny-skia` hands PREMULTIPLIED pixels; a PNG holds straight ones, and
    // skipping this darkens every soft edge.
    let pixels: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(SIDE, SIDE, pixels).ok_or(Refusal::IconUnusable)
}

/// A picture resampled to `w`×`h` with Lanczos3, in PREMULTIPLIED light.
///
/// Resampled straight, the transparent pixels around a logo — black with zero
/// alpha — bleed into its edge and draw a dark halo round it; premultiplied,
/// a pixel with no alpha adds no colour. In place and in eight bits: a copy in
/// floating point would be four times the decoded picture, which for the
/// largest one the decoder allows is a quarter of a gigabyte in the footer's
/// process. What eight bits cost is precision at the faintest edge pixels.
fn resized(mut picture: image::RgbaImage, w: u32, h: u32) -> image::RgbaImage {
    for p in picture.pixels_mut() {
        let a = u16::from(p[3]);
        for c in &mut p.0[..3] {
            *c = u8::try_from((u16::from(*c) * a + 127) / 255).unwrap_or(u8::MAX);
        }
    }
    let mut out = image::imageops::resize(&picture, w, h, image::imageops::FilterType::Lanczos3);
    for p in out.pixels_mut() {
        let a = u16::from(p[3]);
        for c in &mut p.0[..3] {
            // Lanczos rings: a colour may pass its alpha, and is brought back.
            // A pixel with no alpha has no colour.
            *c = (u16::from(*c) * 255 + a / 2)
                .checked_div(a)
                .map_or(0, |v| u8::try_from(v).unwrap_or(u8::MAX));
        }
    }
    out
}

/// The PNG's bytes.
fn encoded(image: &image::RgbaImage) -> Result<Vec<u8>, Refusal> {
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|_| Refusal::IconUnusable)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    /// A scratch directory of this test process, emptied first.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-icons-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/web")
                .join(name),
        )
        .unwrap()
    }

    fn png(w: u32, h: u32, colour: [u8; 4]) -> Vec<u8> {
        encoded(&image::RgbaImage::from_pixel(w, h, image::Rgba(colour))).unwrap()
    }

    fn read_png(path: &Path) -> image::RgbaImage {
        image::load_from_memory(&std::fs::read(path).unwrap())
            .unwrap()
            .to_rgba8()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    #[test]
    fn every_format_is_known_by_its_first_bytes() {
        assert_eq!(sniff(&png(1, 1, RED)), Some(Kind::Png));
        assert_eq!(sniff(&[0, 0, 1, 0, 1, 0]), Some(Kind::Ico));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(Kind::Jpeg));
        assert_eq!(sniff(b"GIF89a...."), Some(Kind::Gif));
        assert_eq!(sniff(b"RIFF\x10\0\0\0WEBPVP8 "), Some(Kind::WebP));
        assert_eq!(
            sniff(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
            Some(Kind::Svg)
        );
        assert_eq!(
            sniff(b"\xEF\xBB\xBF<?xml version='1.0'?>\n<!-- made by hand -->\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x\">\n<svg>"),
            Some(Kind::Svg)
        );
        // An internal subset is where entities live: not walked past.
        assert_eq!(sniff(b"<!DOCTYPE svg [<!ENTITY a 'b'>]><svg>"), None);
        assert_eq!(sniff(b"<svgfoo>"), None);
        assert_eq!(sniff(b"<!doctype html><html>"), None);
        assert_eq!(sniff(b"hello"), None);
        assert_eq!(sniff(b""), None);
        // A name says nothing: the bytes of a page called icon.png are a page.
        assert_eq!(sniff(&fixture("plain-head.html")), None);
    }

    #[test]
    fn a_png_becomes_a_padded_square_enlarged_at_most_twice() {
        let dir = scratch("png");
        let file = dir.join("wide.png");
        std::fs::write(&file, png(64, 32, RED)).unwrap();
        let out = prepare(&IconChoice::File { path: file }, None, &dir.join("out")).unwrap();
        let name = out.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("icon-")
                && name.ends_with(".png")
                && name.len() == "icon-12345678.png".len()
        );
        let img = read_png(&out);
        assert_eq!(
            img.dimensions(),
            (128, 128),
            "64 px doubled, not stretched to 512"
        );
        assert_eq!(img.get_pixel(64, 64).0, RED, "the middle is the picture");
        assert_eq!(img.get_pixel(64, 2)[3], 0, "the band above is transparent");
        assert_eq!(img.get_pixel(64, 125)[3], 0, "and the band below");
        // The same bytes are the same file: the name is the content.
        let again = prepare(
            &IconChoice::File {
                path: dir.join("wide.png"),
            },
            None,
            &dir.join("out"),
        )
        .unwrap();
        assert_eq!(again, out);
    }

    #[test]
    fn a_large_image_comes_down_to_the_side_with_no_halo() {
        let dir = scratch("large");
        // A red disc on transparency, larger than the side. The transparent
        // pixels are GREEN: a PNG keeps the colour of a pixel with no alpha,
        // exporters leave all sorts there, and resampled straight it bleeds
        // into the edge — premultiplied, a pixel with no alpha adds nothing.
        let mut img = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([0, 255, 0, 0]));
        for (x, y, p) in img.enumerate_pixels_mut() {
            let (dx, dy) = (f64::from(x) - 512.0, f64::from(y) - 512.0);
            if dx.hypot(dy) < 400.0 {
                *p = image::Rgba(RED);
            }
        }
        let file = dir.join("disc.png");
        std::fs::write(&file, encoded(&img).unwrap()).unwrap();
        let out =
            read_png(&prepare(&IconChoice::File { path: file }, None, &dir.join("out")).unwrap());
        assert_eq!(out.dimensions(), (SIDE, SIDE));
        // Every partly transparent pixel of the edge is still RED.
        for p in out.pixels().filter(|p| p[3] > 0 && p[3] < 255) {
            assert!(p[0] > 200 && p[1] < 40 && p[2] < 40, "a halo pixel {p:?}");
        }
    }

    #[test]
    fn an_ico_gives_its_largest_frame_whatever_its_depth() {
        use image::ExtendedColorType;
        use image::codecs::ico::{IcoEncoder, IcoFrame};
        let small = image::RgbaImage::from_pixel(16, 16, image::Rgba(RED));
        let large = image::RgbImage::from_pixel(64, 64, image::Rgb([0, 0, 255]));
        let frames = [
            IcoFrame::as_png(small.as_raw(), 16, 16, ExtendedColorType::Rgba8).unwrap(),
            // 24 bits: the decoder alone would prefer the 32-bit sixteen.
            IcoFrame::as_png(large.as_raw(), 64, 64, ExtendedColorType::Rgb8).unwrap(),
        ];
        let mut ico = Vec::new();
        IcoEncoder::new(&mut ico).encode_images(&frames).unwrap();
        assert_eq!(sniff(&ico), Some(Kind::Ico));
        let dir = scratch("ico");
        let file = dir.join("favicon.ico");
        std::fs::write(&file, &ico).unwrap();
        let out =
            read_png(&prepare(&IconChoice::File { path: file }, None, &dir.join("out")).unwrap());
        assert_eq!(out.dimensions(), (128, 128));
        assert_eq!(out.get_pixel(64, 64).0, BLUE);
    }

    #[test]
    fn an_ico_with_a_bitmap_frame_decodes_through_its_single_frame_copy() {
        use image::ExtendedColorType;
        use image::codecs::ico::{IcoEncoder, IcoFrame};
        // A bitmap (DIB) frame, the way old favicons are, with no PNG inside:
        // a BITMAPINFOHEADER whose height counts the colour rows AND the mask
        // rows, BGRA rows from the bottom, then the one-bit mask.
        let mut dib = Vec::new();
        for v in [40u32, 48, 96] {
            dib.extend_from_slice(&v.to_le_bytes());
        }
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend(std::iter::repeat_n(0u8, 24));
        for _ in 0..48 * 48 {
            dib.extend_from_slice(&[255, 0, 0, 255]);
        }
        dib.extend(std::iter::repeat_n(0u8, 48 * 8));
        let frame = IcoFrame::with_encoded(dib, 48, 48, ExtendedColorType::Rgba8).unwrap();
        let mut ico = Vec::new();
        IcoEncoder::new(&mut ico).encode_images(&[frame]).unwrap();
        let one = largest_frame(&ico).unwrap();
        assert!(one.starts_with(&[0, 0, 1, 0, 1, 0]), "a one-frame ICO");
        let img = decoded(&one, image::ImageFormat::Ico).unwrap();
        assert_eq!(img.dimensions(), (48, 48));
        assert_eq!(img.get_pixel(24, 24).0, BLUE);
    }

    #[test]
    fn an_svg_is_drawn_and_its_external_references_are_not_followed() {
        let dir = scratch("svg");
        // A red drawing ON DISK, and an icon that points at it. With the
        // default resolver the red would be read into the icon.
        let red = dir.join("red.svg");
        std::fs::write(
            &red,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="#f00"/></svg>"##,
        )
        .unwrap();
        let icon = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100" height="100">
                 <image x="0" y="0" width="100" height="100" href="{0}"/>
                 <image x="0" y="0" width="100" height="100" xlink:href="file://{0}"/>
                 <image x="0" y="0" width="100" height="100" href="red.svg"/>
                 <rect x="0" y="0" width="10" height="10" fill="#00f"/>
               </svg>"##,
            red.display()
        );
        // THE FIXTURE PROVES ITSELF: drawn with the default options, the file
        // on disk does reach the picture — so what follows is not a vacuous pass.
        {
            use resvg::{tiny_skia, usvg};
            let options = usvg::Options {
                resources_dir: Some(dir.clone()),
                ..usvg::Options::default()
            };
            let tree = usvg::Tree::from_data(icon.as_bytes(), &options).unwrap();
            let mut pixmap = tiny_skia::Pixmap::new(100, 100).unwrap();
            resvg::render(
                &tree,
                tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            let p = pixmap.pixel(50, 50).unwrap().demultiply();
            assert_eq!(
                (p.red(), p.alpha()),
                (255, 255),
                "the default resolver reads the disk"
            );
        }
        let file = dir.join("icon.svg");
        std::fs::write(&file, &icon).unwrap();
        let out =
            read_png(&prepare(&IconChoice::File { path: file }, None, &dir.join("out")).unwrap());
        assert_eq!(out.dimensions(), (SIDE, SIDE));
        assert!(
            out.pixels().all(|p| p[3] == 0 || p[0] < 16),
            "a red pixel came from the disk"
        );
        assert_eq!(
            out.get_pixel(20, 20).0,
            BLUE,
            "the icon's own drawing is there"
        );
        assert_eq!(out.get_pixel(256, 256)[3], 0);
    }

    #[test]
    fn a_tiny_image_and_a_non_image_are_refused() {
        let dir = scratch("refused");
        let tiny = dir.join("tiny.png");
        std::fs::write(&tiny, png(16, 16, RED)).unwrap();
        assert_eq!(
            prepare(&IconChoice::File { path: tiny }, None, &dir),
            Err(Refusal::IconUnusable)
        );
        let page = dir.join("page.png");
        std::fs::write(&page, b"<!doctype html><title>404</title>").unwrap();
        assert_eq!(
            prepare(&IconChoice::File { path: page }, None, &dir),
            Err(Refusal::IconUnusable)
        );
        // A PNG's magic with nothing after it: it does not decode.
        let broken = dir.join("broken.png");
        std::fs::write(&broken, b"\x89PNG\r\n\x1a\nnot really").unwrap();
        assert_eq!(
            prepare(&IconChoice::File { path: broken }, None, &dir),
            Err(Refusal::IconUnusable)
        );
        assert_eq!(
            prepare(&IconChoice::File { path: dir.clone() }, None, &dir),
            Err(Refusal::IconUnusable)
        );
        assert_eq!(
            prepare(
                &IconChoice::File {
                    path: dir.join("absent")
                },
                None,
                &dir
            ),
            Err(Refusal::IconUnusable)
        );
        // Over the ceiling, without reading it (the file is sparse).
        let big = dir.join("big.png");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(FILE_MOST + 1)
            .unwrap();
        assert_eq!(
            prepare(&IconChoice::File { path: big }, None, &dir),
            Err(Refusal::TooLarge)
        );
        // Nothing was written for any of them.
        assert!(
            std::fs::read_dir(&dir)
                .unwrap()
                .filter_map(Result::ok)
                .all(|e| !e.file_name().to_string_lossy().starts_with("icon-"))
        );
    }

    #[test]
    fn a_collection_name_is_found_by_number_and_not_by_text() {
        let root = scratch("theme");
        for (dir, colour) in [("8x8/apps", RED), ("84x84/apps", BLUE), ("48x48/apps", RED)] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join("thing.png"), png(64, 64, colour)).unwrap();
        }
        assert_eq!(
            look_up(&root, "thing"),
            Some(root.join("84x84/apps/thing.png"))
        );
        // `@2` doubles the pixels.
        std::fs::create_dir_all(root.join("48x48@2/apps")).unwrap();
        std::fs::write(root.join("48x48@2/apps/thing.png"), png(64, 64, RED)).unwrap();
        assert_eq!(
            look_up(&root, "thing"),
            Some(root.join("48x48@2/apps/thing.png"))
        );
        // And a scalable drawing wins over any size.
        std::fs::create_dir_all(root.join("scalable/apps")).unwrap();
        std::fs::write(root.join("scalable/apps/thing.svg"), "<svg/>").unwrap();
        assert_eq!(
            look_up(&root, "thing"),
            Some(root.join("scalable/apps/thing.svg"))
        );
        assert_eq!(look_up(&root, "nothing"), None);

        std::fs::remove_dir_all(root.join("scalable")).unwrap();
        std::fs::remove_dir_all(root.join("48x48@2")).unwrap();
        let out = scratch("theme-out");
        let made = prepare(
            &IconChoice::Collection {
                name: "thing".into(),
            },
            Some(&root),
            &out,
        )
        .unwrap();
        assert_eq!(read_png(&made).get_pixel(64, 64).0, BLUE);
        assert_eq!(
            prepare(
                &IconChoice::Collection {
                    name: "thing".into()
                },
                None,
                &out
            ),
            Err(Refusal::SourceAbsent)
        );
        assert_eq!(
            prepare(
                &IconChoice::Collection {
                    name: "../84x84/apps/thing".into()
                },
                Some(&root),
                &out
            ),
            Err(Refusal::UnknownItem)
        );
        assert_eq!(size_of_dir("48x48"), Some(48));
        assert_eq!(size_of_dir("16x16@3"), Some(48));
        assert_eq!(size_of_dir("24x24@2x"), Some(48));
        assert_eq!(size_of_dir("scalable"), None);
        assert_eq!(size_of_dir("x"), None);
    }

    #[test]
    fn the_collection_is_searched_by_its_names() {
        let dir = scratch("names");
        let names = dir.join("names.txt");
        std::fs::write(&names, "org.gnome.Nautilus\nfirefox\nfirefox-developer-edition\nnautilus\n../escape\n\nfirefox\n").unwrap();
        let got: Vec<IconChoice> = from_collection(&names, "Firefox", 5)
            .into_iter()
            .map(|c| c.choice)
            .collect();
        assert_eq!(
            got,
            [
                IconChoice::Collection {
                    name: "firefox".into()
                },
                IconChoice::Collection {
                    name: "firefox-developer-edition".into()
                },
            ]
        );
        let nautilus: Vec<String> = from_collection(&names, "nautilus", 5)
            .into_iter()
            .map(|c| c.about)
            .collect();
        assert_eq!(
            nautilus,
            ["collection · nautilus", "collection · org.gnome.Nautilus"]
        );
        assert!(
            from_collection(&names, "escape", 5).is_empty(),
            "a name with a separator is never offered"
        );
        assert!(from_collection(&dir.join("absent"), "firefox", 5).is_empty());
    }

    #[test]
    fn the_dashboard_offers_every_drawing_and_the_themes_first() {
        let cache = scratch("dashboard");
        std::fs::write(
            cache.join(METADATA_FILE),
            fixture("dashboard-metadata.json"),
        )
        .unwrap();
        let got = from_dashboard(&cache, "spotify", 10).unwrap();
        let choices: Vec<IconChoice> = got.iter().map(|c| c.choice.clone()).collect();
        assert_eq!(
            choices,
            [
                IconChoice::Dashboard {
                    name: "spotify".into(),
                    variant: String::new()
                },
                IconChoice::Dashboard {
                    name: "spotify-light".into(),
                    variant: "light".into()
                },
                IconChoice::Dashboard {
                    name: "spotify-dark".into(),
                    variant: "dark".into()
                },
            ]
        );
        assert_eq!(got[0].about, "dashboardicons · spotify · SVG");
        // A dark theme wants the light-coloured drawing first.
        let dark = from_dashboard_in(&cache, "spotify", 10, Some(true)).unwrap();
        assert_eq!(
            dark[0].choice,
            IconChoice::Dashboard {
                name: "spotify-light".into(),
                variant: "light".into()
            }
        );
        let light = from_dashboard_in(&cache, "spotify", 10, Some(false)).unwrap();
        assert_eq!(
            light[0].choice,
            IconChoice::Dashboard {
                name: "spotify-dark".into(),
                variant: "dark".into()
            }
        );
        assert_eq!(
            from_dashboard(&cache, "s", 2).unwrap().len(),
            2,
            "the limit counts candidates"
        );
    }

    #[test]
    fn a_real_site_ranks_its_icons_manifest_first_and_maskable_last() {
        let head = html::head_of(&fixture("proton-mail-head.html"), "https://mail.proton.me/");
        let manifest = html::manifest_icons(
            &fixture("proton-mail-manifest.json"),
            "https://mail.proton.me/assets/manifest.webmanifest",
        );
        let (name, found) = ranked(&head, Some(manifest), None, None);
        assert_eq!(name.as_deref(), Some("Proton Mail"));
        let urls: Vec<&str> = found
            .iter()
            .map(|c| match &c.choice {
                IconChoice::Site { url, .. } => url.as_str(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            urls[0], "https://mail.proton.me/assets/android-chrome-512x512.png",
            "the largest `any` first"
        );
        assert_eq!(found[0].about, "manifest · 512 px · PNG");
        let first_touch = urls.iter().position(|u| u.contains("apple-touch")).unwrap();
        let first_icon = urls.iter().position(|u| u.contains("favicon")).unwrap();
        let first_mask = urls.iter().position(|u| u.contains("maskable")).unwrap();
        assert!(
            first_touch < first_icon && first_icon < first_mask,
            "{urls:#?}"
        );
        assert_eq!(
            urls[first_touch],
            "https://mail.proton.me/assets/apple-touch-icon-1024x1024.png"
        );
        assert!(
            urls[first_icon].ends_with(".svg"),
            "the SVG icon before the ICO: {urls:#?}"
        );
        // `favicon.ico` is declared twice and offered once, and no address is
        // offered twice.
        assert_eq!(
            urls.iter().filter(|u| u.ends_with("favicon.ico")).count(),
            1
        );
        let mut unique = urls.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), urls.len());
    }

    #[test]
    fn the_probes_take_their_places_and_a_small_declared_icon_is_left_out() {
        let head = html::head_of(
            b"<head><link rel=icon sizes=16x16 href=/tiny.png><link rel=icon sizes=64x64 href=/ok.png></head>",
            "https://a.example/",
        );
        let (_, found) = ranked(
            &head,
            None,
            Some("https://a.example/apple-touch-icon.png".into()),
            Some("https://a.example/favicon.ico".into()),
        );
        let abouts: Vec<&str> = found.iter().map(|c| c.about.as_str()).collect();
        assert_eq!(
            abouts,
            [
                "apple-touch-icon · PNG",
                "icon · 64 px · PNG",
                "favicon.ico · ICO"
            ]
        );
    }
}
