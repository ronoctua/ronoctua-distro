//! The shortcut manual — `LEADER h s`.
//!
//! # Why it exists
//!
//! The chords left the list the leader's tap opens. They were not choices about
//! what one is doing there — the leader is already released, and pressing `←`
//! at that moment does nothing —, so they took twenty-five lines to say what
//! exists in another mode of use. The tap's list halved, and what left needed a
//! place.
//!
//! # Why it is not a file
//!
//! A manual written by hand is the second copy of the shortcut table, and the
//! day it lies is the day someone trusts it. Here the lines come out of
//! [`crate::actions::acordes`] and [`crate::actions::emergency`] at the moment
//! of opening: changing a shortcut changes the manual, with no intermediate
//! step. It is the same reason niri's binds are generated.
//!
//! # Why it is not a buffer
//!
//! It opens **in place of the shortcut list**, in the footer's expanded region
//! — the fourth use of the same surface, beside which-key, the `M-x` and the
//! history. There is no second surface anywhere in the system (invariant 14),
//! and a manual that is only read needs no window: it needs scrolling, which
//! the region already has.
//!
//! It is read-only by construction, and not by a lock: there is no input path
//! that writes to it.

use ronoctua_i18n::{Texts, key};

/// One line of the manual: the key and what it does.
pub struct Line {
    /// How the combination is written — `SUPER + ←`.
    pub key: String,
    /// What it does, in the active language.
    pub description: String,
    /// A section header, and not a shortcut.
    pub title: bool,
}

/// Assembles the whole manual, in the active language.
///
/// `acorde` is how the chords' key appears — `SUPER` in production, `ALT` in
/// the nested session. It comes from outside because whoever knows that is
/// whoever defines the bind (see [`crate::leader_chord`]), and not this module.
#[must_use]
pub fn assemble(texts: &Texts, acorde: &str) -> Vec<Line> {
    let mut out = vec![Line {
        key: String::new(),
        description: texts.t(key::HELP_CHORDS),
        title: true,
    }];

    // THE CHORDS, in the table's order — which is the order in which they make
    // sense together (focus, move, window, spaces, open), and not alphabetical.
    for a in crate::actions::acordes_completos() {
        out.push(Line {
            key: format!("{acorde} + {}", a.shows),
            description: texts.t(&a.label()),
            title: false,
        });
    }

    // THE EMERGENCY BINDS. They are what one looks for when the leader does not
    // answer — and that is why the manual needs them more than any other line. A
    // manual that only documents the happy path fails exactly when it is
    // consulted.
    out.push(Line {
        key: String::new(),
        description: texts.t(key::HELP_EMERGENCY),
        title: true,
    });
    for (key, _, command) in crate::actions::emergency() {
        let description = if command == crate::actions::LEADER_ALTERNATE {
            texts.t(key::HELP_LEADER_ALTERNATE)
        } else {
            texts.t(&format!("{}{command}", ronoctua_cmd::KEY_PREFIX_OF))
        };
        out.push(Line {
            key: key.to_owned(),
            description,
            title: false,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use ronoctua_i18n::Texts;

    #[test]
    fn every_chord_shows_up_in_the_manual() {
        // THE MANUAL MUST NOT LIE BY OMISSION. It exists because the chords left the
        // tap's list: if one of them does not arrive here, it stops being
        // discoverable anywhere — worse than before.
        let texts = Texts::from_env();
        let m = super::assemble(&texts, "SUPER");
        for a in crate::actions::acordes_completos() {
            assert!(
                m.iter().any(|l| l.key.ends_with(a.shows)),
                "o acorde `{}` nao aparece no manual",
                a.shows
            );
        }
        // And the emergency ones, for the same reason, with even more cause.
        for (key, _, _) in crate::actions::emergency() {
            assert!(
                m.iter().any(|l| l.key == key),
                "o bind de emergencia `{key}` nao aparece no manual"
            );
        }
    }

    #[test]
    fn no_row_comes_out_without_text() {
        // A key that does not exist becomes `[key]` on screen (the `t()` does
        // that on purpose, so the error is visible). Here it would be visible
        // LATE: in the manual, which is opened once a month.
        let texts = Texts::from_env();
        for l in super::assemble(&texts, "SUPER") {
            assert!(!l.description.is_empty(), "linha sem descricao");
            assert!(
                !l.description.starts_with('['),
                "chave de texto faltando: {}",
                l.description
            );
            assert!(l.title || !l.key.is_empty(), "atalho sem tecla");
        }
    }

    #[test]
    fn the_chords_label_goes_into_the_key() {
        // The manual is the only place where the chord's key appears written out.
        // In the nested session it is `ALT`, and not `ALT SPC` — holding Space
        // along is what fires the SEQUENCE, the opposite of the chord.
        let texts = Texts::from_env();
        let m = super::assemble(&texts, "ALT");
        assert!(m.iter().any(|l| l.key.starts_with("ALT + ")));
        assert!(!m.iter().any(|l| l.key.contains("ALT SPC")));
    }
}
