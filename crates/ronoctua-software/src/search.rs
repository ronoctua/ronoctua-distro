//! One query, every source, in parallel: each source answers with a
//! [`Batch`] as soon as it has one, tagged with the query's generation, so the
//! screen draws the local Nix answer at once and the network ones as they
//! arrive — and drops any batch of an older query.
//!
//! # What each source answers from
//!
//! | source | answers from | `from` says |
//! |---|---|---|
//! | Nix | the index built from the system's nixpkgs (`RONOCTUA_NIX_INDEX`) | the revision, short |
//! | Flatpak | Flathub's search API; the local appstream catalogue when that is off or fails | the catalogue's date, offline |
//! | AppImage | AppImageHub's feed, kept a day and fetched again only when the network is allowed | the kept feed's date |
//! | Android | the F-Droid catalogue already fetched and verified — never fetched by a search | its date |
//! | Web | nothing: a web app is made from an address, not found | — |
//!
//! A source this machine has no support for (`crate::state::Supports`: the
//! minimal profile has no Waydroid and no `appimage-run`) is
//! [`Refusal::SourceAbsent`] before anything is read: a row nobody could
//! install is not a result. The F-Droid index is 61 MB, so a search never
//! fetches it: that is [`crate::Order::Refresh`], through the queue, and until
//! it ran the answer is [`Refusal::SourceUnprepared`].
//!
//! No row is marked [`crate::found::Badge::Installed`] here: the screen knows
//! what is installed from its own snapshot, and a search that listed the
//! machine for each query would pay for it on every key press.

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use crate::found::{Batch, Found, Status};
use crate::state::Supports;
use crate::{Context, Refusal, Source};

/// The most rows ONE source answers with, in its own order (best first).
///
/// Thirty: the view shows the sources together and a person reads a screen of
/// them, not a catalogue; what ranked thirty-first under one source is found by
/// typing one more letter. It is also half of Flathub's own page ceiling
/// ([`crate::flathub::MOST_HITS`]), so one request answers it.
pub const MOST: usize = 30;

/// How many characters of the nixpkgs revision `from` carries — git's short
/// form, which is also what NixOS puts in a version (`26.05.20260920.abc1234`).
pub const SHORT_REV: usize = 7;

/// What a search (and a program's details) reaches beyond the [`Context`]:
/// the network, the SYSTEM Flatpak installation and the clock. Parameters, so
/// a test hands a fake network and a scratch installation and never reads the
/// machine's.
#[derive(Clone, Copy)]
pub struct Reach<'a> {
    /// The network: Flathub's search and details, AppImageHub's feed,
    /// GitHub's release pages.
    #[cfg(feature = "net")]
    pub net: &'a dyn crate::net::Fetch,
    /// The system installation's root (`/var/lib/flatpak`), whose catalogue
    /// answers when the person's has none, and whose runtimes count as
    /// installed.
    pub flatpak_system: &'a Path,
    /// Now, in seconds since the epoch: how old a kept catalogue is.
    pub now: u64,
}

impl Reach<'static> {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            #[cfg(feature = "net")]
            net: &crate::net::Net,
            flatpak_system: Path::new(crate::appstream::SYSTEM_ROOT),
            now: crate::appimage::now(),
        }
    }
}

/// Ask every source in `sources` for `query`, calling `sink` once per source.
/// `language` is the screen's (`pt-BR`), for sources whose text is
/// translated (Flathub, F-Droid). Blocks until every source answered: call it
/// on a thread.
pub fn run(
    ctx: &Context,
    query: &str,
    sources: &[Source],
    language: &str,
    generation: u64,
    sink: &(dyn Fn(Batch) + Sync),
) {
    run_with(
        ctx,
        query,
        sources,
        language,
        generation,
        sink,
        &Reach::system(),
    );
}

/// [`run`], reaching through `reach`.
///
/// Each source named in `sources` answers EXACTLY once, however often it is
/// named and even when its thread cannot be started (it then answers on this
/// one) or it panics (it then answers [`Refusal::ToolFailed`]): a source that
/// never answered would leave the screen saying "asking" for ever. A query
/// that is only blanks does no work at all: every source answers at once,
/// with nothing.
pub fn run_with(
    ctx: &Context,
    query: &str,
    sources: &[Source],
    language: &str,
    generation: u64,
    sink: &(dyn Fn(Batch) + Sync),
    reach: &Reach<'_>,
) {
    let mut asked: Vec<Source> = Vec::with_capacity(sources.len());
    for s in sources {
        if !asked.contains(s) {
            asked.push(*s);
        }
    }
    let query = query.trim();
    if query.is_empty() {
        for source in asked {
            sink(Batch {
                source,
                generation,
                found: Vec::new(),
                status: Status::Answered { from: None },
            });
        }
        return;
    }
    std::thread::scope(|scope| {
        for source in asked {
            let one = move || sink(batch_of(ctx, query, source, language, generation, reach));
            let spawned = std::thread::Builder::new()
                .name(format!("search-{}", source.word()))
                .spawn_scoped(scope, one);
            if let Err(e) = spawned {
                tracing::warn!(source = source.word(), error = %e, "a search thread could not start");
                sink(batch_of(ctx, query, source, language, generation, reach));
            }
        }
    });
}

/// One source's batch; a panic inside it is a refusal, never a silence.
fn batch_of(
    ctx: &Context,
    query: &str,
    source: Source,
    language: &str,
    generation: u64,
    reach: &Reach<'_>,
) -> Batch {
    let (found, status) = std::panic::catch_unwind(AssertUnwindSafe(|| {
        answer(ctx, query, source, language, reach)
    }))
    .unwrap_or_else(|_| refused(Refusal::ToolFailed));
    Batch {
        source,
        generation,
        found,
        status,
    }
}

/// What one source says about a (non-blank) query.
fn answer(
    ctx: &Context,
    query: &str,
    source: Source,
    language: &str,
    reach: &Reach<'_>,
) -> (Vec<Found>, Status) {
    // A WEB APP IS NEVER FOUND: it is made from an address the person types.
    if source == Source::Web {
        return (Vec::new(), Status::Answered { from: None });
    }
    if !Supports::of(&ctx.tools).has(source) {
        return refused(Refusal::SourceAbsent);
    }
    match source {
        Source::Nix => nix(ctx, query),
        Source::Flatpak => flatpak(ctx, query, language, reach),
        Source::AppImage => appimage(ctx, query, reach),
        Source::Android => android(ctx, query, language),
        Source::Web => (Vec::new(), Status::Answered { from: None }),
    }
}

fn refused(refusal: Refusal) -> (Vec<Found>, Status) {
    (Vec::new(), Status::Refused { refusal })
}

/// The index of the system's nixpkgs. A library (a nested set such as
/// `python3Packages.requests`) is found only by a query that names a nested
/// attribute — one with a dot: typing `requests` means a program.
fn nix(ctx: &Context, query: &str) -> (Vec<Found>, Status) {
    let Some(index) = ctx.tools.nix_index.as_deref() else {
        return refused(Refusal::SourceAbsent);
    };
    if let Err(r) = crate::nix_index::check(index) {
        return refused(r);
    }
    let found = crate::nix_index::search(index, query, query.contains('.'), MOST);
    let from = ctx.tools.nixpkgs_rev.as_deref().map(short_rev);
    (found, Status::Answered { from })
}

/// Flathub's API when the person allows the network and Flathub; otherwise,
/// or when that fails, the local catalogue — the person's installation's, or
/// the system's when the person's has none — with its date. `Off` when the
/// person turned the online search off and there is no catalogue to read.
fn flatpak(ctx: &Context, query: &str, language: &str, reach: &Reach<'_>) -> (Vec<Found>, Status) {
    let online = ctx.prefs.online && ctx.prefs.sources.flathub;
    // What the online search said when it did not answer.
    #[cfg(feature = "net")]
    let failed = if online {
        match crate::flathub::search_with(reach.net, query, language, MOST) {
            Ok(found) => return (found, Status::Answered { from: None }),
            Err(r) => {
                tracing::debug!(
                    refusal = r.word(),
                    "Flathub did not answer; the local catalogue does"
                );
                Some(r)
            }
        }
    } else {
        None
    };
    // Without the network compiled in, "online" can only be offline.
    #[cfg(not(feature = "net"))]
    let failed = {
        let _ = language;
        online.then_some(Refusal::Offline)
    };
    let Some(xml) = local_catalogue(&ctx.roots, reach.flatpak_system) else {
        return failed.map_or((Vec::new(), Status::Off), refused);
    };
    match crate::appstream::current(&xml, &ctx.roots.ours_cache()) {
        Ok(catalogue) => (
            crate::appstream::search_in(&catalogue, query, MOST),
            Status::Answered {
                from: crate::appstream::fetched(&xml).map(date_of),
            },
        ),
        Err(e) => {
            tracing::warn!(%e, file = %xml.display(), "the offline catalogue could not be read");
            refused(failed.unwrap_or(Refusal::ToolFailed))
        }
    }
}

/// The local Flathub catalogue: the person's installation's, or the system's
/// (under `system`) when the person's has none; `None` when neither exists.
#[must_use]
pub fn local_catalogue(roots: &crate::Roots, system: &Path) -> Option<PathBuf> {
    [
        crate::appstream::user_source(roots),
        crate::appstream::source_under(system),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// AppImageHub's feed: the kept one while it is younger than a day, a fetched
/// one when it is not and the person allows the network and AppImageHub, and
/// the kept one of any age when fetching fails. `Off` when the network is not
/// allowed and nothing was ever kept.
fn appimage(ctx: &Context, query: &str, reach: &Reach<'_>) -> (Vec<Found>, Status) {
    #[cfg(feature = "net")]
    {
        let online = ctx.prefs.online && ctx.prefs.sources.appimagehub;
        let kept = crate::appimagehub::kept_at(&ctx.roots);
        if !online && !kept.is_file() {
            return (Vec::new(), Status::Off);
        }
        match crate::appimagehub::catalogue_with(reach.net, &ctx.roots, online, reach.now) {
            Ok(items) => (
                crate::appimagehub::search_in(&items, query, MOST),
                Status::Answered {
                    from: modified(&kept).map(date_of),
                },
            ),
            Err(f) => refused(f.refusal.unwrap_or(Refusal::ToolFailed)),
        }
    }
    #[cfg(not(feature = "net"))]
    {
        // The catalogue's reader is the network half of the crate: a build
        // without it has no AppImageHub at all.
        let _ = (ctx, query, reach);
        refused(Refusal::Offline)
    }
}

/// The F-Droid catalogue already fetched — compacted only after its signed
/// chain verified (`crate::fdroid::refresh_with`), so what is on disk is what
/// was verified — searched in `language` ([`crate::fdroid::search_apps`], as
/// `fdroid::search_for` does, holding the catalogue once for its date too).
fn android(ctx: &Context, query: &str, language: &str) -> (Vec<Found>, Status) {
    let Some(catalogue) = crate::fdroid::catalogue(&ctx.roots) else {
        return refused(Refusal::SourceUnprepared);
    };
    (
        crate::fdroid::search_apps(&catalogue.apps, query, language, MOST),
        Status::Answered {
            from: Some(date_of(catalogue.timestamp / 1000)),
        },
    )
}

/// The first [`SHORT_REV`] characters of a revision.
fn short_rev(rev: &str) -> String {
    rev.trim().chars().take(SHORT_REV).collect()
}

/// When a file was last written, in seconds since the epoch.
#[cfg(feature = "net")]
fn modified(path: &Path) -> Option<u64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    t.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// A day as `from` carries it: `YYYY-MM-DD`, in UTC — data, the same in every
/// language, and what a person reads as "the catalogue of that day".
#[must_use]
pub fn date_of(secs: u64) -> String {
    let (year, month, day) = civil_of(secs / 86_400);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's `civil_from_days` for days since 1970-01-01 (never
/// before it: the clock this reads cannot go there).
fn civil_of(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{AppKey, Roots, Tools};

    /// A small index in the real format (`nix/scripts/software-index.py`).
    pub(crate) const INDEX: &str = "\
#ronoctua-software-nix-index\t1\t5
gimp\tgimp\t3.0.8\t\tgimp\tGPL-3.0-or-later\thttps://www.gimp.org/\tGNU Image Manipulation Program
google-chrome\tgoogle-chrome\t151.0.7922.71\tu\tgoogle-chrome-stable\tunfree\thttps://www.google.com/chrome/browser/\tFreeware web browser developed by Google
hello\thello\t2.12.3\t\thello\tGPL-3.0-or-later\thttps://www.gnu.org/software/hello/manual/\tProgram that produces a familiar, friendly greeting
python3Packages.requests\trequests\t2.32.5\tl\t\tApache-2.0\thttps://requests.readthedocs.io\tHTTP library for Python
ripgrep\tripgrep\t15.1.0\t\trg\tUnlicense\thttps://github.com/BurntSushi/ripgrep\tUtility that combines the usability of The Silver Searcher with the raw speed of grep
";

    pub(crate) const REV: &str = "3f2b1c0d9e8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

    pub(crate) fn fixture(path: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(path),
        )
        .unwrap()
    }

    pub(crate) fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-search-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A machine that supports every source, with the index above; no
    /// program is ever run by a search, so the tools are paths to nothing.
    pub(crate) fn context(root: &Path) -> Context {
        let index = root.join("index.tsv");
        std::fs::write(&index, INDEX).unwrap();
        let nowhere = || Some(root.join("no-such-program"));
        Context {
            tools: Tools {
                nix: nowhere(),
                flatpak: nowhere(),
                appimage_run: nowhere(),
                unsquashfs: nowhere(),
                waydroid: nowhere(),
                chrome: nowhere(),
                nix_index: Some(index),
                nixpkgs_rev: Some(REV.into()),
                ..Tools::default()
            },
            roots: Roots::under(root),
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        }
    }

    /// A Flathub catalogue under `installation`, fetched at `when`.
    pub(crate) fn plant_catalogue(installation: &Path, when: u64) -> PathBuf {
        let xml = crate::appstream::source_under(installation);
        std::fs::create_dir_all(xml.parent().unwrap()).unwrap();
        std::fs::write(&xml, fixture("flatpak/appstream.xml")).unwrap();
        let stamp = xml.parent().unwrap().parent().unwrap().join(".timestamp");
        let file = std::fs::File::create(&stamp).unwrap();
        file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(when))
            .unwrap();
        xml
    }

    fn one(batches: &[Batch], source: Source) -> &Batch {
        let mut of = batches.iter().filter(|b| b.source == source);
        let b = of
            .next()
            .unwrap_or_else(|| panic!("{} never answered", source.word()));
        assert!(of.next().is_none(), "{} answered twice", source.word());
        b
    }

    #[cfg(feature = "net")]
    fn reach<'a>(net: &'a dyn crate::net::Fetch, system: &'a Path) -> Reach<'a> {
        Reach {
            net,
            flatpak_system: system,
            now: 1_790_000_000,
        }
    }

    #[cfg(not(feature = "net"))]
    fn reach(system: &Path) -> Reach<'_> {
        Reach {
            flatpak_system: system,
            now: 1_790_000_000,
        }
    }

    #[test]
    fn a_day_is_written_the_same_in_every_language() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(951_782_400), "2000-02-29");
        assert_eq!(date_of(1_790_000_000), "2026-09-21");
        assert_eq!(date_of(4_107_542_399), "2100-02-28");
        assert_eq!(short_rev(REV), "3f2b1c0");
    }

    #[cfg(feature = "net")]
    mod online {
        use super::*;
        use crate::found::Badge;
        use crate::net::canned::Canned;

        /// Every batch a search gave, in the order they came.
        fn search(
            ctx: &Context,
            query: &str,
            sources: &[Source],
            language: &str,
            reach: &Reach<'_>,
        ) -> Vec<Batch> {
            let got = Mutex::new(Vec::new());
            run_with(
                ctx,
                query,
                sources,
                language,
                7,
                &|b| got.lock().unwrap().push(b),
                reach,
            );
            got.into_inner().unwrap()
        }

        fn ids(found: &[Found]) -> Vec<&str> {
            found.iter().map(|f| f.key.id()).collect()
        }

        const FLATHUB_PT: &str = "https://flathub.org/api/v2/search?locale=pt-BR";

        #[test]
        fn a_blank_query_answers_every_source_at_once_with_nothing() {
            let root = scratch("blank");
            let mut ctx = context(&root);
            // Were any work done, this would be a refusal.
            ctx.tools.nix_index = None;
            let net = Canned::default();
            let system = root.join("system");
            let batches = search(&ctx, "  \t ", &Source::ALL, "en", &reach(&net, &system));
            assert_eq!(batches.len(), Source::ALL.len());
            for source in Source::ALL {
                let b = one(&batches, source);
                assert_eq!(b.generation, 7);
                assert!(b.found.is_empty());
                assert_eq!(b.status, Status::Answered { from: None });
            }
            assert!(net.asked.borrow().is_empty(), "nobody was asked");
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn every_source_answers_once_however_often_it_is_named() {
            let root = scratch("once");
            let mut ctx = context(&root);
            ctx.prefs.online = false;
            let net = Canned::default();
            let system = root.join("system");
            let batches = search(
                &ctx,
                "gimp",
                &[
                    Source::Nix,
                    Source::Web,
                    Source::Nix,
                    Source::Android,
                    Source::Web,
                ],
                "en",
                &reach(&net, &system),
            );
            assert_eq!(batches.len(), 3);
            assert_eq!(ids(&one(&batches, Source::Nix).found), ["gimp"]);
            let web = one(&batches, Source::Web);
            assert!(web.found.is_empty(), "a web app is never found");
            assert_eq!(web.status, Status::Answered { from: None });
            assert!(batches.iter().all(|b| b.generation == 7));
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn nix_answers_from_its_index_and_says_which_revision() {
            let root = scratch("nix");
            let ctx = context(&root);
            let net = Canned::default();
            let system = root.join("system");
            let r = reach(&net, &system);
            let batches = search(&ctx, "rg", &[Source::Nix], "en", &r);
            let nix = one(&batches, Source::Nix);
            assert_eq!(ids(&nix.found)[0], "ripgrep");
            assert_eq!(
                nix.status,
                Status::Answered {
                    from: Some("3f2b1c0".into())
                }
            );
            // A library is found by its attribute path, and only by it.
            let b = search(&ctx, "requests", &[Source::Nix], "en", &r);
            assert!(one(&b, Source::Nix).found.is_empty());
            let b = search(&ctx, "python3Packages.requests", &[Source::Nix], "en", &r);
            assert_eq!(
                ids(&one(&b, Source::Nix).found),
                ["python3Packages.requests"]
            );
            // No index, or a foreign one: the machine cannot search Nix.
            let mut none = context(&root);
            none.tools.nix_index = None;
            let b = search(&none, "rg", &[Source::Nix], "en", &r);
            assert_eq!(
                one(&b, Source::Nix).status,
                Status::Refused {
                    refusal: Refusal::SourceAbsent
                }
            );
            std::fs::write(root.join("index.tsv"), "not an index\n").unwrap();
            let b = search(&ctx, "rg", &[Source::Nix], "en", &r);
            assert_eq!(
                one(&b, Source::Nix).status,
                Status::Refused {
                    refusal: Refusal::SourceAbsent
                }
            );
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn a_source_the_machine_does_not_have_is_refused_before_anything_is_read() {
            let root = scratch("absent");
            let mut ctx = context(&root);
            ctx.tools = Tools {
                nix_index: ctx.tools.nix_index.clone(),
                ..Tools::default()
            };
            let net = Canned::default();
            let system = root.join("system");
            let asked = [
                Source::Nix,
                Source::Flatpak,
                Source::AppImage,
                Source::Android,
            ];
            let batches = search(&ctx, "gimp", &asked, "en", &reach(&net, &system));
            for source in asked {
                assert_eq!(
                    one(&batches, source).status,
                    Status::Refused {
                        refusal: Refusal::SourceAbsent
                    },
                    "{}",
                    source.word()
                );
            }
            assert!(net.asked.borrow().is_empty());
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn flathub_answers_in_the_persons_language_and_marks_the_verified() {
            let root = scratch("flathub");
            let ctx = context(&root);
            let net = Canned::new(vec![(
                FLATHUB_PT,
                200,
                vec![],
                fixture("flatpak/flathub-search-calculator.json"),
            )]);
            let system = root.join("system");
            let batches = search(
                &ctx,
                "calculator",
                &[Source::Flatpak],
                "pt_BR",
                &reach(&net, &system),
            );
            let flatpak = one(&batches, Source::Flatpak);
            assert_eq!(flatpak.status, Status::Answered { from: None });
            assert_eq!(
                ids(&flatpak.found),
                [
                    "org.gnome.Calculator",
                    "org.kde.kalk",
                    "dev.edfloreshz.Calculator"
                ],
                "the API's own order"
            );
            assert!(flatpak.found[0].badges.contains(&Badge::Verified));
            let posted = net.posted.borrow();
            let body: serde_json::Value = serde_json::from_str(&posted[0].1).unwrap();
            assert_eq!(body["query"], "calculator");
            assert_eq!(body["hits_per_page"], MOST);
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn without_flathub_the_local_catalogue_answers_and_says_its_date() {
            let root = scratch("offline-catalogue");
            let ctx = context(&root);
            plant_catalogue(&ctx.roots.data.join("flatpak"), 1_790_000_000);
            let system = root.join("system");
            // Flathub fails (nothing is canned: the fake is offline).
            let net = Canned::default();
            let batches = search(
                &ctx,
                "password",
                &[Source::Flatpak],
                "en",
                &reach(&net, &system),
            );
            let flatpak = one(&batches, Source::Flatpak);
            assert_eq!(
                flatpak.status,
                Status::Answered {
                    from: Some("2026-09-21".into())
                }
            );
            assert_eq!(ids(&flatpak.found)[0], "com.bixense.PasswordCalculator");
            assert_eq!(net.asked.borrow().len(), 1, "Flathub was asked first");
            // Turned off: nobody is asked, and the catalogue still answers.
            let mut off = context(&root);
            off.prefs.sources.flathub = false;
            let net = Canned::default();
            let batches = search(
                &off,
                "password",
                &[Source::Flatpak],
                "en",
                &reach(&net, &system),
            );
            assert!(!one(&batches, Source::Flatpak).found.is_empty());
            assert!(net.asked.borrow().is_empty());
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn the_systems_catalogue_answers_when_the_person_has_none() {
            let root = scratch("system-catalogue");
            let mut ctx = context(&root);
            ctx.prefs.online = false;
            let system = root.join("system");
            plant_catalogue(&system, 86_400);
            let net = Canned::default();
            let batches = search(
                &ctx,
                "gnuclad",
                &[Source::Flatpak],
                "en",
                &reach(&net, &system),
            );
            let flatpak = one(&batches, Source::Flatpak);
            assert_eq!(ids(&flatpak.found), ["net.launchpad.Gnuclad"]);
            assert_eq!(
                flatpak.status,
                Status::Answered {
                    from: Some("1970-01-02".into())
                }
            );
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn with_no_catalogue_flatpak_is_off_when_turned_off_and_refused_when_it_failed() {
            let root = scratch("no-catalogue");
            let system = root.join("system");
            let net = Canned::default();
            let mut ctx = context(&root);
            ctx.prefs.online = false;
            let b = search(
                &ctx,
                "gimp",
                &[Source::Flatpak],
                "en",
                &reach(&net, &system),
            );
            assert_eq!(one(&b, Source::Flatpak).status, Status::Off);
            let ctx = context(&root);
            let b = search(
                &ctx,
                "gimp",
                &[Source::Flatpak],
                "en",
                &reach(&net, &system),
            );
            assert_eq!(
                one(&b, Source::Flatpak).status,
                Status::Refused {
                    refusal: Refusal::Offline
                }
            );
            // Flathub throttling is said as such, not as "offline".
            let throttled = Canned::new(vec![(
                "https://flathub.org/api/v2/search?locale=en",
                429,
                vec![],
                Vec::new(),
            )]);
            let b = search(
                &ctx,
                "gimp",
                &[Source::Flatpak],
                "en",
                &reach(&throttled, &system),
            );
            assert_eq!(
                one(&b, Source::Flatpak).status,
                Status::Refused {
                    refusal: Refusal::RateLimited
                }
            );
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn appimagehub_is_fetched_once_kept_and_off_when_never_kept() {
            let root = scratch("appimagehub");
            let system = root.join("system");
            let mut ctx = context(&root);
            ctx.prefs.online = false;
            let net = Canned::default();
            let b = search(
                &ctx,
                "audacity",
                &[Source::AppImage],
                "en",
                &reach(&net, &system),
            );
            assert_eq!(one(&b, Source::AppImage).status, Status::Off);
            // Allowed, and nothing kept: asked, failing, refused.
            let ctx = context(&root);
            let b = search(
                &ctx,
                "audacity",
                &[Source::AppImage],
                "en",
                &reach(&net, &system),
            );
            assert_eq!(
                one(&b, Source::AppImage).status,
                Status::Refused {
                    refusal: Refusal::Offline
                }
            );
            // Fetched, and kept.
            let net = Canned::new(vec![(
                crate::appimagehub::FEED,
                200,
                vec![],
                fixture("appimage/feed-cut.json"),
            )]);
            let b = search(
                &ctx,
                "audacity",
                &[Source::AppImage],
                "en",
                &reach(&net, &system),
            );
            let appimage = one(&b, Source::AppImage);
            assert_eq!(ids(&appimage.found), ["audacity"]);
            assert!(matches!(
                &appimage.status,
                Status::Answered { from: Some(_) }
            ));
            // Turned off now: the kept feed still answers, and nobody is asked.
            let mut off = context(&root);
            off.prefs.sources.appimagehub = false;
            let none = Canned::default();
            let b = search(
                &off,
                "obsidian",
                &[Source::AppImage],
                "en",
                &reach(&none, &system),
            );
            assert_eq!(ids(&one(&b, Source::AppImage).found), ["obsidian"]);
            assert!(none.asked.borrow().is_empty());
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn android_searches_only_a_catalogue_already_here_in_the_persons_language() {
            let root = scratch("android");
            let ctx = context(&root);
            let net = Canned::default();
            let system = root.join("system");
            let r = reach(&net, &system);
            let b = search(&ctx, "f-droid", &[Source::Android], "pt-BR", &r);
            assert_eq!(
                one(&b, Source::Android).status,
                Status::Refused {
                    refusal: Refusal::SourceUnprepared
                },
                "a search never fetches the 61 MB index"
            );
            assert!(net.asked.borrow().is_empty() && net.fetched.borrow().is_empty());
            plant_fdroid(&ctx.roots);
            let b = search(&ctx, "f-droid", &[Source::Android], "pt-BR", &r);
            let android = one(&b, Source::Android);
            assert_eq!(
                android.status,
                Status::Answered {
                    from: Some("2026-09-21".into())
                }
            );
            assert_eq!(ids(&android.found)[0], "org.fdroid.fdroid");
            assert_eq!(
                android.found[0].summary,
                fdroid_summary("org.fdroid.fdroid", "pt-BR")
            );
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn nothing_is_marked_installed_and_every_source_keeps_its_order() {
            let root = scratch("order");
            let ctx = context(&root);
            plant_fdroid(&ctx.roots);
            let net = Canned::new(vec![
                (
                    "https://flathub.org/api/v2/search?locale=en",
                    200,
                    vec![],
                    fixture("flatpak/flathub-search-calculator.json"),
                ),
                (
                    crate::appimagehub::FEED,
                    200,
                    vec![],
                    fixture("appimage/feed-cut.json"),
                ),
            ]);
            let system = root.join("system");
            let batches = search(&ctx, "a", &Source::ALL, "en", &reach(&net, &system));
            assert_eq!(batches.len(), Source::ALL.len());
            for b in &batches {
                assert!(
                    b.found
                        .iter()
                        .all(|f| !f.badges.contains(&Badge::Installed)),
                    "{}",
                    b.source.word()
                );
                assert!(b.found.len() <= MOST);
                assert!(
                    b.found.iter().all(|f| f.key.source() == Some(b.source)),
                    "{} answered for another source",
                    b.source.word()
                );
            }
            let nix = one(&batches, Source::Nix);
            let direct =
                crate::nix_index::search(ctx.tools.nix_index.as_deref().unwrap(), "a", false, MOST);
            assert_eq!(nix.found, direct, "the index's own ranking, untouched");
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn a_source_that_panics_still_answers() {
            struct Panicking;
            impl crate::net::Fetch for Panicking {
                fn get(
                    &self,
                    _: &str,
                    _: &[(&str, &str)],
                    _: u64,
                ) -> Result<crate::net::Got, crate::Failure> {
                    panic!("a fake that breaks")
                }
                fn post_json(
                    &self,
                    _: &str,
                    _: &str,
                    _: u64,
                ) -> Result<crate::net::Got, crate::Failure> {
                    panic!("a fake that breaks")
                }
                fn download(
                    &self,
                    _: &str,
                    _: u64,
                    _: &Path,
                    _: &mut dyn FnMut(u64, Option<u64>) -> bool,
                ) -> Result<(), crate::Failure> {
                    panic!("a fake that breaks")
                }
            }
            let root = scratch("panic");
            let ctx = context(&root);
            let system = root.join("system");
            let b = search(
                &ctx,
                "gimp",
                &[Source::Flatpak, Source::Nix],
                "en",
                &reach(&Panicking, &system),
            );
            assert_eq!(
                one(&b, Source::Flatpak).status,
                Status::Refused {
                    refusal: Refusal::ToolFailed
                }
            );
            assert_eq!(ids(&one(&b, Source::Nix).found), ["gimp"]);
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// The F-Droid fixture, compacted and kept as a refresh would, dated
    /// 2026-09-21.
    /// F-Droid's own summary of an app in `language`, read from the fixture —
    /// the translated text is DATA, and a literal copy of it would be a
    /// Portuguese sentence in a Rust file (invariant 18).
    #[cfg_attr(not(feature = "net"), allow(dead_code))]
    pub(crate) fn fdroid_summary(package: &str, language: &str) -> String {
        let apps = crate::fdroid::compact_index(std::io::Cursor::new(fixture(
            "android/index-v2-cut.json",
        )))
        .unwrap();
        let app = apps.iter().find(|a| a.package == package).unwrap();
        let text = app.summary.get(language).cloned().unwrap();
        assert_ne!(
            Some(&text),
            app.summary.get("en-US"),
            "a translated summary"
        );
        text
    }

    pub(crate) fn plant_fdroid(roots: &Roots) {
        let apps = crate::fdroid::compact_index(std::io::Cursor::new(fixture(
            "android/index-v2-cut.json",
        )))
        .unwrap();
        crate::fdroid::keep(
            roots,
            &crate::fdroid::Catalogue {
                format: crate::fdroid::FORMAT,
                timestamp: 1_790_000_000_000,
                index_sha256: "c".repeat(64),
                apps,
            },
        )
        .unwrap();
    }

    #[cfg(not(feature = "net"))]
    #[test]
    fn without_the_network_the_local_sources_still_answer() {
        let root = scratch("local-only");
        let ctx = context(&root);
        plant_catalogue(&ctx.roots.data.join("flatpak"), 1_790_000_000);
        plant_fdroid(&ctx.roots);
        let system = root.join("system");
        let got = Mutex::new(Vec::new());
        run_with(
            &ctx,
            "calculator",
            &Source::ALL,
            "en",
            1,
            &|b| got.lock().unwrap().push(b),
            &reach(&system),
        );
        let batches = got.into_inner().unwrap();
        assert!(!one(&batches, Source::Flatpak).found.is_empty());
        assert_eq!(
            one(&batches, Source::AppImage).status,
            Status::Refused {
                refusal: Refusal::Offline
            }
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn found_rows_are_never_about_another_key_kind() {
        // The Android rows of a planted catalogue carry Android keys.
        let root = scratch("keys");
        let roots = Roots::under(&root);
        plant_fdroid(&roots);
        let c = crate::fdroid::catalogue(&roots).unwrap();
        let rows = crate::fdroid::search_apps(&c.apps, "aurora", "en", MOST);
        assert!(matches!(rows[0].key, AppKey::Android { .. }));
        let _ = std::fs::remove_dir_all(&root);
    }
}
