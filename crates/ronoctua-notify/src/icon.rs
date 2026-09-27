//! Icon resolution chain, in three steps — section 4.2.
//!
//! Invariant 10 is the reason the order is this one: **UI icons are glyphs
//! from the font**, and the superpackage is only a pixmap fallback for foreign
//! apps. An app that has an entry in the table never reaches the superpackage,
//! and no design system component consults it.
//!
//! ```text
//! 1. app id -> glyph token table       coherent with the rest of the system
//! 2. superpackage, by IconName         the app has its own icon and knows the name
//! 3. pixmap embedded in the message    last resort
//! ```
//!
//! When none of the three resolves, out comes [`Icon::Generic`] — never "nothing".
//! A tray item with no icon is an invisible item, and the user would have no way
//! to know it exists.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// What to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    /// Step 1: a glyph from the system font. The preferred case.
    Glyph(char),
    /// Step 2: a file from the superpackage.
    Pixmap(PathBuf),
    /// Step 3: bytes that came in the notification itself.
    Builtin(Vec<u8>),
    /// Node step resolved.
    Generic,
}

impl Icon {
    /// Which step of the chain produced this. Used by the tests and by the log.
    #[must_use]
    pub const fn stage(&self) -> u8 {
        match self {
            Self::Glyph(_) => 1,
            Self::Pixmap(_) => 2,
            Self::Builtin(_) => 3,
            Self::Generic => 0,
        }
    }
}

/// Where the chain takes each step from.
///
/// The first two are borrowed because they live in the daemon and do not change per
/// lookup; the third comes as a parameter because it belongs to the message.
pub struct Chain<'a> {
    /// Step 1: `app id` -> glyph, already resolved from token to `char`.
    ///
    /// It comes from the `assets/icons/tray-glyphs.nix` table merged with the
    /// user's override. The merge happens OUTSIDE here: this function must
    /// not know two layers exist, otherwise hot-reloading layer 2
    /// would require touching it.
    pub glyphs: &'a BTreeMap<String, char>,
    /// Step 2: the root of the icon superpackage (`share/icons/noctua`).
    pub superpack: Option<&'a std::path::Path>,
    /// The generic glyph, when nothing resolves. It comes from a token.
    pub generic: Option<char>,
}

impl Chain<'_> {
    /// STEP 1 on its own — the only one that does not touch the disk.
    ///
    /// # Why it is separate
    ///
    /// Because whoever is in a hurry can only do this one. The app
    /// launcher draws the whole list before any icon exists, and
    /// step 2 walks theme directories: in a list of four hundred items
    /// that is thousands of `stat` calls inside a frame's budget. It
    /// calls this one on the spot and sends step 2 to a thread.
    ///
    /// `app_id` matches exactly and then by the LAST component: an app that
    /// announces itself as `org.mozilla.firefox` should pick up the `firefox` entry without
    /// the table needing to list every form.
    #[must_use]
    pub fn glyph(&self, app_id: &str) -> Option<char> {
        if let Some(g) = self.glyphs.get(app_id) {
            return Some(*g);
        }
        let short = app_id.rsplit('.').next()?;
        if short == app_id {
            return None;
        }
        self.glyphs.get(short).copied()
    }

    /// Resolves, in this order: table, superpackage, embedded pixmap.
    #[must_use]
    pub fn resolve(
        &self,
        app_id: &str,
        icon_name: Option<&str>,
        embedded: Option<Vec<u8>>,
    ) -> Icon {
        // Step 1.
        if let Some(g) = self.glyph(app_id) {
            return Icon::Glyph(g);
        }

        // Step 2. An `IconName` that is an absolute path is used directly — the
        // specification allows both forms in the same field.
        if let Some(name) = icon_name.filter(|n| !n.is_empty()) {
            let p = PathBuf::from(name);
            if p.is_absolute() && p.exists() {
                return Icon::Pixmap(p);
            }
            if let Some(root) = self.superpack
                && let Some(found_at) = look_up(root, name)
            {
                return Icon::Pixmap(found_at);
            }
        }

        // Step 3.
        if let Some(bytes) = embedded.filter(|b| !b.is_empty()) {
            return Icon::Builtin(bytes);
        }

        match self.generic {
            Some(g) => Icon::Glyph(g),
            None => Icon::Generic,
        }
    }
}

/// Pixels ready for the GPU.
///
/// Declared HERE, and not imported from `ronoctua-render`: what this module
/// returns is pixels, and pixels are not the GPU. It is the same arrangement — and the same reason —
/// as `ronoctua_clipboard::image`, which decodes without depending on the renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes, RGBA8.
    pub pixels: Vec<u8>,
}

/// Where an icon NAME is looked for, in the order it is looked for.
///
/// # Why three kinds of root, and why this order
///
/// It is the freedesktop icon lookup, flattened to what this system has: the
/// CURRENT THEME first, then `hicolor` — the theme every program installs its
/// own icon into, and the one every theme inherits — then the old `pixmaps`
/// directories, which the specification keeps as the last resort.
///
/// The current theme is the superpackage, so it keeps the first word: an app
/// whose icon the superpackage redraws is drawn in the system's style, which is
/// the reason the superpackage exists. What it did NOT have was the rest of the
/// chain. A name was looked up in the superpackage and nowhere else, so every
/// program the person installed themselves — a Flatpak's export, a package in
/// `~/.nix-profile`, an AppImage that put its icon in `~/.local/share/icons` —
/// came out with the category glyph, and a whole list of category glyphs looks
/// deliberate.
///
/// Plain data, with public fields: whoever has other roots (a test, a probe
/// against a store path) builds one by hand, and [`Roots::of_session`] is the
/// one place that reads the environment for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots {
    /// The current theme's root — the superpackage, `share/icons/noctua`.
    /// `None` on a machine that did not install it (the minimal profile),
    /// which then goes straight to `hicolor`.
    pub theme: Option<PathBuf>,
    /// Every `icons/hicolor` directory, the person's own first.
    pub hicolor: Vec<PathBuf>,
    /// Every `pixmaps` directory, where a file sits with no size and no
    /// context around it.
    pub pixmaps: Vec<PathBuf>,
}

impl Roots {
    /// The session's roots, around the given superpackage.
    ///
    /// Reads `XDG_DATA_HOME`, `HOME` and `XDG_DATA_DIRS` — see [`Roots::of`],
    /// which is the same answer from values handed in.
    #[must_use]
    pub fn of_session(superpack: Option<PathBuf>) -> Self {
        let data_dirs = std::env::var("XDG_DATA_DIRS").ok();
        Self::of(
            superpack,
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
            data_dirs.as_deref(),
        )
    }

    /// The roots, from the environment's values handed in.
    ///
    /// # What `XDG_DATA_DIRS` brings, and why that is enough
    ///
    /// On this system it is built from the profiles NixOS knows about, so the
    /// one variable covers every place a program the person installed can have
    /// put its icon: `~/.nix-profile/share`, the per-user profile, the system's
    /// `sw/share` — and, once Flatpak is on, its two `exports/share`. Listing
    /// those here by name would be a second copy of a list the system already
    /// keeps, and the copy is the one that goes stale.
    ///
    /// `XDG_DATA_HOME` (or `~/.local/share`) comes first and `~/.icons` right
    /// after it — the person's own before anything installed — and a directory
    /// named twice is searched once. When `XDG_DATA_DIRS` is unset or empty the
    /// specification's own default stands, as `ronoctua_apps::directories`
    /// already does for the applications.
    #[must_use]
    pub fn of(
        superpack: Option<PathBuf>,
        data_home: Option<PathBuf>,
        home: Option<PathBuf>,
        data_dirs: Option<&str>,
    ) -> Self {
        let data_home = data_home.or_else(|| home.as_ref().map(|h| h.join(".local/share")));
        let data_dirs: Vec<PathBuf> = data_dirs
            .filter(|d| !d.is_empty())
            .unwrap_or("/usr/local/share:/usr/share")
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .collect();

        // A directory named twice is searched ONCE: the variable is assembled
        // from several places that can name the same one, and the walk of a
        // name that is not there costs a few hundred `stat`s per root.
        fn once(list: &mut Vec<PathBuf>, p: PathBuf) {
            if !list.contains(&p) {
                list.push(p);
            }
        }
        let mut hicolor = Vec::new();
        if let Some(d) = &data_home {
            once(&mut hicolor, d.join("icons/hicolor"));
        }
        if let Some(h) = &home {
            once(&mut hicolor, h.join(".icons/hicolor"));
        }
        let mut pixmaps = Vec::new();
        for d in &data_dirs {
            once(&mut hicolor, d.join("icons/hicolor"));
            once(&mut pixmaps, d.join("pixmaps"));
        }
        Self {
            theme: superpack,
            hicolor,
            pixmaps,
        }
    }

    /// Where `icon`'s file is, walking the chain in order.
    ///
    /// **It touches the disk**, like [`look_up`]: call it on a thread.
    #[must_use]
    pub fn find(&self, icon: &str) -> Option<PathBuf> {
        if icon.is_empty() {
            return None;
        }
        // AN ABSOLUTE PATH is used directly: the spec allows both forms in the
        // same field, and a hand-installed app's `.desktop` usually uses this one.
        let path = std::path::Path::new(icon);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_path_buf());
        }
        // A NAME IS ONE COMPONENT. An `Icon=` is somebody else's text, and a
        // relative one with a `/` in it would be joined onto every root —
        // `../../` included — and climb out of the directories this walks.
        if icon.contains('/') {
            return None;
        }
        // A NAME WITH AN EXTENSION is out of the specification and common in
        // the wild (`Icon=foo.png`). A theme holds `foo.png` under the name
        // `foo`, so that is the name a theme is asked for; the pixmaps
        // directories are asked for the file as written first, because that is
        // where such an entry expects it to be.
        let name = without_image_extension(icon);
        self.theme
            .iter()
            .chain(&self.hicolor)
            .find_map(|root| look_up(root, name))
            .or_else(|| {
                self.pixmaps
                    .iter()
                    .find_map(|dir| in_pixmaps(dir, icon, name))
            })
    }
}

/// The extensions an `Icon=` sometimes carries, against the specification.
const IMAGE_EXTENSIONS: [&str; 3] = [".png", ".svg", ".xpm"];

/// `foo.png` → `foo`; anything else as it is.
fn without_image_extension(icon: &str) -> &str {
    IMAGE_EXTENSIONS
        .iter()
        .find_map(|e| icon.strip_suffix(e))
        .filter(|n| !n.is_empty())
        .unwrap_or(icon)
}

/// A `pixmaps` directory's file for this icon: the name as written, then with
/// the two extensions this module can draw.
fn in_pixmaps(dir: &std::path::Path, icon: &str, name: &str) -> Option<PathBuf> {
    std::iter::once(dir.join(icon))
        // The file as written only when it is one this module can DRAW: an
        // `.xpm` found first would be an undecodable answer standing in front
        // of a `.png` of the same name one line below.
        .filter(|_| icon != name && !icon.ends_with(".xpm"))
        .chain(["svg", "png"].map(|ext| dir.join(format!("{name}.{ext}"))))
        .find(|p| p.is_file())
}

/// Finds `icon`'s file in the superpackage ALONE and rasterises it into a
/// square of `side` px.
///
/// The chain's first link, kept with its old signature because a probe calls
/// it with nothing else to hand: `ronoctua-test-icons` proves the superpackage
/// is REACHED, and a fallback behind it would let the probe pass on an icon
/// that came from somewhere else. Everything that draws a program's icon for a
/// person goes through [`rasterize_in`] with [`Roots::of_session`].
///
/// # This TOUCHES THE DISK
///
/// See [`rasterize_in`].
#[must_use]
pub fn rasterize(root: Option<&std::path::Path>, icon: &str, side: u32) -> Option<Rgba> {
    rasterize_in(
        &Roots {
            theme: root.map(std::path::Path::to_path_buf),
            ..Roots::default()
        },
        icon,
        side,
    )
}

/// Finds `icon`'s file along the chain and rasterises it into a square of
/// `side` px.
///
/// # This TOUCHES THE DISK
///
/// It walks theme directories and decodes an image. The caller runs on a thread:
/// in the app launcher that is twelve icons per page out of a superpackage
/// of 335,031 files, and none of that fits in a frame's budget.
///
/// `None` when there is no file for this name — and that is an ANSWER, which the
/// caller keeps so as not to ask again.
#[must_use]
pub fn rasterize_in(roots: &Roots, icon: &str, side: u32) -> Option<Rgba> {
    if icon.is_empty() || side == 0 {
        return None;
    }
    let target = roots.find(icon)?;
    let bytes = std::fs::read(&target).ok()?;
    // THE BYTES DECIDE, and the extension only breaks a tie. An absolute
    // `Icon=` is whatever file somebody pointed at: a `.png` that is an ICO,
    // an SVG saved without its extension. Every raster format starts with a
    // binary signature and never with `<`, so a document that opens like
    // markup is the vector, and everything else goes to the raster decoders,
    // which read the signature themselves.
    if target.extension().is_some_and(|e| e == "svg") || looks_like_svg(&bytes) {
        // FILL: here the file's size means nothing — the same
        // theme publishes 16 px and 512 px `.svg` files — and what matters is filling the
        // cell the row reserved.
        return from_svg(&bytes, side, Fit::Fill);
    }
    from_raster(&bytes, side)
}

/// Does this file open like an XML document?
///
/// A byte-order mark and blank space may come first; after them an SVG — like
/// any XML — starts with `<`, and no raster signature does.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let rest = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    rest.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b'<')
}

/// What to do when the drawing is smaller than the square asked for.
///
/// The choice exists because the two right answers are opposites, and confusing them
/// cost the Lucide preview: it said "24 px" and drew the same as
/// "512 px", because the drawing was stretched to the edge in both cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// It grows to the edge.
    ///
    /// For an **application icon**: the file's intrinsic size means
    /// nothing there — the same theme publishes 16 px and 512 px `.svg` files — and what
    /// matters is filling the cell the row reserved.
    Fill,
    /// It respects the document's size, shrinking **only** if it does not fit.
    ///
    /// For whoever is **choosing** the size: the Lucide preview says "24 px"
    /// and has to draw 24 px, otherwise the adjustment has no visible effect at all.
    Natural,
}

/// An SVG rasterised, at the final size.
///
/// # Why rasterise at the size, and not shrink afterwards
///
/// Because it is a VECTOR: rasterising at 512 px and shrinking to 18 blurs the thin
/// lines the shrinking has no way to reconstruct. An application icon at 18
/// px is almost nothing but thin lines — that is where the difference shows.
///
/// `side` is the **ceiling**: in [`Fit::Fill`] the result is always
/// `side × side`; in [`Fit::Natural`] it is the document's size, and it only
/// reaches `side` when the document is bigger than that.
#[must_use]
pub fn from_svg(bytes: &[u8], side: u32, fit: Fit) -> Option<Rgba> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options).ok()?;
    let sz = tree.size();
    if sz.width() <= 0.0 || sz.height() <= 0.0 || side == 0 {
        return None;
    }
    // NO STRETCHING, like the clipboard's preview: a deformed icon makes whoever
    // is looking doubt what they are seeing. The scale is the smaller of the two, and what
    // is left over becomes transparency — which the pipeline's alpha lets through.
    let fits = (side as f32 / sz.width()).min(side as f32 / sz.height());
    let scale = match fit {
        Fit::Fill => fits,
        // NEVER ENLARGE. A 24 px document enlarged to fill the box is
        // the same image as a 512 px one — and that was exactly the defect.
        Fit::Natural => fits.min(1.0),
    };
    let round_it = |v: f32| (v.round() as u32).max(1);
    let (width, height) = match fit {
        Fit::Fill => (side, side),
        Fit::Natural => (round_it(sz.width() * scale), round_it(sz.height() * scale)),
    };
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let dx = (width as f32 - sz.width() * scale) / 2.0;
    let dy = (height as f32 - sz.height() * scale) / 2.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_translate(dx, dy).pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(Rgba {
        width,
        height,
        // `tiny-skia` delivers **premultiplied** RGBA, and the pipeline
        // expects straight: without undoing it, every semitransparent pixel comes out too
        // dark — and the symptom is "the icons look dirty", which nobody connects
        // to alpha multiplication.
        pixels: pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let d = p.demultiply();
                [d.red(), d.green(), d.blue(), d.alpha()]
            })
            .collect(),
    })
}

/// A raster image — PNG, JPEG, ICO, or any format the workspace's decoders
/// read — shrunk to the final size.
///
/// # The format comes from the SIGNATURE, never from the name
///
/// The first bytes are asked (`image::guess_format`), because the name is
/// the part that lies: a favicon saved as `icon.png`, a `.desktop` pointing at
/// whatever file a program shipped. A signature no decoder here knows is `None`
/// — an answer, like a file that is not there.
///
/// An ICO is a directory of pictures, and the decoder hands back the one with
/// the most colour depth and, among those, the LARGEST — the right one for a
/// reduction.
#[must_use]
pub fn from_raster(bytes: &[u8], side: u32) -> Option<Rgba> {
    let format = image::guess_format(bytes).ok()?;
    let img = image::load_from_memory_with_format(bytes, format).ok()?;
    // `Lanczos3` and not `Nearest`: the reduction is large — 512 px to 18 — and a
    // nearest-neighbour resampler loses the icon's thin lines
    // entirely, leaving a blur of blocks.
    let scaled = img.resize(side, side, image::imageops::FilterType::Lanczos3);
    let rgba = scaled.to_rgba8();
    Some(Rgba {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

/// Looks for `name.{svg,png}` in one theme's root — the superpackage, or a
/// `hicolor`.
///
/// It walks the sizes in DECREASING order: the footer draws small, and
/// shrinking a large icon comes out better than enlarging a small one.
///
/// **It touches the disk**, and that is why it is public: the app launcher
/// calls it from a thread, far from the event loop. See [`Chain::glyph`].
#[must_use]
pub fn look_up(root: &std::path::Path, name: &str) -> Option<PathBuf> {
    let mut sizes: Vec<(SizeRank, PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|p| {
            (
                size_rank(&p.file_name().unwrap_or_default().to_string_lossy()),
                p,
            )
        })
        .collect();
    // A stable, decreasing order. Without this, the choice would depend on the filesystem's
    // order and would change between machines. See `size_rank` for why it is
    // not the NAMES that are sorted.
    sizes.sort();

    for (_, dir) in sizes {
        for ext in ["svg", "png"] {
            let target = dir.join(format!("{name}.{ext}"));
            if target.exists() {
                return Some(target);
            }
            // freedesktop themes put the files in a context
            // subdirectory (`apps/`, `places/`, …).
            let Ok(contexts) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut ctxs: Vec<_> = contexts
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            ctxs.sort();
            for ctx in ctxs {
                let target = ctx.join(format!("{name}.{ext}"));
                if target.exists() {
                    return Some(target);
                }
            }
        }
    }
    None
}

/// Where a theme's size directory goes in the walk; lower is earlier.
///
/// `(kind, larger first, name)`: `scalable` before any size, then every
/// `NxN` or `NxN@S` by the PIXELS it holds (`N × S`) from the most, then
/// whatever a theme names otherwise (`symbolic`), by name.
type SizeRank = (u8, std::cmp::Reverse<u32>, String);

/// The rank of one size directory, from its name.
///
/// # Why not the names, sorted
///
/// Because a name is text and a size is a number. Sorted as text and reversed,
/// the walk went `scalable`, `96x96`, `8x8`, `84x84`, `64x64@2` — the
/// superpackage has all five —, so a name drawn only in PNG was taken from the EIGHT-pixel
/// file and enlarged, while an eighty-four-pixel one sat in the next folder.
/// The docstring above always promised the largest; the sort only kept that
/// promise while every size had the same number of digits.
///
/// And `@S` counts: `16x16@3` holds a 48-pixel picture, which is what a
/// reduction cares about, and it ties with `48x48`.
fn size_rank(dir: &str) -> SizeRank {
    if dir == "scalable" {
        return (0, std::cmp::Reverse(u32::MAX), dir.to_owned());
    }
    let pixels = || -> Option<u32> {
        let (size, scale) = dir.split_once('@').unwrap_or((dir, "1"));
        let (w, h) = size.split_once('x')?;
        let (w, h, s) = (
            w.parse::<u32>().ok()?,
            h.parse::<u32>().ok()?,
            scale.parse::<u32>().ok()?,
        );
        (w == h && s > 0).then(|| w.saturating_mul(s))
    };
    match pixels() {
        Some(px) => (1, std::cmp::Reverse(px), dir.to_owned()),
        None => (2, std::cmp::Reverse(0), dir.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> BTreeMap<String, char> {
        BTreeMap::from([("firefox".to_owned(), '\u{f269}')])
    }

    #[test]
    fn stage_1_the_table_wins() {
        // Invariant 10 puts the glyph first: it is what keeps the visual
        // coherence. An app with an entry in the table NEVER reaches the superpackage.
        let t = table();
        let c = Chain {
            glyphs: &t,
            superpack: None,
            generic: None,
        };
        let i = c.resolve("firefox", Some("firefox"), Some(vec![1, 2, 3]));
        assert_eq!(i.stage(), 1);
        assert_eq!(i, Icon::Glyph('\u{f269}'));
    }

    #[test]
    fn the_app_id_matches_by_its_last_component() {
        // `org.mozilla.firefox` has to find `firefox` without the table needing to
        // list every name variation out there.
        let t = table();
        let c = Chain {
            glyphs: &t,
            superpack: None,
            generic: None,
        };
        assert_eq!(c.resolve("org.mozilla.firefox", None, None).stage(), 1);
    }

    #[test]
    fn stage_2_the_superpack() {
        let dir = std::env::temp_dir().join("ronoctua-test-icon-2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("48x48/apps")).unwrap();
        std::fs::write(dir.join("48x48/apps/strange.png"), b"x").unwrap();

        let t = table();
        let c = Chain {
            glyphs: &t,
            superpack: Some(&dir),
            generic: None,
        };
        let i = c.resolve("com.example.strange", Some("strange"), None);
        assert_eq!(i.stage(), 2, "it resolved {i:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_largest_size_wins() {
        // Shrinking a large icon comes out better than enlarging a small one, and the
        // order must not depend on the filesystem's order.
        let dir = std::env::temp_dir().join("ronoctua-test-icon-size");
        let _ = std::fs::remove_dir_all(&dir);
        for sz in ["16x16", "512x512", "48x48"] {
            std::fs::create_dir_all(dir.join(format!("{sz}/apps"))).unwrap();
            std::fs::write(dir.join(format!("{sz}/apps/x.png")), b"x").unwrap();
        }
        let t = BTreeMap::new();
        let c = Chain {
            glyphs: &t,
            superpack: Some(&dir),
            generic: None,
        };
        let Icon::Pixmap(p) = c.resolve("x", Some("x"), None) else {
            panic!("it did not find it")
        };
        assert!(p.to_string_lossy().contains("512x512"), "found {p:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stage_3_the_pixmap_of_the_message_itself() {
        let t = BTreeMap::new();
        let c = Chain {
            glyphs: &t,
            superpack: None,
            generic: None,
        };
        let i = c.resolve("does-not-exist", Some("also-does-not-exist"), Some(vec![9]));
        assert_eq!(i.stage(), 3);
    }

    #[test]
    fn with_no_stage_at_all_the_generic_one_comes_out() {
        // NEVER "nothing": a tray item with no icon is invisible, and the user
        // would have no way to know it exists.
        let t = BTreeMap::new();
        let c = Chain {
            glyphs: &t,
            superpack: None,
            generic: Some('?'),
        };
        assert_eq!(c.resolve("x", None, None), Icon::Glyph('?'));
    }

    #[test]
    fn an_empty_pixmap_does_not_count_as_stage_3() {
        // `Some(vec![])` happens: a client that fills the field with no data.
        // Treating it as an icon would leave an empty rectangle on screen.
        let t = BTreeMap::new();
        let c = Chain {
            glyphs: &t,
            superpack: None,
            generic: Some('?'),
        };
        assert_eq!(c.resolve("x", None, Some(Vec::new())).stage(), 1);
    }

    /// A scratch directory of this test's own, gone before and after.
    struct Scratch(PathBuf);

    impl Scratch {
        fn fresh(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("ronoctua-icons-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("mkdir");
            Self(dir)
        }

        /// Writes `bytes` at `rel`, making the directories on the way.
        fn put(&self, rel: &str, bytes: &[u8]) -> PathBuf {
            let at = self.0.join(rel);
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("mkdir");
            std::fs::write(&at, bytes).expect("write");
            at
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A real image, in a real format, of one colour — so a test can tell
    /// WHICH file was drawn by the pixel that came out.
    fn encoded(side: u32, colour: [u8; 4], format: image::ImageFormat) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(side, side, image::Rgba(colour));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, format).expect("encode");
        out.into_inner()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    /// The colour at the middle of a rasterised square.
    fn middle(r: &Rgba) -> [u8; 4] {
        let i = ((r.height / 2 * r.width + r.width / 2) * 4) as usize;
        [
            r.pixels[i],
            r.pixels[i + 1],
            r.pixels[i + 2],
            r.pixels[i + 3],
        ]
    }

    /// THE DEFECT: sorted as TEXT and reversed, `8x8` came before `84x84`, and
    /// a name drawn only in PNG was taken from the eight-pixel file.
    #[test]
    fn sizes_are_ranked_as_numbers_and_the_scale_counts() {
        let mut dirs = vec![
            "8x8", "84x84", "symbolic", "16x16@3", "48x48", "scalable", "64x64@2", "32x16",
        ];
        dirs.sort_by_key(|d| size_rank(d));
        assert_eq!(
            dirs,
            [
                "scalable", // the vector, whatever the sizes
                "64x64@2",  // 128 pixels
                "84x84",
                "16x16@3", // 48 pixels, tied with the next, and the name breaks the tie
                "48x48", "8x8", "32x16", // not a square: a theme's name for something else
                "symbolic",
            ]
        );
    }

    #[test]
    fn the_largest_png_is_the_one_drawn() {
        let s = Scratch::fresh("largest");
        s.put(
            "theme/8x8/apps/only-png.png",
            &encoded(8, RED, image::ImageFormat::Png),
        );
        s.put(
            "theme/84x84/apps/only-png.png",
            &encoded(84, GREEN, image::ImageFormat::Png),
        );
        s.put(
            "theme/16x16@2/apps/only-png.png",
            &encoded(32, BLUE, image::ImageFormat::Png),
        );
        let roots = Roots {
            theme: Some(s.0.join("theme")),
            ..Roots::default()
        };
        let found = roots.find("only-png").expect("found");
        assert!(found.to_string_lossy().contains("/84x84/"), "{found:?}");
        let r = rasterize_in(&roots, "only-png", 18).expect("drawn");
        assert_eq!(middle(&r), GREEN, "the pixel came from another file");
    }

    /// THE DEFECT: a name was looked up in the superpackage and nowhere else,
    /// so every program the person installed — a Flatpak export, a package of
    /// `~/.nix-profile` — came out with its category glyph.
    #[test]
    fn a_name_the_theme_does_not_have_is_found_in_hicolor_then_pixmaps() {
        let s = Scratch::fresh("chain");
        // The superpackage redraws `shared`, and has nothing of the rest.
        s.put(
            "theme/48x48/apps/shared.png",
            &encoded(48, RED, image::ImageFormat::Png),
        );
        // A Flatpak export's hicolor, the SECOND of two: the first has nothing.
        std::fs::create_dir_all(s.0.join("first/icons/hicolor/48x48/apps")).expect("mkdir");
        s.put(
            "exports/icons/hicolor/16x16/apps/org.example.App.png",
            &encoded(16, RED, image::ImageFormat::Png),
        );
        s.put(
            "exports/icons/hicolor/128x128/apps/org.example.App.png",
            &encoded(128, GREEN, image::ImageFormat::Png),
        );
        s.put(
            "exports/icons/hicolor/48x48/apps/shared.png",
            &encoded(48, BLUE, image::ImageFormat::Png),
        );
        // The last resort, with and without the extension written in `Icon=`.
        s.put(
            "share/pixmaps/legacy.png",
            &encoded(32, BLUE, image::ImageFormat::Png),
        );
        let roots = Roots {
            theme: Some(s.0.join("theme")),
            hicolor: vec![
                s.0.join("first/icons/hicolor"),
                s.0.join("exports/icons/hicolor"),
            ],
            pixmaps: vec![s.0.join("share/pixmaps")],
        };

        let app = rasterize_in(&roots, "org.example.App", 18).expect("hicolor reached");
        assert_eq!(middle(&app), GREEN, "hicolor's largest, not its 16 px");
        // THE THEME KEEPS THE FIRST WORD: the app is drawn in the system's style
        // when the superpackage redraws it.
        let shared = rasterize_in(&roots, "shared", 18).expect("drawn");
        assert_eq!(middle(&shared), RED, "hicolor beat the theme");
        assert_eq!(
            rasterize_in(&roots, "legacy", 18).map(|r| middle(&r)),
            Some(BLUE),
            "pixmaps, by the bare name"
        );
        assert_eq!(
            rasterize_in(&roots, "legacy.png", 18).map(|r| middle(&r)),
            Some(BLUE),
            "pixmaps, by the file name an `Icon=` sometimes carries"
        );
        // AND THE THEME IS ASKED FOR THE NAME, not the file name.
        assert_eq!(
            rasterize_in(&roots, "shared.png", 18).map(|r| middle(&r)),
            Some(RED)
        );
        // Nothing anywhere is an answer, not an invention.
        assert!(rasterize_in(&roots, "zzz-nowhere", 18).is_none());
    }

    /// The vector wins in hicolor too — `scalable` before any size.
    #[test]
    fn hicolor_prefers_the_vector() {
        let s = Scratch::fresh("vector");
        s.put(
            "h/48x48/apps/v.png",
            &encoded(48, RED, image::ImageFormat::Png),
        );
        s.put(
            "h/scalable/apps/v.svg",
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="rgb(0,255,0)"/></svg>"#,
        );
        let roots = Roots {
            hicolor: vec![s.0.join("h")],
            ..Roots::default()
        };
        let r = rasterize_in(&roots, "v", 18).expect("drawn");
        assert_eq!(middle(&r), GREEN);
    }

    /// THE DEFECT: `image` had no ICO decoder, and a favicon — the last resort
    /// of every site, and what a hand-made `.desktop` points at — was `None`.
    ///
    /// And the BYTES decide: the file here is called `.png` and is an ICO, and
    /// the SVG has no extension at all.
    #[test]
    fn an_ico_decodes_and_the_signature_beats_the_name() {
        let s = Scratch::fresh("ico");
        let ico = encoded(32, GREEN, image::ImageFormat::Ico);
        assert_eq!(&ico[..4], &[0, 0, 1, 0], "not an ICO header");
        let r = from_raster(&ico, 18).expect("the ICO decodes");
        assert_eq!((r.width, r.height), (18, 18));
        assert_eq!(middle(&r), GREEN);

        let lying = s.put("favicon.png", &ico);
        let r = rasterize_in(&Roots::default(), &lying.to_string_lossy(), 18).expect("drawn");
        assert_eq!(middle(&r), GREEN);

        let bare = s.put(
            "drawing",
            b"\xEF\xBB\xBF\n  <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\">\
              <rect width=\"4\" height=\"4\" fill=\"rgb(0,0,255)\"/></svg>",
        );
        let r = rasterize_in(&Roots::default(), &bare.to_string_lossy(), 18).expect("drawn");
        assert_eq!(middle(&r), BLUE, "a vector with no extension");
        // A signature nobody here reads is an answer, not a panic.
        assert!(from_raster(b"GIF89 is not quite", 18).is_none());
    }

    /// An `Icon=` is somebody else's text: a relative name with a `/` would be
    /// joined onto every root and climb out of it.
    #[test]
    fn a_name_is_one_component() {
        let s = Scratch::fresh("climb");
        s.put("outside.png", &encoded(8, RED, image::ImageFormat::Png));
        std::fs::create_dir_all(s.0.join("theme/48x48/apps")).expect("mkdir");
        let roots = Roots {
            theme: Some(s.0.join("theme/48x48/apps")),
            pixmaps: vec![s.0.join("theme")],
            ..Roots::default()
        };
        assert_eq!(roots.find("../outside"), None);
        assert_eq!(roots.find("../outside.png"), None);
        // And an absolute path that is not there is not looked up as a name.
        assert_eq!(roots.find("/nonexistent/at/all.png"), None);
    }

    /// The old entry point still looks in the superpackage ALONE: the probe
    /// that proves the superpackage is reached must not pass on a fallback.
    #[test]
    fn rasterize_looks_only_in_the_given_root() {
        let s = Scratch::fresh("alone");
        s.put(
            "theme/48x48/apps/here.png",
            &encoded(48, GREEN, image::ImageFormat::Png),
        );
        let root = s.0.join("theme");
        assert!(rasterize(Some(&root), "here", 18).is_some());
        assert!(rasterize(None, "here", 18).is_none());
    }

    #[test]
    fn the_session_roots_follow_the_specification() {
        let r = Roots::of(
            Some(PathBuf::from("/pack")),
            None,
            Some(PathBuf::from("/home/someone")),
            Some(
                "/home/someone/.nix-profile/share::/run/current-system/sw/share:/home/someone/.nix-profile/share",
            ),
        );
        assert_eq!(r.theme, Some(PathBuf::from("/pack")));
        assert_eq!(
            r.hicolor,
            [
                // The person's own first — `~/.local/share` when
                // `XDG_DATA_HOME` is unset — and `~/.icons` right after.
                "/home/someone/.local/share/icons/hicolor",
                "/home/someone/.icons/hicolor",
                "/home/someone/.nix-profile/share/icons/hicolor",
                "/run/current-system/sw/share/icons/hicolor",
            ]
            .map(PathBuf::from),
            "an empty entry skipped and a repeated one searched once"
        );
        assert_eq!(
            r.pixmaps,
            [
                "/home/someone/.nix-profile/share/pixmaps",
                "/run/current-system/sw/share/pixmaps",
            ]
            .map(PathBuf::from)
        );
        // UNSET OR EMPTY is the specification's default, not nothing.
        for dirs in [None, Some("")] {
            let d = Roots::of(None, Some(PathBuf::from("/data")), None, dirs);
            assert_eq!(
                d.hicolor,
                [
                    "/data/icons/hicolor",
                    "/usr/local/share/icons/hicolor",
                    "/usr/share/icons/hicolor",
                ]
                .map(PathBuf::from)
            );
        }
    }
}
