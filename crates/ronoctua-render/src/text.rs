//! Text drawing — section 7.7.
//!
//! # Two atlases, not one
//!
//! Section 5.2 wants the predictable repertoire in the atlas from startup; 6.5
//! wants a browser with 10,473 glyphs. That is not solved with a single atlas:
//!
//! - [`Atlas::Pinned`] — ASCII, Latin-1, box-drawing, icons in use as
//!   tokens. Populated at startup and **never subjected to `trim()`**.
//! - [`Atlas::Scratch`] — only the glyph browser uses it. `trim()` per frame.
//!
//! The browser rasterises only the visible rows (~50 glyphs), so it never
//! touches the pinned atlas, and 5.2's performance guarantee stays intact.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;

/// The face embedded in the binary.
///
/// **Not a convenience, a requirement**: the installer runs on a TTY before
/// any GPU is configured (section 7.1), and a minimal environment may not have
/// `share/fonts` mounted. Without the embedded face, there is nothing to draw.
///
/// It is `noctua-iosevka-core`'s Regular — the 4 normal-width RIBBI
/// faces, the same subset that goes on the ISO.
pub const EMBEDDED_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_REGULAR"));

/// The EMOJI face, embedded alongside the Iosevka.
///
/// # Why it exists, and why it does not contradict §6.4
///
/// The Iosevka draws 75 of the 1,184 single-codepoint emoji and none of the 2,760
/// sequences — the rest would come out as `.notdef`, which in this font is a visible
/// little box. Tofu is silent fallback in different clothes.
///
/// §6.4 forbids the renderer from CONSULTING fontconfig, and that still holds:
/// the chain here is DECLARED in two lines of code, not discovered at run
/// time. Which face draws what is still a question with a fixed
/// answer, and the `renderer-loads-three-faces` check fixes it —
/// written when the third face came in, because until then it was only a
/// promise in a comment, which is the same thing as an absent guard.
///
/// CBDT/CBLC bitmaps — the format `swash` decodes and for which
/// `glyphon` already has a separate colour atlas.
pub const EMOJI_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_EMOJI"));

/// The LUCIDE face, with its `cmap` rewritten by `nix/lucidegen.nix`.
///
/// # Why it is remapped, and why that is not fussiness
///
/// Lucide publishes its 1,668 icons at `U+E038..U+E6BB`. The Iosevka here is
/// patched with Nerd Fonts, which occupies **634 of those same codepoints** — and
/// the primary face (the one asked for by name, see `new`) always wins the
/// fallback chain. Without remapping, 38% of the icons would appear drawn as ANOTHER
/// symbol: not as tofu, which one sees, but as a pretty, wrong icon.
///
/// The new range is plane 15 (`U+F2000`), and what checks that it is free
/// in the other two faces is the generator, before writing anything at all.
///
/// The SOURCE codepoint still exists, in `lcp` in the database: it is what serves
/// whoever pastes into a page with the actual Lucide font. They are two facts, from
/// two different sources — not two copies of one.
pub const LUCIDE_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_LUCIDE"));

/// How many faces the bank has to have once assembled.
///
/// An EXACT number, and not `> 0`. The old guard could only say "some face
/// loaded": with it, the emoji face could disappear from the build and the only symptom
/// would be tofu coming back on the screen of whoever looks for an emoji.
pub const FACES: usize = 3;

/// Which atlas to use for a run of text.
///
/// Choosing wrong does not break the drawing, but it destroys the performance
/// guarantee — and silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atlas {
    /// A predictable repertoire. Populated at startup, never pruned.
    Pinned,
    /// Transient, LRU. Only the glyph browser (6.5) and tray pixmaps.
    Scratch,
}

/// A run positioned in CELLS.
///
/// Position in cells, not in pixels: with the exact cell (0.600 at), the
/// layout is integer arithmetic and there is no text measurement (section 4.1).
#[derive(Debug, Clone)]
pub struct Span {
    /// The already-elided text, ready to draw.
    pub text: String,
    /// The column, from the left.
    pub column: u16,
    /// The row: 0 = modeline, 1 = minibuffer (section 4.1).
    pub line: u16,
    /// Colour, from [`ronoctua_tokens::color`].
    pub color: tokens::Color,
    /// Size in px, when it is **not** the renderer's.
    ///
    /// # Why per run
    ///
    /// The glyph browser needs the icon large and the surrounding text at the
    /// normal size, on the same screen. A second renderer would give a second
    /// atlas and a second copy of the font just to draw two sizes; here
    /// each run already has its own glyphon `Buffer`, and the metric is its own.
    ///
    /// `None` is the size the renderer was built with — the common case,
    /// and that is why it forces nobody to say anything.
    pub size: Option<f32>,
}

/// Where the top of each run goes, so the LINE shares a MIDDLE.
///
/// # Why the middle, and not the baseline nor the top
///
/// Because it is the middle the eye reads as "they are on the same line" when the sizes
/// differ. While everything had a single size this did not show — top,
/// middle and baseline moved together. One larger icon beside the search field is
/// enough for the text to rise to the icon's top and the line to fall apart.
///
/// # Why centring the BOX gets the ink right
///
/// Because the line height is symmetrical: the line box is `1.45 at` and the font's
/// box is `1.25 at` (ascender 0.965 + descender 0.285), so `0.10 at` is left
/// above and `0.10 at` below. The middle of the line box is the middle of the
/// font box — and the middle of the font box sits `0.34 at` above the baseline,
/// which is exactly the middle of a Nerd icon's ink (most go from `0.04`
/// to `0.64 at`). Centring the box, measuring the ink and centring the ink give
/// the same result, and the box is the only one of the three the renderer knows without
/// opening the font.
///
/// On a single-size line this returns exactly `line × line height`, which
/// is what was there before: it is what keeps the system's footer and the modeline
/// from moving a pixel, since they never mix sizes.
///
/// It takes `(line in cells, height of the run's box)` and returns the top in
/// pixels, in the same order.
#[must_use]
pub fn aligned_tops(spans: &[(u16, f32)], entrelinha: f32) -> Vec<f32> {
    // The largest box on each line. There are few lines per frame, so the linear
    // search costs less than the table that would avoid it.
    let mut largest: Vec<(u16, f32)> = Vec::new();
    for (line, caixa) in spans {
        match largest.iter_mut().find(|(l, _)| l == line) {
            Some((_, c)) => *c = c.max(*caixa),
            None => largest.push((*line, *caixa)),
        }
    }
    spans
        .iter()
        .map(|(line, caixa)| {
            let da_linha = largest
                .iter()
                .find(|(l, _)| l == line)
                .map_or(*caixa, |(_, c)| *c);
            f32::from(*line).mul_add(entrelinha, (da_linha - caixa) / 2.0)
        })
        .collect()
}

/// Draws text onto a wgpu surface.
pub struct Text {
    face: glyphon::FontSystem,
    swash: glyphon::SwashCache,
    /// How many `prepare` calls have run. Only for timing the first few.
    preparos: u8,
    /// glyphon's `Cache` holds pipelines and bind group layouts. Creating one
    /// per frame — as an earlier version did — would recreate the pipeline on
    /// every draw, which on its own would blow section 5.2's budget.
    viewport: glyphon::Viewport,
    atlas_pinned: glyphon::TextAtlas,
    atlas_scratch: glyphon::TextAtlas,
    renderer_pinned: glyphon::TextRenderer,
    renderer_scratch: glyphon::TextRenderer,
    size: f32,
}

impl Text {
    /// Builds the drawer on an already-created device.
    ///
    /// The font comes embedded ([`EMBEDDED_FONT`]) and **not** from fontconfig
    /// (section 6.4).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format_of: wgpu::TextureFormat,
        size: f32,
    ) -> Result<Self> {
        // THE BANK IS BUILT HERE, and not by `new_with_fonts`.
        //
        // cosmic-text 0.19's `new_with_fonts` does
        // `db.set_monospace_family("Noto Sans Mono")` — a font that is NOT
        // in our bank, because we only load our own face. Asking for
        // `Family::Monospace` resolved to a non-existent name and fell into
        // the fallback machinery.
        //
        // Measured cost of the difference, on the FIRST shaping:
        //
        //     Family::Monospace           3,187 ms
        //     Family::Name(FAMILY_MONO)     122 ms     <- 26x
        //
        // It is not only performance: with the generic, which face drew the footer
        // depended on the fallback, which contradicts section 6.4 ("the renderer does not
        // use fontconfig") just as much as consulting fontconfig would.
        // See `ronoctua-bench-shaping`.
        let mut db = glyphon::fontdb::Database::new();
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            EMBEDDED_FONT,
        )));
        // THE SECOND FACE. cosmic-text's `Shaping::Advanced` already searches by
        // name when the first does not have the codepoint; with a bank of one face
        // only, that search had nowhere to land and the result was `.notdef`.
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            EMOJI_FONT,
        )));
        // THE THIRD. It is reached by the same path as the emoji one — by
        // CODEPOINT, and not by family name — and that is why the
        // remapping had to be done: with the source codepoints, the
        // Iosevka would answer first for 634 of them. Measured with a probe that
        // asks `cosmic-text` which face drew the glyph; the `renderer-loads-three-faces`
        // check repeats it on every build.
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            LUCIDE_FONT,
        )));
        // All three generics point at our face: if some day a
        // run asks for `Serif`, let it land here and not on the system.
        db.set_monospace_family(tokens::font::FAMILY_MONO);
        db.set_sans_serif_family(tokens::font::FAMILY_UI);
        db.set_serif_family(tokens::font::FAMILY_UI);
        let face = glyphon::FontSystem::new_with_locale_and_db("en-US".to_owned(), db);

        // It fails loudly if the embedded face did not load: carrying on from here would
        // only produce invisible text, which is worse to diagnose.
        let n = face.db().faces().count();
        anyhow::ensure!(
            n == FACES,
            "o banco tem {n} faces e devia ter {FACES} (Iosevka, emoji e Lucide). \
             Uma face a menos volta a desenhar tofu no lugar dos emoji ou dos \
             icones, e seguir daqui so produziria isso na tela."
        );
        tracing::debug!(faces = n, "faces embutidas carregadas");

        let cache = glyphon::Cache::new(device);
        let viewport = glyphon::Viewport::new(device, &cache);
        let mut atlas_pinned = glyphon::TextAtlas::new(device, queue, &cache, format_of);
        let mut atlas_scratch = glyphon::TextAtlas::new(device, queue, &cache, format_of);
        let renderer_pinned = glyphon::TextRenderer::new(
            &mut atlas_pinned,
            device,
            wgpu::MultisampleState::default(),
            None,
        );
        let renderer_scratch = glyphon::TextRenderer::new(
            &mut atlas_scratch,
            device,
            wgpu::MultisampleState::default(),
            None,
        );

        Ok(Self {
            face,
            swash: glyphon::SwashCache::new(),
            preparos: 0,
            viewport,
            atlas_pinned,
            atlas_scratch,
            renderer_pinned,
            renderer_scratch,
            size,
        })
    }

    /// The width of one cell, in logical px. Exact (section 4.1).
    #[must_use]
    pub fn cell_width(&self) -> f32 {
        tokens::metrics::cell_width(self.size)
    }

    /// The height of one line, in logical px.
    #[must_use]
    pub fn line_height(&self) -> f32 {
        tokens::metrics::line_height(self.size)
    }

    /// Prunes **only** the transient atlas.
    ///
    /// The pinned one is never pruned — it is what preserves section 5.2's guarantee
    /// even with the glyph browser open.
    pub fn trim_scratch(&mut self) {
        self.atlas_scratch.trim();
    }

    /// Prepares the runs for the next frame.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        atlas: Atlas,
        spans: &[Span],
    ) -> Result<()> {
        self.viewport
            .update(queue, glyphon::Resolution { width, height });

        let cw = self.cell_width();
        let lh = self.line_height();
        let metrica = glyphon::Metrics::new(self.size, lh);
        // THE METRIC OF A RUN THAT ASKS FOR ANOTHER SIZE.
        //
        // The line height follows the size: with the renderer's, a large
        // run would be clipped top and bottom by the line box.
        let metrica_de = |t: &Span| {
            t.size.map_or(metrica, |px| {
                glyphon::Metrics::new(px, px * tokens::font::LINE_HEIGHT)
            })
        };

        // In cosmic-text 0.19, `set_size` and `set_text` no longer take the
        // FontSystem, and `set_text` gained an `Option<Align>` at the end. Only
        // `Buffer::new` and `shape_until_scroll` still ask for it.
        // Shaping and atlas upload are costs of different natures: one is
        // rustybuzz over the font's tables, the other is swash rasterising and
        // uploading a texture. Only separated can one tell which to optimise.
        let t_moldar = std::time::Instant::now();
        let mut buffers = Vec::with_capacity(spans.len());
        for t in spans {
            let mut b = glyphon::Buffer::new(&mut self.face, metrica_de(t));
            b.set_size(Some(width as f32), Some(lh));
            b.set_text(
                &t.text,
                // By NAME, never by the generic. See the comment in `new`.
                &glyphon::Attrs::new().family(glyphon::Family::Name(tokens::font::FAMILY_MONO)),
                glyphon::Shaping::Advanced,
                // Alignment comes from the LAYOUT, not from the shaper: the positions
                // already come out in cells from `layout::resolve`.
                None,
            );
            b.shape_until_scroll(&mut self.face, false);
            buffers.push(b);
        }

        let moldagem = t_moldar.elapsed();

        // EACH RUN'S BOX: it is the line height of its own metric, the same one
        // `metrica_de` built. See `aligned_tops`.
        let caixas: Vec<(u16, f32)> = spans
            .iter()
            .map(|t| (t.line, metrica_de(t).line_height))
            .collect();
        let tops = aligned_tops(&caixas, lh);

        let areas = spans
            .iter()
            .zip(&buffers)
            .zip(&tops)
            .map(|((t, buf), topo)| {
                let [r, g, b, a] = t.color.to_rgba8();
                glyphon::TextArea {
                    buffer: buf,
                    left: f32::from(t.column) * cw,
                    top: *topo,
                    scale: 1.0,
                    bounds: glyphon::TextBounds {
                        left: 0,
                        top: 0,
                        right: width as i32,
                        bottom: height as i32,
                    },
                    default_color: glyphon::Color::rgba(r, g, b, a),
                    custom_glyphs: &[],
                }
            });

        let (renderer, atlas_ref) = match atlas {
            Atlas::Pinned => (&mut self.renderer_pinned, &mut self.atlas_pinned),
            Atlas::Scratch => (&mut self.renderer_scratch, &mut self.atlas_scratch),
        };

        let t_atlas = std::time::Instant::now();
        renderer
            .prepare(
                device,
                queue,
                &mut self.face,
                atlas_ref,
                &self.viewport,
                areas,
                &mut self.swash,
            )
            .context("glyphon nao conseguiu preparar o texto")?;

        // Only the first few: after that they would be one line per 1 s tick.
        if self.preparos < 3 {
            self.preparos += 1;
            tracing::info!(
                n = self.preparos,
                spans = spans.len(),
                moldagem_ms = moldagem.as_secs_f64() * 1000.0,
                atlas_ms = t_atlas.elapsed().as_secs_f64() * 1000.0,
                "partida: preparar"
            );
        }

        Ok(())
    }

    /// Emits the draw commands into the given `render pass`.
    pub fn draw(&self, atlas: Atlas, pass: &mut wgpu::RenderPass<'_>) -> Result<()> {
        let (renderer, atlas_ref) = match atlas {
            Atlas::Pinned => (&self.renderer_pinned, &self.atlas_pinned),
            Atlas::Scratch => (&self.renderer_scratch, &self.atlas_scratch),
        };
        renderer
            .render(atlas_ref, &self.viewport, pass)
            .context("glyphon nao conseguiu desenhar")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_font_is_not_empty() {
        // It catches the case of `include_bytes!` having pointed at the wrong or
        // truncated file — which would only show up as "nothing draws".
        assert!(
            EMBEDDED_FONT.len() > 1_000_000,
            "a face embutida tem {} bytes, o que e pequeno demais para a Iosevka",
            EMBEDDED_FONT.len()
        );
        // The TrueType signature.
        assert_eq!(&EMBEDDED_FONT[0..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    /// The cell's height, as the renderer builds it: `20 × 1.45`.
    const LH: f32 = 29.0;
    /// A run's box at icon scale: `40 × 1.45`.
    const ICON: f32 = 58.0;
    /// The search field's box: `18 × 1.45`.
    const TEXT: f32 = 26.1;

    #[test]
    fn a_line_of_a_single_size_does_not_move() {
        // The footer and the modeline never mix sizes. If this test
        // fails, the alignment change touched something that had nothing to
        // gain from it — and touching the footer is touching the whole system.
        let tops = aligned_tops(&[(0, TEXT), (0, TEXT), (1, TEXT)], LH);
        assert_eq!(tops, vec![0.0, 0.0, LH]);
    }

    #[test]
    fn the_smaller_sits_in_the_middle_of_the_larger() {
        // The browser header's case: a 40 px magnifying glass and the 18 px search
        // field on the SAME line. What the owner asked for, literally: "the middle
        // of the line of icons and of texts aligned".
        let tops = aligned_tops(&[(0, ICON), (0, TEXT)], LH);
        assert_eq!(tops[0], 0.0, "o maior manda, e ele nao se move");
        // The proof is the MIDDLE, and not the top: the centre of each box has to fall
        // in the same place, which is what "being on the same line" means
        // when the sizes differ.
        let medium = |topo: f32, caixa: f32| topo + caixa / 2.0;
        assert!((medium(tops[0], ICON) - medium(tops[1], TEXT)).abs() < 0.001);
    }

    #[test]
    fn each_line_has_its_own_middle() {
        // A large icon on line 0 must not push line 1's text: aligning
        // happens within the line, otherwise the whole screen slides.
        let tops = aligned_tops(&[(0, ICON), (0, TEXT), (1, TEXT)], LH);
        assert_eq!(tops[2], LH);
    }
}
