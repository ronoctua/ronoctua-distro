//! Which face answers for a script the primary one does not have.
//!
//! # Why this exists at all, when cosmic-text has one
//!
//! Because cosmic-text's table names the PROPORTIONAL CJK faces, and this
//! system is a cell grid. `Noto Sans CJK JP` puts a Latin letter and an
//! ideograph at widths that have no common divisor with our cell; `Noto Sans
//! Mono CJK JP` — the same design, the monospaced cut — advances exactly half
//! as much for the Latin as for the ideograph, which is the one shape a grid of
//! cells can place without lying about where the next column starts.
//!
//! Supplying our own is a supported thing to do:
//! `FontSystem::new_with_locale_and_db_and_fallback` takes it.
//!
//! # And why it is DECLARED, in a generated file
//!
//! Section 6.4 forbids the renderer from consulting fontconfig, and the reason
//! is not taste — fontconfig is not there in the installer or in a minimal
//! environment, and a renderer that needed it would draw tofu exactly where
//! nobody can debug it. So the answer to "who draws Arabic" has to be fixed at
//! build time.
//!
//! Fixed in TWO places it would be two answers, and the whole point of
//! `nix/fontconfig.nix` is that there is one: that file writes fontconfig's
//! `.conf`, the list of faces to collect, and — through
//! `nix/scripts/gen-chain.py` — the table included below.
//!
//! # What a family in the table is worth
//!
//! Nothing, until a file provides it. `nix/coverage-fonts.nix` reads the same
//! `coverage` list and refuses to build when one has no face, and
//! `ronoctua-test-faces` asks the assembled database who actually draws each
//! [`PROMISED`] probe. A chain nobody checks is a comment.

/// The declared chain.
///
/// A unit struct: it holds nothing, because every answer is a `const` table.
/// cosmic-text wants a `Box<dyn Fallback>` and keeps it for the life of the
/// `FontSystem`, so there is exactly one of these per renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Chain;

/// The locale a renderer starts at, before anyone has chosen a language.
///
/// It matches `ronoctua_i18n::DEFAULT`, and it is written here rather than
/// depended on: this crate has no i18n dependency and must not grow one — a
/// renderer that needed the text layer could not be linked by the installer.
/// The `Texts` a door builds moves it with [`super::text::Text::set_locale`],
/// and a test in the shell holds the two together.
pub const DEFAULT_LOCALE: &str = "en-GB";

include!(concat!(env!("OUT_DIR"), "/chain.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::fallback::Fallback;

    /// THE DEFECT THIS CATCHES: a Han locale silently losing its face.
    ///
    /// `han_unification` answers by locale, and every arm but the last is an
    /// exact string. A tag written `zh_TW` here, or `ja-JP` where the caller
    /// says `ja`, compiles and falls through to Simplified Chinese — Japanese
    /// text drawn with Chinese glyph shapes, which is legible and wrong, and
    /// which nobody who does not read the language will ever report.
    #[test]
    fn every_han_locale_answers_with_its_own_face() {
        let ja = Chain.script_fallback(unicode_script::Script::Han, "ja");
        let ko = Chain.script_fallback(unicode_script::Script::Han, "ko");
        let other = Chain.script_fallback(unicode_script::Script::Han, "en-GB");
        assert_eq!(ja, ["Noto Sans Mono CJK JP"]);
        assert_eq!(ko, ["Noto Sans Mono CJK KR"]);
        assert_eq!(other, ["Noto Sans Mono CJK SC"]);
        assert_ne!(ja, other, "a named locale must not resolve to the default");
    }

    /// Korean is drawn by the Korean cut, and its script is not in `scripts`.
    ///
    /// THE DEFECT THIS CATCHES, and it caught it: the first version of the
    /// generator listed the scripts the Han unification answers for by hand and
    /// left Hangul out of the list. Hangul is not in `scripts` either — nothing
    /// but the CJK faces draws it — so it fell through to the empty arm and
    /// every Korean character in the system was tofu, with all fifty-odd
    /// families present and correct.
    #[test]
    fn korean_is_answered_by_the_korean_cut() {
        assert_eq!(
            Chain.script_fallback(unicode_script::Script::Hangul, "pt-BR"),
            ["Noto Sans Mono CJK KR"],
        );
    }

    /// Hiragana and Katakana are JAPANESE, whatever the interface's language.
    ///
    /// They are not unified with anything — no other language writes them — so
    /// routing them through the interface's locale, as Han must be, would draw
    /// あ with a Simplified Chinese face for a Brazilian reader. Upstream
    /// hard-codes `ja` for exactly this and the generator copies that.
    #[test]
    fn the_kana_do_not_follow_the_interface_s_language() {
        for script in [
            unicode_script::Script::Hiragana,
            unicode_script::Script::Katakana,
        ] {
            assert_eq!(
                Chain.script_fallback(script, "pt-BR"),
                ["Noto Sans Mono CJK JP"],
                "{script:?} is Japanese regardless of who is reading"
            );
        }
    }

    /// The CJK we ask for is the MONOSPACED cut, and that is the whole reason
    /// this module exists rather than upstream's table.
    ///
    /// THE DEFECT THIS CATCHES: somebody "simplifying" the Nix table back to
    /// `Noto Sans CJK JP`. It would still draw — that is the trouble — and
    /// every column after an ideograph would sit somewhere else.
    #[test]
    fn the_cjk_face_is_the_monospaced_one() {
        for locale in ["ja", "ko", "zh-TW", "zh-HK", "en-GB"] {
            let got = Chain.script_fallback(unicode_script::Script::Han, locale);
            assert!(
                got[0].starts_with("Noto Sans Mono CJK"),
                "{locale} resolves to {got:?}, which is not a monospaced cut"
            );
        }
    }

    /// A script with no entry answers with nothing, and nothing is CORRECT:
    /// cosmic-text then tries `common_fallback` and finally sweeps the
    /// database. An empty slice is a narrower answer, not a missing one.
    #[test]
    fn an_unnamed_script_falls_through_rather_than_inventing_a_face() {
        assert!(
            Chain
                .script_fallback(unicode_script::Script::Latin, "en-GB")
                .is_empty(),
            "Latin is drawn by the primary face, which is asked for by NAME"
        );
    }

    /// Every family the per-script table names is in [`COVERAGE`], because
    /// `COVERAGE` is what `nix/coverage-fonts.nix` finds files for.
    ///
    /// THE DEFECT THIS CATCHES: the two lists are both derived from the same
    /// attrset today, and this is what notices the day somebody writes one of
    /// them out by hand — a family promised to a script, with no face behind
    /// it, is tofu again.
    #[test]
    fn every_promised_family_has_a_face_collected_for_it() {
        for (family, probe) in PROMISED {
            assert!(
                COVERAGE.contains(family),
                "{family} draws {probe:?} and is not in the coverage list"
            );
        }
        for family in COMMON {
            assert!(
                COVERAGE.contains(family),
                "{family} is in the common fallback and is not collected"
            );
        }
    }

    /// No probe is claimed twice, and none is a character the PRIMARY face has.
    ///
    /// A probe's whole job is to identify one face; a Latin letter or a digit
    /// would be answered by the Iosevka and the test would pass while proving
    /// nothing at all.
    #[test]
    fn a_probe_belongs_to_exactly_one_family() {
        let mut seen = std::collections::BTreeMap::new();
        for (family, probe) in PROMISED {
            assert!(
                !probe.is_ascii(),
                "{family}'s probe {probe:?} is ASCII, which the primary face draws"
            );
            if let Some(other) = seen.insert(*probe, *family) {
                panic!("{probe:?} is the probe of both {other} and {family}");
            }
        }
        assert!(
            seen.len() > 20,
            "only {} scripts are promised; the table looks truncated",
            seen.len()
        );
    }
}
