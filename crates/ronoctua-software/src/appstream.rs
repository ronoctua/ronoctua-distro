//! The local Flathub catalogue, for searching when the network is off or the
//! online search is turned off.
//!
//! # What it reads
//!
//! The appstream data flatpak keeps per remote:
//! `$XDG_DATA_HOME/flatpak/appstream/flathub/<arch>/active/appstream.xml` for
//! the user installation, fetched by [`crate::flatpak::refresh`] (and by
//! `flatpak update`, once a day). When the user installation has none yet, the
//! SYSTEM installation's copy under `/var/lib/flatpak` answers instead — it is
//! data, not an installation, and a machine whose flake declares Flathub for
//! the system has it before the person ever prepared the user remote.
//!
//! Only the uncompressed `appstream.xml` is read. The `.gz` beside it would
//! need a gzip reader this crate does not have, and it is never needed: flatpak
//! itself writes the uncompressed file whenever it deploys a catalogue ("Old
//! appstream format don't have uncompressed file, so we uncompress it" —
//! `flatpak-dir.c`, `flatpak_dir_deploy_appstream`), so the two always exist
//! together.
//!
//! # Why a cache of our own
//!
//! The file is 50 MB of XML with every language of every app (4 769 components
//! on the machine this was measured on, 3 352 of them applications). It is
//! streamed ONCE per catalogue into a compact JSON under
//! [`crate::Roots::ours_cache`] — id, name, summary, keywords, the names in
//! other languages as search terms, the icon's path — and every later search
//! reads that. The cache is keyed by the source's REAL path, size and
//! modification time ([`Stamp`]): ostree checks every file out with the
//! modification time of the epoch, so the time alone never changes, but
//! `active` is a symlink to a directory named after the catalogue's commit, and
//! the canonical path does.

use std::io::BufRead;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use quick_xml::events::{BytesStart, Event};
use serde::{Deserialize, Serialize};

use crate::found::{Badge, Found, Picture};
use crate::{AppKey, Roots};

/// The version of the cache's shape; a new one rebuilds every cache.
pub const FORMAT: u32 = 1;

/// The system installation's catalogue root, the fallback of [`search`].
pub const SYSTEM_ROOT: &str = "/var/lib/flatpak";

/// At most this many search terms (keywords and names in other languages) are
/// kept per app: a catalogue entry with hundreds is a keyword list stuffed
/// for ranking, and it would cost every search.
pub const MOST_TERMS: usize = 64;

/// One app of the catalogue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Entry {
    /// The Flatpak application id (from the bundle's ref, which is what
    /// `flatpak install` takes — a legacy entry's `<id>` ends in `.desktop`).
    pub id: String,
    /// Its name, untranslated (data).
    pub name: String,
    /// Its summary, untranslated.
    pub summary: String,
    /// Its keywords, in every language, folded-distinct.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// Its name in other languages — a search term: a person searching
    /// "calculadora" means the calculator.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    /// The cached 128 (or 64) px icon, when the catalogue has one on disk.
    pub icon: Option<PathBuf>,
    /// The newest release's version.
    pub version: String,
    /// The licence expression.
    pub license: Option<String>,
    /// Who publishes it.
    pub developer: Option<String>,
    /// Its homepage.
    pub homepage: Option<String>,
    /// The runtime it needs (`org.gnome.Platform/x86_64/49`).
    pub runtime: Option<String>,
    /// Flathub verified the publisher.
    pub verified: bool,
    /// A command-line program (`console-application`).
    pub console: bool,
}

/// What a cache is keyed by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Stamp {
    /// The catalogue's canonical path (through `active`, so it names the commit).
    pub file: PathBuf,
    /// Its size.
    pub size: u64,
    /// Its modification time, seconds.
    pub modified: i64,
}

/// The stamp of a catalogue file as it is now.
///
/// # Errors
///
/// When the file is not there.
pub fn stamp_of(xml: &Path) -> std::io::Result<Stamp> {
    let file = std::fs::canonicalize(xml)?;
    let meta = std::fs::metadata(&file)?;
    Ok(Stamp {
        file,
        size: meta.len(),
        modified: meta.mtime(),
    })
}

#[derive(Serialize, Deserialize)]
struct Cached {
    format: u32,
    stamp: Stamp,
    apps: Vec<Entry>,
}

/// The folded forms a search runs over, made once per load. The two term
/// lists are joined with a newline on EACH side of every term, so "is a term
/// exactly this" is one `contains` of `\n<query>\n`.
#[derive(Debug, Clone)]
struct Hay {
    name: String,
    id: String,
    last: String,
    names: String,
    keywords: String,
    summary: String,
}

fn joined(terms: &[String]) -> String {
    let fold = ronoctua_apps::fold::fold;
    terms.iter().fold(String::from("\n"), |mut s, t| {
        s.push_str(&fold(t));
        s.push('\n');
        s
    })
}

impl Hay {
    fn of(e: &Entry) -> Self {
        let fold = ronoctua_apps::fold::fold;
        let id = fold(&e.id);
        Self {
            name: fold(&e.name),
            last: id.rsplit('.').next().unwrap_or_default().to_owned(),
            id,
            names: joined(&e.names),
            keywords: joined(&e.keywords),
            summary: fold(&e.summary),
        }
    }
}

/// A loaded catalogue.
#[derive(Debug, Clone)]
pub struct Catalogue {
    /// What it was read from.
    pub stamp: Stamp,
    /// Its apps, in the catalogue's order.
    pub apps: Vec<Entry>,
    hay: Vec<Hay>,
}

impl Catalogue {
    /// A catalogue of these apps.
    #[must_use]
    pub fn new(stamp: Stamp, apps: Vec<Entry>) -> Self {
        let hay = apps.iter().map(Hay::of).collect();
        Self { stamp, apps, hay }
    }
}

/// The user installation's catalogue file for this machine's architecture.
#[must_use]
pub fn user_source(roots: &Roots) -> PathBuf {
    source_under(&roots.data.join("flatpak"))
}

/// The catalogue file under one installation's root.
#[must_use]
pub fn source_under(installation: &Path) -> PathBuf {
    installation
        .join("appstream")
        .join(crate::flatpak::REMOTE)
        .join(crate::flatpak::arch())
        .join("active/appstream.xml")
}

/// When a catalogue was last fetched (seconds since the epoch): flatpak
/// touches `<arch>/.timestamp` on every appstream update, and the XML's own
/// time is the epoch. For "offline catalogue of <date>" on screen.
#[must_use]
pub fn fetched(xml: &Path) -> Option<u64> {
    let stamp = xml.parent()?.parent()?.join(".timestamp");
    let modified = std::fs::metadata(stamp).ok()?.modified().ok()?;
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn cache_name(xml: &Path) -> String {
    // FNV-1a over the source's path: two installations never share a file,
    // and a collision only costs a rebuild (the stamp inside says whose it is).
    let hash = xml
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("flathub-appstream-v{FORMAT}-{hash:016x}.json")
}

/// The catalogue at `xml`, from its cache in `cache_dir` when that matches,
/// otherwise parsed again and cached.
///
/// Measured on Flathub's (50 MB, 2026-09-26, release build): parsed in
/// 0.14 s with a resident peak of 11 MB — the stream never holds the file —
/// into a 1.8 MB cache that reads back in 8 ms; a search over it takes 0.4 ms.
/// A debug build parses in 2.1 s. Still a file and a parse: call it on a
/// thread.
///
/// # Errors
///
/// When the catalogue cannot be read or parsed.
pub fn load(xml: &Path, cache_dir: &Path) -> anyhow::Result<Catalogue> {
    let stamp = stamp_of(xml)?;
    let file = cache_dir.join(cache_name(xml));
    if let Some(cached) = std::fs::read(&file)
        .ok()
        .and_then(|b| serde_json::from_slice::<Cached>(&b).ok())
        .filter(|c| c.format == FORMAT && c.stamp == stamp)
    {
        return Ok(Catalogue::new(stamp, cached.apps));
    }
    let reader = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(xml)?);
    let apps = parse(reader, xml.parent().map(|p| p.join("icons")).as_deref())?;
    let cached = Cached {
        format: FORMAT,
        stamp: stamp.clone(),
        apps,
    };
    let written = std::fs::create_dir_all(cache_dir).and_then(|()| {
        ronoctua_xdg::write_atomically(
            cache_dir,
            &cache_name(xml),
            &serde_json::to_vec(&cached).unwrap_or_default(),
        )
    });
    if let Err(e) = written {
        tracing::warn!(%e, "the offline catalogue could not be cached");
    }
    Ok(Catalogue::new(stamp, cached.apps))
}

/// Bring the USER catalogue's cache up to date with its file — what a refresh
/// calls, so the cost is paid by the job and not by the next search.
///
/// # Errors
///
/// As [`load`]; a missing catalogue is one.
pub fn rebuild(roots: &Roots) -> anyhow::Result<()> {
    load(&user_source(roots), &roots.ours_cache()).map(drop)
}

/// The last catalogue loaded in this process, so a search per key press costs
/// a `stat` and not a parse.
static LOADED: Mutex<Option<Arc<Catalogue>>> = Mutex::new(None);

/// The catalogue at `xml`, through [`LOADED`].
///
/// # Errors
///
/// As [`load`].
pub fn current(xml: &Path, cache_dir: &Path) -> anyhow::Result<Arc<Catalogue>> {
    let stamp = stamp_of(xml)?;
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(c) = loaded.as_ref().filter(|c| c.stamp == stamp) {
        return Ok(Arc::clone(c));
    }
    let c = Arc::new(load(xml, cache_dir)?);
    *loaded = Some(Arc::clone(&c));
    Ok(c)
}

/// Let go of the loaded catalogue (a few megabytes), for a screen that has
/// been idle; the next search reads the cache again.
pub fn forget() {
    *LOADED.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// The catalogue [`search`] reads: the user installation's, or the system's
/// when the user has none; `None` when neither exists. With [`fetched`], the
/// date a screen shows beside offline results.
#[must_use]
pub fn source(roots: &Roots) -> Option<PathBuf> {
    [user_source(roots), source_under(Path::new(SYSTEM_ROOT))]
        .into_iter()
        .find(|p| p.is_file())
}

/// Search the local catalogue; empty when there is none yet. The user
/// installation's catalogue first, the system's when the user has none.
/// Reads (and on the first call may parse) a file: call it on a thread.
#[must_use]
pub fn search(roots: &Roots, query: &str, limit: usize) -> Vec<Found> {
    let Some(xml) = source(roots) else {
        return Vec::new();
    };
    match current(&xml, &roots.ours_cache()) {
        Ok(catalogue) => search_in(&catalogue, query, limit),
        Err(e) => {
            tracing::warn!(%e, file = %xml.display(), "the offline catalogue could not be read");
            Vec::new()
        }
    }
}

/// The tier from which a row matched by what it DOES, not by its name.
const DESCRIPTION: u8 = 6;

/// How well a row matches, lower is better: the name exactly (or the id's last
/// part), the name's start, the name or id anywhere, a name in another
/// language exactly, a keyword exactly, either of those containing the query,
/// the summary, and finally every word of a multi-word query somewhere.
///
/// The exact tiers are what make "calculadora" find the calculator first:
/// measured on Flathub's catalogue, a dozen apps carry the word among their
/// keywords or in a longer translated name, and without them the shortest
/// name among those won.
fn tier(h: &Hay, q: &str, words: &[&str]) -> Option<u8> {
    let exact = format!("\n{q}\n");
    if h.name == q || h.last == q {
        Some(0)
    } else if h.name.starts_with(q) {
        Some(1)
    } else if h.name.contains(q) || h.id.contains(q) {
        Some(2)
    } else if h.names.contains(&exact) {
        Some(3)
    } else if h.keywords.contains(&exact) {
        Some(4)
    } else if h.names.contains(q) || h.keywords.contains(q) {
        Some(5)
    } else if h.summary.contains(q) {
        Some(DESCRIPTION)
    } else if words.len() > 1
        && words.iter().all(|w| {
            h.name.contains(w)
                || h.id.contains(w)
                || h.names.contains(w)
                || h.keywords.contains(w)
                || h.summary.contains(w)
        })
    {
        Some(DESCRIPTION + 1)
    } else {
        None
    }
}

/// Search a loaded catalogue: folded, ranked by [`tier`], then verified
/// publishers first, then the shorter name (the closer match), then the name.
#[must_use]
pub fn search_in(catalogue: &Catalogue, query: &str, limit: usize) -> Vec<Found> {
    let q = ronoctua_apps::fold::fold(query.trim());
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut hits: Vec<(u8, &Entry, &Hay)> = catalogue
        .apps
        .iter()
        .zip(&catalogue.hay)
        .filter_map(|(e, h)| tier(h, &q, &words).map(|t| (t, e, h)))
        .collect();
    hits.sort_by(|a, b| {
        (a.0, !a.1.verified, a.2.name.len(), &a.2.name).cmp(&(
            b.0,
            !b.1.verified,
            b.2.name.len(),
            &b.2.name,
        ))
    });
    hits.into_iter()
        .take(limit)
        .map(|(t, e, _)| found_of(e, t >= DESCRIPTION))
        .collect()
}

/// Whether a licence expression says the program is not free. Flathub's
/// catalogue marks those `LicenseRef-proprietary` (sometimes `=<url>`); the
/// API's `is_free_license` is the better answer online.
#[must_use]
pub fn is_proprietary(license: &str) -> bool {
    license.contains("LicenseRef-proprietary")
}

/// A catalogue entry as a search row.
#[must_use]
pub fn found_of(e: &Entry, by_description: bool) -> Found {
    let mut badges = vec![Badge::Sandboxed];
    if e.verified {
        badges.push(Badge::Verified);
    }
    if e.license.as_deref().is_some_and(is_proprietary) {
        badges.push(Badge::Unfree);
    }
    if e.console {
        badges.push(Badge::CommandLine);
    }
    Found {
        key: AppKey::Flatpak { id: e.id.clone() },
        name: e.name.clone(),
        summary: e.summary.clone(),
        version: e.version.clone(),
        badges,
        download_bytes: None,
        icon: e.icon.clone().map(|path| Picture::File { path }),
        homepage: e.homepage.clone(),
        license: e.license.clone(),
        publisher: e.developer.clone(),
        program: None,
        by_description,
    }
}

// ─── the streaming reader ────────────────────────────────────────────────────

/// What the text being gathered will become.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Gather {
    Id,
    Name { translated: bool },
    Summary,
    License,
    DeveloperName,
    NestedDeveloperName,
    Homepage,
    Bundle { runtime: Option<String> },
    Icon { width: u32 },
    Keyword,
    Verified,
}

/// One component being read.
#[derive(Debug, Default)]
struct Component {
    kind: String,
    id: String,
    bundle: Option<String>,
    icons: Vec<(u32, String)>,
    newest: Option<(u64, String)>,
    entry: Entry,
}

impl Component {
    fn gathered(&mut self, what: Gather, text: String) {
        if text.is_empty() {
            return;
        }
        let e = &mut self.entry;
        match what {
            Gather::Id => self.id = text,
            Gather::Name { translated: false } => e.name = text,
            Gather::Name { translated: true } => e.names.push(text),
            Gather::Summary => e.summary = text,
            Gather::License => e.license = Some(text),
            Gather::DeveloperName => e.developer = Some(text),
            Gather::NestedDeveloperName => {
                e.developer.get_or_insert(text);
            }
            Gather::Homepage => e.homepage = Some(text),
            Gather::Bundle { runtime } => {
                self.bundle = Some(text);
                e.runtime = runtime;
            }
            Gather::Icon { width } => self.icons.push((width, text)),
            Gather::Keyword => e.keywords.push(text),
            Gather::Verified => e.verified = text == "true",
        }
    }

    /// The entry, when the component is an application with a Flatpak id.
    fn finish(mut self, icons: Option<&Path>) -> Option<Entry> {
        if !matches!(
            self.kind.as_str(),
            "desktop-application" | "desktop" | "console-application"
        ) {
            return None;
        }
        // The bundle's ref is what `flatpak install` takes; without one, the
        // component's id (a legacy one ends in `.desktop`).
        let id = match self.bundle.as_deref() {
            Some(reference) => {
                let mut parts = reference.split('/');
                (parts.next() == Some("app"))
                    .then(|| parts.next().unwrap_or_default().to_owned())?
            }
            None => self
                .id
                .strip_suffix(".desktop")
                .unwrap_or(&self.id)
                .to_owned(),
        };
        if !crate::flatpak::valid_id(&id) {
            return None;
        }
        let mut e = self.entry;
        e.id = id;
        if e.name.is_empty() {
            e.name.clone_from(&e.id);
        }
        e.console = self.kind == "console-application";
        e.version = self.newest.map(|(_, v)| v).unwrap_or_default();
        let fold = ronoctua_apps::fold::fold;
        let base = fold(&e.name);
        let mut seen = std::collections::BTreeSet::from([base]);
        e.names.retain(|n| seen.insert(fold(n)));
        e.keywords.retain(|k| seen.insert(fold(k)));
        e.names.truncate(MOST_TERMS);
        e.keywords
            .truncate(MOST_TERMS.saturating_sub(e.names.len()));
        if let Some(dir) = icons {
            self.icons
                .sort_by_key(|(w, _)| std::cmp::Reverse(*w == 128));
            let files = self
                .icons
                .iter()
                .map(|(_, f)| f.clone())
                .chain(std::iter::once(format!("{}.png", e.id)));
            e.icon = files
                .flat_map(|f| ["128x128", "64x64"].map(|size| dir.join(size).join(&f)))
                .find(|p| p.is_file());
        }
        Some(e)
    }
}

/// An attribute's value, normalised as XML 1.0 says (entities resolved,
/// whitespace characters as spaces). Flathub's catalogue declares no version,
/// which the specification reads as 1.0.
fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.try_get_attribute(name).ok().flatten().and_then(|a| {
        a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .ok()
            .map(std::borrow::Cow::into_owned)
    })
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What an element opening at `path` (the names ABOVE it, `components` first)
/// gathers, if anything.
fn gather_for(path: &[Vec<u8>], e: &BytesStart<'_>) -> Option<Gather> {
    let name = e.local_name();
    let parent = path.last().map(Vec::as_slice);
    match (path.len(), parent, name.as_ref()) {
        (2, Some(b"component"), b"id") => Some(Gather::Id),
        (2, Some(b"component"), b"name") => Some(Gather::Name {
            translated: attr(e, "xml:lang").is_some(),
        }),
        (2, Some(b"component"), b"summary") => {
            attr(e, "xml:lang").is_none().then_some(Gather::Summary)
        }
        (2, Some(b"component"), b"project_license") => Some(Gather::License),
        (2, Some(b"component"), b"developer_name") => attr(e, "xml:lang")
            .is_none()
            .then_some(Gather::DeveloperName),
        (2, Some(b"component"), b"url") => {
            (attr(e, "type").as_deref() == Some("homepage")).then_some(Gather::Homepage)
        }
        (2, Some(b"component"), b"bundle") => {
            (attr(e, "type").as_deref() == Some("flatpak")).then(|| Gather::Bundle {
                runtime: attr(e, "runtime"),
            })
        }
        (2, Some(b"component"), b"icon") => {
            (attr(e, "type").as_deref() == Some("cached")).then(|| Gather::Icon {
                width: attr(e, "width").and_then(|w| w.parse().ok()).unwrap_or(0),
            })
        }
        (3, Some(b"developer"), b"name") => attr(e, "xml:lang")
            .is_none()
            .then_some(Gather::NestedDeveloperName),
        (3, Some(b"keywords"), b"keyword") => Some(Gather::Keyword),
        (3, Some(b"custom"), b"value") => (attr(e, "key").as_deref()
            == Some("flathub::verification::verified"))
        .then_some(Gather::Verified),
        _ => None,
    }
}

/// Read a catalogue, streaming: the file is never whole in memory, and only
/// the applications come out (`addon`, `runtime`, `generic` and the rest are
/// dropped). `icons` is the catalogue's `icons` directory, whose
/// `128x128/<file>` is kept when it exists.
///
/// # Errors
///
/// When the XML does not parse.
pub fn parse(reader: impl BufRead, icons: Option<&Path>) -> anyhow::Result<Vec<Entry>> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut component: Option<Component> = None;
    let mut gathering: Option<(Gather, usize)> = None;
    let mut text = String::new();
    let mut out = Vec::new();
    loop {
        let event = xml.read_event_into(&mut buf)?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let empty = matches!(event, Event::Empty(_));
                if path.len() == 1 && e.local_name().as_ref() == b"component" {
                    component = Some(Component {
                        kind: attr(e, "type").unwrap_or_default(),
                        ..Component::default()
                    });
                } else if let Some(c) = component.as_mut() {
                    if path.len() == 3
                        && path.last().is_some_and(|p| p == b"releases")
                        && e.local_name().as_ref() == b"release"
                    {
                        let when = attr(e, "timestamp")
                            .and_then(|t| t.parse::<u64>().ok())
                            .unwrap_or(0);
                        if let Some(version) = attr(e, "version")
                            && c.newest.as_ref().is_none_or(|(w, _)| when > *w)
                        {
                            c.newest = Some((when, version));
                        }
                    } else if gathering.is_none() && !empty {
                        gathering = gather_for(&path, e).map(|g| (g, path.len()));
                        text.clear();
                    }
                }
                if !empty {
                    path.push(e.local_name().as_ref().to_vec());
                }
            }
            Event::End(_) => {
                let _ = path.pop();
                if let Some((what, _)) = gathering.take_if(|(_, depth)| *depth == path.len())
                    && let Some(c) = component.as_mut()
                {
                    c.gathered(what, squash(&text));
                }
                if path.len() == 1
                    && let Some(entry) = component.take().and_then(|c| c.finish(icons))
                {
                    out.push(entry);
                }
            }
            Event::Text(t) if gathering.is_some() => text.push_str(&t.decode()?),
            Event::CData(t) if gathering.is_some() => text.push_str(&t.decode()?),
            Event::GeneralRef(r) if gathering.is_some() => {
                if let Some(c) = r.resolve_char_ref()? {
                    text.push(c);
                } else {
                    match r.decode()?.as_ref() {
                        "lt" => text.push('<'),
                        "gt" => text.push('>'),
                        "amp" => text.push('&'),
                        "apos" => text.push('\''),
                        "quot" => text.push('"'),
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/flatpak/appstream.xml");

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-appstream-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entries() -> Vec<Entry> {
        parse(FIXTURE.as_bytes(), None).unwrap()
    }

    #[test]
    fn only_applications_come_out_with_the_id_flatpak_takes() {
        let ids: Vec<String> = entries().into_iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            [
                "com.bixense.PasswordCalculator",
                "com.writerduet.writersolo",
                "dev.heppen.webapps",
                "net.launchpad.Gnuclad"
            ],
            "the runtime and the addon are dropped, and the legacy id loses its .desktop"
        );
    }

    #[test]
    fn an_entry_keeps_what_a_row_shows() {
        let all = entries();
        let web = all.iter().find(|e| e.id == "dev.heppen.webapps").unwrap();
        assert_eq!(
            web.name, "Quick Web Apps",
            "the untranslated name, not the Czech one"
        );
        assert_eq!(web.summary, "Web applications at your fingertips");
        assert_eq!(web.names, ["Rychlé webové aplikace"]);
        assert!(
            web.keywords.contains(&"Manager".to_owned())
                && web.keywords.contains(&"Aplikace".to_owned())
        );
        assert_eq!(
            web.keywords.iter().filter(|k| *k == "Software").count(),
            1,
            "folded-distinct"
        );
        assert!(web.verified);
        assert_eq!(web.developer.as_deref(), Some("hepp3n"));
        assert_eq!(web.homepage.as_deref(), Some("https://webapps.heppen.dev"));
        assert_eq!(web.version, "3.0.0");
        assert_eq!(web.runtime.as_deref(), Some("org.gnome.Platform/x86_64/49"));
        let solo = all
            .iter()
            .find(|e| e.id == "com.writerduet.writersolo")
            .unwrap();
        assert_eq!(solo.developer.as_deref(), Some("WriterDuet"));
        assert!(!solo.verified);
        let gnuclad = all
            .iter()
            .find(|e| e.id == "net.launchpad.Gnuclad")
            .unwrap();
        assert!(gnuclad.console);
        let pw = all
            .iter()
            .find(|e| e.id == "com.bixense.PasswordCalculator")
            .unwrap();
        assert_eq!(pw.version, "1.1", "the newest release, by its time");
    }

    #[test]
    fn a_search_ranks_the_name_before_what_it_does() {
        let cat = Catalogue::new(
            Stamp {
                file: PathBuf::new(),
                size: 0,
                modified: 0,
            },
            entries(),
        );
        let names = |q: &str| {
            search_in(&cat, q, 10)
                .into_iter()
                .map(|f| (f.name, f.by_description))
                .collect::<Vec<_>>()
        };
        assert_eq!(names("gnuclad"), [("Gnuclad".to_owned(), false)]);
        assert_eq!(names("Calcul")[0].0, "Password Calculator");
        // "rychle" is the Czech name, folded; "screenplay" only the summary.
        assert_eq!(names("rychle"), [("Quick Web Apps".to_owned(), false)]);
        assert_eq!(names("screenplay"), [("WriterSolo".to_owned(), true)]);
        assert_eq!(names("cladogram generator"), [("Gnuclad".to_owned(), true)]);
        assert!(names("   ").is_empty());
        assert!(search_in(&cat, "app", 0).is_empty());
    }

    #[test]
    fn an_exact_translated_name_beats_a_keyword_and_a_longer_name() {
        let app = |id: &str, name: &str, names: &[&str], keywords: &[&str]| Entry {
            id: id.into(),
            name: name.into(),
            names: names.iter().map(|n| (*n).to_owned()).collect(),
            keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
            ..Entry::default()
        };
        let cat = Catalogue::new(
            Stamp {
                file: PathBuf::new(),
                size: 0,
                modified: 0,
            },
            vec![
                app("org.a.Bmi", "BMI", &[], &["Calculadora"]),
                app("org.a.Ab", "Ab", &["Calculadora científica"], &[]),
                app("org.gnome.Calculator", "Calculator", &["Calculadora"], &[]),
            ],
        );
        let ids: Vec<String> = search_in(&cat, "calculadora", 10)
            .into_iter()
            .map(|f| f.key.id().to_owned())
            .collect();
        assert_eq!(ids, ["org.gnome.Calculator", "org.a.Bmi", "org.a.Ab"]);
    }

    #[test]
    fn a_row_says_sandboxed_and_what_else_it_is() {
        let all = entries();
        let row = |id: &str| found_of(all.iter().find(|e| e.id == id).unwrap(), false);
        assert_eq!(
            row("dev.heppen.webapps").badges,
            [Badge::Sandboxed, Badge::Verified]
        );
        assert_eq!(
            row("com.writerduet.writersolo").badges,
            [Badge::Sandboxed, Badge::Unfree]
        );
        assert_eq!(
            row("net.launchpad.Gnuclad").badges,
            [Badge::Sandboxed, Badge::CommandLine]
        );
        assert_eq!(
            row("dev.heppen.webapps").key,
            AppKey::Flatpak {
                id: "dev.heppen.webapps".into()
            }
        );
    }

    #[test]
    fn an_entity_is_read_as_its_character() {
        let xml = r#"<components><component type="desktop-application"><id>org.x.Y</id><name>Tom &amp; Jerry&#x21;</name><summary>It&apos;s  a
            test</summary><bundle type="flatpak">app/org.x.Y/x86_64/stable</bundle></component></components>"#;
        let e = &parse(xml.as_bytes(), None).unwrap()[0];
        assert_eq!(e.name, "Tom & Jerry!");
        assert_eq!(e.summary, "It's a test");
    }

    #[test]
    fn a_broken_catalogue_is_an_error_and_not_a_panic() {
        assert!(
            parse(
                "<components><component type=\"desktop\"><id>x</name>".as_bytes(),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn the_cache_follows_the_catalogue_and_the_icon_is_kept_when_it_exists() {
        let dir = scratch("cache");
        let roots = Roots::under(&dir);
        let xml = user_source(&roots);
        let commit = xml.parent().unwrap().parent().unwrap().join("aaaa");
        std::fs::create_dir_all(commit.join("icons/128x128")).unwrap();
        std::fs::write(commit.join("appstream.xml"), FIXTURE).unwrap();
        std::fs::write(commit.join("icons/128x128/dev.heppen.webapps.png"), b"png").unwrap();
        std::os::unix::fs::symlink("aaaa", xml.parent().unwrap()).unwrap();
        let first = load(&xml, &roots.ours_cache()).unwrap();
        let web = first
            .apps
            .iter()
            .find(|e| e.id == "dev.heppen.webapps")
            .unwrap();
        assert_eq!(
            web.icon.as_deref(),
            Some(
                xml.parent()
                    .unwrap()
                    .join("icons/128x128/dev.heppen.webapps.png")
                    .as_path()
            )
        );
        assert!(
            first
                .apps
                .iter()
                .find(|e| e.id == "net.launchpad.Gnuclad")
                .unwrap()
                .icon
                .is_none()
        );
        // The cache is what the second load reads: change a name IN THE CACHE,
        // and the changed name is what comes back.
        let files: Vec<PathBuf> = std::fs::read_dir(roots.ours_cache())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        let text = std::fs::read_to_string(&files[0])
            .unwrap()
            .replace("Quick Web Apps", "From The Cache");
        std::fs::write(&files[0], text).unwrap();
        let again = load(&xml, &roots.ours_cache()).unwrap();
        assert!(again.apps.iter().any(|e| e.name == "From The Cache"));
        // A new catalogue is a new commit directory: the stamp changes.
        let next = xml.parent().unwrap().parent().unwrap().join("bbbb");
        std::fs::create_dir_all(&next).unwrap();
        std::fs::write(next.join("appstream.xml"), "<components></components>").unwrap();
        std::fs::remove_file(xml.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("bbbb", xml.parent().unwrap()).unwrap();
        assert!(load(&xml, &roots.ours_cache()).unwrap().apps.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Measured against the SYSTEM catalogue of the machine it runs on, read
    /// only: `cargo test -p ronoctua-software --all-features -- --ignored
    /// --nocapture measure`.
    #[test]
    #[ignore = "reads /var/lib/flatpak; a measurement, not a check"]
    fn measure_the_system_catalogue() {
        let peak = || {
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("VmHWM:"))
                        .map(str::to_owned)
                })
                .unwrap_or_default()
        };
        let xml = source_under(Path::new(SYSTEM_ROOT));
        let size = std::fs::metadata(&xml).map(|m| m.len()).unwrap_or(0);
        println!(
            "source {} ({size} bytes); before: {}",
            xml.display(),
            peak()
        );
        let started = std::time::Instant::now();
        let reader = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(&xml).unwrap());
        let apps = parse(reader, xml.parent().map(|p| p.join("icons")).as_deref()).unwrap();
        println!(
            "parsed {} apps in {:?}; after: {}",
            apps.len(),
            started.elapsed(),
            peak()
        );
        let with_icon = apps.iter().filter(|a| a.icon.is_some()).count();
        let json = serde_json::to_vec(&Cached {
            format: FORMAT,
            stamp: Stamp {
                file: xml,
                size,
                modified: 0,
            },
            apps,
        })
        .unwrap();
        println!(
            "{with_icon} with an icon on disk; cache {} bytes",
            json.len()
        );
        let started = std::time::Instant::now();
        let back: Cached = serde_json::from_slice(&json).unwrap();
        let cat = Catalogue::new(back.stamp, back.apps);
        println!("cache read in {:?}", started.elapsed());
        let started = std::time::Instant::now();
        let hits = search_in(&cat, "calculator", 20);
        println!(
            "search in {:?}: {:?}",
            started.elapsed(),
            hits.iter().take(5).map(|f| &f.name).collect::<Vec<_>>()
        );
        let started = std::time::Instant::now();
        let hits = search_in(&cat, "calculadora", 20);
        println!(
            "search in {:?}: {:?}",
            started.elapsed(),
            hits.iter().take(5).map(|f| &f.name).collect::<Vec<_>>()
        );
    }
}
