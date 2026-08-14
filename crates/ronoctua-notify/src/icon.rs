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
        embutido: Option<Vec<u8>>,
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
                && let Some(achado) = look_up(root, name)
            {
                return Icon::Pixmap(achado);
            }
        }

        // Step 3.
        if let Some(bytes) = embutido.filter(|b| !b.is_empty()) {
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

/// Finds `icon`'s file and rasterises it into a square of `side` px.
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
pub fn rasterize(root: Option<&std::path::Path>, icon: &str, side: u32) -> Option<Rgba> {
    if icon.is_empty() || side == 0 {
        return None;
    }
    // AN ABSOLUTE PATH is used directly: the spec allows both forms in the same
    // field, and a hand-installed app's `.desktop` usually uses this one.
    let path = std::path::Path::new(icon);
    let target = if path.is_absolute() && path.exists() {
        path.to_path_buf()
    } else {
        look_up(root?, icon)?
    };
    let bytes = std::fs::read(&target).ok()?;
    if target.extension().is_some_and(|e| e == "svg") {
        // FILL: here the file's size means nothing — the same
        // theme publishes 16 px and 512 px `.svg` files — and what matters is filling the
        // cell the row reserved.
        return from_svg(&bytes, side, Fit::Fill);
    }
    from_raster(&bytes, side)
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
pub fn from_svg(bytes: &[u8], side: u32, encaixe: Fit) -> Option<Rgba> {
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
    let scale = match encaixe {
        Fit::Fill => fits,
        // NEVER ENLARGE. A 24 px document enlarged to fill the box is
        // the same image as a 512 px one — and that was exactly the defect.
        Fit::Natural => fits.min(1.0),
    };
    let arredondar = |v: f32| (v.round() as u32).max(1);
    let (width, height) = match encaixe {
        Fit::Fill => (side, side),
        Fit::Natural => (
            arredondar(sz.width() * scale),
            arredondar(sz.height() * scale),
        ),
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

/// A PNG (or JPEG) shrunk to the final size.
#[must_use]
pub fn from_raster(bytes: &[u8], side: u32) -> Option<Rgba> {
    let img = image::load_from_memory(bytes).ok()?;
    // `Lanczos3` and not `Nearest`: the reduction is large — 512 px to 18 — and a
    // nearest-neighbour resampler loses the icon's thin lines
    // entirely, leaving a blur of blocks.
    let escalada = img.resize(side, side, image::imageops::FilterType::Lanczos3);
    let rgba = escalada.to_rgba8();
    Some(Rgba {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

/// Looks for `name.{svg,png}` in the superpackage.
///
/// It walks the sizes in DECREASING order: the footer draws small, and
/// shrinking a large icon comes out better than enlarging a small one.
///
/// **It touches the disk**, and that is why it is public: the app launcher
/// calls it from a thread, far from the event loop. See [`Chain::glyph`].
#[must_use]
pub fn look_up(root: &std::path::Path, name: &str) -> Option<PathBuf> {
    let mut tamanhos: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    // A stable, decreasing order. Without this, the choice would depend on the filesystem's
    // order and would change between machines.
    tamanhos.sort();
    tamanhos.reverse();

    for dir in tamanhos {
        for ext in ["svg", "png"] {
            let target = dir.join(format!("{name}.{ext}"));
            if target.exists() {
                return Some(target);
            }
            // freedesktop themes put the files in a context
            // subdirectory (`apps/`, `places/`, …).
            let Ok(contextos) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut ctxs: Vec<_> = contextos
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
        let dir = std::env::temp_dir().join("ronoctua-teste-icone-2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("48x48/apps")).unwrap();
        std::fs::write(dir.join("48x48/apps/estranho.png"), b"x").unwrap();

        let t = table();
        let c = Chain {
            glyphs: &t,
            superpack: Some(&dir),
            generic: None,
        };
        let i = c.resolve("com.exemplo.estranho", Some("estranho"), None);
        assert_eq!(i.stage(), 2, "resolveu {i:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_largest_size_wins() {
        // Shrinking a large icon comes out better than enlarging a small one, and the
        // order must not depend on the filesystem's order.
        let dir = std::env::temp_dir().join("ronoctua-teste-icone-tam");
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
            panic!("nao achou")
        };
        assert!(p.to_string_lossy().contains("512x512"), "achou {p:?}");
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
        let i = c.resolve("desconhecido", Some("tambem-nao-existe"), Some(vec![9]));
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
}
