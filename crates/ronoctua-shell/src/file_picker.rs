//! THE FILE PICKER — the one way this system asks a person for a file, a few
//! files, a folder, or a name to save under.
//!
//! `files::Picker` is another thing (the owner and group chooser of the file
//! manager's facts); this is `FilePicker`, in a file of its own.
//!
//! # Why one
//!
//! Before it, five places asked for a path in five ways: the player browsed its
//! own three folders with a grid of its own (what is left of it is the rules
//! of the kinds, `player_kinds.rs`), and the
//! programs' icon, a package file, a Syncthing folder, a tunnel's file and the
//! camera's backdrops were a line to TYPE a path on. A line to type a path on
//! is a question only a person who already knows the answer can answer. The
//! picker is what every one of them asks now, and `one-file-browser` refuses a
//! sixth listing of a folder written for choosing.
//!
//! # What it is, and what it is not
//!
//! A COMPONENT: rows, keys and clicks, with no door of its own. The door that
//! hosts it — a page of the player, a level of the programs' view — draws its
//! rows as it draws its own ([`Row`] carries `files::Field`s, the fields every
//! view of this shell is drawn with), hands it every key while it is open
//! ([`FilePicker::claims`], [`FilePicker::key`]) and turns a [`Step`] into
//! what the door does: [`Step::Picked`] is the answer, [`Step::Back`] is the
//! person going back without one. Both doors of a view host the same picker
//! the same way, so what one door picks the other picks (the owner's rule).
//!
//! # What a person does in it
//!
//! - THE PLACES are the file manager's sidebar — the same rows, from
//!   `files::sidebar_of`, with the home on top ([`Context`]) — and after
//!   them the devices mounted when the picker opened, read once off the loop
//!   by a door that has one ([`FilePicker::watch_with`]); under them the
//!   folder that is open, `..` first.
//! - A LIST, or a GRID where the door lights one cell under the pointer
//!   ([`Context::grid`]), filled DOWN each column — the order a listing is
//!   read in, so `↓` is the next name. The door lights the cell and takes the
//!   click through one answer (`chrome::Across`) — a grid cell is lit
//!   alone.
//! - TYPING NARROWS the folder to the names that hold what was typed; text
//!   that starts with `/` or `~` is a PATH, walked down to the deepest folder
//!   it names and narrowed by the rest. `C-v` does the same with what was
//!   copied, a `file://` address included.
//! - `RET` on a folder enters it; on a file it is the answer — or, typed, a
//!   path that names a file, or an address where the request takes one.
//!   `C-RET` answers the other way the page offers ([`Alternate`]).
//! - SEVERAL, where the request takes several: `C-SPC` marks by name, and the
//!   pointer marks with `Ctrl` held and a range with `Shift`
//!   ([`FilePicker::mark_target`]); "Take the marked" answers them all.
//! - SAVING has a name row on top, typed like the rest; a name that is
//!   already there is written over only after a question, "no" first.
//! - THE FOLDER IS REMEMBERED per purpose, in `file-picker.json` — merged,
//!   by purpose, as every file the doors share — and WATCHED while it is open
//!   in a door with a loop (`ronoctua_files::notice`, woken through the
//!   door's [`crate::player_subtitles::Wake`]): a file that lands in it is on
//!   screen without a key, and no clock asks.

use std::path::{Path, PathBuf};

use ronoctua_files::mode::Explanation;
use ronoctua_i18n::key;
use ronoctua_keymap::{Key, Sym};
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit, Ink};

/// The file the picker remembers its folders in: one per purpose.
pub const FILE: &str = "file-picker.json";

// ─── the request ───────────────────────────────────────────────────────────

/// What a host asks for.
#[derive(Debug, Clone)]
pub struct Request {
    /// What it is for — `"player-queue"`, `"software-icon"` — which keys the
    /// folder remembered for it.
    pub purpose: &'static str,
    /// One file or several, a folder, or a name to save under.
    pub mode: Mode,
    /// What the folder shows besides its folders: a ring, turned by `TAB`,
    /// whose last is [`Filter::ANY`].
    pub filters: &'static [Filter],
    /// Where it opens.
    pub start: Start,
    /// A typed web address is an answer too (the player plays one).
    pub addresses: bool,
    /// Answers of the page besides a file: rows of their own, each with its
    /// key ([`PLAY_FOLDER`], [`ENQUEUE_FOLDER`]).
    pub extras: &'static [Extra],
    /// What `C-RET` answers, when the page offers a second way.
    pub alternate: Option<Alternate>,
}

/// What is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// A file — or several, marked.
    Open {
        /// Several may be marked and taken together.
        many: bool,
    },
    /// A folder: the one that is open, taken with "Choose this folder".
    Folder,
    /// A name to save under, in the folder that is open.
    Save {
        /// The name it starts with.
        name: String,
        /// What every name ends in — added when it is not typed.
        extension: Option<&'static str>,
    },
}

/// What a folder shows of its files: a name for the ring, and the glyph of
/// each file it keeps — `None` leaves it out.
///
/// The second argument is the file's media type, looked up ONCE when the
/// folder is read (`mime::by_name` walks every pattern of the database, and a
/// filter asked on every frame froze a folder before — `player_kinds`).
#[derive(Clone, Copy)]
pub struct Filter {
    /// The key of its name on the ring.
    pub label: &'static str,
    /// The glyph of a file it keeps, or `None`.
    pub keeps: fn(&Path, Option<&'static str>) -> Option<char>,
}

impl std::fmt::Debug for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

impl Filter {
    /// Every file, by the glyph of its type — the last of every ring.
    pub const ANY: Self = Self {
        label: key::FILE_PICKER_FILTER_ANY,
        keeps: any_file,
    };
}

/// Every file keeps: the glyph of its type, or the generic file's.
fn any_file(_: &Path, mime: Option<&'static str>) -> Option<char> {
    Some(mime.map_or(tokens::icon::FILE, ronoctua_files::mime::icon_of))
}

/// Pictures: every file whose type is an image.
pub const IMAGES: Filter = Filter {
    label: key::FILE_PICKER_FILTER_IMAGES,
    keeps: an_image,
};

/// An image's glyph, or `None`.
fn an_image(_: &Path, mime: Option<&'static str>) -> Option<char> {
    mime.filter(|m| m.starts_with("image/"))
        .map(ronoctua_files::mime::icon_of)
}

/// The ring of a request for pictures.
pub const PICTURES: &[Filter] = &[IMAGES, Filter::ANY];

/// The ring of a request that takes any file.
pub const EVERY_FILE: &[Filter] = &[Filter::ANY];

/// Where the picker opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// Where it was left for this purpose, else the home.
    Remembered,
    /// Where it was left for this purpose, else one of the person's user
    /// directories.
    Place(Xdg),
    /// Here, whatever was remembered — the film's own folder, for its
    /// subtitles.
    At(PathBuf),
}

/// One of the freedesktop user directories: its key, and the folder under
/// home the specification falls back to when nothing declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xdg {
    /// `XDG_MUSIC_DIR`.
    pub id: &'static str,
    /// `Music`.
    pub fallback: &'static str,
}

impl Xdg {
    /// The music directory.
    pub const MUSIC: Self = Self {
        id: "XDG_MUSIC_DIR",
        fallback: "Music",
    };
    /// The videos directory.
    pub const VIDEOS: Self = Self {
        id: "XDG_VIDEOS_DIR",
        fallback: "Videos",
    };
    /// The pictures directory.
    pub const PICTURES: Self = Self {
        id: "XDG_PICTURES_DIR",
        fallback: "Pictures",
    };
    /// The downloads directory.
    pub const DOWNLOAD: Self = Self {
        id: "XDG_DOWNLOAD_DIR",
        fallback: "Downloads",
    };
    /// The documents directory.
    pub const DOCUMENTS: Self = Self {
        id: "XDG_DOCUMENTS_DIR",
        fallback: "Documents",
    };
}

/// An answer of the page besides a file — a row of its own, and a key.
///
/// ITS ID IS A ROW OF [`OF_THE_VIEW`], so the key it is taught by explains
/// itself (`every-hint-key-explains-itself`): a host picks among the ones
/// declared here, and a new one is declared here first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extra {
    /// Its id — the hint's, and what [`Picked::Extra`] says.
    pub id: &'static str,
    /// The key of its row's label.
    pub label_key: &'static str,
    /// Its key, as the keymap writes it.
    pub key: &'static str,
}

/// The folder that is open, played whole.
pub const PLAY_FOLDER: Extra = Extra {
    id: "file-picker-play-folder",
    label_key: key::FILE_PICKER_ROW_PLAY_FOLDER,
    key: "C-p",
};

/// The folder that is open, put in the queue whole.
pub const ENQUEUE_FOLDER: Extra = Extra {
    id: "file-picker-enqueue-folder",
    label_key: key::FILE_PICKER_ROW_ENQUEUE_FOLDER,
    key: "C-e",
};

/// The second way a page answers, on `C-RET` — each a row of
/// [`OF_THE_VIEW`], for the same reason as [`Extra`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternate {
    /// Play it now, right after what plays, the queue kept.
    PlayNow,
}

impl Alternate {
    /// Its id in [`OF_THE_VIEW`].
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PlayNow => "file-picker-play-now",
        }
    }
}

/// Which way an answer was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum How {
    /// `RET`, a second click.
    #[default]
    Normal,
    /// `C-RET` — [`Request::alternate`].
    Alternate,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    /// Files, a folder, or where to save — never empty.
    Paths {
        /// What was chosen.
        paths: Vec<PathBuf>,
        /// Which way.
        how: How,
    },
    /// A web address, typed.
    Address(String),
    /// One of the page's extras, on the folder that is open.
    Extra {
        /// Which ([`Extra::id`]).
        id: &'static str,
        /// The folder.
        folder: PathBuf,
    },
}

/// What a key or a click did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing that shows.
    Nothing,
    /// The rows changed: draw again.
    Changed,
    /// The answer.
    Picked(Picked),
    /// Back, with no answer — `Esc` on an empty line.
    Back,
    /// A sentence to say, and nothing else.
    Say(Explanation),
    /// `C-v`: the door reads the clipboard off its loop and hands what was
    /// copied to [`FilePicker::pasted`].
    Paste,
}

// ─── the context ───────────────────────────────────────────────────────────

/// What the door knows that the picker does not: whose home, which places,
/// whether it lights one cell of a grid, and where the folders are
/// remembered.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// The home — `~` in what is typed and shown, and the first place.
    pub home: Option<PathBuf>,
    /// The places, in order.
    pub places: Vec<Place>,
    /// The door lights one cell of a grid under the pointer and takes a click
    /// by its column (`chrome::hover_cells`): the folder may be a grid.
    pub grid: bool,
    /// Where `file-picker.json` is — `None` remembers nothing (a photograph,
    /// a test).
    pub remembers: Option<PathBuf>,
    /// This machine's user directories, as declared — `false` for a home that
    /// is not this machine's, where each is its fallback under the home.
    pub machine: bool,
}

/// A place of the pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// What it is called.
    pub name: PlaceName,
    /// Its glyph.
    pub glyph: char,
    /// Where it is.
    pub path: PathBuf,
}

/// A place's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceName {
    /// The home: a word of the language.
    Home,
    /// A folder's or a device's own name — data.
    Named(String),
}

impl Context {
    /// This machine: its home, the file manager's sidebar, and the
    /// configuration directory.
    #[must_use]
    pub fn of_this_machine(grid: bool) -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let side = crate::files::sidebar_of(
            home.as_deref(),
            None,
            ronoctua_files::sidebar::store::config_dir(),
        );
        let mut places = home.iter().map(|h| home_place(h)).collect::<Vec<_>>();
        places.extend(places_of(side.rows()));
        Self {
            home,
            places,
            grid,
            // A TEST REMEMBERS NOTHING: a pick in one wrote the person's own
            // `file-picker.json` (`tests-do-not-touch-the-machine`).
            remembers: if cfg!(test) {
                None
            } else {
                ronoctua_store::config_dir()
            },
            machine: true,
        }
    }

    /// A home that is NOT this machine's — a photograph's, a test's: the home
    /// and the user directories under it that are there, and nothing
    /// remembered. The same picture on every machine.
    #[must_use]
    pub fn in_home(home: &Path, grid: bool) -> Self {
        let mut places = vec![home_place(home)];
        for xdg in [
            Xdg::MUSIC,
            Xdg::VIDEOS,
            Xdg::PICTURES,
            Xdg::DOCUMENTS,
            Xdg::DOWNLOAD,
        ] {
            let path = home.join(xdg.fallback);
            if path.is_dir() {
                places.push(Place {
                    name: PlaceName::Named(xdg.fallback.to_owned()),
                    glyph: tokens::icon::FOLDER,
                    path,
                });
            }
        }
        Self {
            home: Some(home.to_path_buf()),
            places,
            grid,
            remembers: None,
            machine: false,
        }
    }

    /// Where a user directory is, for this context.
    fn resolved(&self, xdg: Xdg) -> PathBuf {
        if self.machine {
            return ronoctua_xdg::user_dir(xdg.id, xdg.fallback);
        }
        self.home
            .as_ref()
            .map_or_else(|| PathBuf::from("/"), |h| h.join(xdg.fallback))
    }
}

/// The home's place.
fn home_place(home: &Path) -> Place {
    Place {
        name: PlaceName::Home,
        glyph: tokens::icon::FOLDER,
        path: home.to_path_buf(),
    }
}

/// The sidebar's rows a picker can open: its folders, and its devices that
/// are mounted. Its files, programs and the trash are not places to choose
/// from.
fn places_of<'a>(rows: impl IntoIterator<Item = &'a ronoctua_files::sidebar::Row>) -> Vec<Place> {
    use ronoctua_files::sidebar::Target as Side;
    rows.into_iter()
        .filter_map(|r| {
            let (path, glyph) = match &r.target {
                Side::Place(p) => (p.clone(), tokens::icon::FOLDER),
                Side::Device(_) if r.mounted == Some(true) => (
                    r.at.clone()?,
                    if r.removable {
                        tokens::icon::DISK_REMOVABLE
                    } else {
                        tokens::icon::DISK
                    },
                ),
                _ => return None,
            };
            let name = r.label.clone().unwrap_or_else(|| name_of(&path));
            Some(Place {
                name: PlaceName::Named(name),
                glyph,
                path,
            })
        })
        .collect()
}

/// The devices of `rack` a person may pick from: the volumes the file
/// manager lists (`ronoctua_disks::listed`, which leaves out what is the
/// system's or another person's), mounted and not quiet, each where it is
/// mounted — by the name the sidebar gives it, and the glyph of a drive that
/// comes out, when its drive does.
fn device_places(rack: &ronoctua_disks::Rack) -> Vec<Place> {
    let uid = ronoctua_files::who::my_uid();
    ronoctua_disks::listed(&rack.volumes, uid, false)
        .into_iter()
        .filter(|v| !v.system)
        .filter_map(|v| {
            let path = PathBuf::from(v.place()?);
            let removable = ronoctua_disks::attached_to(v, &rack.volumes)
                .and_then(|at| rack.drives.iter().find(|d| d.at == at))
                .is_some_and(|d| d.removable);
            Some(Place {
                name: PlaceName::Named(v.name().to_owned()),
                glyph: if removable {
                    tokens::icon::DISK_REMOVABLE
                } else {
                    tokens::icon::DISK
                },
                path,
            })
        })
        .collect()
}

// ─── what is remembered ────────────────────────────────────────────────────

/// `file-picker.json`: the folder each purpose was left in.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Remembered {
    /// By purpose.
    #[serde(default)]
    pub folders: std::collections::BTreeMap<String, PathBuf>,
}

/// The store, under `dir`.
fn store(dir: Option<PathBuf>) -> ronoctua_store::Store<Remembered> {
    ronoctua_store::Store::at(dir, FILE)
}

/// Remembers `folder` for `purpose` — MERGED into the file as it is on disk:
/// another door may have remembered another purpose since this one read it.
/// A file that does not parse is not written over (`Store::parsed`).
fn remember(dir: Option<PathBuf>, purpose: &str, folder: &Path) {
    let store = store(dir);
    let Some(mut now) = store.parsed() else {
        tracing::warn!(
            purpose,
            "file-picker.json does not parse; the folder is not remembered"
        );
        return;
    };
    if now.folders.get(purpose).map(PathBuf::as_path) == Some(folder) {
        return;
    }
    now.folders.insert(purpose.to_owned(), folder.to_path_buf());
    if let Err(e) = store.write(&now) {
        tracing::warn!(error = %e, purpose, "the picker's folder could not be remembered");
    }
}

// ─── the keys ──────────────────────────────────────────────────────────────

/// Where a key of the picker applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// Across the cells of a grid.
    OnAGrid,
    /// While something is typed: `DEL` takes a letter off.
    Typed,
    /// While nothing is typed: `DEL` goes up.
    Untyped,
    /// While the folder shows through a ring of filters.
    WithFilters,
    /// While several may be marked.
    Many,
    /// While [`Alternate::PlayNow`] is the page's second way.
    PlaysNow,
    /// While the extra of this id is on the page.
    Extra,
}

/// The keys the picker teaches, and where each applies. Every id is a word
/// (`file-picker-<what>`) and a sentence under the pointer
/// (`about-file-picker-<what>`).
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "file-picker-down", Where::Anywhere),
    ("<up>", "file-picker-up", Where::Anywhere),
    ("<left>", "file-picker-left", Where::OnAGrid),
    ("<right>", "file-picker-right", Where::OnAGrid),
    ("RET", "file-picker-enter", Where::Anywhere),
    ("C-RET", "file-picker-play-now", Where::PlaysNow),
    ("C-SPC", "file-picker-mark", Where::Many),
    ("DEL", "file-picker-erase", Where::Typed),
    ("DEL", "file-picker-parent", Where::Untyped),
    ("TAB", "file-picker-filter", Where::WithFilters),
    ("C-p", "file-picker-play-folder", Where::Extra),
    ("C-e", "file-picker-enqueue-folder", Where::Extra),
    ("C-v", "file-picker-paste", Where::Anywhere),
    ("C-h", "file-picker-hidden", Where::Anywhere),
];

/// A key, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    /// `Esc`: the typed line cleared, else back.
    Escape,
    /// `RET`, or `C-RET`.
    Enter(How),
    /// An arrow, a page, an end.
    Move(Sym),
    /// `DEL`.
    Erase,
    /// `TAB`, or `S-TAB` back.
    Filter(bool),
    /// `C-SPC`.
    Mark,
    /// `C-v`.
    Paste,
    /// `C-h`.
    Hidden,
    /// An extra's key.
    Extra(Extra),
    /// A letter.
    Type(char),
}

/// What `key` is to a picker with these `extras` — `None` for a key it
/// leaves to its door.
fn read(key: Key, extras: &[Extra]) -> Option<Act> {
    let m = key.mods;
    let plain = !m.ctrl && !m.alt && !m.logo;
    if let Some(e) = extras
        .iter()
        .find(|e| Key::parse(e.key).is_ok_and(|k| k == key))
    {
        return Some(Act::Extra(*e));
    }
    Some(match key.sym {
        Sym::Esc => Act::Escape,
        Sym::Enter if m.ctrl && !m.alt && !m.logo => Act::Enter(How::Alternate),
        Sym::Enter if plain => Act::Enter(How::Normal),
        Sym::Up
        | Sym::Down
        | Sym::Left
        | Sym::Right
        | Sym::PageUp
        | Sym::PageDown
        | Sym::Home
        | Sym::End
            if plain =>
        {
            Act::Move(key.sym)
        }
        Sym::Backspace if plain => Act::Erase,
        Sym::Tab if plain => Act::Filter(!m.shift),
        Sym::Space if m.ctrl && !m.alt && !m.logo => Act::Mark,
        Sym::Space if plain => Act::Type(' '),
        Sym::Char('v') if m.ctrl && !m.alt && !m.logo => Act::Paste,
        Sym::Char('h') if m.ctrl && !m.alt && !m.logo => Act::Hidden,
        Sym::Char(c) if plain && !c.is_control() => Act::Type(c),
        _ => return None,
    })
}

// ─── the rows ──────────────────────────────────────────────────────────────

/// What a key or a click acts on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// A title, a sentence: nothing.
    Nothing,
    /// The name a file is saved under.
    Name,
    /// What was typed: a path, an address.
    Typed,
    /// The ring of filters.
    Filter,
    /// An extra, by its id.
    Extra(&'static str),
    /// The folder that is open, chosen.
    Here,
    /// The marked files, taken.
    Take,
    /// A place of the pane.
    Place(PathBuf),
    /// The folder above.
    Up,
    /// A folder of the listing.
    Folder(PathBuf),
    /// A file of the listing.
    File(PathBuf),
    /// An answer to the question on screen.
    Answer(bool),
}

impl Target {
    /// Its identity across a rebuild.
    #[must_use]
    pub fn spot(&self) -> String {
        match self {
            Self::Nothing => "pick-nothing".to_owned(),
            Self::Name => "pick-name".to_owned(),
            Self::Typed => "pick-typed".to_owned(),
            Self::Filter => "pick-filter".to_owned(),
            Self::Extra(id) => format!("pick-{id}"),
            Self::Here => "pick-here".to_owned(),
            Self::Take => "pick-take".to_owned(),
            Self::Place(p) => format!("pick-place-{}", p.display()),
            Self::Up => "pick-..".to_owned(),
            Self::Folder(p) | Self::File(p) => format!("pick-{}", p.display()),
            Self::Answer(yes) => format!("pick-answer-{yes}"),
        }
    }
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Placed and cut.
    pub fields: Vec<Field>,
    /// When the line is a row of a GRID: its cells. Empty for a row that is
    /// one thing, whose band is the whole row.
    pub cells: Vec<Cell>,
    /// A title or a sentence: the cursor skips it.
    pub heading: bool,
    /// The cursor is on it.
    pub focused: bool,
    /// What a key does here — on a grid row, the cell the cursor is on, or
    /// its first.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
    /// The cell a text caret stands before, on the line being typed.
    pub caret: Option<u16>,
    /// The rest of the row above it — a value wrapped under itself — and not
    /// a row of its own: the cursor does not stop on it, and the pointer
    /// lights it with the row it continues.
    pub continues: bool,
}

/// One cell of a grid row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Its first column, as a field's `at`.
    pub from: u16,
    /// How many columns it takes.
    pub width: u16,
    /// What it is.
    pub target: Target,
}

/// Which part of the page an item is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// The rows on top: the name, what was typed, the ring, the extras.
    Top,
    /// The places.
    Places,
    /// The folder that is open.
    Listing,
}

/// One thing the cursor can stand on.
#[derive(Debug, Clone)]
struct Item {
    target: Target,
    section: Section,
    glyph: char,
    name: String,
    /// The clusters of `name` that hold what was typed.
    lit: Vec<usize>,
    marked: bool,
    bytes: Option<u64>,
}

/// One entry of the folder, read.
#[derive(Debug, Clone)]
struct Entry {
    path: PathBuf,
    name: String,
    folder: bool,
    hidden: bool,
    mime: Option<&'static str>,
    bytes: u64,
    /// Its glyph under the filter in use, `None` left out — found when the
    /// folder or the filter changes, never per frame.
    glyph: Option<char>,
}

/// The folder watched, and whether it moved — and the devices mounted, each
/// time udisks2 says they changed (`ronoctua_disks::watch_while`). A copy of
/// a picker watches nothing until its door asks it to
/// (`FilePicker::watch_with`).
#[derive(Default)]
struct Watch {
    notice: Option<ronoctua_files::notice::Notice>,
    stirred: std::sync::Arc<std::sync::atomic::AtomicBool>,
    devices: std::sync::Arc<std::sync::Mutex<Option<Vec<Place>>>>,
    /// The disks' watch goes on while this is up: the picker that closes
    /// lets it down, and the thread ends at udisks2's next signal.
    disks_alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.disks_alive
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Clone for Watch {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("watching", &self.notice.is_some())
            .finish_non_exhaustive()
    }
}

/// Why a folder was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Refused {
    /// The system's words, wrapped in ours (`file-picker-refused`).
    Said(String),
    /// A folder of the `/private` vault while it is not open, or not
    /// answering — in our words alone.
    Vault(ronoctua_files::listing::Withheld),
}

/// The picker.
#[derive(Debug, Clone)]
pub struct FilePicker {
    request: Request,
    context: Context,
    /// The folder open.
    at: PathBuf,
    entries: Vec<Entry>,
    /// Why the folder could not be read.
    refused: Option<Refused>,
    /// Which filter of the ring.
    filter: usize,
    /// What was typed — the name, when saving.
    query: String,
    /// The hidden files are shown.
    hidden: bool,
    /// The files marked, by path.
    marks: Vec<PathBuf>,
    /// The cursor, by identity.
    cursor: Target,
    /// A person moved it: settling leaves it where it is.
    moved: bool,
    /// The folder just left by going up, for the cursor to land on.
    came_from: Option<PathBuf>,
    /// The file a save would write over, while that is asked — and the
    /// answer the cursor is on.
    asking: Option<(PathBuf, bool)>,
    /// The width the rows were last drawn at, in cells — what a key walks a
    /// grid by.
    drawn: std::cell::Cell<u16>,
    /// How many lines the door shows of the rows — what `PgUp` and `PgDn`
    /// walk half of ([`Self::page_is`]).
    page: std::cell::Cell<u16>,
    watch: Watch,
    /// The places the disks' watch added — the ones a later reading may
    /// take away again ([`Self::devices_are`]).
    devices_added: Vec<PathBuf>,
}

impl FilePicker {
    /// Opens a picker for `request`.
    #[must_use]
    pub fn open(request: Request, context: &Context) -> Self {
        debug_assert!(
            request
                .filters
                .last()
                .is_none_or(|f| f.label == Filter::ANY.label),
            "a ring of filters ends with every file"
        );
        let remembered = store(context.remembers.clone())
            .read()
            .folders
            .get(request.purpose)
            .filter(|p| p.is_dir())
            .cloned();
        let home = context.home.clone().unwrap_or_else(|| PathBuf::from("/"));
        let at = match &request.start {
            Start::At(p) => deepest(p),
            Start::Remembered => remembered.unwrap_or(home),
            Start::Place(xdg) => remembered.unwrap_or_else(|| deepest(&context.resolved(*xdg))),
        };
        let query = match &request.mode {
            Mode::Save { name, .. } => name.clone(),
            Mode::Open { .. } | Mode::Folder => String::new(),
        };
        let cursor = if matches!(request.mode, Mode::Save { .. }) {
            Target::Name
        } else {
            Target::Nothing
        };
        let mut picker = Self {
            request,
            context: context.clone(),
            at: at.clone(),
            entries: Vec::new(),
            refused: None,
            filter: 0,
            query,
            hidden: false,
            marks: Vec::new(),
            cursor,
            moved: false,
            came_from: None,
            asking: None,
            drawn: std::cell::Cell::new(0),
            page: std::cell::Cell::new(0),
            watch: Watch::default(),
            devices_added: Vec::new(),
        };
        picker.read_at(&at);
        picker.settle();
        picker
    }

    /// The folder open.
    #[cfg(test)]
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.at
    }

    /// What was typed.
    #[cfg(test)]
    #[must_use]
    pub fn typed_text(&self) -> &str {
        &self.query
    }

    /// Whether a question is on screen.
    #[cfg(test)]
    #[must_use]
    pub const fn asking(&self) -> bool {
        self.asking.is_some()
    }

    // ── the folder ──

    /// Reads `dir` into the entries, the cursor left to [`Self::settle`].
    fn read_at(&mut self, dir: &Path) {
        self.at = dir.to_path_buf();
        self.entries.clear();
        self.refused = None;
        match ronoctua_files::listing::read_dir_at(dir, ronoctua_files::listing::Sort::default()) {
            Ok(read) => {
                for e in read.into_iter().filter(|e| !e.is_parent()) {
                    let path = dir.join(&e.name);
                    let folder = e.opens_as() == ronoctua_files::Kind::Directory;
                    // THE TYPE, ONCE: the expensive lookup of a listing.
                    let mime = if folder {
                        None
                    } else {
                        ronoctua_files::mime::by_name(&e.name).map(ronoctua_files::mime::canonical)
                    };
                    self.entries.push(Entry {
                        // A DOT FILE, by the only rule Unix has (`..` is
                        // filtered out above).
                        hidden: e.name.starts_with('.'),
                        path,
                        name: e.name,
                        folder,
                        mime,
                        bytes: e.bytes,
                        glyph: None,
                    });
                }
            }
            // A FOLDER OF THE /private VAULT, not open: said in our words,
            // never the journal's English (`listing::Withheld`).
            Err(e) => {
                self.refused = Some(match ronoctua_files::listing::withheld(&e) {
                    Some(w) => Refused::Vault(w),
                    None => Refused::Said(
                        e.root_cause()
                            .to_string()
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .to_owned(),
                    ),
                });
            }
        }
        self.glyphs();
        if let Some(n) = self.watch.notice.as_mut() {
            n.look_at(dir);
        }
    }

    /// Each file's glyph under the filter in use.
    fn glyphs(&mut self) {
        let filter = self.current_filter();
        for e in &mut self.entries {
            e.glyph = if e.folder {
                None
            } else {
                filter.and_then(|f| (f.keeps)(&e.path, e.mime))
            };
        }
    }

    /// The filter in use: none on a request with no ring, which shows every
    /// file.
    fn current_filter(&self) -> Option<Filter> {
        self.request
            .filters
            .get(self.filter)
            .copied()
            .or(Some(Filter::ANY))
    }

    /// Another folder: the typed line cleared — a name typed in one folder
    /// narrows nothing in the next — except the name a file is saved under.
    fn go_to(&mut self, dir: &Path, came_from: Option<PathBuf>) {
        let dir = deepest(dir);
        self.query = match self.request.mode {
            Mode::Save { .. } => self.name_part().to_owned(),
            Mode::Open { .. } | Mode::Folder => String::new(),
        };
        self.read_at(&dir);
        self.came_from = came_from;
        self.moved = false;
        if !matches!(self.request.mode, Mode::Save { .. }) {
            self.cursor = Target::Nothing;
        }
        self.settle();
    }

    /// The folder read again where it is — it moved on disk.
    pub fn folder_changed(&mut self) {
        let at = self.at.clone();
        self.read_at(&at);
        self.settle();
    }

    /// Watches the folder open while the picker is up, and wakes the door
    /// when it moves — the door then calls [`Self::look`]. And reads the
    /// devices mounted, off the loop and again each time udisks2 says they
    /// moved, for the places — a stick plugged in while the picker is up is
    /// a place, and one pulled out is not (on this machine only: a
    /// photograph's home has none). A door with no loop (a photograph, a
    /// test) watches nothing.
    pub fn watch_with(&mut self, wake: &crate::player_subtitles::Wake) {
        if self.watch.notice.is_some() {
            return;
        }
        let (stirred, woken) = (self.watch.stirred.clone(), wake.clone());
        // `for_listing`: the /private vault's status too — a folder of the
        // vault is read only while it is open.
        match ronoctua_files::notice::Notice::for_listing(None, move || {
            stirred.store(true, std::sync::atomic::Ordering::SeqCst);
            woken.call();
            true
        }) {
            Ok(mut n) => {
                n.look_at(&self.at);
                self.watch.notice = Some(n);
            }
            Err(e) => tracing::info!(error = %e, "the picker's folder is not watched"),
        }
        if self.context.machine {
            self.read_the_devices(wake);
        }
    }

    /// The devices mounted, read on udisks2's own thread — the bus is never
    /// asked on the loop — each time it says they moved, and handed to
    /// [`Self::look`] through the wake, while the picker is up.
    fn read_the_devices(&self, wake: &crate::player_subtitles::Wake) {
        let (slot, wake) = (self.watch.devices.clone(), wake.clone());
        let alive = self.watch.disks_alive.clone();
        alive.store(true, std::sync::atomic::Ordering::SeqCst);
        let watched = ronoctua_disks::watch_while(move |rack| {
            if !alive.load(std::sync::atomic::Ordering::SeqCst) {
                return false;
            }
            if let Ok(mut s) = slot.lock() {
                *s = Some(device_places(&rack));
            }
            wake.call();
            true
        });
        if let Err(e) = watched {
            tracing::info!(error = %e, "no devices among the picker's places");
        }
    }

    /// The folder read again if it moved since the last look, and the devices
    /// taken when their read came back; `true` when either changed the rows.
    pub fn look(&mut self) -> bool {
        let devices = self.watch.devices.lock().ok().and_then(|mut s| s.take());
        let mut changed = false;
        if let Some(devices) = devices {
            changed = self.devices_are(devices);
        }
        if self
            .watch
            .stirred
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.folder_changed();
            changed = true;
        }
        changed
    }

    /// The devices among the places, after the sidebar's — the ones already
    /// there by their path (the sidebar holds a device a person pinned) left
    /// once, and the ones an earlier reading added and this one no longer
    /// lists (a stick pulled out) taken away. `true` when the places changed.
    fn devices_are(&mut self, devices: Vec<Place>) -> bool {
        let before = self.context.places.len();
        let gone: Vec<PathBuf> = self
            .devices_added
            .iter()
            .filter(|p| !devices.iter().any(|d| d.path == **p))
            .cloned()
            .collect();
        self.context.places.retain(|p| !gone.contains(&p.path));
        self.devices_added.retain(|p| !gone.contains(p));
        let mut changed = self.context.places.len() != before;
        for d in devices {
            if !self.context.places.iter().any(|p| p.path == d.path) {
                self.devices_added.push(d.path.clone());
                self.context.places.push(d);
                changed = true;
            }
        }
        if changed {
            self.settle();
        }
        changed
    }

    /// How many lines of the rows the door shows — told by the door before it
    /// hands the picker a key, so `PgUp` and `PgDn` walk half of what is on
    /// screen.
    pub fn page_is(&self, lines: u16) {
        self.page.set(lines);
    }

    // ── what was typed ──

    /// Whether what was typed is a path: it starts with `/` or `~`.
    fn is_a_path(&self) -> bool {
        self.query.starts_with('/') || self.query == "~" || self.query.starts_with("~/")
    }

    /// The typed path, `~` against the home.
    fn expanded(&self, typed: &str) -> PathBuf {
        let home = self
            .context
            .home
            .clone()
            .unwrap_or_else(|| PathBuf::from("/"));
        if typed == "~" {
            return home;
        }
        if let Some(rest) = typed.strip_prefix("~/") {
            return home.join(rest);
        }
        PathBuf::from(typed)
    }

    /// The folder a typed path names, and what is typed after it.
    fn path_parts(&self) -> Option<(PathBuf, &str)> {
        if !self.is_a_path() {
            return None;
        }
        if self.query == "~" {
            return Some((self.expanded("~"), ""));
        }
        let cut = self.query.rfind('/')?;
        let (dir, rest) = (&self.query[..=cut], &self.query[cut + 1..]);
        Some((self.expanded(dir), rest))
    }

    /// What narrows the folder — the name after a typed path's last `/`.
    fn name_part(&self) -> &str {
        self.path_parts()
            .map_or(self.query.as_str(), |(_, rest)| rest)
    }

    /// What was typed, followed: a path walks to the deepest folder it names.
    fn follow_the_query(&mut self) {
        if let Some((dir, _)) = self.path_parts() {
            let there = deepest(&dir);
            if there != self.at {
                self.read_at(&there);
                self.came_from = None;
            }
        }
        self.moved = false;
        self.settle();
    }

    /// Text typed — a letter, or several at once.
    pub fn typed(&mut self, text: &str) -> Step {
        if self.asking.is_some() {
            return Step::Nothing;
        }
        let before = self.query.len();
        self.query.extend(text.chars().filter(|c| !c.is_control()));
        if self.query.len() == before {
            return Step::Nothing;
        }
        self.follow_the_query();
        Step::Changed
    }

    /// What was copied, as what was typed: its first line — a `file://`
    /// address read as the path it names.
    pub fn pasted(&mut self, text: &str) -> Step {
        if self.asking.is_some() {
            return Step::Nothing;
        }
        let line = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or_default();
        if line.is_empty() {
            return Step::Nothing;
        }
        self.query = line
            .strip_prefix("file://")
            .map_or_else(|| line.to_owned(), unescaped);
        self.follow_the_query();
        Step::Changed
    }

    // ── the items ──

    /// Whether `name` holds what narrows the folder, and which of its
    /// clusters do — case folded, cluster by cluster.
    fn matches(&self, name: &str) -> Option<Vec<usize>> {
        let wanted: Vec<String> = ronoctua_render::layout::cells_of(self.name_part())
            .map(str::to_lowercase)
            .collect();
        if wanted.is_empty() || matches!(self.request.mode, Mode::Save { .. }) {
            return Some(Vec::new());
        }
        let have: Vec<String> = ronoctua_render::layout::cells_of(name)
            .map(str::to_lowercase)
            .collect();
        (0..=have.len().saturating_sub(wanted.len()))
            .find(|&i| have.len() >= wanted.len() && have[i..i + wanted.len()] == wanted[..])
            .map(|i| (i..i + wanted.len()).collect())
    }

    /// Every item, in reading order.
    fn items(&self) -> Vec<Item> {
        let mut out = Vec::new();
        let top = |target: Target| Item {
            target,
            section: Section::Top,
            glyph: ' ',
            name: String::new(),
            lit: Vec::new(),
            marked: false,
            bytes: None,
        };
        match self.request.mode {
            Mode::Save { .. } => out.push(top(Target::Name)),
            Mode::Open { .. } | Mode::Folder if !self.query.is_empty() => {
                out.push(top(Target::Typed));
            }
            Mode::Open { .. } | Mode::Folder => {}
        }
        if self.request.filters.len() > 1 && self.request.mode != Mode::Folder {
            out.push(top(Target::Filter));
        }
        for e in self.request.extras {
            out.push(top(Target::Extra(e.id)));
        }
        if self.request.mode == Mode::Folder {
            out.push(top(Target::Here));
        }
        if !self.marks.is_empty() {
            out.push(top(Target::Take));
        }
        for p in &self.context.places {
            out.push(Item {
                target: Target::Place(p.path.clone()),
                section: Section::Places,
                glyph: p.glyph,
                name: match &p.name {
                    PlaceName::Home => String::new(),
                    PlaceName::Named(n) => n.clone(),
                },
                lit: Vec::new(),
                marked: false,
                bytes: None,
            });
        }
        if self.at.parent().is_some() {
            out.push(Item {
                target: Target::Up,
                section: Section::Listing,
                glyph: tokens::icon::FOLDER,
                name: ronoctua_files::Entry::PARENT.to_owned(),
                lit: Vec::new(),
                marked: false,
                bytes: None,
            });
        }
        let folders_only = self.request.mode == Mode::Folder;
        for e in self.entries.iter().filter(|e| self.hidden || !e.hidden) {
            let glyph = if e.folder {
                tokens::icon::FOLDER
            } else if folders_only {
                continue;
            } else {
                match e.glyph {
                    Some(g) => g,
                    None => continue,
                }
            };
            let Some(lit) = self.matches(&e.name) else {
                continue;
            };
            let marked = self.marks.contains(&e.path);
            out.push(Item {
                target: if e.folder {
                    Target::Folder(e.path.clone())
                } else {
                    Target::File(e.path.clone())
                },
                section: Section::Listing,
                glyph,
                name: e.name.clone(),
                lit,
                marked,
                bytes: (!e.folder).then_some(e.bytes),
            });
        }
        out
    }

    /// Where the cursor is among `items`.
    fn cursor_in(&self, items: &[Item]) -> Option<usize> {
        items.iter().position(|i| i.target == self.cursor)
    }

    /// The cursor back on an item after the items changed: the folder just
    /// left, the first entry, the first match of what was typed — and on the
    /// name, while saving.
    fn settle(&mut self) {
        if self.asking.is_some() {
            return;
        }
        let items = self.items();
        if self.moved && self.cursor_in(&items).is_some() {
            return;
        }
        if matches!(self.request.mode, Mode::Save { .. }) && !self.moved {
            self.cursor = Target::Name;
            return;
        }
        let came = self.came_from.clone();
        let listed = |i: &&Item| i.section == Section::Listing && i.target != Target::Up;
        let landing = items
            .iter()
            .find(|i| matches!((&i.target, came.as_ref()), (Target::Folder(p), Some(c)) if p == c))
            .or_else(|| items.iter().find(listed))
            .or_else(|| items.iter().find(|i| i.target == Target::Typed))
            .or_else(|| items.iter().find(|i| i.target == Target::Up))
            .or_else(|| items.first());
        self.cursor = landing.map_or(Target::Nothing, |i| i.target.clone());
    }

    /// How many columns a grid of `items` takes in `room` cells: three where
    /// the room holds three of the longest name, two where names are long,
    /// one where a column would cut every name to a stub
    /// (`space.player.browseColumn`) — and one in a door that lights no cell.
    fn columns_of(&self, items: &[&Item], room: u16) -> u16 {
        if !self.context.grid {
            return 1;
        }
        let widest = items
            .iter()
            .map(|i| ronoctua_protocol::cells(&i.name))
            .max()
            .unwrap_or(0)
            .saturating_add(GLYPH_CELLS + GAP_CELLS);
        let floor = tokens::space::PLAYER_BROWSECOLUMN;
        if room / 3 >= widest.max(floor) {
            3
        } else if room / 2 >= floor {
            2
        } else {
            1
        }
    }

    // ── the rows ──

    /// Every row, at `cols` cells wide.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn rows(&self, cols: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.drawn.set(cols);
        let (left, room) = crate::power::extent(cols);
        let mut out: Vec<Row> = Vec::new();
        // A QUESTION ON SCREEN is all that is drawn: it waits for an answer.
        if let Some((path, yes)) = &self.asking {
            let sentence = texts.t_with(
                key::FILE_PICKER_ASK_OVERWRITE,
                &[
                    ("name", &name_of(path)),
                    ("folder", &self.shown(path.parent().unwrap_or(path))),
                ],
            );
            for line in crate::chrome::note_lines(&sentence, room) {
                out.push(inert(vec![piece(
                    left,
                    line,
                    tokens::color::TEXT_SECONDARY,
                )]));
            }
            for (answer, label) in [
                (false, key::FILE_PICKER_ANSWER_NO),
                (true, key::FILE_PICKER_ANSWER_YES),
            ] {
                let focused = *yes == answer;
                out.push(pair_row(
                    crate::chrome::field_pair(
                        left,
                        room,
                        &texts.t(label),
                        "",
                        crate::chrome::Kind::Setting,
                        focused,
                    ),
                    Target::Answer(answer),
                    focused,
                ));
            }
            return out;
        }
        let items = self.items();
        let cursor = self.cursor_in(&items);
        let here = |i: usize| cursor == Some(i);
        // THE ROWS ON TOP, each a pair.
        for (i, item) in items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.section == Section::Top)
        {
            out.extend(self.top_row(item, here(i), left, room, texts));
        }
        if !matches!(self.request.mode, Mode::Save { .. }) && self.query.is_empty() {
            for line in crate::chrome::note_lines(&texts.t(key::FILE_PICKER_TYPE_NOTE), room) {
                out.push(inert(vec![piece(left, line, tokens::color::TEXT_DIM)]));
            }
        }
        // THE PLACES.
        let places: Vec<(usize, &Item)> = items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.section == Section::Places)
            .collect();
        if !places.is_empty() {
            if !out.is_empty() {
                out.push(inert(crate::chrome::AIR));
            }
            out.push(inert(vec![piece(
                left,
                texts.t(key::FILE_PICKER_PLACES),
                tokens::color::TEXT_HEADING,
            )]));
            self.section_rows(&places, cursor, left, room, texts, &mut out);
        }
        // THE FOLDER THAT IS OPEN, titled by its path.
        out.push(inert(crate::chrome::AIR));
        out.push(inert(vec![piece(
            left,
            ronoctua_render::layout::elide_start(&self.shown(&self.at), room),
            tokens::color::TEXT_HEADING,
        )]));
        if let Some(refused) = &self.refused {
            let said = match refused {
                Refused::Said(why) => {
                    texts.t_with(key::FILE_PICKER_REFUSED, &[("why", why.as_str())])
                }
                Refused::Vault(w) => texts.t(w.sentence()),
            };
            for line in crate::chrome::note_lines(&said, room) {
                out.push(inert(vec![piece(left, line, tokens::color::STATE_WARNING)]));
            }
        }
        let listing: Vec<(usize, &Item)> = items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.section == Section::Listing)
            .collect();
        self.section_rows(&listing, cursor, left, room, texts, &mut out);
        let entries = listing
            .iter()
            .filter(|(_, i)| i.target != Target::Up)
            .count();
        if self.refused.is_none() && entries == 0 {
            let said =
                if self.name_part().is_empty() || matches!(self.request.mode, Mode::Save { .. }) {
                    texts.t(key::FILE_PICKER_EMPTY)
                } else {
                    texts.t_with(key::FILE_PICKER_NO_MATCH, &[("typed", self.name_part())])
                };
            for line in crate::chrome::note_lines(&said, room) {
                out.push(inert(vec![piece(left, line, tokens::color::TEXT_DIM)]));
            }
        }
        out
    }

    /// A row on top: its label, its value, and the caret on the line typed.
    fn top_row(
        &self,
        item: &Item,
        focused: bool,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (label, value) = match &item.target {
            Target::Name => (texts.t(key::FILE_PICKER_NAME), self.saved_name()),
            Target::Typed => (texts.t(key::FILE_PICKER_TYPED), self.query.clone()),
            Target::Filter => (
                texts.t(key::FILE_PICKER_SHOWS),
                self.current_filter()
                    .map(|f| texts.t(f.label))
                    .unwrap_or_default(),
            ),
            Target::Extra(id) => (
                self.request
                    .extras
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| texts.t(e.label_key))
                    .unwrap_or_default(),
                String::new(),
            ),
            Target::Here => (texts.t(key::FILE_PICKER_HERE), self.shown(&self.at)),
            Target::Take => (
                texts.t_args(
                    key::FILE_PICKER_TAKE,
                    &[(
                        "count",
                        ronoctua_i18n::Arg::Number(i64::try_from(self.marks.len()).unwrap_or(0)),
                    )],
                ),
                String::new(),
            ),
            _ => (String::new(), String::new()),
        };
        let typed_line = matches!(item.target, Target::Name | Target::Typed);
        // A PATH IS CUT FROM ITS START, and what is being typed keeps its end
        // in sight: the letter just typed is the one a person looks for.
        let value = if typed_line || item.target == Target::Here {
            let keep = room.saturating_sub(tokens::space::VIEW_LABELCELLS.min(room / 2));
            ronoctua_render::layout::elide_start(&value, keep)
        } else {
            value
        };
        let fields = crate::chrome::field_pair(
            left,
            room,
            &label,
            &value,
            crate::chrome::Kind::Setting,
            focused,
        );
        let caret = typed_line
            .then(|| {
                fields.get(1).map(|v| {
                    // BEFORE THE EXTENSION when saving: it is added, not typed.
                    let typed = match item.target {
                        Target::Name => ronoctua_render::layout::width(&value).saturating_sub(
                            self.extension_added()
                                .map_or(0, |e| ronoctua_render::layout::width(e) + 1),
                        ),
                        _ => ronoctua_render::layout::width(&value),
                    };
                    v.at + typed
                })
            })
            .flatten();
        // A CHOSEN VALUE IS NEVER CUT: what the filter shows ("What the
        // player plays") is longer than the room after the value's column in
        // a narrow door, and cut it read `What the playe…`. It wraps under
        // itself, in its own column (`chrome::aligned_pair`), each line the
        // rest of the row. What is typed and a path keep their cut from the
        // start, above: their end is what a person looks for.
        let at = fields.get(1).map(|v| v.at);
        if !typed_line
            && item.target != Target::Here
            && let Some(at) = at
            && ronoctua_render::layout::width(&value) > (left + room).saturating_sub(at)
        {
            return crate::chrome::aligned_pair(
                left,
                at,
                left + room,
                &label,
                &value,
                crate::chrome::Kind::Setting,
                focused,
            )
            .into_iter()
            .enumerate()
            .map(|(i, fields)| Row {
                continues: i > 0,
                ..pair_row(fields, item.target.clone(), focused)
            })
            .collect();
        }
        let mut row = pair_row(fields, item.target.clone(), focused);
        row.caret = caret;
        vec![row]
    }

    /// The name a save writes, as shown: what was typed, and the extension
    /// when it was not typed.
    fn saved_name(&self) -> String {
        let name = self.name_part();
        match self.extension_added() {
            Some(ext) => format!("{name}.{ext}"),
            None => name.to_owned(),
        }
    }

    /// The extension a save adds — `None` when there is none, or it is typed.
    fn extension_added(&self) -> Option<&'static str> {
        let Mode::Save {
            extension: Some(ext),
            ..
        } = self.request.mode
        else {
            return None;
        };
        let name = self.name_part();
        let dotted = format!(".{ext}");
        (!name.to_lowercase().ends_with(&dotted.to_lowercase())).then_some(ext)
    }

    /// The rows of a section: a list, or a grid of cells filled DOWN each
    /// column — the order a listing is read in, so `↓` is the next name and
    /// `←`/`→` go across ([`Self::walk`]; the player's grid before the picker,
    /// `player_rows::Builder::grid`, filled the same way).
    fn section_rows(
        &self,
        items: &[(usize, &Item)],
        cursor: Option<usize>,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
        out: &mut Vec<Row>,
    ) {
        if items.is_empty() {
            return;
        }
        let only: Vec<&Item> = items.iter().map(|(_, i)| *i).collect();
        let columns = self.columns_of(&only, room);
        if !self.context.grid {
            for (n, item) in items {
                out.push(self.list_row(item, cursor == Some(*n), left, room, texts));
            }
            return;
        }
        let width = room / columns.max(1);
        let per = down_a_column(items.len(), columns);
        for r in 0..per {
            let mut fields = Vec::new();
            let mut cells = Vec::new();
            let mut focused = None;
            for c in 0..columns {
                let Some((n, item)) = items.get(usize::from(c) * per + r) else {
                    continue;
                };
                let from = left + c * width;
                let here = cursor == Some(*n);
                if here {
                    focused = Some(cells.len());
                }
                self.entry_fields(
                    item,
                    here,
                    from,
                    width.saturating_sub(GAP_CELLS),
                    texts,
                    &mut fields,
                );
                cells.push(Cell {
                    from,
                    width,
                    target: item.target.clone(),
                });
            }
            let target = cells
                .get(focused.unwrap_or(0))
                .map_or(Target::Nothing, |c| c.target.clone());
            out.push(Row {
                spot: target.spot(),
                fields,
                cells,
                heading: false,
                focused: focused.is_some(),
                target,
                caret: None,
                continues: false,
            });
        }
    }

    /// A row that is one entry: its glyph, its name, and a file's size at the
    /// edge.
    fn list_row(
        &self,
        item: &Item,
        focused: bool,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Row {
        let mut fields = Vec::new();
        let size = item
            .bytes
            .map(|b| crate::files::size_text(b, texts.decimal_mark()));
        let size_cells = size
            .as_deref()
            .map_or(0, |s| ronoctua_render::layout::width(s) + GAP_CELLS);
        self.entry_fields(
            item,
            focused,
            left,
            room.saturating_sub(size_cells),
            texts,
            &mut fields,
        );
        if let Some(s) = size.filter(|_| room > size_cells + GLYPH_CELLS + GAP_CELLS) {
            let at = (left + room).saturating_sub(ronoctua_render::layout::width(&s));
            fields.push(piece(at, s, tokens::color::TEXT_DIM));
        }
        Row {
            spot: item.target.spot(),
            fields,
            cells: Vec::new(),
            heading: false,
            focused,
            target: item.target.clone(),
            caret: None,
            continues: false,
        }
    }

    /// An entry's glyph and name in `room` cells from `from`: the name in the
    /// cursor's colour under it, the letters that hold what was typed lit,
    /// and a marked file's glyph the mark's.
    fn entry_fields(
        &self,
        item: &Item,
        here: bool,
        from: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
        out: &mut Vec<Field>,
    ) {
        let (glyph, glyph_colour) = if item.marked {
            (tokens::icon::SELECTION, tokens::color::STATE_ACCENT)
        } else {
            (item.glyph, tokens::color::TEXT_DIM)
        };
        out.push(piece(from, glyph.to_string(), glyph_colour));
        let whole = match &item.target {
            Target::Place(_) if item.name.is_empty() => texts.t(key::FILE_PICKER_HOME),
            _ => item.name.clone(),
        };
        let name = ronoctua_render::layout::elide_end(&whole, room.saturating_sub(GLYPH_CELLS));
        let colour = if here {
            tokens::color::TEXT_HERE_ACTS
        } else {
            tokens::color::TEXT_PRIMARY
        };
        let field = piece(from + GLYPH_CELLS, name.clone(), colour);
        // A LETTER CUT AWAY lights nothing — and neither does the ellipsis
        // standing where it was.
        let shown = ronoctua_render::layout::cells_of(&name).count();
        let cut = name != whole;
        let lit: Vec<usize> = item
            .lit
            .iter()
            .copied()
            .filter(|&k| k < shown && !(cut && k + 1 == shown))
            .collect();
        if lit.is_empty() {
            out.push(field);
        } else {
            out.extend(crate::chrome::lit_letters(&field, &lit, here));
        }
    }

    /// A path as a person reads it: under the home, from `~`.
    fn shown(&self, p: &Path) -> String {
        if let Some(home) = self.context.home.as_ref()
            && let Ok(rest) = p.strip_prefix(home)
        {
            return if rest.as_os_str().is_empty() {
                "~".to_owned()
            } else {
                format!("~/{}", rest.display())
            };
        }
        p.to_string_lossy().into_owned()
    }

    // ── the keys ──

    /// Whether the picker takes `key` — what a door asks before it hands a
    /// key to the view that hosts it: every letter is the picker's while it
    /// is up.
    #[must_use]
    pub fn claims(&self, key: Key) -> bool {
        read(key, self.request.extras).is_some()
    }

    /// A key.
    pub fn key(&mut self, key: Key) -> Step {
        let Some(act) = read(key, self.request.extras) else {
            return Step::Nothing;
        };
        if self.asking.is_some() {
            return self.answering(act);
        }
        match act {
            Act::Escape => {
                if self.query.is_empty() || matches!(self.request.mode, Mode::Save { .. }) {
                    return Step::Back;
                }
                self.query.clear();
                self.follow_the_query();
                Step::Changed
            }
            Act::Enter(how) => {
                let target = self.cursor.clone();
                self.act(&target, how)
            }
            Act::Move(sym) => self.walk(sym),
            Act::Erase => {
                if crate::line::pop_cluster(&mut self.query) {
                    self.follow_the_query();
                    return Step::Changed;
                }
                self.up()
            }
            Act::Filter(forward) => self.turn(forward),
            Act::Mark => self.mark_here(),
            Act::Paste => Step::Paste,
            Act::Hidden => {
                self.hidden = !self.hidden;
                self.settle();
                Step::Changed
            }
            Act::Extra(e) => Step::Picked(Picked::Extra {
                id: e.id,
                folder: self.at.clone(),
            }),
            Act::Type(c) => self.typed(&c.to_string()),
        }
    }

    /// A key while the question is on screen: the arrows walk its two
    /// answers, `RET` gives the one the cursor is on, `Esc` is its no.
    fn answering(&mut self, act: Act) -> Step {
        let Some((path, yes)) = self.asking.clone() else {
            return Step::Nothing;
        };
        match act {
            Act::Move(Sym::Up | Sym::Down | Sym::Left | Sym::Right) => {
                self.asking = Some((path, !yes));
                Step::Changed
            }
            Act::Enter(_) => self.act(&Target::Answer(yes), How::Normal),
            Act::Escape => self.act(&Target::Answer(false), How::Normal),
            _ => Step::Nothing,
        }
    }

    /// The ring of filters, turned.
    fn turn(&mut self, forward: bool) -> Step {
        let n = self.request.filters.len();
        if n < 2 || self.request.mode == Mode::Folder {
            return Step::Nothing;
        }
        self.filter = if forward {
            (self.filter + 1) % n
        } else {
            (self.filter + n - 1) % n
        };
        self.glyphs();
        self.settle();
        Step::Changed
    }

    /// Up a folder, the cursor on the one just left.
    fn up(&mut self) -> Step {
        let Some(parent) = self.at.parent().map(Path::to_path_buf) else {
            return Step::Nothing;
        };
        let from = self.at.clone();
        self.go_to(&parent, Some(from));
        Step::Changed
    }

    /// `C-SPC`: the file under the cursor marked, or no longer — and the
    /// cursor on to the next.
    fn mark_here(&mut self) -> Step {
        if self.request.mode != (Mode::Open { many: true }) {
            return Step::Nothing;
        }
        let Target::File(p) = self.cursor.clone() else {
            return Step::Nothing;
        };
        self.toggle_mark(&p);
        // ON TO THE NEXT NAME, in the order they are read — down a grid's
        // column, then the next — so a run of files is marked by holding the
        // key.
        let items = self.items();
        if let Some(next) = self
            .cursor_in(&items)
            .and_then(|at| items.get(at + 1))
            .filter(|i| i.section == Section::Listing)
        {
            self.cursor = next.target.clone();
            self.moved = true;
        }
        Step::Changed
    }

    fn toggle_mark(&mut self, p: &Path) {
        if let Some(i) = self.marks.iter().position(|m| m == p) {
            self.marks.remove(i);
        } else {
            self.marks.push(p.to_path_buf());
        }
    }

    /// The cursor walked: a grid by its columns — `↓` the next name, down
    /// the column, and `←`/`→` the same row of the column beside — the rest a
    /// row at a time. Past a section's edge, the next section's, in the same
    /// column when it has one.
    fn walk(&mut self, sym: Sym) -> Step {
        let items = self.items();
        if items.is_empty() {
            return Step::Nothing;
        }
        let at = self.cursor_in(&items).unwrap_or(0);
        let room = crate::power::extent(self.drawn.get()).1;
        // EACH SECTION'S SHAPE: where it starts, how many it holds, and how
        // many a column of it holds.
        let shape = |section: Section| -> (usize, usize, usize) {
            let first = items.iter().position(|i| i.section == section).unwrap_or(0);
            let of: Vec<&Item> = items.iter().filter(|i| i.section == section).collect();
            let columns = if section == Section::Top || !self.context.grid {
                1
            } else {
                self.columns_of(&of, room).max(1)
            };
            (first, of.len(), down_a_column(of.len(), columns))
        };
        let section = items[at].section;
        let (first, count, per) = shape(section);
        let i = at - first;
        let (row, column) = (i % per, i / per);
        let to = match sym {
            // ON THE RING the arrows turn it.
            Sym::Left | Sym::Right if items[at].target == Target::Filter => {
                return self.turn(sym == Sym::Right);
            }
            Sym::Left if column > 0 => at - per,
            Sym::Right if i + per < count => at + per,
            Sym::Left | Sym::Right => return Step::Nothing,
            Sym::Down if row + 1 < per && i + 1 < count => at + 1,
            // PAST A SECTION'S LAST ROW, the next section's first — in the
            // same column when it has as many.
            Sym::Down => match items.get(first + count) {
                Some(next) => {
                    let (start, len, n) = shape(next.section);
                    let columns = len.div_ceil(n).max(1);
                    start + (column.min(columns - 1) * n).min(len - 1)
                }
                None => return Step::Nothing,
            },
            Sym::Up if row > 0 => at - 1,
            Sym::Up if first == 0 => return Step::Nothing,
            // ABOVE A SECTION'S FIRST ROW, the last row of the one before, in
            // the same column when it has one.
            Sym::Up => {
                let (start, len, n) = shape(items[first - 1].section);
                let columns = len.div_ceil(n).max(1);
                start + (column.min(columns - 1) * n + n - 1).min(len - 1)
            }
            Sym::Home => 0,
            Sym::End => items.len() - 1,
            Sym::PageDown | Sym::PageUp => {
                let by = usize::from(self.page.get() / 2).max(1);
                let mut step = Step::Nothing;
                for _ in 0..by {
                    match self.walk(if sym == Sym::PageDown {
                        Sym::Down
                    } else {
                        Sym::Up
                    }) {
                        Step::Nothing => break,
                        moved => step = moved,
                    }
                }
                return step;
            }
            _ => return Step::Nothing,
        };
        if to == at {
            return Step::Nothing;
        }
        self.cursor = items[to].target.clone();
        self.moved = true;
        Step::Changed
    }

    /// What `RET` does on `target`.
    fn act(&mut self, target: &Target, how: How) -> Step {
        match target {
            Target::Nothing => Step::Nothing,
            Target::Name => self.save_named(),
            Target::Typed => self.typed_enter(how),
            Target::Filter => self.turn(true),
            Target::Extra(id) => Step::Picked(Picked::Extra {
                id,
                folder: self.at.clone(),
            }),
            Target::Here => self.picked(vec![self.at.clone()], how),
            // THE MARKS ARE SPENT once taken: the page may stay for more.
            Target::Take => {
                let marks = std::mem::take(&mut self.marks);
                self.picked(marks, how)
            }
            Target::Place(p) | Target::Folder(p) => {
                let p = p.clone();
                self.go_to(&p, None);
                Step::Changed
            }
            Target::Up => self.up(),
            Target::File(p) => match self.request.mode.clone() {
                Mode::Open { many } => {
                    let mut paths = if many {
                        std::mem::take(&mut self.marks)
                    } else {
                        Vec::new()
                    };
                    if !paths.contains(p) {
                        paths.push(p.clone());
                    }
                    self.picked(paths, how)
                }
                // A FILE CHOSEN WHILE SAVING is its name. Of the kind saved
                // (its extension the request's), it is there: written over
                // only after the question. Of another kind, only its name is
                // taken — the extension the request's, and the name row the
                // cursor's, for a `RET` there — so what the row shows is the
                // file that is written.
                Mode::Save { extension, .. } => {
                    let of_the_kind = extension.is_none_or(|ext| {
                        p.extension()
                            .is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(ext))
                    });
                    self.cursor = Target::Name;
                    self.moved = false;
                    if of_the_kind {
                        self.query = name_of(p);
                        self.asking = Some((p.clone(), false));
                    } else {
                        self.query = p
                            .file_stem()
                            .map_or_else(|| name_of(p), |stem| stem.to_string_lossy().into_owned());
                    }
                    Step::Changed
                }
                Mode::Folder => Step::Nothing,
            },
            Target::Answer(true) => match self.asking.take() {
                Some((p, _)) => self.picked(vec![p], How::Normal),
                None => Step::Nothing,
            },
            Target::Answer(false) => {
                self.asking = None;
                Step::Changed
            }
        }
    }

    /// `RET` on what was typed: an address, a path that names a file — or a
    /// folder, while a folder is asked for.
    fn typed_enter(&mut self, how: How) -> Step {
        let typed = self.query.trim().to_owned();
        if typed.is_empty() {
            return Step::Nothing;
        }
        if typed.contains("://") && !typed.starts_with("file://") {
            return if self.request.addresses {
                Step::Picked(Picked::Address(typed))
            } else {
                Step::Say(said(key::FILE_PICKER_NO_ADDRESS, &[]))
            };
        }
        let path = if self.is_a_path() {
            self.expanded(&typed)
        } else {
            self.at.join(&typed)
        };
        match self.request.mode {
            Mode::Folder if path.is_dir() => self.picked(vec![path], how),
            Mode::Open { .. } if path.is_file() => self.picked(vec![path], how),
            Mode::Open { .. } | Mode::Folder => {
                Step::Say(said(key::FILE_PICKER_NOTHING_THERE, &[("typed", typed)]))
            }
            Mode::Save { .. } => self.save_named(),
        }
    }

    /// `RET` on the name: the file it names in the folder open — after the
    /// question, when one of that name is there.
    fn save_named(&mut self) -> Step {
        let name = self.name_part().trim().to_owned();
        if name.is_empty() {
            return Step::Say(said(key::FILE_PICKER_NO_NAME, &[]));
        }
        // A `/` IN A NAME is a folder it does not say it is: `a/b` would
        // write into a folder `a` that the person never walked to, and may
        // not be there. A path is typed from `/` or `~`.
        if name == "." || name == ".." || name.contains(['/', '\0']) {
            return Step::Say(said(key::FILE_PICKER_BAD_NAME, &[("name", name)]));
        }
        let file = match self.extension_added() {
            Some(ext) => format!("{name}.{ext}"),
            None => name,
        };
        let path = self.at.join(&file);
        if path.is_dir() {
            return Step::Say(said(key::FILE_PICKER_IS_A_FOLDER, &[("name", file)]));
        }
        if path.exists() {
            self.asking = Some((path, false));
            return Step::Changed;
        }
        self.picked(vec![path], How::Normal)
    }

    /// The answer — and the folder it came from remembered for the purpose.
    fn picked(&mut self, paths: Vec<PathBuf>, how: How) -> Step {
        let Some(first) = paths.first() else {
            return Step::Nothing;
        };
        let folder = if self.request.mode == Mode::Folder {
            first.clone()
        } else {
            first
                .parent()
                .map_or_else(|| self.at.clone(), Path::to_path_buf)
        };
        remember(
            self.context.remembers.clone(),
            self.request.purpose,
            &folder,
        );
        Step::Picked(Picked::Paths { paths, how })
    }

    // ── the pointer ──

    /// A click on `target` — found in a door's own rows, which carry the
    /// picker's targets, by the cell under the pointer (`chrome::Across`,
    /// the answer the door's hover lights with): the cursor comes to it.
    /// `true` when it was ALREADY there — the second click, which the door
    /// turns into `RET` (invariant 19).
    pub fn click_target(&mut self, target: Target) -> bool {
        if target == Target::Nothing {
            return false;
        }
        if let (Target::Answer(yes), Some((path, now))) = (&target, self.asking.clone()) {
            self.asking = Some((path, *yes));
            return *yes == now;
        }
        let again = self.cursor == target;
        self.cursor = target;
        self.moved = true;
        again
    }

    /// A click with `Ctrl` held on `target`, found in a door's own rows — the
    /// file there marked, or no longer — or with `Shift` — every file from
    /// the cursor to it marked. `true` when that changed the rows.
    pub fn mark_target(&mut self, target: Target, range: bool) -> bool {
        if self.request.mode != (Mode::Open { many: true }) || self.asking.is_some() {
            return false;
        }
        let Target::File(p) = target else {
            return false;
        };
        if range {
            let files: Vec<PathBuf> = self
                .items()
                .into_iter()
                .filter_map(|i| match i.target {
                    Target::File(f) => Some(f),
                    _ => None,
                })
                .collect();
            let to = files.iter().position(|f| *f == p);
            let from = match &self.cursor {
                Target::File(c) => files.iter().position(|f| f == c),
                _ => None,
            }
            .or(to);
            if let (Some(a), Some(b)) = (from, to) {
                for f in &files[a.min(b)..=a.max(b)] {
                    if !self.marks.contains(f) {
                        self.marks.push(f.clone());
                    }
                }
            }
        } else {
            self.toggle_mark(&p);
        }
        self.cursor = Target::File(p);
        self.moved = true;
        true
    }

    // ── the hint ──

    /// The keys that work here.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, id, w)| self.applies(*w, id))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// Whether a key of the table applies now.
    fn applies(&self, w: Where, id: &str) -> bool {
        if self.asking.is_some() {
            return matches!(
                id,
                "file-picker-down" | "file-picker-up" | "file-picker-enter"
            );
        }
        match w {
            Where::Anywhere => true,
            Where::OnAGrid => {
                self.context.grid && {
                    let items = self.items();
                    let (_, room) = crate::power::extent(self.drawn.get());
                    self.cursor_in(&items).is_some_and(|at| {
                        let section = items[at].section;
                        let of: Vec<&Item> =
                            items.iter().filter(|i| i.section == section).collect();
                        section != Section::Top && self.columns_of(&of, room) > 1
                    })
                }
            }
            Where::Typed => !self.query.is_empty(),
            Where::Untyped => self.query.is_empty() && self.at.parent().is_some(),
            Where::WithFilters => {
                self.request.filters.len() > 1 && self.request.mode != Mode::Folder
            }
            Where::Many => self.request.mode == (Mode::Open { many: true }),
            Where::PlaysNow => self.request.alternate.is_some_and(|a| a.id() == id),
            Where::Extra => self.request.extras.iter().any(|e| e.id == id),
        }
    }
}

// ─── hosted over a view's own rows ─────────────────────────────────────────

/// A picker a VIEW hosts over its own rows — the programs', Syncthing's,
/// the network's, the camera's — and what it was opened for: the view draws
/// [`Hosted::rows`] in place of its own while it is up, hands it every key it
/// claims, and does [`Hosting`] with what comes back. Both doors of the view
/// host the same one, so what one door picks the other picks.
#[derive(Debug, Clone)]
pub struct Hosted<F> {
    /// The picker.
    pub picker: FilePicker,
    /// What the view opened it for.
    pub asked: F,
    /// The key of the title its rows go under.
    pub title: &'static str,
}

/// What a hosted picker's step asks of the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hosting<F> {
    /// Draw again.
    Nothing,
    /// Back, with no answer: the view's own rows come back.
    Back,
    /// A sentence to say.
    Say(Explanation),
    /// `C-v`: the door reads the clipboard for [`Hosted::pasted`].
    Paste,
    /// The answer, and what it was asked for.
    Picked(F, Picked),
}

impl<F: Clone> Hosted<F> {
    /// A picker for `request`, opened for `asked`, under `title` — its folder
    /// watched, and the devices read, through `wake` when the door has a
    /// loop ([`FilePicker::watch_with`]); the door then calls [`Self::look`].
    #[must_use]
    pub fn open(
        request: Request,
        asked: F,
        title: &'static str,
        wake: Option<&crate::player_subtitles::Wake>,
    ) -> Self {
        let mut picker = FilePicker::open(request, &Context::of_this_machine(false));
        if let Some(wake) = wake {
            picker.watch_with(wake);
        }
        Self {
            picker,
            asked,
            title,
        }
    }

    /// What the wake brought, taken: `true` when the rows changed.
    pub fn look(&mut self) -> bool {
        self.picker.look()
    }

    /// What a step is to the view.
    #[must_use]
    pub fn hosting(&self, step: Step) -> Hosting<F> {
        match step {
            Step::Nothing | Step::Changed => Hosting::Nothing,
            Step::Back => Hosting::Back,
            Step::Say(e) => Hosting::Say(e),
            Step::Paste => Hosting::Paste,
            Step::Picked(p) => Hosting::Picked(self.asked.clone(), p),
        }
    }

    /// A key the door read — [`FilePicker::claims`] first.
    pub fn key(&mut self, key: Key) -> Hosting<F> {
        let step = self.picker.key(key);
        self.hosting(step)
    }

    /// What was copied.
    pub fn pasted(&mut self, text: &str) -> Hosting<F> {
        let step = self.picker.pasted(text);
        self.hosting(step)
    }

    /// The rows, under the title, as the view's own rows — `make` turns each.
    #[must_use]
    pub fn rows<R>(
        &self,
        cols: u16,
        texts: &ronoctua_i18n::Texts,
        make: impl Fn(Row) -> R,
    ) -> Vec<R> {
        let (left, _) = crate::power::extent(cols);
        let mut out = vec![make(inert(vec![piece(
            left,
            texts.t(self.title),
            tokens::color::TEXT_HEADING,
        )]))];
        out.extend(self.picker.rows(cols, texts).into_iter().map(make));
        out
    }

    /// A click on the drawn row whose target is `target`: `true` for the
    /// second, which the door turns into `RET`.
    pub fn clicked(&mut self, target: Target) -> bool {
        self.picker.click_target(target)
    }
}

/// The cells of a grid entry's glyph: the glyph and a cell of air.
const GLYPH_CELLS: u16 = tokens::space::PLAYER_QUEUEGLYPH;

/// The air between two cells of a grid, and before a size at the edge.
const GAP_CELLS: u16 = tokens::space::PLAYER_QUEUEGLYPH;

/// How many of `count` a column of a grid of `columns` holds — the grid is
/// filled down each column, so entry `i` is on row `i % per`, in column
/// `i / per`.
fn down_a_column(count: usize, columns: u16) -> usize {
    count.div_ceil(usize::from(columns.max(1))).max(1)
}

/// The deepest folder `p` names that is there.
fn deepest(p: &Path) -> PathBuf {
    let mut at = p.to_path_buf();
    while !at.is_dir() {
        match at.parent() {
            Some(up) => at = up.to_path_buf(),
            None => return PathBuf::from("/"),
        }
    }
    at
}

/// A path's last part, or the whole path for the root.
fn name_of(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// `%20` and its kin, back to the bytes they stand for.
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(Some(hi)), Some(Some(lo))) = (
                bytes.get(i + 1).map(|b| hex(*b)),
                bytes.get(i + 2).map(|b| hex(*b)),
            )
        {
            out.push(u8::try_from(hi * 16 + lo).unwrap_or(b'?'));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A sentence for [`Step::Say`].
fn said(key: &'static str, args: &[(&'static str, String)]) -> Explanation {
    Explanation {
        key,
        args: args.to_vec(),
    }
}

/// A field at `at`.
const fn piece(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        ink: Ink::Grid,
        at,
        text,
        color,
        hit: Hit::Nothing,
        explain: None,
    }
}

/// A row the cursor skips.
fn inert(fields: Vec<Field>) -> Row {
    Row {
        fields,
        cells: Vec::new(),
        heading: true,
        focused: false,
        target: Target::Nothing,
        spot: String::new(),
        caret: None,
        continues: false,
    }
}

/// A row of a pair.
fn pair_row(fields: Vec<Field>, target: Target, focused: bool) -> Row {
    Row {
        fields,
        cells: Vec::new(),
        heading: false,
        focused,
        spot: target.spot(),
        target,
        caret: None,
        continues: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn key(notation: &str) -> Key {
        Key::parse(notation).expect("a key")
    }

    /// A tree of its own, under the lane's scratch space when there is one.
    fn tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-picker-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        for f in [
            "Music/a.flac",
            "Music/B.mp3",
            "Music/cover.jpg",
            "Music/notes.txt",
            "Music/.hidden.flac",
            "Music/Album/01.ogg",
            "Videos/film.mkv",
            "Pictures/sea.png",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        root
    }

    fn request(mode: Mode, filters: &'static [Filter]) -> Request {
        Request {
            purpose: "test",
            mode,
            filters,
            start: Start::Place(Xdg::MUSIC),
            addresses: false,
            extras: &[],
            alternate: None,
        }
    }

    fn names(p: &FilePicker) -> Vec<String> {
        p.items()
            .into_iter()
            .filter(|i| i.section == Section::Listing)
            .map(|i| i.name)
            .collect()
    }

    /// FAILS IF: the folder lists what its filter leaves out, a hidden file
    /// without being asked, or anything before `..`.
    #[test]
    fn a_folder_lists_its_folders_then_what_the_filter_keeps() {
        let root = tree("filter");
        let p = FilePicker::open(
            request(Mode::Open { many: false }, PICTURES),
            &Context::in_home(&root, true),
        );
        assert_eq!(p.folder(), root.join("Music"));
        assert_eq!(names(&p), ["..", "Album", "cover.jpg"]);
        let mut p = p;
        assert_eq!(p.key(key("TAB")), Step::Changed);
        assert_eq!(
            names(&p),
            ["..", "Album", "a.flac", "B.mp3", "cover.jpg", "notes.txt"]
        );
        p.key(key("C-h"));
        assert!(names(&p).contains(&".hidden.flac".to_owned()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: typing does not narrow the folder, a path typed does not
    /// walk to the folder it names, or the cursor does not land on the first
    /// match.
    #[test]
    fn typing_narrows_and_a_path_walks() {
        let root = tree("typing");
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        p.typed("fla");
        assert_eq!(names(&p), ["..", "a.flac"]);
        assert_eq!(p.cursor, Target::File(root.join("Music/a.flac")));
        p.key(key("ESC"));
        assert!(p.typed_text().is_empty());
        p.typed("~/Vid");
        assert_eq!(p.folder(), root);
        assert_eq!(names(&p), ["..", "Videos"]);
        p.typed("eos/fi");
        assert_eq!(p.folder(), root.join("Videos"));
        assert_eq!(names(&p), ["..", "film.mkv"]);
        assert_eq!(
            p.key(key("RET")),
            Step::Picked(Picked::Paths {
                paths: vec![root.join("Videos/film.mkv")],
                how: How::Normal
            })
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a pasted `file://` address is not the path it names, or a
    /// typed web address is taken where the request takes none.
    #[test]
    fn a_pasted_address_and_a_typed_one() {
        let root = tree("paste");
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        let url = format!("file://{}/Pictures/sea%2Epng", root.display());
        assert_eq!(p.pasted(&url), Step::Changed);
        assert_eq!(p.folder(), root.join("Pictures"));
        assert_eq!(names(&p), ["..", "sea.png"]);
        p.key(key("ESC"));
        p.typed("https://example.org/a.flac");
        p.cursor = Target::Typed;
        assert!(matches!(p.key(key("RET")), Step::Say(_)));
        let mut r = request(Mode::Open { many: false }, EVERY_FILE);
        r.addresses = true;
        let mut p = FilePicker::open(r, &Context::in_home(&root, false));
        p.typed("https://example.org/a.flac");
        assert_eq!(
            p.key(key("RET")),
            Step::Picked(Picked::Address("https://example.org/a.flac".into()))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: marking takes one file where several were marked, or
    /// `C-SPC` marks where one file is asked for.
    #[test]
    fn several_are_marked_by_name_and_taken_together() {
        let root = tree("marks");
        let mut p = FilePicker::open(
            request(Mode::Open { many: true }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        p.cursor = Target::File(root.join("Music/a.flac"));
        p.key(key("C-SPC"));
        assert_eq!(p.cursor, Target::File(root.join("Music/B.mp3")));
        p.key(key("C-SPC"));
        assert_eq!(p.marks.len(), 2);
        p.cursor = Target::Take;
        assert_eq!(
            p.key(key("RET")),
            Step::Picked(Picked::Paths {
                paths: vec![root.join("Music/a.flac"), root.join("Music/B.mp3")],
                how: How::Normal
            })
        );
        let mut one = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        one.cursor = Target::File(root.join("Music/a.flac"));
        assert_eq!(one.key(key("C-SPC")), Step::Nothing);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the filter's chosen value is cut in a narrow door
    /// (`What the playe…`) instead of wrapped under itself, a line of it is
    /// a row the cursor stops on, or a line leaves the filter's target.
    #[test]
    fn a_chosen_filter_wraps_and_is_never_cut() {
        let root = tree("filter-narrow");
        let p = FilePicker::open(
            request(Mode::Open { many: false }, PICTURES),
            &Context::in_home(&root, false),
        );
        let t = texts();
        let shown = t.t(p.current_filter().expect("a filter").label);
        let mut wrapped = false;
        for cols in 12..60 {
            let rows: Vec<Row> = p
                .rows(cols, &t)
                .into_iter()
                .filter(|r| r.target == Target::Filter)
                .collect();
            assert!(!rows.is_empty() && !rows[0].continues, "{cols}: {rows:?}");
            assert!(rows[1..].iter().all(|r| r.continues), "{cols}: {rows:?}");
            wrapped |= rows.len() > 1;
            let column = rows[0].fields.last().map(|f| f.at).unwrap();
            let value: Vec<&str> = rows
                .iter()
                .flat_map(|r| r.fields.iter().filter(|f| f.at == column))
                .map(|f| f.text.as_str())
                .collect();
            // A WORD WIDER THAN THE ROOM is split across lines, so the
            // letters are compared and not the spaces between the lines.
            assert_eq!(
                value.concat().replace(' ', ""),
                shown.replace(' ', ""),
                "{cols}: the value was cut"
            );
        }
        assert!(wrapped, "no width was narrow enough to wrap `{shown}`");
    }

    /// FAILS IF: a save writes over a file without asking, the question is
    /// not born on its "no", or the extension is not added.
    #[test]
    fn a_save_over_a_file_asks_no_first() {
        let root = tree("save");
        std::fs::write(root.join("Music/night.rset"), b"{}").unwrap();
        let mut p = FilePicker::open(
            request(
                Mode::Save {
                    name: "night".into(),
                    extension: Some("rset"),
                },
                EVERY_FILE,
            ),
            &Context::in_home(&root, false),
        );
        assert_eq!(p.saved_name(), "night.rset");
        assert_eq!(p.key(key("RET")), Step::Changed);
        assert!(p.asking());
        let t = texts();
        let rows = p.rows(60, &t);
        let no = rows
            .iter()
            .position(|r| r.target == Target::Answer(false))
            .unwrap();
        let yes = rows
            .iter()
            .position(|r| r.target == Target::Answer(true))
            .unwrap();
        assert!(no < yes, "no is drawn first");
        assert!(rows[no].focused, "the cursor is born on no");
        assert_eq!(p.key(key("RET")), Step::Changed);
        assert!(!p.asking(), "no leaves the file as it was");
        p.key(key("RET"));
        p.key(key("<down>"));
        assert_eq!(
            p.key(key("RET")),
            Step::Picked(Picked::Paths {
                paths: vec![root.join("Music/night.rset")],
                how: How::Normal
            })
        );
        let mut fresh = FilePicker::open(
            request(
                Mode::Save {
                    name: "day".into(),
                    extension: Some("rset"),
                },
                EVERY_FILE,
            ),
            &Context::in_home(&root, false),
        );
        assert_eq!(
            fresh.key(key("RET")),
            Step::Picked(Picked::Paths {
                paths: vec![root.join("Music/day.rset")],
                how: How::Normal
            })
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the folder picked is not remembered for its purpose, the
    /// write loses another purpose's, or the next picker for it does not open
    /// there.
    #[test]
    fn the_folder_is_remembered_per_purpose_and_merged() {
        let root = tree("remember");
        let config = root.join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join(FILE),
            format!(
                "{{\"folders\":{{\"other\":\"{}\"}}}}",
                root.join("Videos").display()
            ),
        )
        .unwrap();
        let mut context = Context::in_home(&root, false);
        context.remembers = Some(config.clone());
        let mut p = FilePicker::open(request(Mode::Open { many: false }, EVERY_FILE), &context);
        p.go_to(&root.join("Pictures"), None);
        p.cursor = Target::File(root.join("Pictures/sea.png"));
        assert!(matches!(p.key(key("RET")), Step::Picked(_)));
        let kept = store(Some(config)).read();
        assert_eq!(kept.folders.get("test"), Some(&root.join("Pictures")));
        assert_eq!(kept.folders.get("other"), Some(&root.join("Videos")));
        let again = FilePicker::open(request(Mode::Open { many: false }, EVERY_FILE), &context);
        assert_eq!(again.folder(), root.join("Pictures"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a folder is not chosen by the row that says so, or a file
    /// is offered where a folder is asked for.
    #[test]
    fn a_folder_is_chosen_where_it_is_open() {
        let root = tree("folder");
        let mut p = FilePicker::open(
            request(Mode::Folder, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        assert_eq!(names(&p), ["..", "Album"]);
        p.cursor = Target::Here;
        assert_eq!(
            p.key(key("RET")),
            Step::Picked(Picked::Paths {
                paths: vec![root.join("Music")],
                how: How::Normal
            })
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: `DEL` with nothing typed does not go up with the cursor on
    /// the folder just left, or `Esc` with nothing typed is not the way back.
    #[test]
    fn del_goes_up_and_esc_goes_back() {
        let root = tree("up");
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        assert_eq!(p.key(key("DEL")), Step::Changed);
        assert_eq!(p.folder(), root);
        assert_eq!(p.cursor, Target::Folder(root.join("Music")));
        assert_eq!(p.key(key("ESC")), Step::Back);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a file that lands in the folder open is not on screen after
    /// the door is woken — the folder is watched, and no clock asks.
    #[test]
    fn a_file_that_lands_is_on_screen() {
        let root = tree("watch");
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let tx = std::sync::Mutex::new(tx);
        p.watch_with(&crate::player_subtitles::Wake::new(move || {
            let _ = tx.lock().map(|t| t.send(()));
        }));
        // WRITTEN AGAIN until the door is woken: the watch's thread takes
        // the folder a moment after it is asked to, and a file written
        // before that moment was never an event.
        let mut woken = false;
        for n in 0..50 {
            std::fs::write(root.join(format!("Music/new{n}.flac")), b"").unwrap();
            if rx
                .recv_timeout(std::time::Duration::from_millis(400))
                .is_ok()
            {
                woken = true;
                break;
            }
        }
        assert!(woken, "the folder's move wakes the door");
        assert!(p.look());
        assert!(names(&p).iter().any(|n| n.starts_with("new")));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the grid is not filled down each column — `↓` the next
    /// name, `→` the same row of the next column — or `↓` off a section's
    /// last row does not reach the next section.
    #[test]
    fn the_arrows_walk_a_grid_down_its_columns() {
        let root = tree("walk");
        for n in 0..9 {
            std::fs::write(root.join(format!("Music/t{n}.flac")), b"").unwrap();
        }
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, true),
        );
        let t = texts();
        let rows = p.rows(90, &t);
        // DOWN EACH COLUMN: the first cells of consecutive grid rows are
        // consecutive names of the listing.
        let listing: Vec<Target> = p
            .items()
            .into_iter()
            .filter(|i| i.section == Section::Listing)
            .map(|i| i.target)
            .collect();
        let firsts: Vec<Target> = rows
            .iter()
            .filter(|r| r.cells.len() > 1 && r.cells.iter().any(|c| listing.contains(&c.target)))
            .map(|r| r.cells[0].target.clone())
            .collect();
        assert!(firsts.len() > 1, "a grid of several rows: {rows:?}");
        assert_eq!(firsts[..], listing[..firsts.len()], "read down the column");
        let start = p.cursor.clone();
        let at = listing.iter().position(|t| *t == start).unwrap();
        p.key(key("<down>"));
        assert_eq!(p.cursor, listing[at + 1], "down is the next name");
        p.key(key("<up>"));
        assert_eq!(p.cursor, start);
        p.key(key("<right>"));
        let rows = p.rows(90, &t);
        let from = rows
            .iter()
            .position(|r| r.cells.iter().any(|c| c.target == start))
            .unwrap();
        let to = rows.iter().position(|r| r.focused).unwrap();
        assert_eq!(to, from, "right is the same row");
        assert_ne!(p.cursor, start);
        p.key(key("<left>"));
        assert_eq!(p.cursor, start);
        // UP FROM THE LISTING'S FIRST ROW reaches the places above it.
        p.cursor = Target::Up;
        p.key(key("<up>"));
        assert!(matches!(p.cursor, Target::Place(_)), "{:?}", p.cursor);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: `PgDn` walks one name where the door shows a page — half of
    /// what the door said it shows is the step.
    #[test]
    fn a_page_is_half_of_what_the_door_shows() {
        let root = tree("page");
        for n in 0..30 {
            std::fs::write(root.join(format!("Music/t{n:02}.flac")), b"").unwrap();
        }
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        let t = texts();
        let _ = p.rows(60, &t);
        let items = p.items();
        let at = p.cursor_in(&items).unwrap();
        p.page_is(20);
        p.key(key("<next>"));
        assert_eq!(p.cursor_in(&p.items()), Some(at + 10));
        p.key(key("<prior>"));
        assert_eq!(p.cursor_in(&p.items()), Some(at));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: saving takes a name with a `/` — a folder never walked to —
    /// or a file of another kind chosen while saving is written over under
    /// its own name, where the name row shows another.
    #[test]
    fn a_save_takes_a_name_and_not_a_folder() {
        let root = tree("save-name");
        let mut p = FilePicker::open(
            request(
                Mode::Save {
                    name: "a/b".into(),
                    extension: Some("rset"),
                },
                EVERY_FILE,
            ),
            &Context::in_home(&root, false),
        );
        assert!(
            matches!(p.key(key("RET")), Step::Say(e) if e.key == key::FILE_PICKER_BAD_NAME),
            "a slash is refused"
        );
        let flac = root.join("Music/a.flac");
        assert_eq!(p.act(&Target::File(flac), How::Normal), Step::Changed);
        assert!(!p.asking(), "a file of another kind is not written over");
        assert_eq!(p.saved_name(), "a.rset", "its name, the saved kind");
        assert_eq!(
            p.key(key("RET")),
            Step::Picked(Picked::Paths {
                paths: vec![root.join("Music/a.rset")],
                how: How::Normal
            })
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a device read off the loop is not among the places, one
    /// the sidebar already holds is listed twice, or one pulled out while
    /// the picker is up stays among them.
    #[test]
    fn the_devices_mounted_are_places() {
        let root = tree("devices");
        let mut p = FilePicker::open(
            request(Mode::Open { many: false }, EVERY_FILE),
            &Context::in_home(&root, false),
        );
        let stick = Place {
            name: PlaceName::Named("STICK".into()),
            glyph: tokens::icon::DISK_REMOVABLE,
            path: root.join("Music/Album"),
        };
        let again = Place {
            name: PlaceName::Named("Music".into()),
            glyph: tokens::icon::DISK,
            path: root.join("Music"),
        };
        *p.watch.devices.lock().unwrap() = Some(vec![stick.clone(), again]);
        assert!(p.look(), "the devices change the rows");
        let places: Vec<PathBuf> = p.context.places.iter().map(|p| p.path.clone()).collect();
        assert_eq!(
            places.iter().filter(|x| **x == root.join("Music")).count(),
            1,
            "once"
        );
        assert!(places.contains(&stick.path));
        // THE STICK PULLED OUT goes from the places; the device the sidebar
        // holds stays, as the sidebar's.
        *p.watch.devices.lock().unwrap() = Some(Vec::new());
        assert!(p.look(), "a device gone changes the rows");
        assert!(
            !p.context.places.iter().any(|x| x.path == stick.path),
            "the stick stayed"
        );
        assert!(
            p.context
                .places
                .iter()
                .any(|x| x.path == root.join("Music"))
        );
        *p.watch.devices.lock().unwrap() = Some(vec![stick]);
        assert!(p.look(), "plugged in again");
        let t = texts();
        let rows = p.rows(80, &t);
        assert!(
            rows.iter()
                .flat_map(|r| &r.fields)
                .any(|f| f.text.contains("STICK")),
            "drawn among the places"
        );
        assert!(!p.look(), "taken once");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a key the hint teaches is not one the picker reads, or one
    /// it reads for another thing — every row of the table parses and is
    /// claimed.
    #[test]
    fn every_key_taught_is_a_key_read() {
        let extras = [PLAY_FOLDER, ENQUEUE_FOLDER];
        for (notation, id, _) in OF_THE_VIEW {
            let k = Key::parse(notation).unwrap_or_else(|_| panic!("{notation} parses"));
            assert!(read(k, &extras).is_some(), "{id}: {notation} is read");
        }
        assert_eq!(
            read(key("C-q"), &extras),
            None,
            "a key it does not read is the door's"
        );
    }

    /// FAILS IF: a row of the picker says a word no key holds in a language.
    #[test]
    fn every_word_is_a_key_in_both_languages() {
        let root = tree("words");
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let mut r = request(Mode::Open { many: true }, PICTURES);
            r.extras = &[PLAY_FOLDER, ENQUEUE_FOLDER];
            r.alternate = Some(Alternate::PlayNow);
            let mut p = FilePicker::open(r, &Context::in_home(&root, true));
            p.marks.push(root.join("Music/cover.jpg"));
            let hint = p.hint(&t);
            for rows in [p.rows(80, &t), {
                p.typed("zzz");
                p.rows(80, &t)
            }] {
                for f in rows.iter().flat_map(|r| &r.fields) {
                    assert!(!f.text.contains('['), "{language}: {}", f.text);
                }
            }
            for piece in hint {
                assert!(!piece.text.contains('['), "{language}: {}", piece.text);
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
