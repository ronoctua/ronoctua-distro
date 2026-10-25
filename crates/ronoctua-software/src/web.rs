//! Web apps: a `.desktop` that opens a site in Chrome with nothing of Chrome's
//! (`--app=`, never `--kiosk`). The `.desktop` file is the single source of
//! truth; its `X-Ronoctua-Web-*` keys carry what the editor needs.
//!
//! # Why `--app=` and nothing else
//!
//! Read in Chromium's source, not guessed. An `--app=URL` window is a
//! `TYPE_APP` browser with no web-app controller: no omnibox, no tab strip,
//! no toolbar, and on a compositor that offers server-side decorations Chrome
//! asks for them — so a tiled window is the page and the compositor's border.
//! It works with a Chrome already running, in the person's own profile and
//! logins. `--kiosk` and `--start-fullscreen` are read from the FIRST Chrome
//! process of the session: passed by a web app that happens to start Chrome,
//! they lock every later window of ordinary browsing in full screen; passed to
//! a Chrome already running, they are ignored. `--user-data-dir` is a second
//! browser without the person's logins. And `--enable-features=` is replaced,
//! not added to — the last one on a command line wins, and the wrapper passes
//! its own. [`open_argv`] carries none of the four, and a test says so.
//!
//! # Why the URL is not in `Exec=`
//!
//! `Exec=` runs `ronoctua-software web-open <slug>`, and the slug is
//! `[a-z0-9-]`: nothing in it needs quoting, and nothing a site's address can
//! hold reaches a line whose grammar has quotes, escapes and `%` codes. The
//! address lives in `X-Ronoctua-Web-Url=`, read back by [`open_argv`] and
//! handed to Chrome as ONE argument, with no shell anywhere.
//!
//! # Chrome's window id
//!
//! [`app_id`] is Chromium's own arithmetic, read in its source
//! (`GenerateApplicationNameFromURL`, `GetShortcutFileName`,
//! `ReplaceIllegalCharactersInPath`): it becomes the `.desktop`'s
//! `StartupWMClass`, how the modeline names the window, and why two web apps
//! for the same host, path and profile are refused — Chrome would give both
//! windows one id.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use url::Url;

use crate::found::Picture;
use crate::order::{IconChoice, WebSpec};
use crate::progress::{Phase, Report};
use crate::state::{Installed, Origin};
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Refusal, Roots};

/// The command every web app's `Exec=` runs: a STABLE path, which the next
/// generation of the system answers too (a store path would die with the
/// generation that wrote it).
pub const SOFTWARE: &str = "/run/current-system/sw/bin/ronoctua-software";

/// The `TryExec=` of every web app: without Chrome the launcher hides them,
/// instead of offering a line that does nothing. Chrome is a part (not
/// free): its program in the session's view, there only once it arrived.
#[must_use]
pub fn chrome() -> PathBuf {
    ronoctua_parts::view::bin("google-chrome-stable")
}

/// Every web app's `.desktop` is `<PREFIX><slug>.desktop`. Deliberately not
/// `br.ronoctua.`, which the launcher ranks as the distro's own.
pub const PREFIX: &str = "ronoctua-web-";

/// The longest address kept (Chromium's own display limit is 2 MiB; a person's
/// web app is a few dozen bytes, and anything near this is not one).
pub const URL_MOST: usize = 2048;

/// The longest name kept, in characters.
pub const NAME_MOST: usize = 64;

/// The profile directory Chrome makes first, and the one used when there is
/// no `Local State` to read.
pub const DEFAULT_PROFILE: &str = "Default";

/// The freedesktop main categories a web app may take. What is not one of
/// these becomes [`FALLBACK_CATEGORY`].
pub const CATEGORIES: [&str; 13] = [
    "AudioVideo",
    "Audio",
    "Video",
    "Development",
    "Education",
    "Game",
    "Graphics",
    "Network",
    "Office",
    "Science",
    "Settings",
    "System",
    "Utility",
];

/// A web app needs the network to be anything, so `Network` is its category
/// when it names none — what picks the launcher's fallback glyph.
pub const FALLBACK_CATEGORY: &str = "Network";

/// The most a prepared icon may weigh: a 512 px PNG is well under a
/// megabyte, so a file above this is not one the preview wrote.
pub const PREPARED_MOST: u64 = 8 << 20;

/// One web app, as the editor reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebApp {
    /// Its slug (the file is `ronoctua-web-<slug>.desktop`).
    pub slug: String,
    /// The name in the launcher.
    pub name: String,
    /// The address it opens.
    pub url: String,
    /// Chrome's profile directory.
    pub profile: String,
    /// Where its icon came from.
    pub icon: Option<IconChoice>,
    /// The icon file it uses now.
    pub icon_file: Option<PathBuf>,
    /// Its category.
    pub category: Option<String>,
}

/// One Chrome profile, from `Local State`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeProfile {
    /// The directory (`Default`, `Profile 1`): what `--profile-directory` takes.
    pub directory: String,
    /// The name the person gave it in Chrome (data).
    pub name: String,
    /// It is the one Chrome used last.
    pub last_used: bool,
}

/// The desktop-file id of a slug's web app.
#[must_use]
pub fn desktop_id(slug: &str) -> String {
    format!("{PREFIX}{slug}.desktop")
}

/// The web apps, read from their `.desktop` files, as installed rows. Reads
/// the disk: call it on a thread.
///
/// A row whose address no longer passes [`canonical_url`] (an edit by hand)
/// carries that refusal as its trouble instead of vanishing: a person who
/// broke it is shown what broke.
#[must_use]
pub fn installed(roots: &Roots) -> Vec<Installed> {
    list(roots)
        .into_iter()
        .map(|app| Installed {
            key: AppKey::Web {
                slug: app.slug.clone(),
            },
            desktop_ids: vec![desktop_id(&app.slug)],
            version: String::new(),
            size_bytes: None,
            update: None,
            unfree: false,
            origin: Origin::Mine,
            icon: app.icon_file.clone().map(|path| Picture::File { path }),
            trouble: canonical_url(&app.url).err(),
            name: app.name,
        })
        .collect()
}

/// The web apps, whole, for the editor, by slug. Reads the disk: call it on
/// a thread.
#[must_use]
pub fn list(roots: &Roots) -> Vec<WebApp> {
    let Ok(dir) = std::fs::read_dir(roots.applications()) else {
        return Vec::new();
    };
    let mut out: Vec<WebApp> = dir
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let slug = name.strip_prefix(PREFIX)?.strip_suffix(".desktop")?;
            if !is_slug(slug) {
                return None;
            }
            web_app_of(slug, &std::fs::read_to_string(e.path()).ok()?)
        })
        .collect();
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    out
}

/// One web app, by its slug. Reads the disk.
#[must_use]
pub fn read(roots: &Roots, slug: &str) -> Option<WebApp> {
    if !is_slug(slug) {
        return None;
    }
    let text = std::fs::read_to_string(roots.applications().join(desktop_id(slug))).ok()?;
    web_app_of(slug, &text)
}

/// One web app, wherever its entry is: the person's own first (it shadows
/// the system's of the same name, as the launcher reads them), then each
/// folder of `$XDG_DATA_DIRS` — where a DECLARED web app's entry is, in the
/// person's profile the system built. Reads the disk.
#[must_use]
pub fn read_anywhere(roots: &Roots, slug: &str) -> Option<WebApp> {
    if !is_slug(slug) {
        return None;
    }
    read(roots, slug).or_else(|| read_system(roots, slug))
}

/// One web app's entry in the folders of `$XDG_DATA_DIRS` only — the
/// declared copy the system built, never the person's own. Reads the disk.
#[must_use]
pub fn read_system(roots: &Roots, slug: &str) -> Option<WebApp> {
    if !is_slug(slug) {
        return None;
    }
    roots.data_dirs.iter().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join("applications").join(desktop_id(slug))).ok()?;
        web_app_of(slug, &text)
    })
}

/// A web app out of its `.desktop` text; `None` when the file is not one of
/// ours (no `X-Ronoctua-Web-Url`). Pure.
#[must_use]
pub fn web_app_of(slug: &str, text: &str) -> Option<WebApp> {
    use ronoctua_apps::desktop;
    let fields = desktop::fields(text);
    let url = desktop::raw_bytes(&fields, "X-Ronoctua-Web-Url")?
        .trim()
        .to_owned();
    let name = desktop::raw_bytes(&fields, "Name")
        .map(str::trim)
        .unwrap_or_default()
        .to_owned();
    let profile = desktop::raw_bytes(&fields, "X-Ronoctua-Web-Profile")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or(DEFAULT_PROFILE)
        .to_owned();
    let icon = desktop::raw_bytes(&fields, "X-Ronoctua-Web-Icon")
        .and_then(|j| serde_json::from_str(j).ok());
    let icon_file = desktop::raw_bytes(&fields, "Icon")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let category = desktop::list(&fields, "Categories")
        .first()
        .map(|c| (*c).to_owned());
    Some(WebApp {
        slug: slug.to_owned(),
        name,
        url,
        profile,
        icon,
        icon_file,
        category,
    })
}

/// Chrome's profiles (read-only), last used first.
///
/// Read from `$XDG_CONFIG_HOME/google-chrome/Local State` and never written:
/// that file is Chrome's, rewritten by it while it runs. No file, or nothing
/// readable in it, is one profile — [`DEFAULT_PROFILE`], which is what Chrome
/// makes first. Reads the disk: call it on a thread.
#[must_use]
pub fn chrome_profiles(roots: &Roots) -> Vec<ChromeProfile> {
    let path = roots.config.join("google-chrome").join("Local State");
    let found = std::fs::read(path)
        .map(|b| profiles_of(&b))
        .unwrap_or_default();
    if found.is_empty() {
        vec![ChromeProfile {
            directory: DEFAULT_PROFILE.into(),
            name: DEFAULT_PROFILE.into(),
            last_used: true,
        }]
    } else {
        found
    }
}

/// The profiles `Local State` lists (`profile.info_cache`), the last used
/// (`profile.last_used`) first, then `Default`, then the others in their
/// number's order (`Profile 2` before `Profile 10`). Pure.
#[must_use]
pub fn profiles_of(local_state: &[u8]) -> Vec<ChromeProfile> {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(local_state) else {
        return Vec::new();
    };
    let profile = json.get("profile");
    let last = profile
        .and_then(|p| p.get("last_used"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(DEFAULT_PROFILE);
    let Some(cache) = profile
        .and_then(|p| p.get("info_cache"))
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let mut out: Vec<ChromeProfile> = cache
        .iter()
        .filter(|(dir, _)| is_profile_dir(dir))
        .map(|(dir, info)| ChromeProfile {
            directory: dir.clone(),
            name: info
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(dir)
                .to_owned(),
            last_used: dir == last,
        })
        .collect();
    let number = |d: &str| {
        d.rsplit(' ')
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(u64::MAX)
    };
    out.sort_by(|a, b| {
        (
            !a.last_used,
            a.directory != DEFAULT_PROFILE,
            number(&a.directory),
            &a.directory,
        )
            .cmp(&(
                !b.last_used,
                b.directory != DEFAULT_PROFILE,
                number(&b.directory),
                &b.directory,
            ))
    });
    out
}

/// A typed address made canonical, or why it cannot be a web app
/// (`UrlScheme`, `UrlCredentials`, `UrlInvalid`). Schemeless input gets
/// `https://`.
///
/// Only `http` and `https` open — `--app=` would run a `javascript:`, a
/// `data:` or a `file:` address as readily as a page, and a `chrome:` one is
/// Chrome's own settings. A name and password in an address are refused: they
/// would be written into a file in the home and shown in the editor. A host is
/// required, the result is at most [`URL_MOST`] bytes, and whatever the URL
/// grammar still leaves raw of `` ` `` `$` `'` `"` `\` `{` `}` `|` `^` and
/// white space is percent-encoded — the value is written into a `.desktop`,
/// and those encoded are the same address to every server and the same path
/// to Chrome and to [`app_id`], which would otherwise escape them apart.
///
/// # Errors
///
/// The refusal, as a value.
pub fn canonical_url(typed: &str) -> Result<String, crate::Refusal> {
    let t = typed.trim();
    if t.is_empty() || t.len() > URL_MOST {
        return Err(Refusal::UrlInvalid);
    }
    let with_scheme = if has_scheme(t) {
        t.to_owned()
    } else {
        format!("https://{t}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| Refusal::UrlInvalid)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Refusal::UrlScheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Refusal::UrlCredentials);
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(Refusal::UrlInvalid);
    }
    let out = escaped(url.as_str());
    if out.len() > URL_MOST {
        return Err(Refusal::UrlInvalid);
    }
    Ok(out)
}

/// Whether typed text starts with a scheme. `localhost:8080` and
/// `example.org:8443/x` do not: what follows their colon is a port. Pure.
fn has_scheme(t: &str) -> bool {
    if let Some(at) = t.find("://")
        && !t[..at].contains('/')
    {
        return true;
    }
    let Some((scheme, rest)) = t.split_once(':') else {
        return false;
    };
    let is_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    let port = rest.split(['/', '?', '#']).next().unwrap_or_default();
    is_scheme && !(!port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
}

/// Percent-encode what [`canonical_url`] promises never to leave raw.
fn escaped(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        if matches!(c, '`' | '$' | '\'' | '"' | '\\' | '{' | '}' | '|' | '^')
            || c.is_whitespace()
            || c.is_control()
        {
            let mut buffer = [0u8; 4];
            for b in c.encode_utf8(&mut buffer).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The Wayland `app_id` Chrome gives an `--app=` window of that address in
/// that profile (`chrome-<host>__<path>-<Profile>`): the `.desktop`'s
/// `StartupWMClass`, and how the modeline names the window.
///
/// Chromium, read in its source:
///
/// * `GenerateApplicationNameFromURL` is `host + "_" + path` — `GURL::host()`,
///   which carries NO port, and the query and the fragment dropped. So
///   `localhost:8080/x` and `localhost:9090/x` are one window id.
/// * `GetShortcutFileName` is `"chrome-" + name + "-" + profile directory`,
///   through `ReplaceIllegalCharactersInPath(…, '_')` — `" * / : < > ? \ |`,
///   control, format and non-characters anywhere, white space, `.` and `~`
///   at either end — and then every space to `_`.
/// * `GetXdgAppIdForWebApp` takes that file name without `.desktop`.
///
/// An address that does not parse gives the id of an empty host and path.
#[must_use]
pub fn app_id(url: &str, profile: &str) -> String {
    let (host, path) = Url::parse(url).map_or_else(
        |_| (String::new(), String::new()),
        |u| {
            (
                u.host_str().unwrap_or_default().to_owned(),
                u.path().to_owned(),
            )
        },
    );
    let name = format!("chrome-{host}_{path}-{profile}");
    let last = name.chars().count().saturating_sub(1);
    name.chars()
        .enumerate()
        .map(|(i, c)| {
            let at_an_end = i == 0 || i == last;
            if illegal_anywhere(c)
                || (at_an_end && (c.is_whitespace() || c == '.' || c == '~'))
                || c == ' '
            {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// `base::i18n`'s set of characters illegal anywhere in a file name:
/// `"*/:<>?\|`, control (`Cc`), format (`Cf`) and non-characters.
fn illegal_anywhere(c: char) -> bool {
    let n = u32::from(c);
    matches!(c, '"' | '*' | '/' | ':' | '<' | '>' | '?' | '\\' | '|')
        || c.is_control()
        || is_format(n)
        || (0xFDD0..=0xFDEF).contains(&n)
        || n & 0xFFFE == 0xFFFE
}

/// Unicode's general category `Cf` (Unicode 15.1), which the standard
/// library does not expose. An address's host and path are ASCII once
/// parsed, so this only ever meets a profile directory's name.
fn is_format(n: u32) -> bool {
    const CF: [(u32, u32); 21] = [
        (0x00AD, 0x00AD),
        (0x0600, 0x0605),
        (0x061C, 0x061C),
        (0x06DD, 0x06DD),
        (0x070F, 0x070F),
        (0x0890, 0x0891),
        (0x08E2, 0x08E2),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x2064),
        (0x2066, 0x206F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x110CD, 0x110CD),
        (0x13430, 0x1343F),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0001, 0xE0001),
        (0xE0020, 0xE007F),
    ];
    CF.iter().any(|&(lo, hi)| (lo..=hi).contains(&n))
}

/// Run `WebCreate`, `WebEdit` or `Remove { key: Web }`.
///
/// Writes the person's files (the `.desktop`, the icon) and may reach the
/// network for the icon: call it on a thread. In the `.#dev` rehearsal every
/// check runs and nothing is written; what would be written is printed on
/// standard error.
///
/// # Errors
///
/// The refusal of the check that failed, or [`Refusal::ToolFailed`] with the
/// file system's words when a write failed.
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    match order {
        Order::WebCreate { spec } => write(ctx, None, false, spec, report),
        Order::WebEdit { slug, spec } => write(ctx, Some(slug), false, spec, report),
        Order::Remove {
            key: AppKey::Web { slug },
            ..
        } => remove(ctx, slug, report),
        other => Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("not a web order: {}", other.word()),
        }),
    }
}

/// Write the person's own entry of a DECLARED web app — made when the
/// system's declared one is the only one: the editor's change to it
/// ([`crate::declare::edit_declared_web`], which declares it again).
///
/// # Errors
///
/// As an edit.
#[cfg(all(feature = "net", feature = "draw"))]
pub(crate) fn write_declared(
    ctx: &Context,
    slug: &str,
    spec: &WebSpec,
    report: &mut dyn Report,
) -> Done {
    write(ctx, Some(slug), true, spec, report)
}

/// Create (`existing` `None`) or rewrite a web app; `may_make` lets a
/// rewrite make the person's entry the slug does not have yet (a declared
/// one's).
fn write(
    ctx: &Context,
    existing: Option<&str>,
    may_make: bool,
    spec: &WebSpec,
    report: &mut dyn Report,
) -> Done {
    report.step(Phase::Resolving, 0, 0, &spec.name);
    if chrome_here(ctx).is_none() {
        return Err(Failure::refused(Refusal::NoBrowser));
    }
    let url = canonical_url(&spec.url).map_err(Failure::refused)?;
    let profile = known_profile(&ctx.roots, &spec.profile)?;
    let host = Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default();
    let name = Some(clean(&spec.name))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| host.clone());
    let slug = match existing {
        Some(slug) => {
            if !is_slug(slug) || (!may_make && read(&ctx.roots, slug).is_none()) {
                return Err(Failure {
                    refusal: Some(Refusal::UnknownItem),
                    trouble: format!("no web app `{slug}`"),
                });
            }
            slug.to_owned()
        }
        None => unique_slug(&ctx.roots, &name, &host),
    };
    let id = app_id(&url, &profile);
    if let Some(other) = list(&ctx.roots)
        .into_iter()
        .find(|w| w.slug != slug && app_id(&w.url, &w.profile) == id)
    {
        return Err(Failure {
            refusal: Some(Refusal::DuplicateWindow),
            trouble: format!("`{}` already opens {id}", other.slug),
        });
    }
    let category = spec
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| CATEGORIES.contains(c))
        .unwrap_or(FALLBACK_CATEGORY)
        .to_owned();
    let applications = ctx.roots.applications();
    let own = ctx.roots.ours().join("webapps").join(&slug);
    let outcome = Outcome {
        key: Some(AppKey::Web { slug: slug.clone() }),
        desktop_ids: vec![desktop_id(&slug)],
        amount: None,
        rehearsed: ctx.rehearse,
    };
    if ctx.rehearse {
        let entry = Entry {
            slug: &slug,
            name: &name,
            url: &url,
            profile: &profile,
            host: &host,
            icon: &own.join("icon-….png"),
            category: &category,
            choice: &spec.icon,
        };
        let _ = writeln!(
            std::io::stderr(),
            "rehearse: write {}\n{}",
            applications.join(desktop_id(&slug)).display(),
            desktop_text(&entry)
        );
        return Ok(outcome);
    }

    let png = icon_bytes(ctx, spec, report)?;
    report.step(Phase::Integrating, 0, 0, &name);
    let icon_name = format!("icon-{}.png", &crate::run::sha256(&png)[..8]);
    ronoctua_xdg::write_atomically(&own, &icon_name, &png).map_err(|e| wrote(&own, &e))?;
    let icon = own.join(&icon_name);
    let entry = Entry {
        slug: &slug,
        name: &name,
        url: &url,
        profile: &profile,
        host: &host,
        icon: &icon,
        category: &category,
        choice: &spec.icon,
    };
    // AT THE TOP of `applications`, atomically: the launcher rescans on the
    // directory's modification time, which a rename moves and an edit in
    // place does not.
    ronoctua_xdg::write_atomically(
        &applications,
        &desktop_id(&slug),
        desktop_text(&entry).as_bytes(),
    )
    .map_err(|e| wrote(&applications, &e))?;
    // Only now is an older icon unused (an edit with a new one).
    if let Ok(dir) = std::fs::read_dir(&own) {
        for e in dir.filter_map(Result::ok) {
            let n = e.file_name();
            let n = n.to_string_lossy();
            if n.starts_with("icon-") && n.ends_with(".png") && *n != *icon_name {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(outcome)
}

/// The icon's PNG: the one the screen already prepared, or the choice turned
/// into one here.
fn icon_bytes(ctx: &Context, spec: &WebSpec, report: &mut dyn Report) -> Result<Vec<u8>, Failure> {
    // The preview's PNG when it is still there — it lives in a cache, and a
    // sweep between the preview and the order is not a failure: the choice
    // is prepared again below.
    if let Some(prepared) = spec.prepared.as_deref().filter(|p| p.is_file()) {
        let size = std::fs::metadata(prepared)
            .map_err(|e| wrote(prepared, &e))?
            .len();
        if size > PREPARED_MOST {
            return Err(Failure {
                refusal: Some(Refusal::TooLarge),
                trouble: format!("{} is {size} bytes", prepared.display()),
            });
        }
        let bytes = std::fs::read(prepared).map_err(|e| wrote(prepared, &e))?;
        // Ours, but read back from a path: it has to BE a PNG.
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(Failure {
                refusal: Some(Refusal::IconUnusable),
                trouble: format!("{} is not a PNG", prepared.display()),
            });
        }
        return Ok(bytes);
    }
    report.step(Phase::Resolving, 0, 0, &spec.name);
    #[cfg(feature = "draw")]
    {
        let cache = ctx.roots.ours_cache().join("icons");
        let path = crate::icons::prepare(&spec.icon, ctx.tools.icons.as_deref(), &cache)
            .map_err(Failure::refused)?;
        std::fs::read(&path).map_err(|e| wrote(&path, &e))
    }
    #[cfg(not(feature = "draw"))]
    {
        let _ = ctx;
        Err(Failure {
            refusal: Some(Refusal::IconUnusable),
            trouble: "this build cannot draw an icon".into(),
        })
    }
}

/// Remove a web app: its `.desktop` and its directory. The site's login stays
/// where it always was — in Chrome's profile, which this never touches — so a
/// web app made again later opens signed in.
fn remove(ctx: &Context, slug: &str, report: &mut dyn Report) -> Done {
    if !is_slug(slug) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let desktop = ctx.roots.applications().join(desktop_id(slug));
    let own = ctx.roots.ours().join("webapps").join(slug);
    if !desktop.exists() && !own.exists() {
        return Err(Failure {
            refusal: Some(Refusal::UnknownItem),
            trouble: format!("no web app `{slug}`"),
        });
    }
    report.step(Phase::Removing, 0, 0, slug);
    let outcome = Outcome {
        key: Some(AppKey::Web {
            slug: slug.to_owned(),
        }),
        desktop_ids: Vec::new(),
        amount: None,
        rehearsed: ctx.rehearse,
    };
    if ctx.rehearse {
        let _ = writeln!(
            std::io::stderr(),
            "rehearse: remove {} and {}",
            desktop.display(),
            own.display()
        );
        return Ok(outcome);
    }
    match std::fs::remove_file(&desktop) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(wrote(&desktop, &e)),
    }
    match std::fs::remove_dir_all(&own) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(wrote(&own, &e)),
    }
    Ok(outcome)
}

/// A file-system failure, in its own words.
fn wrote(path: &Path, e: &std::io::Error) -> Failure {
    Failure {
        refusal: Some(Refusal::ToolFailed),
        trouble: format!("{}: {e}", path.display()),
    }
}

/// The argument vector that opens a web app: Chrome, the profile, `--app=`.
/// Never `--kiosk`, never `--enable-features` (the last one given wins, and it
/// would replace the wrapper's), never a `--user-data-dir`.
///
/// Reads the web app's `.desktop` — the person's own, else a declared one in
/// `$XDG_DATA_DIRS` ([`read_anywhere`]). The address is checked again on the
/// way out: the file is the person's and may have been edited by hand.
///
/// # Errors
///
/// `NoBrowser` without Chrome; the URL's refusal when the `.desktop` holds a
/// bad one.
pub fn open_argv(ctx: &Context, slug: &str) -> Result<Vec<String>, crate::Refusal> {
    let chrome = chrome_here(ctx).ok_or(Refusal::NoBrowser)?;
    let app = read_anywhere(&ctx.roots, slug).ok_or(Refusal::UnknownItem)?;
    argv_for(chrome, &app.url, &app.profile)
}

/// The argument vector that opens an address ONCE, in the same kind of window
/// a web app gets, without creating anything — the editor's "try it" before
/// saving.
///
/// # Errors
///
/// `NoBrowser` without Chrome, the URL's refusal, and `ProfileUnknown` for a
/// profile Chrome does not have.
pub fn once_argv(ctx: &Context, url: &str, profile: &str) -> Result<Vec<String>, Refusal> {
    let chrome = chrome_here(ctx).ok_or(Refusal::NoBrowser)?;
    let url = canonical_url(url)?;
    let profile = known_profile(&ctx.roots, profile)
        .map_err(|f| f.refusal.unwrap_or(Refusal::ProfileUnknown))?;
    argv_for(chrome, &url, &profile)
}

/// Chrome, when it is here: a part, which may not have arrived — asked on
/// every use, never once at start.
fn chrome_here(ctx: &Context) -> Option<&Path> {
    ctx.tools.chrome.as_deref().filter(|c| c.exists())
}

/// Chrome, the profile and `--app=`. Pure.
fn argv_for(chrome: &Path, url: &str, profile: &str) -> Result<Vec<String>, Refusal> {
    let url = canonical_url(url)?;
    if !is_profile_dir(profile) {
        return Err(Refusal::ProfileUnknown);
    }
    Ok(vec![
        chrome.display().to_string(),
        format!("--profile-directory={profile}"),
        format!("--app={url}"),
    ])
}

/// The profile, when Chrome has it — or when it is [`DEFAULT_PROFILE`], which
/// Chrome makes on its first start (a web app made before Chrome ever ran is
/// not refused for it).
fn known_profile(roots: &Roots, asked: &str) -> Result<String, Failure> {
    let asked = asked.trim();
    let known = is_profile_dir(asked)
        && (asked == DEFAULT_PROFILE
            || chrome_profiles(roots).iter().any(|p| p.directory == asked));
    if known {
        Ok(asked.to_owned())
    } else {
        Err(Failure {
            refusal: Some(Refusal::ProfileUnknown),
            trouble: format!("Chrome has no profile `{asked}`"),
        })
    }
}

/// A directory name Chrome could have made: one path component, printable.
fn is_profile_dir(dir: &str) -> bool {
    !dir.is_empty()
        && dir.len() <= 64
        && dir != "."
        && dir != ".."
        && !dir.contains(['/', '\\'])
        && !dir.chars().any(char::is_control)
}

/// A slug as this module makes them: `[a-z0-9-]`, not starting with `-`.
#[must_use]
pub fn is_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && !slug.starts_with('-')
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A slug for a new web app: the name's, else the host's, else `web`, with
/// `-2`, `-3`… when one is taken (by a `.desktop` or by a directory an
/// earlier one left).
fn unique_slug(roots: &Roots, name: &str, host: &str) -> String {
    let base = [crate::slug_of(name), crate::slug_of(host)]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "web".to_owned());
    // NOR A DECLARED ONE'S: the system's entry of that name (a new app of
    // the person's would shadow it, and the reconciler would take the new
    // one for the copy the declaration replaces), or a slug the declaration
    // names for another computer.
    let declared: Vec<String> = crate::declaration::read_declared(roots)
        .map(|d| {
            d.apps
                .iter()
                .filter_map(|e| match e {
                    ronoctua_person::declared::Entry::Web(w) => Some(w.slug.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let taken = |s: &str| {
        roots.applications().join(desktop_id(s)).exists()
            || roots.ours().join("webapps").join(s).exists()
            || read_system(roots, s).is_some()
            || declared.iter().any(|d| d == s)
    };
    if !taken(&base) {
        return base;
    }
    (2u32..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| !taken(s))
        .unwrap_or(base)
}

/// A name as it is written: no control character, no backslash (the
/// `.desktop` string escapes are not read back by our parser, and a `\n`
/// would be a second line), white space runs as one space, at most
/// [`NAME_MOST`] characters.
fn clean(name: &str) -> String {
    let flat: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .filter(|c| *c != '\\')
        .collect();
    flat.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(NAME_MOST)
        .collect()
}

/// A value as it goes into the `.desktop`: nothing that could end the line or
/// be read as an escape.
fn value(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '\\')
        .collect()
}

/// What a web app's `.desktop` is made of.
struct Entry<'a> {
    slug: &'a str,
    name: &'a str,
    url: &'a str,
    profile: &'a str,
    host: &'a str,
    icon: &'a Path,
    category: &'a str,
    choice: &'a IconChoice,
}

/// The `.desktop` text. Pure.
fn desktop_text(e: &Entry<'_>) -> String {
    let choice = storable(e.choice, e.icon);
    let json = serde_json::to_string(&choice).unwrap_or_default();
    let chrome = chrome();
    let chrome = chrome.display();
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.5\n\
         Name={name}\n\
         Comment={host}\n\
         Exec={SOFTWARE} web-open {slug}\n\
         TryExec={chrome}\n\
         Icon={icon}\n\
         Terminal=false\n\
         StartupNotify=true\n\
         StartupWMClass={class}\n\
         Categories={category};\n\
         X-Ronoctua-Web-Url={url}\n\
         X-Ronoctua-Web-Profile={profile}\n\
         X-Ronoctua-Web-Icon={json}\n",
        name = value(e.name),
        host = value(e.host),
        slug = e.slug,
        icon = value(&e.icon.display().to_string()),
        class = value(&app_id(e.url, e.profile)),
        category = value(e.category),
        url = value(e.url),
        profile = value(e.profile),
        json = value(&json),
    )
}

/// The icon choice as it can be kept in a `.desktop` value: its compact JSON
/// must hold no backslash (JSON escapes `"` and `\` with one), so every string
/// in it is made free of both first. A file path that cannot be is replaced by
/// the icon's own copy — editing then starts from the picture that is on
/// screen, which is what the person chose. Pure.
fn storable(choice: &IconChoice, copy: &Path) -> IconChoice {
    let plain = |s: &str| {
        s.chars()
            .filter(|c| !c.is_control() && !matches!(c, '"' | '\\'))
            .collect::<String>()
    };
    match choice {
        IconChoice::Site { url, page } => IconChoice::Site {
            url: escaped(url),
            page: page.as_deref().map(escaped),
        },
        IconChoice::Dashboard { name, variant } => IconChoice::Dashboard {
            name: plain(name),
            variant: if matches!(variant.as_str(), "light" | "dark") {
                variant.clone()
            } else {
                String::new()
            },
        },
        IconChoice::Collection { name } => IconChoice::Collection { name: plain(name) },
        IconChoice::File { path } => {
            let fine = path.to_str().is_some_and(|p| plain(p) == p);
            IconChoice::File {
                path: if fine {
                    path.clone()
                } else {
                    copy.to_path_buf()
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a declared web app — its entry in the person's profile the
    /// system built, a folder of `$XDG_DATA_DIRS` — does not open, or the
    /// person's own entry of the same name does not shadow it, as it does in
    /// the launcher.
    #[test]
    fn a_declared_web_app_opens_from_the_systems_entry() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-software-web-anywhere-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let roots = Roots::under(&root);
        let entry = |url: &str| {
            format!(
                "[Desktop Entry]\nType=Application\nName=Mail\nX-Ronoctua-Web-Url={url}\nX-Ronoctua-Web-Profile=Default\nX-Ronoctua-Declared=web:mail\n"
            )
        };
        let system = roots.data_dirs[0].join("applications");
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(
            system.join(desktop_id("mail")),
            entry("https://declared.example.org/"),
        )
        .unwrap();
        assert!(
            read(&roots, "mail").is_none(),
            "the person has no entry of their own"
        );
        assert_eq!(
            read_anywhere(&roots, "mail").map(|w| w.url),
            Some("https://declared.example.org/".to_owned())
        );
        std::fs::create_dir_all(roots.applications()).unwrap();
        std::fs::write(
            roots.applications().join(desktop_id("mail")),
            entry("https://mine.example.org/"),
        )
        .unwrap();
        assert_eq!(
            read_anywhere(&roots, "mail").map(|w| w.url),
            Some("https://mine.example.org/".to_owned())
        );
        assert!(read_anywhere(&roots, "../mail").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A path that IS there, standing in for Chrome: Chrome is a part, and
    /// one that has not arrived is no browser (`chrome_here`). Never run.
    const FAKE_CHROME: &str = "/bin/sh";

    /// A context over a scratch tree of this test process.
    fn scratch(name: &str) -> (Context, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-web-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = Context {
            tools: crate::Tools {
                chrome: Some(FAKE_CHROME.into()),
                ..crate::Tools::default()
            },
            roots: Roots::under(&dir),
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        (ctx, dir)
    }

    /// A PNG already prepared, the way the screen hands one over.
    fn prepared(dir: &Path, seed: u8) -> PathBuf {
        let path = dir.join(format!("prepared-{seed}.png"));
        // A 1×1 PNG, varied by one byte of its data so two differ.
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[seed; 16]);
        std::fs::write(&path, png).unwrap();
        path
    }

    fn spec(name: &str, url: &str, icon: PathBuf) -> WebSpec {
        WebSpec {
            name: name.into(),
            url: url.into(),
            profile: DEFAULT_PROFILE.into(),
            icon: IconChoice::Site {
                url: "https://web.whatsapp.com/favicon.ico".into(),
                page: None,
            },
            prepared: Some(icon),
            category: Some("Network".into()),
        }
    }

    fn done(ctx: &Context, order: &Order) -> Done {
        run(ctx, order, &mut Vec::new())
    }

    #[test]
    fn an_address_is_made_canonical_or_refused() {
        let ok = |t: &str| canonical_url(t).unwrap();
        assert_eq!(ok("web.whatsapp.com"), "https://web.whatsapp.com/");
        assert_eq!(
            ok("  https://Mail.Google.com/mail/u/0/  "),
            "https://mail.google.com/mail/u/0/"
        );
        assert_eq!(
            ok("http://intranet.example:8080/x"),
            "http://intranet.example:8080/x"
        );
        assert_eq!(ok("localhost:8080"), "https://localhost:8080/");
        assert_eq!(
            ok("example.org:8443/a?b=1#c"),
            "https://example.org:8443/a?b=1#c"
        );
        assert_eq!(ok("https://café.example/"), "https://xn--caf-dma.example/");
        // What the grammar leaves raw is encoded.
        assert_eq!(
            ok("https://x.example/a b$c'd\"e`f{g}|^?q=\\'"),
            "https://x.example/a%20b%24c%27d%22e%60f%7Bg%7D%7C%5E?q=%5C%27"
        );
        for bad in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "chrome://settings",
            "about:blank",
            "ftp://x.example/",
            "mailto:a@example.org",
        ] {
            assert_eq!(canonical_url(bad), Err(Refusal::UrlScheme), "{bad}");
        }
        assert_eq!(
            canonical_url("https://me:secret@example.org/"),
            Err(Refusal::UrlCredentials)
        );
        assert_eq!(
            canonical_url("https://me@example.org/"),
            Err(Refusal::UrlCredentials)
        );
        for bad in [
            "",
            "   ",
            "https://",
            "http://[::1",
            "https:// spaced .example",
        ] {
            assert_eq!(canonical_url(bad), Err(Refusal::UrlInvalid), "{bad:?}");
        }
        let long = format!("https://x.example/{}", "a".repeat(URL_MOST));
        assert_eq!(canonical_url(&long), Err(Refusal::UrlInvalid));
    }

    #[test]
    fn the_window_id_is_chromiums() {
        // The table from Chromium's arithmetic (web_app_helpers.cc,
        // web_app_shortcut_linux.cc, file_util_icu.cc).
        for (url, profile, id) in [
            (
                "https://web.whatsapp.com/",
                "Default",
                "chrome-web.whatsapp.com__-Default",
            ),
            (
                "https://mail.google.com/mail/u/0/",
                "Default",
                "chrome-mail.google.com__mail_u_0_-Default",
            ),
            // GURL::host() has no port; the query and fragment are dropped.
            (
                "http://localhost:8080/x?y=1#z",
                "Default",
                "chrome-localhost__x-Default",
            ),
            (
                "https://a.example/",
                "Profile 1",
                "chrome-a.example__-Profile_1",
            ),
            (
                "https://a.example/p:q|r*s",
                "Default",
                "chrome-a.example__p_q_r_s-Default",
            ),
            ("https://[::1]:9/x", "Default", "chrome-[__1]__x-Default"),
            // An illegal-at-the-end character at the end of the profile.
            ("https://a.example/", "Odd.", "chrome-a.example__-Odd_"),
            (
                "https://a.example/",
                "Zero\u{200B}Width",
                "chrome-a.example__-Zero_Width",
            ),
        ] {
            assert_eq!(app_id(url, profile), id, "{url} {profile}");
        }
    }

    #[test]
    fn chromes_profiles_come_last_used_first() {
        let (ctx, _dir) = scratch("profiles");
        // No file: the one Chrome makes first.
        assert_eq!(
            chrome_profiles(&ctx.roots),
            [ChromeProfile {
                directory: "Default".into(),
                name: "Default".into(),
                last_used: true,
            }]
        );
        let state = br#"{"profile":{"last_used":"Profile 2","info_cache":{
            "Default":{"name":"Person 1"},"Profile 10":{"name":"Ten"},
            "Profile 2":{"name":"Work"},"Profile 3":{"name":"  "},"../x":{"name":"bad"}}}}"#;
        let got = profiles_of(state);
        let dirs: Vec<(&str, &str, bool)> = got
            .iter()
            .map(|p| (p.directory.as_str(), p.name.as_str(), p.last_used))
            .collect();
        assert_eq!(
            dirs,
            [
                ("Profile 2", "Work", true),
                ("Default", "Person 1", false),
                ("Profile 3", "Profile 3", false),
                ("Profile 10", "Ten", false),
            ]
        );
        let chrome = ctx.roots.config.join("google-chrome");
        std::fs::create_dir_all(&chrome).unwrap();
        std::fs::write(chrome.join("Local State"), state).unwrap();
        assert_eq!(chrome_profiles(&ctx.roots), got);
        assert!(profiles_of(b"not json").is_empty());
    }

    #[test]
    fn a_web_app_is_a_desktop_file_that_reads_back_as_itself() {
        let (ctx, dir) = scratch("create");
        let icon = prepared(&dir, 1);
        let order = Order::WebCreate {
            spec: spec("WhatsApp", "web.whatsapp.com", icon),
        };
        let outcome = done(&ctx, &order).unwrap();
        assert_eq!(
            outcome.key,
            Some(AppKey::Web {
                slug: "whatsapp".into()
            })
        );
        assert_eq!(outcome.desktop_ids, ["ronoctua-web-whatsapp.desktop"]);

        let path = ctx
            .roots
            .applications()
            .join("ronoctua-web-whatsapp.desktop");
        let text = std::fs::read_to_string(&path).unwrap();
        // THE ROUND TRIP: our writer, the launcher's reader and its Exec grammar.
        let fields = ronoctua_apps::desktop::fields(&text);
        let exec = ronoctua_apps::desktop::raw_bytes(&fields, "Exec").unwrap();
        let assembled = ronoctua_apps::exec::assemble(
            exec,
            ronoctua_apps::exec::Context {
                icon: "",
                name: "WhatsApp",
                file: &path.to_string_lossy(),
                open: &[],
            },
        );
        assert_eq!(
            assembled,
            Some((
                SOFTWARE.to_owned(),
                vec!["web-open".to_owned(), "whatsapp".to_owned()]
            ))
        );
        let key = |k: &str| {
            ronoctua_apps::desktop::raw_bytes(&fields, k)
                .unwrap()
                .to_owned()
        };
        assert_eq!(key("Type"), "Application");
        assert_eq!(key("Name"), "WhatsApp");
        assert_eq!(key("TryExec"), chrome().to_str().unwrap());
        assert_eq!(key("StartupWMClass"), "chrome-web.whatsapp.com__-Default");
        assert_eq!(key("X-Ronoctua-Web-Url"), "https://web.whatsapp.com/");
        assert_eq!(key("Categories"), "Network;");
        assert!(
            !text.contains("/nix/store"),
            "a store path in a .desktop dies with its generation"
        );
        let icon_file = PathBuf::from(key("Icon"));
        assert!(icon_file.starts_with(ctx.roots.ours().join("webapps/whatsapp")));
        assert!(
            icon_file
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("icon-")
        );

        let app = read(&ctx.roots, "whatsapp").unwrap();
        assert_eq!(
            app.icon,
            Some(IconChoice::Site {
                url: "https://web.whatsapp.com/favicon.ico".into(),
                page: None,
            })
        );
        assert_eq!(app.icon_file.as_deref(), Some(icon_file.as_path()));
        assert_eq!(app.category.as_deref(), Some("Network"));
        let rows = installed(&ctx.roots);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "WhatsApp");
        assert_eq!(rows[0].icon, Some(Picture::File { path: icon_file }));
        assert_eq!(rows[0].trouble, None);

        // AND IT OPENS AS A WINDOW OF THE PERSON'S CHROME, NOTHING ELSE.
        let argv = open_argv(&ctx, "whatsapp").unwrap();
        assert_eq!(
            argv,
            [
                FAKE_CHROME,
                "--profile-directory=Default",
                "--app=https://web.whatsapp.com/"
            ]
        );
        for never in [
            "--kiosk",
            "--start-fullscreen",
            "--user-data-dir",
            "--enable-features",
        ] {
            assert!(
                argv.iter().all(|a| !a.starts_with(never)),
                "{never} in {argv:?}"
            );
        }
    }

    #[test]
    fn one_window_id_is_one_web_app_and_one_name_many_slugs() {
        let (ctx, dir) = scratch("duplicate");
        done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Chat", "https://chat.example/", prepared(&dir, 1)),
            },
        )
        .unwrap();
        // The same host, path and profile: Chrome would give both one window id.
        let again = done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Other", "https://chat.example/?x=1", prepared(&dir, 2)),
            },
        );
        assert_eq!(again.unwrap_err().refusal, Some(Refusal::DuplicateWindow));
        // The same name, another address: another slug.
        let second = done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Chat", "https://chat.example/b", prepared(&dir, 3)),
            },
        );
        assert_eq!(
            second.unwrap().key,
            Some(AppKey::Web {
                slug: "chat-2".into()
            })
        );
        // A name with nothing file-safe in it takes the host's.
        let third = done(
            &ctx,
            &Order::WebCreate {
                spec: spec("日本", "https://news.example/", prepared(&dir, 4)),
            },
        );
        assert_eq!(
            third.unwrap().key,
            Some(AppKey::Web {
                slug: "news-example".into()
            })
        );
    }

    #[test]
    fn an_edit_keeps_the_slug_and_drops_the_old_icon() {
        let (ctx, dir) = scratch("edit");
        done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Mail", "https://mail.example/", prepared(&dir, 1)),
            },
        )
        .unwrap();
        let before = read(&ctx.roots, "mail").unwrap().icon_file.unwrap();
        let mut changed = spec(
            "Post\nOffice\\",
            "https://mail.example/inbox",
            prepared(&dir, 2),
        );
        changed.category = Some("NotACategory".into());
        let edited = done(
            &ctx,
            &Order::WebEdit {
                slug: "mail".into(),
                spec: changed,
            },
        )
        .unwrap();
        assert_eq!(
            edited.key,
            Some(AppKey::Web {
                slug: "mail".into()
            })
        );
        let app = read(&ctx.roots, "mail").unwrap();
        assert_eq!(
            app.name, "Post Office",
            "a newline and a backslash never reach the file"
        );
        assert_eq!(app.url, "https://mail.example/inbox");
        assert_eq!(app.category.as_deref(), Some(FALLBACK_CATEGORY));
        let after = app.icon_file.unwrap();
        assert_ne!(before, after);
        assert!(
            !before.exists() && after.exists(),
            "the old icon is gone, the new one is there"
        );
        assert_eq!(list(&ctx.roots).len(), 1);
        // An edit of what does not exist.
        let ghost = done(
            &ctx,
            &Order::WebEdit {
                slug: "ghost".into(),
                spec: spec("G", "g.example", prepared(&dir, 3)),
            },
        );
        assert_eq!(ghost.unwrap_err().refusal, Some(Refusal::UnknownItem));
    }

    #[test]
    fn removing_takes_the_file_and_the_directory_and_nothing_else() {
        let (ctx, dir) = scratch("remove");
        done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Mail", "https://mail.example/", prepared(&dir, 1)),
            },
        )
        .unwrap();
        // A file of somebody else's beside ours stays.
        let theirs = ctx.roots.applications().join("theirs.desktop");
        std::fs::write(&theirs, "[Desktop Entry]\nType=Application\n").unwrap();
        let order = Order::Remove {
            key: AppKey::Web {
                slug: "mail".into(),
            },
            keep_data: true,
        };
        done(&ctx, &order).unwrap();
        assert!(
            !ctx.roots
                .applications()
                .join("ronoctua-web-mail.desktop")
                .exists()
        );
        assert!(!ctx.roots.ours().join("webapps/mail").exists());
        assert!(theirs.exists());
        assert_eq!(
            done(&ctx, &order).unwrap_err().refusal,
            Some(Refusal::UnknownItem)
        );
        let escape = Order::Remove {
            key: AppKey::Web {
                slug: "../x".into(),
            },
            keep_data: true,
        };
        assert_eq!(
            done(&ctx, &escape).unwrap_err().refusal,
            Some(Refusal::UnknownItem)
        );
    }

    #[test]
    fn what_cannot_be_a_web_app_is_refused_before_anything_is_written() {
        let (mut ctx, dir) = scratch("refused");
        let refused = |ctx: &Context, s: WebSpec| {
            done(ctx, &Order::WebCreate { spec: s })
                .unwrap_err()
                .refusal
        };
        assert_eq!(
            refused(&ctx, spec("X", "javascript:alert(1)", prepared(&dir, 1))),
            Some(Refusal::UrlScheme)
        );
        let mut other = spec("X", "x.example", prepared(&dir, 1));
        other.profile = "Profile 9".into();
        assert_eq!(refused(&ctx, other), Some(Refusal::ProfileUnknown));
        let mut not_png = spec("X", "x.example", dir.join("page.png"));
        std::fs::write(dir.join("page.png"), b"<html>").unwrap();
        not_png.prepared = Some(dir.join("page.png"));
        assert_eq!(refused(&ctx, not_png), Some(Refusal::IconUnusable));
        assert!(!ctx.roots.applications().exists() || list(&ctx.roots).is_empty());
        ctx.tools.chrome = None;
        assert_eq!(
            refused(&ctx, spec("X", "x.example", prepared(&dir, 1))),
            Some(Refusal::NoBrowser)
        );
        assert_eq!(open_argv(&ctx, "x"), Err(Refusal::NoBrowser));
        assert!(!ctx.roots.applications().exists(), "nothing was written");
    }

    #[test]
    fn a_rehearsal_checks_everything_and_writes_nothing() {
        let (mut ctx, dir) = scratch("rehearse");
        ctx.rehearse = true;
        let outcome = done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Mail", "mail.example", prepared(&dir, 1)),
            },
        )
        .unwrap();
        assert!(outcome.rehearsed);
        assert!(!ctx.roots.applications().exists());
        assert!(!ctx.roots.ours().exists());
        let bad = done(
            &ctx,
            &Order::WebCreate {
                spec: spec("Mail", "file:///x", prepared(&dir, 1)),
            },
        );
        assert_eq!(
            bad.unwrap_err().refusal,
            Some(Refusal::UrlScheme),
            "the checks still run"
        );
    }

    #[test]
    fn opening_once_needs_nothing_written() {
        let (ctx, _dir) = scratch("once");
        assert_eq!(
            once_argv(&ctx, "news.example/today", "Default").unwrap(),
            [
                FAKE_CHROME,
                "--profile-directory=Default",
                "--app=https://news.example/today"
            ]
        );
        assert_eq!(
            once_argv(&ctx, "news.example", "Nope"),
            Err(Refusal::ProfileUnknown)
        );
        assert_eq!(
            once_argv(&ctx, "data:,x", "Default"),
            Err(Refusal::UrlScheme)
        );
    }

    #[test]
    fn a_hand_edited_file_is_shown_with_what_broke() {
        let (ctx, _dir) = scratch("hand");
        let apps = ctx.roots.applications();
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join("ronoctua-web-broken.desktop"),
            "[Desktop Entry]\nType=Application\nName=Broken\nX-Ronoctua-Web-Url=file:///etc/passwd\n",
        )
        .unwrap();
        // Not ours at all: no web key.
        std::fs::write(
            apps.join("ronoctua-web-plain.desktop"),
            "[Desktop Entry]\nName=Plain\n",
        )
        .unwrap();
        let rows = installed(&ctx.roots);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].trouble, Some(Refusal::UrlScheme));
        assert_eq!(open_argv(&ctx, "broken"), Err(Refusal::UrlScheme));
        assert_eq!(read(&ctx.roots, "broken").unwrap().profile, DEFAULT_PROFILE);
    }

    #[test]
    fn an_icon_choice_is_kept_without_a_backslash() {
        let copy = Path::new("/home/x/.local/share/ronoctua/software/webapps/a/icon-00000000.png");
        let odd = IconChoice::File {
            path: PathBuf::from("/home/x/my \"best\" icon\\.png"),
        };
        assert_eq!(
            storable(&odd, copy),
            IconChoice::File {
                path: copy.to_path_buf()
            }
        );
        let site = IconChoice::Site {
            url: "https://a.example/i.png?q=a\\b\"c".into(),
            page: None,
        };
        let kept = storable(&site, copy);
        let json = serde_json::to_string(&kept).unwrap();
        assert!(!json.contains('\\'), "{json}");
        assert_eq!(serde_json::from_str::<IconChoice>(&json).unwrap(), kept);
        let dash = IconChoice::Dashboard {
            name: "github-light".into(),
            variant: "sideways".into(),
        };
        assert_eq!(
            storable(&dash, copy),
            IconChoice::Dashboard {
                name: "github-light".into(),
                variant: String::new()
            }
        );
    }

    #[test]
    fn a_slug_names_only_our_files() {
        assert!(is_slug("whatsapp") && is_slug("chat-2") && is_slug("a1"));
        for bad in ["", "-x", "A", "a b", "../x", "a/b", "a.b"] {
            assert!(!is_slug(bad), "{bad}");
        }
        assert!(
            has_scheme("https://x")
                && has_scheme("javascript:alert(1)")
                && has_scheme("about:blank")
        );
        assert!(
            !has_scheme("localhost:8080")
                && !has_scheme("x.example:8443/y")
                && !has_scheme("x.example/a:b")
        );
    }

    /// THE TABLE THE NIX SIDE READS TOO (`web-app-ids-agree`): a launcher
    /// entry a person DECLARES is written by Nix (`nix/declared-apps/web.nix`),
    /// which repeats this arithmetic without a URL parser. Every `agree` row
    /// is this function's answer; every `refused` row is an address the Nix
    /// side refuses because a browser rewrites it (or rejects it) first —
    /// which this proves, so the refusals are exactly where the two would
    /// part.
    #[test]
    fn the_declared_web_app_ids_are_this_arithmetic() {
        let table: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/web-app-ids.json")).unwrap();
        let text = |row: &serde_json::Value, key: &str| row[key].as_str().unwrap().to_owned();
        let agree = table["agree"].as_array().unwrap();
        let refused = table["refused"].as_array().unwrap();
        assert!(agree.len() >= 10 && refused.len() >= 10);
        for row in agree {
            let (url, profile) = (text(row, "url"), text(row, "profile"));
            assert_eq!(app_id(&url, &profile), text(row, "id"), "{url} {profile}");
        }
        for row in refused {
            let url = text(row, "url");
            let rewritten = Url::parse(&url).map_or(true, |u| u.as_str() != url);
            assert!(
                rewritten,
                "{url} is kept as written: the Nix side need not refuse it"
            );
        }
    }

    #[cfg(feature = "draw")]
    #[test]
    fn an_icon_not_prepared_is_prepared_here() {
        let (ctx, dir) = scratch("draw");
        let source = dir.join("mine.png");
        let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 128, 0, 255]));
        img.save(&source).unwrap();
        let mut s = spec("Mine", "mine.example", source.clone());
        s.prepared = None;
        s.icon = IconChoice::File { path: source };
        done(&ctx, &Order::WebCreate { spec: s }).unwrap();
        let icon = read(&ctx.roots, "mine").unwrap().icon_file.unwrap();
        let back = image::open(&icon).unwrap();
        assert_eq!((back.width(), back.height()), (128, 128));
    }
}
