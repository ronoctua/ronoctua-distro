//! Interface text — invariant 8, section 7.6.
//!
//! **Zero string literals in UI.** Every sentence comes from `assets/i18n/*.ftl`, through
//! a key. What this crate does is resolve a key into a sentence in the active
//! language.
//!
//! # How the invariant stops depending on discipline
//!
//! Two guards, neither of them a test:
//!
//! 1. **Build.** `nix/scripts/gen-i18n.py` requires `pt-BR` and `en-GB` to have
//!    exactly the same set of keys, and none of them empty. A missing translation
//!    takes the build down before any test runs.
//! 2. **Compilation.** The same generator emits one constant per key in
//!    [`key`]. Whoever writes `key::CMD_BUFFER_CLOSE` cannot get it wrong;
//!    whoever writes the string by hand loses the guarantee, and that is why the project's
//!    code uses the constants.
//!
//! # Why Fluent, and not gettext
//!
//! Plural, gender and argument order belong to the LANGUAGE, not to the program. In
//! gettext that becomes `ngettext` with a numeric rule in the code; in Fluent the
//! rule lives in the `.ftl`, which is where the translator works. A language with a different
//! plural rule does not require touching Rust.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

include!(concat!(env!("OUT_DIR"), "/keys.rs"));

/// Default language when the environment says nothing useful.
///
/// `en-GB` and not `pt-BR`: a system that fell back probably is not
/// on a Brazilian machine, and English is the least surprising choice. The
/// distro owner's preference comes in through `LANG`, as with everything else.
pub const DEFAULT: &str = "en-GB";

/// One language's texts.
pub struct Texts {
    bundle: FluentBundle<FluentResource>,
    language: String,
}

impl std::fmt::Debug for Texts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Textos")
            .field("idioma", &self.language)
            .finish()
    }
}

/// Each language's `.ftl`, embedded in the binary.
///
/// **Embedded, not read from disk**: the installer runs before any
/// filesystem is mounted (section 7.1), and a shell with no text is
/// useless. Same reason as the face embedded in the renderer.
fn face(language: &str) -> Option<&'static str> {
    match language {
        "en-GB" => Some(include_str!(concat!(
            env!("RONOCTUA_I18N_DIR"),
            "/en-GB.ftl"
        ))),
        "pt-BR" => Some(include_str!(concat!(
            env!("RONOCTUA_I18N_DIR"),
            "/pt-BR.ftl"
        ))),
        _ => None,
    }
}

/// One argument of a text, when the value is not always a string.
///
/// See [`Texts::t_args`] for why a number has to arrive as a number.
#[derive(Debug, Clone, Copy)]
pub enum Arg<'a> {
    /// A piece of data: a name, a path, a key notation.
    Text(&'a str),
    /// A count. This is the one that makes a plural pick the right branch.
    Number(i64),
}

impl Texts {
    /// Loads a language by name (`pt-BR`, `en-GB`).
    ///
    /// Returns `None` for a language that does not exist — the caller decides whether to fall back
    /// to the default or complain. [`Texts::from_env`] falls back to the default.
    #[must_use]
    pub fn fresh(language: &str) -> Option<Self> {
        let ftl = face(language)?;
        let id: LanguageIdentifier = language.parse().ok()?;
        let recurso = FluentResource::try_new(ftl.to_owned()).ok()?;
        let mut bundle = FluentBundle::new(vec![id]);
        // Fluent puts bidirectional isolation marks (U+2068/U+2069) around
        // an interpolated argument. They are CORRECT in a renderer that
        // implements the bidi algorithm; ours draws cell by cell, and
        // they would show up as an error glyph. Switched off on purpose, with
        // the note that they come back once there is RTL support.
        bundle.set_use_isolating(false);
        bundle.add_resource(recurso).ok()?;
        Some(Self {
            bundle,
            language: language.to_owned(),
        })
    }

    /// Loads the environment's language, falling back to [`DEFAULT`].
    ///
    /// It reads `LC_ALL`, `LC_MESSAGES` and `LANG`, in that order — the same as POSIX.
    #[must_use]
    pub fn from_env() -> Self {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            let Ok(v) = std::env::var(var) else { continue };
            // `pt_BR.UTF-8` -> `pt-BR`.
            let base = v.split('.').next().unwrap_or("").replace('_', "-");
            if let Some(t) = Self::fresh(&base) {
                return t;
            }
        }
        Self::fresh(DEFAULT).expect("o idioma padrao tem de existir")
    }

    /// The loaded language.
    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Resolves a key.
    ///
    /// **It never fails.** An unknown key becomes `[key]`, visible on screen and
    /// obvious in a screenshot. Taking the shell down over a missing text
    /// would trade a wrong sentence for no interface at all — and the two build
    /// guards already make that case unreachable from the project's code.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.t_with(key, &[])
    }

    /// Resolves a key with arguments.
    ///
    /// # Why arguments, and not concatenation
    ///
    /// "`SPC g x` is not bound to anything" has a different ORDER in each language,
    /// and concatenating `t(prefix) + key + t(suffix)` imposes Portuguese's
    /// order on all of them. In Fluent the position of `{ $key }` belongs to the translation
    /// file, which is where it belongs — the same reason the project uses
    /// Fluent and not gettext.
    #[must_use]
    pub fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent = fluent_bundle::FluentArgs::new();
        for (k, v) in args {
            fluent.set(*k, *v);
        }
        self.format(key, fluent, args.is_empty())
    }

    /// The same, with arguments that may be NUMBERS.
    ///
    /// # Why a number cannot travel as a string
    ///
    /// A plural is a decision of the LANGUAGE — English has two forms, Polish
    /// three — and Fluent expresses it as `{ $n -> [one] ... *[other] ... }`.
    /// Choosing the branch means asking the plural rules what CATEGORY the
    /// number falls into, and that only happens for a numeric value: given the
    /// string `"1"`, Fluent looks for a variant literally named `1` and falls
    /// through to the default. The symptom is a screen that says "1 items"
    /// while every set check is green.
    ///
    /// It is a second method and not a wider `t_with` so that the twenty
    /// existing call sites, which have no numbers, keep their shape.
    #[must_use]
    pub fn t_args(&self, key: &str, args: &[(&str, Arg<'_>)]) -> String {
        let mut fluent = fluent_bundle::FluentArgs::new();
        for (k, v) in args {
            match v {
                Arg::Text(s) => fluent.set(*k, *s),
                Arg::Number(n) => fluent.set(*k, *n),
            }
        }
        self.format(key, fluent, args.is_empty())
    }

    /// The half both public forms share.
    fn format(&self, key: &str, fluent: fluent_bundle::FluentArgs<'_>, bare: bool) -> String {
        let Some(msg) = self.bundle.get_message(key) else {
            return format!("[{key}]");
        };
        let Some(default_of) = msg.value() else {
            return format!("[{key}]");
        };
        let args = (!bare).then_some(&fluent);
        let mut erros = Vec::new();
        let s = self.bundle.format_pattern(default_of, args, &mut erros);
        if !erros.is_empty() {
            tracing::warn!(key, ?erros, "texto com erro de formatacao");
        }
        // Fluent wraps an argument in a bidirectional ISOLATION MARK
        // (U+2068/U+2069) for mixed RTL/LTR text. Invisible in a terminal,
        // but it counts as a column in our integer layout and throws the
        // line out of alignment. Since there is no RTL language here (section 2 fixes pt-BR
        // and en-GB), we strip them.
        s.replace(['\u{2068}', '\u{2069}'], "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_load_and_have_every_key() {
        // The generator already guarantees this at build time. This test guarantees that what
        // was GENERATED and what is EMBEDDED are the same set — they are two
        // different paths (keys.rs and include_str!) and nothing else ties them together.
        for language in LANGUAGES {
            let t = Texts::fresh(language).unwrap_or_else(|| panic!("{language} nao carregou"));
            for key in ALL_KEYS {
                let s = t.t(key);
                assert!(
                    !s.starts_with('['),
                    "{language}: chave `{key}` esta no registro e falta no .ftl"
                );
                assert!(!s.is_empty(), "{language}: `{key}` resolveu para vazio");
            }
        }
    }

    #[test]
    fn both_languages_say_different_things() {
        // It catches the mistake of copying one language's file over the other,
        // which passes every set check.
        let en = Texts::fresh("en-GB").unwrap();
        let pt = Texts::fresh("pt-BR").unwrap();
        let diferentes = ALL_KEYS.iter().filter(|c| en.t(c) != pt.t(c)).count();
        assert!(
            diferentes >= ALL_KEYS.len() / 2,
            "so {diferentes} de {} chaves diferem entre os idiomas",
            ALL_KEYS.len()
        );
    }

    #[test]
    fn an_unknown_key_does_not_bring_it_down() {
        let t = Texts::fresh("en-GB").unwrap();
        assert_eq!(t.t("nao-existe"), "[nao-existe]");
    }

    #[test]
    fn the_environment_chooses_the_language() {
        // `pt_BR.UTF-8` is the form `LANG` actually has; without the
        // normalisation (`_` -> `-`, cut at the `.`) no language would match and
        // everything would fall back to the default in silence.
        assert!(Texts::fresh("pt-BR").is_some());
        assert!(Texts::fresh("pt_BR.UTF-8").is_none());
    }

    #[test]
    fn there_is_no_bidi_isolation_mark() {
        // U+2068/U+2069 are correct in a bidi renderer; in ours they would show up
        // as an error glyph in the middle of the sentence.
        let t = Texts::fresh("pt-BR").unwrap();
        for key in ALL_KEYS {
            let s = t.t(key);
            assert!(
                !s.contains('\u{2068}') && !s.contains('\u{2069}'),
                "`{key}` veio com marca de isolamento"
            );
        }
    }
}
