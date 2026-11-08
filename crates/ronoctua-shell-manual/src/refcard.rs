//! THE EMACS REFERENCE CARD — GNU's own, opened from `LEADER h` and by
//! `LEADER e` (the owner, 2026-11-08), on the whole screen in as many columns
//! as fit.
//!
//! # GNU's card, built from the Emacs this system installs
//!
//! Not the PDF of gnu.org: the same card's source ships with Emacs
//! (`etc/refcards/refcard.tex`, and the Brazilian translation beside it), so
//! `nix/emacs-refcard.nix` builds it for the version installed, ONE COLUMN TO
//! A PAGE — the card's own switch — and makes each page an SVG whose ink is
//! `currentColor`. A page here is a column; the window lays the columns side
//! by side, as the paper card lays three. Nothing is downloaded and nothing is
//! pinned beyond Emacs itself.
//!
//! # The card's ink is the theme's
//!
//! Black on white is paper. The ink is `text.primary` and the ground is the
//! window's (`page_art`), and the drawing's key carries the palette, the
//! height and the page (`page_key`) — a theme changed is a card drawn again,
//! never an old ink left on the GPU.

use std::path::{Path, PathBuf};

/// Where the built card is: `<dir>/<language>/<NN>.svg`, a page a column.
pub const ENV: &str = "RONOCTUA_EMACS_REFCARD";

/// The language whose card stands in for every other.
const FALLBACK: &str = "en";

/// The card's pages for `language`, in their order — its own when the build
/// made one (`pt-BR`), the English otherwise; none when the card is not on
/// this machine.
#[must_use]
pub fn pages(language: &str) -> Vec<PathBuf> {
    std::env::var_os(ENV).map_or_else(Vec::new, |root| pages_in(Path::new(&root), language))
}

/// [`pages`], under `root`.
#[must_use]
pub fn pages_in(root: &Path, language: &str) -> Vec<PathBuf> {
    let Some(dir) = [language, FALLBACK]
        .iter()
        .map(|l| root.join(l))
        .find(|d| d.is_dir())
    else {
        return Vec::new();
    };
    // BY THEIR NAMES, `01.svg` on, as the build writes them: the card is
    // read, never a folder listed.
    (1..)
        .map(|n| dir.join(format!("{n:02}.svg")))
        .take_while(|p| p.is_file())
        .collect()
}

/// The key of a page's drawing: the page, its height in pixels and the
/// palette its ink was resolved in.
#[must_use]
pub fn page_key(page: &Path, height: u32) -> String {
    format!(
        "emacs-card:{}:{height}:{}",
        page.display(),
        ronoctua_tokens::theme::palette()
    )
}

/// A page, drawn `height` pixels tall in the theme's ink.
#[must_use]
pub fn page_art(page: &Path, height: u32) -> Option<ronoctua_render::image::Rgba> {
    let svg = std::fs::read_to_string(page).ok()?;
    let [r, g, b, _] = ronoctua_tokens::color::TEXT_PRIMARY.to_rgba8();
    let svg = svg.replace("currentColor", &format!("#{r:02X}{g:02X}{b:02X}"));
    let drawn = ronoctua_notify::icon::from_svg(
        svg.as_bytes(),
        height,
        ronoctua_notify::icon::Fit::Height,
    )?;
    Some(ronoctua_render::image::Rgba {
        width: drawn.width,
        height: drawn.height,
        pixels: drawn.pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the card is not found where the build puts it, another
    /// language's is not the English, the pages come out of order, or a page
    /// is drawn at any height but the one asked — or in black.
    #[test]
    fn the_card_is_found_by_language_and_drawn_at_the_height_asked() {
        let root = std::env::temp_dir().join(format!("ronoctua-refcard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let page = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100pt" height="200pt" viewBox="0 0 100 200"><rect x="10" y="10" width="80" height="20" fill="currentColor"/></svg>"#;
        for (lang, n) in [("en", "02"), ("en", "01"), ("pt-BR", "01")] {
            std::fs::create_dir_all(root.join(lang)).unwrap();
            std::fs::write(root.join(lang).join(format!("{n}.svg")), page).unwrap();
        }
        let en = pages_in(&root, "de-DE");
        assert_eq!(en.len(), 2, "{en:?}");
        assert!(en[0].ends_with("en/01.svg") && en[1].ends_with("en/02.svg"));
        assert_eq!(pages_in(&root, "pt-BR").len(), 1);
        let art = page_art(&en[0], 400).expect("a drawing");
        assert_eq!((art.width, art.height), (200, 400));
        assert!(art.pixels.chunks(4).any(|p| p[3] > 0), "nothing was drawn");
        assert!(
            !art.pixels
                .chunks(4)
                .any(|p| p[3] == 255 && p[..3] == [0, 0, 0]),
            "the ink is black, not the theme's"
        );
        assert_ne!(page_key(&en[0], 400), page_key(&en[0], 401));
        let _ = std::fs::remove_dir_all(&root);
    }
}
