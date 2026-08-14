//! Asserts WHICH face draws what.
//!
//! # Why this binary exists
//!
//! §6.4 forbids the renderer from consulting fontconfig, and the price of that
//! is that the face chain is DECLARED in three lines of code. A declared chain
//! does not verify itself: a face can disappear from the build, or stop being
//! reached by the fallback, and the only symptom would be the screen.
//!
//! In all three cases the symptom is different, and the worst is not the most
//! visible:
//!
//! - the emoji face disappears  -> tofu, which is seen;
//! - the Lucide face disappears -> tofu, which is seen;
//! - Lucide goes back to using the ORIGIN codepoints -> **the wrong icon**,
//!   drawn with confidence, because 634 of them belong to the Nerd Fonts
//!   inside Iosevka and the primary face beats the fallback. That one is not seen.
//!
//! The last case is what this binary exists to catch, and that is why it
//! witnesses the collision explicitly instead of only counting faces.
//!
//! It needs no GPU: shaping and rustybuzz over the font's tables.

use anyhow::{Result, bail};
use glyphon::cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use ronoctua_render::text::{EMBEDDED_FONT, EMOJI_FONT, FACES, LUCIDE_FONT};
use ronoctua_tokens as tokens;

/// A codepoint the Nerd Fonts occupies INSIDE Lucide's origin range.
///
/// `U+E0A0` is Powerline's `pl-branch` in the patched Iosevka, and it falls in
/// the middle of `U+E038..U+E6BB`, where Lucide publishes its icons. It is the
/// collision's witness: as long as Iosevka answers for it, publishing Lucide in
/// the origin range means drawing the wrong symbol.
const COLLIDED: char = '\u{E0A0}';

/// A codepoint from the REMAPPED range, where Lucide came to live.
const REMAPPED: char = '\u{F2000}';

/// How much the first glyph of `s` ADVANCES, at the footer's text body.
///
/// In pixels, and not in cells: whoever converts is whoever calls, against
/// `metrics::cell_width` — which is the ruler the layout uses.
fn advance(fs: &mut FontSystem, s: &str) -> Result<f32> {
    let size = tokens::font::SIZE_BASE;
    let mut b = Buffer::new(fs, Metrics::new(size, tokens::metrics::line_height(size)));
    b.set_text(
        s,
        &Attrs::new().family(Family::Name(tokens::font::FAMILY_MONO)),
        Shaping::Advanced,
        None,
    );
    b.shape_until_scroll(fs, false);
    for run in b.layout_runs() {
        if let Some(g) = run.glyphs.first() {
            return Ok(g.w);
        }
    }
    bail!("{s:?} nao moldou nenhum glifo")
}

/// Which face drew the first glyph of `s`, and which glyph.
fn who_draws(fs: &mut FontSystem, s: &str) -> Result<(String, u16)> {
    let mut b = Buffer::new(fs, Metrics::new(29.0, 42.0));
    b.set_text(
        s,
        // By NAME, the way the renderer does. It is what makes the question "which
        // face draws this" answerable: with the generic, the answer would depend on
        // the whole fallback machinery.
        &Attrs::new().family(Family::Name(tokens::font::FAMILY_MONO)),
        Shaping::Advanced,
        None,
    );
    b.shape_until_scroll(fs, false);
    for run in b.layout_runs() {
        if let Some(g) = run.glyphs.first() {
            let family = fs
                .db()
                .face(g.font_id)
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
                .unwrap_or_default();
            return Ok((family, g.glyph_id));
        }
    }
    bail!("{s:?} nao moldou nenhum glifo")
}

fn main() -> Result<()> {
    let mut db = glyphon::fontdb::Database::new();
    for face in [EMBEDDED_FONT, EMOJI_FONT, LUCIDE_FONT] {
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(face)));
    }
    db.set_monospace_family(tokens::font::FAMILY_MONO);

    let n = db.faces().count();
    if n != FACES {
        bail!("o banco tem {n} faces e devia ter {FACES}");
    }
    let familias: Vec<String> = db
        .faces()
        .map(|f| {
            f.families
                .first()
                .map_or_else(String::new, |(name, _)| name.clone())
        })
        .collect();
    println!("faces: {familias:?}");

    let mut fs = FontSystem::new_with_locale_and_db("en-US".to_owned(), db);

    // 1. PLAIN TEXT goes to the primary face.
    let (family, _) = who_draws(&mut fs, "A")?;
    if family != tokens::font::FAMILY_MONO {
        bail!("a letra `A` foi desenhada por `{family}`, e nao pela face primaria");
    }

    // 2. THE EMOJI falls on the emoji face, by codepoint fallback.
    let (family, glyph) = who_draws(&mut fs, "\u{1F600}")?;
    if !family.to_lowercase().contains("emoji") {
        bail!("o emoji foi desenhado por `{family}`");
    }
    if glyph == 0 {
        bail!("o emoji caiu em `.notdef` — a face de emoji nao responde por ele");
    }

    // 3. THE LUCIDE ICON falls on the Lucide face, also by codepoint.
    let (family, glyph) = who_draws(&mut fs, &REMAPPED.to_string())?;
    if !family.to_lowercase().contains("lucide") {
        bail!(
            "U+{:04X} foi desenhado por `{family}`, e nao pela face da Lucide",
            REMAPPED as u32
        );
    }
    if glyph == 0 {
        bail!("o icone da Lucide caiu em `.notdef`");
    }

    // 4. THE COLLISION'S WITNESS.
    //
    // This one does not verify our code: it verifies the FACT that forced the
    // remapping. As long as Iosevka answers for `U+E0A0`, publishing Lucide in
    // the origin range means drawing the wrong symbol — and the day it stops
    // answering, this criterion fails and someone re-reads the decision instead
    // of discovering it again through the screen.
    let (family, _) = who_draws(&mut fs, &COLLIDED.to_string())?;
    if family != tokens::font::FAMILY_MONO {
        bail!(
            "U+{:04X} foi desenhado por `{family}`, e nao pela Iosevka.\n\
             A premissa do remapeamento da Lucide mudou: releia \
             nix/lucidegen.nix antes de mexer.",
            COLLIDED as u32
        );
    }

    // 5. THE 75 IOSEVKA HAD AND GAVE UP.
    //
    // The primary face is the one asked for by name, and it beats the fallback
    // on everything it has. Iosevka draws 75 of the default-presentation emoji
    // in monochrome, and that is why `UP!` came out as a little white box of
    // black letters next to a colored `ATM`. It was not tofu: it was the WRONG
    // emoji, and the screen had no way of reporting it — both exist, both show up.
    //
    // The `nix/primary-font.nix` takes them out of the copy the renderer embeds.
    // These six are the runtime witness that that happened.
    for (name, cp) in [
        ("UP!", 0x1F199u32),
        ("relógio", 0x1F550),
        ("lua nova", 0x1F311),
        ("taça de vinho", 0x1F377),
        ("sino", 0x1F514),
        ("coração preto", 0x1F5A4),
    ] {
        let s = char::from_u32(cp).unwrap_or('?').to_string();
        let (family, _) = who_draws(&mut fs, &s)?;
        if !family.to_lowercase().contains("emoji") {
            bail!(
                "`{name}` (U+{cp:04X}) foi desenhado por `{family}`.\n\
                 A face primária voltou a ter os codepoints de presença de \
                 emoji, e eles saem em monocromático ao lado dos coloridos."
            );
        }
    }

    // 6. AND THE TEXT-PRESENTATION ONES DID **NOT** GO ALONG.
    //
    // The other side of the same cut, and what makes it safe: `#`, the digits,
    // `©` and the arrows are sequence bases, not emoji. Drawing them colored by
    // default would break plain text, and losing them from the primary face
    // would break more still — the symptom would show up far from the cause.
    for (name, cp) in [
        ("cerquilha", 0x23u32),
        ("zero", 0x30),
        ("copyright", 0xA9),
        ("marca registrada", 0x2122),
        ("seta à esquerda", 0x2190),
    ] {
        let s = char::from_u32(cp).unwrap_or('?').to_string();
        let (family, glyph) = who_draws(&mut fs, &s)?;
        if family != tokens::font::FAMILY_MONO || glyph == 0 {
            bail!(
                "`{name}` (U+{cp:04X}) saiu da face primária (foi para \
                 `{family}`, glifo {glyph}). O corte levou junto o que era de \
                 presença de TEXTO."
            );
        }
    }

    // 7. THE APPLICATION ICON FITS IN THE CELL THE LAYOUT RESERVES.
    //
    // The `surface::ICON_COLUMNS` reserves TWO cells per icon — one of ink
    // and one of breathing room — and that arithmetic depends on a fact about
    // the FONT: an icon glyph, drawn at the text's body, advances one cell. The
    // fact was written in a comment, and a comment is not defended: the day the
    // patched Iosevka started giving icons one and a half widths, the
    // application's name would be written OVER the icon — on the screen, and in
    // no log.
    //
    // The tolerance is a tenth of a cell: shaping rounds, and an advance of
    // 12.0001 px in a cell of 12 is not an icon that is too wide.
    let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
    let mut worst = (0.0f32, ' ');
    for (_, g) in tokens::category::TABLE
        .iter()
        .chain(tokens::tray::TABLE.iter())
    {
        let w = advance(&mut fs, &g.to_string())?;
        if w > worst.0 {
            worst = (w, *g);
        }
    }
    if worst.0 > cell * 1.1 {
        bail!(
            "U+{:04X} avanca {:.2} px numa celula de {cell:.2} px.\n\
             O `surface::COLUNAS_DO_ICONE` reserva uma celula de tinta, e o \n\
             nome do aplicativo sera escrito por cima do icone.",
            worst.1 as u32,
            worst.0
        );
    }
    println!(
        "OK: o icone mais largo avanca {:.2} px numa celula de {cell:.2} px",
        worst.0
    );

    println!("OK: {FACES} faces — texto na Iosevka, emoji na de emoji, icone na da Lucide");
    println!("OK: os 75 de presença de emoji são da face de emoji, e `#`/`©` não foram junto");
    println!(
        "OK: a Iosevka ainda responde por U+{:04X}, que e por que a Lucide foi remapeada",
        COLLIDED as u32
    );
    Ok(())
}
