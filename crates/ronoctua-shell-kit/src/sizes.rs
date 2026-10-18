//! A size, as a person reads it.
//!
//! # Why a file of its own
//!
//! Because two binaries need the ONE arithmetic: the footer, which draws a
//! transfer's total and a relief's room, and `ronoctua-render-png`, which
//! photographs the brain's page — whose notices and module rows carry sizes the
//! host formats (`crate::mind_wire`). The tool links the kit and nothing of the
//! footer, and `network.rs`, where this used to live, is the network view's. A
//! second copy in the photograph would be a size that reads one way on the
//! screen and another in its picture.

/// `44220000000` -> `44,2 GB`.
///
/// One decimal place, like the link's rate: the total transferred grows all
/// the time, and the second place would be a digit flickering without
/// informing anything.
///
/// THE SEPARATOR IS THE LANGUAGE'S and comes in from
/// [`ronoctua_i18n::Texts::decimal_mark`]. It was a comma written into the
/// format string, so an English screen said `44,2 GB` — which in English is
/// forty-four thousand two hundred gigabytes.
#[must_use]
pub fn readable_bytes(bytes: u64, mark: char) -> String {
    const K: u64 = 1_000;
    const M: u64 = K * 1_000;
    const G: u64 = M * 1_000;
    match bytes {
        0..=999 => format!("{bytes} B"),
        1_000..=999_999 => format!("{} kB", bytes / K),
        1_000_000..=999_999_999 => format!("{} MB", bytes / M),
        _ => {
            let tenth = (bytes + G / 20) / (G / 10);
            format!("{}{mark}{} GB", tenth / 10, tenth % 10)
        }
    }
}

/// A position in a film, as somebody would say it.
///
/// `1:23`, and `1:02:03` once there is an hour of it — never `0:01:23`, which
/// is a stopwatch's idea of a time and not a person's.
#[must_use]
pub fn clock_of(seconds: f64) -> String {
    // AND IT IS THE FACTS SHEET'S OWN FUNCTION, because the two are read side
    // by side and were not the same rule: one truncated and the other rounded,
    // so a 293,8-second track was `4:53` under the timeline and `4:54` two
    // panels over. One function cannot disagree with itself.
    ronoctua_inspect::facts::clock(seconds)
}

/// The air a picture keeps from every side of its box, in logical pixels:
/// `space.files.pictureAirPct` of a CELL at the size of the text now.
///
/// A FRACTION OF A CELL, not a number of pixels, because the reason for the
/// air is written in cells — a whole cell would read as a column gap, a
/// sliver would read as cut — and a cell is 10.8 px at 100 % and 21.6 at
/// 200 %: six pixels was the fraction the argument meant only at the first.
#[must_use]
pub fn picture_air() -> f32 {
    // IN WHOLE PIXELS: an edge of a picture on a fraction of a pixel is a
    // blurred edge, and every box this air is taken from is in whole pixels.
    (ronoctua_tokens::metrics::cell_width(ronoctua_tokens::text::size())
        * f32::from(ronoctua_tokens::space::FILES_PICTUREAIRPCT)
        / 100.0)
        .round()
}

/// The preview's picture, centred in its box with air on all four sides.
///
/// # What `fit_into` alone does not do
///
/// It centres, and it is not enough. The scale is the tighter of the two
/// ratios, so whichever dimension is the tight one comes out exactly as long
/// as the box — the picture is centred and touching two borders at once. Both
/// doors shipped that way and the owner photographed both: the top edge in the
/// window, the right-hand rule in the footer. What he asked for is a rule and
/// not a nudge — *"the blank space on the left has to be always equal to the
/// one on the right, and the blank space below the picture has to be always
/// equal to the space above"* — and centring already promises the halves are equal. The
/// missing half of the promise is that neither of them is zero.
///
/// So the box gives up `sizes::picture_air` on each of its four sides
/// before anything is fitted into it. One function, two doors, because the
/// arithmetic that was written out twice is the arithmetic that came out
/// different twice.
#[must_use]
pub fn picture_in(
    space: ronoctua_render::image::Area,
    w: u32,
    h: u32,
) -> ronoctua_render::image::Area {
    let air = picture_air();
    // AND IT ALWAYS FITS, and never stretches to fill.
    //
    // Drawing a frame across the box unconditionally was the first shape of
    // the double-letterbox fix, and it is wrong for a reason the owner
    // photographed twice: a frame is only the box's shape while it is FRESH.
    // Pause a film and give the preview the whole region and no new frame
    // arrives — so the old one, cut for a small panel, was stretched across a
    // large one. Same for a GIF whose box moved.
    //
    // What removes the second letterbox is the frame being CUT to the box's
    // shape (see `Fit::of`), not the fit being skipped: when the two shapes
    // agree, `fit_into` fills the box exactly. When they do not — because the
    // picture is a moment stale — it letterboxes, which is honest, instead of
    // distorting, which is not.
    ronoctua_render::image::Area {
        x: space.x + air,
        y: space.y + air,
        // NEVER NEGATIVE. A panel at its floor in a two-line strip is smaller
        // than the air twice over, and a negative width would come out of
        // the fit as a scale nobody can see.
        width: (space.width - air - air).max(0.0),
        height: (space.height - air - air).max(0.0),
    }
    // AND IT SHRINKS, NEVER GROWS.
    //
    // `Area::centre` and not `fit_into`, which is the same arithmetic with the
    // enlargement taken out — and `centre` already existed, for the icon
    // browser, with the argument written beside it: *whoever is choosing a
    // size needs to see the size*.
    //
    // THE OWNER READ IT OFF THE SCREEN, on his own `sample-img.ico`: a 390-byte
    // icon blown up to fill a panel. *"As imagens do preview nunca devem
    // expandir além dos seus tamanhos máximo de altura e de largura"*. An
    // enlargement invents pixels, and on a 16-square icon it invents all but
    // 256 of them — what one is shown is not the file.
    //
    // NOTHING THAT MOVES LOSES BY IT: a film's frame and a GIF's composite are
    // both produced AT the box's size (`Fit::of`), so the two answers agree
    // exactly. Where they differ is a frame that is a moment stale, and there
    // drawing it at its own size is the honest half of the same choice the
    // letterbox already makes.
    .centre(w, h)
}

/// A size, already assembled.
///
/// Assembled here rather than through the bundle because a listing draws two
/// hundred of these per frame and the language only decides the separator —
/// which the caller passes in, out of [`ronoctua_i18n::Texts::decimal_mark`].
///
/// THE SEPARATOR USED TO BE WRITTEN IN, and that is the defect this signature
/// exists to close: `5,0 MiB` came out on an English screen as well, where a
/// comma is the thousands separator and the number reads as five thousand.
/// The comment above already described the arrangement; nothing implemented it.
pub fn size_text(bytes: u64, mark: char) -> String {
    use ronoctua_files::size;
    let h = size::human(bytes);
    if matches!(h.unit, size::Unit::Bytes) {
        format!("{} B", h.whole)
    } else {
        let unit = match h.unit {
            size::Unit::Kibi => "KiB",
            size::Unit::Mebi => "MiB",
            size::Unit::Gibi => "GiB",
            size::Unit::Tebi => "TiB",
            size::Unit::Bytes => "B",
        };
        format!("{}{mark}{} {unit}", h.whole, h.tenth)
    }
}

/// How many cells a bar gets.
///
/// Wide enough to read a tenth, narrow enough to leave the number beside it.
pub const BAR_CELLS: u16 = 10;
