//! STARTING A PROGRAM: the launcher's catalogue and its icons, what opens a
//! file, the argument vector a desktop entry runs as, and an entry started
//! from a window.
//!
//! # Why a module of its own
//!
//! The launcher's VIEW (`ronoctua_shell_footer::apps`) is the footer's, and so was all of
//! this — yet the file manager, the git history, the search, the software
//! centre, the player and every window open programs too, and each named the
//! footer's launcher to do it. What starts a program is a door's verb;
//! which programs the footer lists, and how, is the launcher's.

use ronoctua_tokens as tokens;
// The SAME `calloop` as the event loop's, and not a direct dependency: two
// versions of the crate would give two channel types with the same name, and the error
// would show up as a type mismatch without saying they are two crates.
use smithay_client_toolkit::reexports::calloop;
use std::sync::Arc;

/// The catalogue, shared between the event loop and the icon thread.
///
/// An `Arc` and not a copy: the thread needs the icon names of the visible rows,
/// and cloning four hundred entries per request would mean paying for the scan again
/// on every scroll.
pub type Catalogue = Arc<Vec<ronoctua_apps::DesktopEntry>>;

/// An application's glyph — the steps that do NOT touch the disk.
///
/// Three attempts, and never "nothing":
///
/// 1. the `app id -> glyph` table, by the desktop id **and** by the `Icon=`;
/// 2. the first `Categories=` the category table knows;
/// 3. the generic glyph.
///
/// # Why the `Icon=` also consults the table
///
/// Because both forms appear, and neither of them is the "right" one: Nautilus
/// has id `org.gnome.Nautilus` and `Icon=org.gnome.Nautilus`; the Firefox of
/// many distributions has id `firefox` and `Icon=firefox`; and there are those with a vendor
/// id and a short `Icon=`. Consulting both costs two lookups in a
/// thirty-entry table and covers all three cases.
///
/// # Why the CATEGORY comes before the generic
///
/// Because the generic is indistinguishable from "I did not resolve it": a whole list of the
/// same little square looks deliberate, and it is precisely the symptom of the table
/// not having reached the Rust. The category gets it wrong rarely and says something — a
/// terminal that looks like a terminal, a game that looks like a game.
#[must_use]
pub fn glyph_of(e: &ronoctua_apps::DesktopEntry) -> char {
    if let Some(g) = in_table(tokens::tray::TABLE, &e.id) {
        return g;
    }
    if !e.icon.is_empty()
        && let Some(g) = in_table(tokens::tray::TABLE, &e.icon)
    {
        return g;
    }
    // IN THE TABLE'S ORDER, and not in the order of the file's categories: a
    // browser declares `Network;WebBrowser`, and both match. What decides is
    // the table, where `WebBrowser` comes first — see `category-glyphs.nix`.
    for (category, glyph) in tokens::category::TABLE {
        if e.categories.iter().any(|c| c == category) {
            return *glyph;
        }
    }
    tokens::icon::GENERIC
}

/// A table lookup, exact and then by the last dotted component.
fn in_table(table: &[(&str, char)], key: &str) -> Option<char> {
    if let Some((_, g)) = table.iter().find(|(k, _)| *k == key) {
        return Some(*g);
    }
    let short = key.rsplit('.').next()?;
    if short == key {
        return None;
    }
    table.iter().find(|(k, _)| *k == short).map(|(_, g)| *g)
}

/// Scans the applications ON A THREAD and returns the catalogue through the channel.
///
/// # Why at startup, and not on opening
///
/// Measured on this machine: **128 entries in 68 ms**. A frame's budget is
/// 16 ms, and the scan is hundreds of `open()` calls — doing it on pressing
/// `LEADER a` would give a four-frame freeze every time, and the gesture of
/// opening the launcher is precisely the one that has to be instant.
///
/// The thread is named because an anonymous thread parked in `read` is what
/// nobody can identify in a `perf`.
pub fn scan(language: &str, sender: calloop::channel::Sender<Catalogue>) {
    // THE LANGUAGE GOES TO THE THREAD, and is not read in there: `Texts` lives in the
    // `Footer`, and the desktop entry's language is the same as the interface's — two
    // reads of the environment could diverge if someone changed the variable.
    let language = language.replace('-', "_");
    let born = std::thread::Builder::new()
        .name("ronoctua-apps".to_owned())
        .spawn(move || {
            let start = std::time::Instant::now();
            let list = ronoctua_apps::scan(&language);
            tracing::info!(
                apps = list.len(),
                ms = start.elapsed().as_millis(),
                "application catalogue scanned"
            );
            // A closed channel = the footer left. Nothing to deliver.
            let _ = sender.send(Arc::new(list));
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not create the scan thread");
    }
}

/// How a launcher entry is started — the program, its words, and the
/// directory it starts in — inside this binary's terminal when the entry
/// says it needs one. `None` when this binary cannot name itself.
///
/// ONE ANSWER FOR EVERY DOOR: the footer's launcher, a notification's "open
/// it" and the windows that open what an install made all start an entry this
/// way, so a program that needs a terminal gets one whichever door opened it.
#[must_use]
pub fn argv_of(e: &ronoctua_apps::DesktopEntry) -> Option<(String, Vec<String>, Option<String>)> {
    let (program, args) = &e.command;
    if !e.terminal {
        return Some((program.clone(), args.clone(), e.working_directory.clone()));
    }
    let me = std::env::current_exe().ok()?;
    let mut inside = vec!["terminal".to_owned(), program.clone()];
    inside.extend_from_slice(args);
    Some((me.to_string_lossy().into_owned(), inside, None))
}

/// How a window's launch by desktop id ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// The entry was found and started — or failed to start, which the
    /// verdict says.
    Started(crate::foreign::Launch),
    /// No entry of the launcher has that id: installed a moment ago and not
    /// in the catalogue yet, or gone.
    NotInLauncher,
}

/// Starts the launcher's entry `desktop_id` FROM A WINDOW, off the loop: the
/// catalogue is read on a thread (the sweep the footer keeps at hand costs
/// tens of milliseconds, which a window's loop does not spend), the entry
/// found by the rule the software view names them by
/// (`launch::same_desktop`), and started as the footer starts it
/// ([`argv_of`], `foreign::launch_now`). `done` hears how, on that thread.
///
/// The window says what happened on its own line — it used to hand the id
/// to the footer, whose refusal then appeared on the FOOTER's line, and the
/// window the person was using said nothing.
pub fn launch_entry(desktop_id: &str, language: &str, done: impl FnOnce(Opened) + Send + 'static) {
    let desktop_id = desktop_id.to_owned();
    let language = language.replace('-', "_");
    let born = std::thread::Builder::new()
        .name("launch-entry".to_owned())
        .spawn(move || {
            let entry = ronoctua_apps::scan(&language)
                .into_iter()
                .find(|e| crate::launch::same_desktop(&e.id, &desktop_id));
            let Some(entry) = entry else {
                done(Opened::NotInLauncher);
                return;
            };
            let opened = match argv_of(&entry) {
                Some((program, args, cwd)) => {
                    tracing::info!(app = %entry.id, program, "opening the application");
                    crate::foreign::launch_now(&program, &args, cwd.as_deref())
                }
                None => crate::foreign::Launch::Failed {
                    program: entry.id.clone(),
                    error: "this binary cannot name itself for the terminal".to_owned(),
                },
            };
            done(Opened::Started(opened));
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not create the launch thread");
    }
}

/// The icon superpackage's root (§6.6), read at RUN TIME.
///
/// # Why not `option_env!`, unlike everything else here
///
/// Because it is the one path the SYSTEM decides. `ronoctua.icons.superpack`
/// exists to leave the 0,61 GiB out of a reduced installation, and a store path
/// compiled into this binary is a reference the closure carries whatever the
/// option says — the switch turned nothing off, and `nix why-depends` is what
/// said so. The other paths here (`setsid`, `choom`, the wasm buffers) are
/// programs the shell cannot run without; this is data it can do without.
///
/// So the value comes from the environment, set by `nix/modules/icons.nix` for
/// the installed system and by `nix/dev.nix` for the nested loop. Unset, the
/// chain starts at `hicolor`: the programs that bring their own icon keep it,
/// and the rest keep their glyphs — which is degradation, and it is what a
/// machine that asked for no superpack wants.
fn superpack() -> Option<String> {
    std::env::var("RONOCTUA_ICONS")
        .ok()
        .filter(|p| !p.is_empty())
}

/// An icon's pixmap side, in pixels.
///
/// The font's body size, and **not** the line height: the line has a `1.45 at`
/// box, and its air is what separates one row from the next — an icon of that
/// height would touch the one above and the one below. A square the size of the body
/// has the height of what the eye reads as a large capital, and fits in the two
/// cells the layout reserves (`18 px` of drawing in `21.6 px` of zone).
#[must_use]
pub fn pixmap_side() -> f32 {
    tokens::text::size()
}

/// The cache key of one app's pixmap: the name AND the side it was drawn at.
///
/// # Why the side is in the key
///
/// Because an `Rgba` is rasterised at a NUMBER OF PIXELS, and `pixmap_side`
/// asks `text::size()` — so the same `Icon=` is a different picture at 100%
/// and at 200%. Keyed by the name alone, the second one can never be uploaded:
/// the renderer's `Images::missing` is `!uploads.contains_key`, and nothing in
/// it removes a key — `clear()` empties the frame's batch and leaves the
/// textures, and `dump()` only acts above its ceiling. Emptying the host's own
/// map does NOT reach that, so the old texture went on being drawn, stretched
/// into the new box, for the life of the footer.
///
/// It is the rule the theme's key already follows for anything a child painted
/// with a token's VALUE, on a second axis — and the one `buffer::svg_key`
/// follows for the same reason on the other side of the process.
#[must_use]
pub fn pixmap_key(icon: &str, side: u32) -> String {
    // `\u{1}` because an `Icon=` is a theme name or an absolute PATH, and a
    // path can hold any byte a filename can — a separator that can appear in
    // one would let two different pairs spell the same key.
    format!("{icon}\u{1}{side}")
}

/// A pixmap request for the thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The raw `Icon=` — a theme name **or** an absolute path.
    pub icon: String,
    /// The square's side, in pixels.
    pub side: u32,
}

/// What the thread returns.
#[derive(Debug, Clone)]
pub struct Painting {
    /// The `Icon=` requested.
    pub icon: String,
    /// The side it was actually drawn at — half of the cache key.
    ///
    /// It travels BACK rather than being read from `pixmap_side()` on arrival,
    /// because the size can move while a request is in flight: read on arrival,
    /// a picture rasterised at 18 would be filed as if it were 36.
    pub side: u32,
    /// The pixels, or `None` when there is no icon for this name.
    ///
    /// `None` is an ANSWER, and not a silent failure: it is what stops
    /// the host from asking again for what already does not exist, on every frame.
    pub rgba: Option<ronoctua_render::image::Rgba>,
}

/// The bridge between the pixel the chain returns and what the renderer consumes.
///
/// The two types are the same field for field, and they are two on purpose:
/// `ronoctua-notify` decodes without depending on the renderer, which drags in the whole
/// of wgpu. The same boundary — and the same crossing — as `ronoctua_clipboard`'s.
fn for_renderer(r: ronoctua_notify::icon::Rgba) -> ronoctua_render::image::Rgba {
    ronoctua_render::image::Rgba {
        width: r.width,
        height: r.height,
        pixels: r.pixels,
    }
}

/// Starts the thread that resolves and rasterises icons.
///
/// # Why one long-lived thread, and not one per request
///
/// Because the expensive part is the SAME for all of them: `icon::look_up` walks the
/// superpackage's size directories, which has 335,031 files, and then every
/// `hicolor` of the session for a name it does not have. One thread
/// per request would pay the creation cost per icon and would still leave twelve rasterisations
/// competing for the processor with the painting.
///
/// The queue is `mpsc` because only it has a blocking `recv`: the thread sleeps
/// while nobody asks.
#[must_use]
pub fn painter(
    sender: calloop::channel::Sender<Painting>,
) -> Option<std::sync::mpsc::Sender<Request>> {
    let (queue, requests) = std::sync::mpsc::channel::<Request>();
    // THE WHOLE CHAIN, and read ONCE: the superpackage, then every `hicolor`
    // and `pixmaps` the session's data directories name — which is where a
    // Flatpak export, `~/.nix-profile` and `~/.local/share/icons` put a
    // program's own icon. The environment is the thread's birth's: it does not
    // change under a running footer, and reading it per icon would be a lookup
    // of variables per row.
    let roots = ronoctua_notify::icon::Roots::of_session(superpack().map(std::path::PathBuf::from));
    let born = std::thread::Builder::new()
        .name("ronoctua-icons".to_owned())
        .spawn(move || {
            while let Ok(p) = requests.recv() {
                let rgba =
                    ronoctua_notify::icon::rasterize_in(&roots, &p.icon, p.side).map(for_renderer);
                if sender
                    .send(Painting {
                        icon: p.icon,
                        side: p.side,
                        rgba,
                    })
                    .is_err()
                {
                    // A closed channel = the footer left.
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(queue),
        Err(e) => {
            tracing::error!(error = %e, "could not create the icons thread");
            None
        }
    }
}

/// What the launcher remembers about the `applications` directories, to tell
/// whether the catalogue has aged. Compared with `!=`, and nothing else.
pub type Stamp = Vec<DirStamp>;

/// One `applications` directory, as far as "has it changed?" goes.
///
/// # Why four things, when an `mtime` used to be enough
///
/// Comparing the CONTENT would require rescanning, which is what the stamp
/// exists to avoid; a directory's `mtime` changes when a `.desktop` enters or
/// leaves, and for a directory somebody writes into, that is still the case
/// that matters. But the directories the session lists are mostly not written
/// into. They are PROFILES:
///
/// - `~/.nix-profile/share/applications` goes through a link into the store,
///   where every `mtime` is the same second of 1970. `nix profile add` builds a
///   NEW generation and moves the link, and the directory it resolves to is a
///   different one with the same `mtime` — so the stamp never moved, and an
///   installed program stayed out of the list until the footer restarted. What
///   moved is WHERE the path resolves ([`DirStamp::real`]), which is the
///   generation itself; the same is true of the system's and the per-user
///   profile across a rebuild.
/// - A `.desktop` added in a SUBDIRECTORY changes that subdirectory's `mtime`
///   and nothing above it, and the scan reads two levels down
///   ([`ronoctua_apps::DEPTH`]); [`DirStamp::below`] is those directories.
/// - The directory's own `mtime` is kept both ways — of the last component
///   itself ([`DirStamp::link`]) and of what it resolves to
///   ([`DirStamp::mtime`]) — because either can be the one that is a link.
///
/// A directory that does not exist is all `None`s, and that counts: creating
/// `~/.local/share/applications` is exactly the gesture of whoever has just
/// installed their first override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirStamp {
    /// Where the path resolves to, through every link on the way — the
    /// generation, for a profile. `None` when it does not exist.
    pub real: Option<std::path::PathBuf>,
    /// The `mtime` of the path's last component, not followed.
    pub link: Option<std::time::SystemTime>,
    /// The `mtime` of the directory it resolves to.
    pub mtime: Option<std::time::SystemTime>,
    /// Every directory below it that the scan reads, in path order: each with
    /// where it resolves to and its `mtime`.
    pub below: Vec<(
        std::path::PathBuf,
        Option<std::path::PathBuf>,
        Option<std::time::SystemTime>,
    )>,
}

/// The session's `applications` directories, stamped.
///
/// # What it costs, and where it runs
///
/// It runs on the event loop, when `LEADER a` opens (`rescan_if_changed`), so it
/// must stay a handful of system calls. Per directory: one `canonicalize`, two
/// `stat`s, and a `readdir` that `stat`s only what is not a `.desktop` — a
/// profile's directory is a hundred links to `.desktop` files and two or three
/// subdirectories, and the file names say which is which without asking.
#[must_use]
pub fn stamp() -> Stamp {
    stamp_of(&ronoctua_apps::directories())
}

/// [`stamp`], over the directories given — the tests' door.
#[must_use]
pub fn stamp_of(dirs: &[std::path::PathBuf]) -> Stamp {
    dirs.iter()
        .map(|d| {
            let mut below = Vec::new();
            subdirectories(d, 1, &mut below);
            below.sort();
            DirStamp {
                real: std::fs::canonicalize(d).ok(),
                link: std::fs::symlink_metadata(d).and_then(|m| m.modified()).ok(),
                mtime: std::fs::metadata(d).and_then(|m| m.modified()).ok(),
                below,
            }
        })
        .collect()
}

/// The directories under `dir` that the scan reads, down to its depth.
///
/// `level` is the depth of what is LISTED here: the scan reads the
/// `applications` directory (level 0) and the directories down to
/// `DEPTH - 1`, following links as it goes — `is_dir` follows them, and so
/// does `metadata` here.
fn subdirectories(
    dir: &std::path::Path,
    level: usize,
    out: &mut Vec<(
        std::path::PathBuf,
        Option<std::path::PathBuf>,
        Option<std::time::SystemTime>,
    )>,
) {
    if level >= ronoctua_apps::DEPTH {
        return;
    }
    let Ok(items) = std::fs::read_dir(dir) else {
        return;
    };
    for item in items.filter_map(Result::ok) {
        let path = item.path();
        // A `.desktop` is a file, and asking would be a `stat` per program.
        if path.extension().is_some_and(|e| e == "desktop") {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        out.push((
            path.clone(),
            std::fs::canonicalize(&path).ok(),
            meta.modified().ok(),
        ));
        subdirectories(&path, level + 1, out);
    }
}

/// Which application declares that it opens this type.
///
/// # Why the catalogue answers, and not `xdg-open`
///
/// Because the catalogue is already in memory: the sweep at startup read every
/// `.desktop` there is, and `MimeType=` came along with the rest. Spawning
/// `xdg-open` would walk the same directories again, in another process, to
/// reach the same answer — and it would do it inside the frame that draws.
///
/// # The order, and why the FIRST match is the right one
///
/// The catalogue is deduplicated by id with the first occurrence winning, and
/// `directories()` puts `$XDG_DATA_HOME` first — so an entry the person wrote
/// themselves already beats the system's. That is the freedesktop override
/// mechanism for WHO an entry is.
///
/// # A default comes before every declaration
///
/// What opens a type by CHOICE is the `[Default Applications]` of the
/// `mimeapps.list` files (`ronoctua_apps::defaults`, read now, in the
/// specification's order): the person's own, the distribution's `/etc/xdg`
/// (`nix/modules/workshop.nix` sends models to OrcaSlicer there), a
/// package's. The first entry a list names that IS installed wins (listed
/// or not, below); one that is not installed is skipped. Only when no list names an
/// installed entry for the type do the declarations below answer — and that
/// order is what keeps a `.step`, which the database makes a `text/plain`,
/// out of the first editor that declares `text/plain`.
///
/// # A default may name what the launcher does not list
///
/// The catalogue is what the launcher LISTS, and `NoDisplay=true` keeps an
/// entry out of it; but a default naming such an entry is a choice like any
/// other — the player's own list names its entry, which is `NoDisplay`, for
/// every audio and video type. So the ids a list names that the catalogue
/// lacks are looked up in the directories themselves
/// ([`ronoctua_apps::named`]), only for this type and only when a file is
/// opened.
#[must_use]
pub fn opens(catalogue: &Catalogue, of_type: &str) -> Option<ronoctua_apps::DesktopEntry> {
    let defaults = ronoctua_apps::defaults();
    let wanted = ronoctua_files::mime::canonical(of_type);
    let unlisted: Vec<ronoctua_apps::DesktopEntry> = defaults
        .for_type(|m| ronoctua_files::mime::canonical(m) == wanted)
        .filter(|id| !catalogue.iter().any(|e| e.id == *id))
        // The name is not shown: the entry only says what runs.
        .filter_map(|id| ronoctua_apps::named(id, "C"))
        .collect();
    opens_by(catalogue, &unlisted, &defaults, of_type).cloned()
}

/// [`opens`], with the defaults and the entries the catalogue does not list
/// given — what a test hands in, so it never reads the machine's own files.
///
/// # A program may decline the name order
///
/// Without a default, the first entry that declares the type opens it, and
/// the catalogue is sorted by name: a design tool that declares every image
/// type, and whose name sorts before the browser's, took every photograph
/// from the browser. An entry that says
/// `X-Ronoctua-Fallback=false` ([`ronoctua_apps::DesktopEntry::fallback`]) is
/// passed by in that order — by type or by parent type, ours or anybody's —
/// and still wins where a default names it, and is still offered where a
/// person chooses (`open with`).
#[must_use]
pub fn opens_by<'a>(
    catalogue: &'a Catalogue,
    unlisted: &'a [ronoctua_apps::DesktopEntry],
    defaults: &ronoctua_apps::defaults::Defaults,
    of_type: &str,
) -> Option<&'a ronoctua_apps::DesktopEntry> {
    let wanted = ronoctua_files::mime::canonical(of_type);
    // THE ALIAS COUNTS: a list may spell `model/3mf` as the old
    // `application/vnd.ms-3mfdocument`, and both name one type.
    let chosen = defaults
        .for_type(|m| ronoctua_files::mime::canonical(m) == wanted)
        .find_map(|id| {
            catalogue
                .iter()
                .find(|e| e.id == id)
                .or_else(|| unlisted.iter().find(|e| e.id == id))
        });
    if chosen.is_some() {
        return chosen;
    }
    let catalogue = || catalogue.iter().filter(|e| e.fallback);
    // OURS FIRST, then anybody's. The player claims every audio and video
    // type, and so do the mpv and the VLC the full profile installs; the
    // catalogue's order between entries of one directory is the order the
    // directory lists them in, which is nobody's decision. A program of this
    // system opening a file this system knows how to play is.
    let ours = |e: &&ronoctua_apps::DesktopEntry| e.id.starts_with(OURS);
    let exact = |e: &&ronoctua_apps::DesktopEntry| e.mime_types.iter().any(|m| m == wanted);
    let parent = |e: &&ronoctua_apps::DesktopEntry| {
        e.mime_types
            .iter()
            .any(|m| ronoctua_files::mime::is_a(wanted, m))
    };
    catalogue()
        .filter(ours)
        .find(exact)
        // EXACTLY the type first. A handler for `text/plain` will open a
        // `text/x-rust` — every editor does — but so will fifty other
        // applications, and picking one of those over the one that named the
        // type is how a `.rs` file ends up in a mail client.
        .or_else(|| catalogue().find(exact))
        .or_else(|| catalogue().filter(ours).find(parent))
        .or_else(|| catalogue().find(parent))
}

/// The prefix of this system's own desktop entries.
pub const OURS: &str = "br.ronoctua.";

/// The command line for opening `path` with this entry.
///
/// It re-assembles the entry's `Exec=` with the path in the field codes, rather
/// than appending it: an `Exec=` of `foo --open %f --window` puts the file in
/// the MIDDLE, and appending would hand `--window` the filename.
#[must_use]
pub fn command_for(
    entry: &ronoctua_apps::DesktopEntry,
    path: &std::path::Path,
) -> (String, Vec<String>) {
    let open = vec![path.to_string_lossy().into_owned()];
    // The raw `Exec=` is not kept on the entry — only the assembled command is
    // — so a line with no field code at all gets the path appended, which is
    // what the spec says to do with an entry that declares a type and no
    // placeholder.
    let (program, mut args) = entry.command.clone();
    if args.iter().any(|a| a.contains('%')) {
        // Should not happen: `assemble` consumed the codes at scan time. If it
        // ever does, appending is still better than passing a literal `%f`.
        tracing::debug!(id = %entry.id, "a field code was left over in the command");
    }
    args.extend(open);
    (program, args)
}

/// What opens a file — ONE answer for both doors.
///
/// Text goes to the editor, and so does a file whose type nothing names: an
/// editor opens anything, and "nothing opens this" about a file with no
/// extension is the one answer that cannot be true. Anything else goes to the
/// application that DECLARES the type, out of the catalogue swept at start —
/// no directory is walked here, and no `xdg-open` is spawned to walk one.
///
/// It used to be decided twice, and differently: the footer sent text to the
/// editor and the window asked only the catalogue, so the same `RET` on the
/// same `notes.txt` opened Emacs in one door and whatever declared
/// `text/plain` in the other — and "nothing opens it" for a `README`.
#[derive(Debug, Clone, PartialEq)]
pub enum Opener {
    /// The editor ([`editor_command`]).
    Editor,
    /// The application that declares the type.
    App(Box<ronoctua_apps::DesktopEntry>),
    /// Nothing declares the type: the sentence names it.
    Nothing(&'static str),
}

/// Which [`Opener`] a file at `path` gets.
#[must_use]
pub fn opener_of(catalogue: &Catalogue, path: &std::path::Path) -> Opener {
    let of_type = ronoctua_files::mime::of_path(path, ronoctua_files::Kind::File, true);
    opener_of_type(catalogue, of_type)
}

/// The same, for a type already known.
pub fn opener_of_type(catalogue: &crate::launch::Catalogue, of_type: &'static str) -> Opener {
    if of_type.starts_with("text/") || of_type == ronoctua_files::mime::UNKNOWN {
        return Opener::Editor;
    }
    crate::launch::opens(catalogue, of_type)
        .map_or(Opener::Nothing(of_type), |e| Opener::App(Box::new(e)))
}

/// The editor's command for `file` — the program, its arguments and the
/// folder it starts in — or `None` when there is no editor.
///
/// THE CLIENT COMES FROM THE ENVIRONMENT (`RONOCTUA_EMACSCLIENT`), and not
/// from an `option_env!`: baked in, the path is a reference the binary carries
/// into every closure, and `ronoctua.editor.emacs = false` in the `minimal`
/// profile removed the package and left the whole of Emacs in the system
/// anyway. `nix/modules/emacs.nix` sets it, inside its own `mkIf`. The door
/// reads the variable and hands it here, so this stays a function of what it
/// is given.
#[must_use]
pub fn editor_command(
    client: Option<&str>,
    file: &std::path::Path,
) -> Option<(String, Vec<String>, Option<String>)> {
    let client = client.filter(|c| !c.is_empty())?;
    Some((
        client.to_owned(),
        vec!["-c".to_owned(), file.to_string_lossy().into_owned()],
        file.parent().map(|d| d.to_string_lossy().into_owned()),
    ))
}

/// Where a native buffer's component is — read from the ENVIRONMENT, by the
/// name of the variable that carries it.
///
/// `RONOCTUA_BUFFER_MEMORY`, `_TRANSFERS` and the rest `nix/buffers.nix`
/// names (the editor's had a key and no longer does — see `foreign::editor`).
/// The three icon browsers are no buffers any more: they are the
/// native icons app (`ronoctua_shell_icons::icons`). The memory panel (`LEADER s m`) and the
/// queue (`LEADER t w`) embed no database at all: everything they draw is read
/// by their host at that moment and pushed in, which is what
/// `capability::system` and `capability::transfers` exist for.
///
/// # Why not `option_env!`, which is what this was
///
/// Baked in, a store path is a REFERENCE, and the reference is charged for at
/// the wrong end: the workspace's derivation was handed the five `.wasm` in its
/// `env`, so editing one COMMENT in any buffer changed its hash and recompiled
/// everything — the whole workspace, clippy and the tests, minutes of Rust for
/// a value that is pure data and that no `.rs` here reads at compile time.
///
/// It is the icon superpack's lesson (`nix/rust.nix`) reached from the other
/// side: there the baked path kept a switch from switching, here it kept the
/// loop from being a loop.
///
/// What supplies it is a WRAPPER around this binary, built by `nix/rust.nix`:
/// the reference lives one derivation above the compilation, so the closure
/// still carries the five and no launcher has to remember anything. Absent —
/// `cargo build` outside Nix, which is the case this keeps compiling — the
/// command says so on screen instead of the build breaking.
pub fn buffer_component(variable: &str) -> Option<String> {
    std::env::var(variable).ok().filter(|v| !v.is_empty())
}

/// Whether two desktop ids name the same entry. The launcher's id is the
/// freedesktop one, without the extension (`org.gimp.GIMP`); a source names
/// the FILE it made (`org.gimp.GIMP.desktop`). Both spellings reach here.
#[must_use]
pub fn same_desktop(a: &str, b: &str) -> bool {
    let bare = |s: &str| s.strip_suffix(".desktop").unwrap_or(s).to_owned();
    bare(a) == bare(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// FAILS IF: the editor's command stops being the client, `-c` and the
    /// file, started in the file's folder — or a missing client is taken for
    /// an editor.
    #[test]
    fn the_editor_is_the_client_in_the_files_folder_or_nothing() {
        let file = Path::new("/home/someone/notes/today.txt");
        assert_eq!(
            editor_command(Some("/nix/store/x-emacs/bin/emacsclient"), file),
            Some((
                "/nix/store/x-emacs/bin/emacsclient".to_owned(),
                vec!["-c".to_owned(), "/home/someone/notes/today.txt".to_owned()],
                Some("/home/someone/notes".to_owned()),
            ))
        );
        assert_eq!(editor_command(None, file), None);
        assert_eq!(editor_command(Some(""), file), None);
    }
}
