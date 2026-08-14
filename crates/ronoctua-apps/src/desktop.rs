//! The parser for a freedesktop application entry.
//!
//! # The `[Desktop Entry]` group only
//!
//! A parser that reads line by line ignoring the `[...]` headers returns the
//! **last** `Name=` and the **last** `Exec=` in the file — and a `.desktop` often has
//! more than one group. Measured on a actual host: **20 of 108**
//! files carry a `[Desktop Action …]`. Cursor's, for example:
//!
//! ```text
//! [Desktop Entry]
//! Name=Cursor
//! Exec=/usr/share/cursor/cursor %F
//! [Desktop Action new-empty-window]
//! Name=New Empty Window
//! Exec=/usr/share/cursor/cursor --new-window %F
//! ```
//!
//! Without tracking the group, the list shows *"New Empty Window"* and `Enter`
//! launches `--new-window`. Neither of the two looks like a parser bug.
//!
//! # Why not a third-party crate
//!
//! Because the expensive part of one of them — locale negotiation — is something we do not have: the
//! language is ONE, chosen at startup. What is left is single-section INI, and
//! `Exec` would still be on us.

/// The group that describes the application. Everything else in the file is ignored.
const GROUP: &str = "[Desktop Entry]";

/// The `key = value` pairs of the `[Desktop Entry]` group, in file order.
///
/// The key comes raw, **with the language suffix** when there is one (`Name[pt_BR]`)
/// — what chooses between the variants is [`localized`], which needs to see them
/// all.
#[must_use]
pub fn fields(text: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let l = line.trim();
        // A comment and a blank line. The spec only recognises `#` in column 1, but
        // trimming first is harmless and covers sloppy files.
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if l.starts_with('[') {
            // ENTER and LEAVE, in that order: the first `[` after our
            // group ends the reading, and a file may have the group in the middle.
            inside = l == GROUP;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((key, valor)) = l.split_once('=') {
            out.push((key.trim_end(), valor.trim_start()));
        }
    }
    out
}

/// The value of `key`, with no language suffix.
#[must_use]
pub fn raw_bytes<'a>(fields: &[(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// The value of `key` in the requested language, with the spec's cascade.
///
/// `Name[pt_BR]` → `Name[pt]` → `Name`. The language arrives as `pt_BR` (
/// `ronoctua-i18n` speaks `pt-BR`; the bridge is swapping `-` for `_`).
///
/// # Why the cascade matters
///
/// Measured on a actual host: **36 of 108** files have `Name[pt_BR]` and **30**
/// have `Comment[pt_BR]`. Without it, a third of the list stays in English on a
/// Portuguese system — and the worst part is that it stays in English silently, because
/// `Name=` always exists.
#[must_use]
pub fn localized<'a>(fields: &[(&'a str, &'a str)], key: &str, language: &str) -> Option<&'a str> {
    let short = language.split('_').next().unwrap_or(language);
    for target in [language, short] {
        if let Some(v) = fields
            .iter()
            .find(|(k, _)| language_suffix(k, key).is_some_and(|s| s == target))
            .map(|(_, v)| *v)
        {
            return Some(v);
        }
    }
    raw_bytes(fields, key)
}

/// The language inside `k`'s brackets, if `k` is a variant of `key`.
///
/// `("Name[pt_BR]", "Name")` → `Some("pt_BR")`. Only the **encoding** suffix
/// comes off: old files write `Name[pt_BR.UTF-8]`, and comparing it raw would make
/// the Portuguese variant go unnoticed.
///
/// # The `@modifier` STAYS — and it cost a screenful to find out
///
/// It used to come off too, on the grounds that neither of our two languages
/// uses one. The premise was right and the conclusion wrong: with the modifier stripped,
/// **`Name[en@shaw]` becomes `en`** — and `org.gnome.Calculator` showed up in the
/// list written in the Shavian alphabet, `𐑒𐑨𐑤𐑒𐑿𐑤𐑱𐑑𐑼`, which the font does not draw.
/// On screen: a row of tofu, with nothing saying why.
///
/// The spec's rule is the opposite of the one that was here: a key WITH a modifier only
/// serves a locale with the SAME modifier. Since ours never have one,
/// keeping it makes those keys simply never match — which is right. The
/// `sr@latin` that motivated the original line falls back to the generic, as it should.
fn language_suffix<'a>(k: &'a str, key: &str) -> Option<&'a str> {
    let rest = k.strip_prefix(key)?;
    let inside = rest.strip_prefix('[')?.strip_suffix(']')?;
    // The encoding only. The `@modifier` is part of the locale's identity.
    Some(inside.split('.').next().unwrap_or(inside))
}

/// A boolean from the spec: only `true` is true.
#[must_use]
pub fn boolean(fields: &[(&str, &str)], key: &str) -> bool {
    raw_bytes(fields, key).is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// A `;`-separated list, without the empties the trailing `;` produces.
#[must_use]
pub fn list<'a>(fields: &[(&'a str, &'a str)], key: &str) -> Vec<&'a str> {
    raw_bytes(fields, key)
        .map(|v| {
            v.split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{boolean, fields, list, localized, raw_bytes};

    /// THE DEFECT THIS MODULE EXISTS TO PREVENT.
    ///
    /// Without tracking the group, `Name` and `Exec` would come from the `[Desktop Action]` and
    /// Cursor would show up as "New Empty Window".
    #[test]
    fn only_the_desktop_entry_group_is_read() {
        let f = "[Desktop Entry]\n\
                 Name=Cursor\n\
                 Exec=/usr/share/cursor/cursor %F\n\
                 [Desktop Action new-empty-window]\n\
                 Name=New Empty Window\n\
                 Exec=/usr/share/cursor/cursor --new-window %F\n";
        let c = fields(f);
        assert_eq!(raw_bytes(&c, "Name"), Some("Cursor"));
        assert_eq!(raw_bytes(&c, "Exec"), Some("/usr/share/cursor/cursor %F"));
    }

    /// The group may not be the file's first.
    #[test]
    fn a_group_in_the_middle_of_the_file_is_found() {
        let f = "[Outra Coisa]\nName=Errado\n[Desktop Entry]\nName=Certo\n";
        assert_eq!(raw_bytes(&fields(f), "Name"), Some("Certo"));
    }

    #[test]
    fn the_language_cascade_falls_back_to_the_raw_one() {
        let f = "[Desktop Entry]\n\
                 Name=Files\nName[pt]=Ficheiros\nName[pt_BR]=Arquivos\n\
                 Comment=Browse files\nComment[pt]=Navegue\n\
                 GenericName=File Manager\n";
        let c = fields(f);
        assert_eq!(localized(&c, "Name", "pt_BR"), Some("Arquivos"));
        // With no `pt_BR`, it falls back to `pt`.
        assert_eq!(localized(&c, "Comment", "pt_BR"), Some("Navegue"));
        // With neither of the two, it falls back to the raw one.
        assert_eq!(localized(&c, "GenericName", "pt_BR"), Some("File Manager"));
        // Another language with no variant at all.
        assert_eq!(localized(&c, "Name", "de_DE"), Some("Files"));
    }

    /// THE DEFECT, SEEN ON SCREEN: `Name[en@shaw]` matching as `en`.
    ///
    /// The system's `org.gnome.Calculator` has `Name[en@shaw]=𐑒𐑨𐑤𐑒𐑿𐑤𐑱𐑑𐑼` —
    /// the Shavian alphabet, which our font does not draw. With the modifier
    /// discarded, the application list showed a row of tofu, and
    /// nothing on screen said why. A key WITH a modifier only serves a
    /// locale with the SAME modifier.
    #[test]
    fn a_locale_modifier_does_not_match_the_bare_language() {
        let f = "[Desktop Entry]\n\
                 Name=Calculator\n\
                 Name[en_CA]=Calculator\n\
                 Name[en@shaw]=\u{10462}\u{10428}\u{10464}\n\
                 Name[sr@latin]=Kalkulator\n";
        let c = fields(f);
        assert_eq!(
            localized(&c, "Name", "en_US"),
            Some("Calculator"),
            "o shaviano venceu: `en@shaw` nao pode casar `en`"
        );
        assert_eq!(localized(&c, "Name", "sr_RS"), Some("Calculator"));
        // And the ENCODING still comes off: `pt_BR.UTF-8` is the same `pt_BR`.
        let g = fields("[Desktop Entry]\nName=Files\nName[pt_BR.UTF-8]=Arquivos\n");
        assert_eq!(localized(&g, "Name", "pt_BR"), Some("Arquivos"));
    }

    #[test]
    fn a_boolean_is_only_true_on_true() {
        let f = "[Desktop Entry]\nNoDisplay=true\nHidden=False\nTerminal=1\n";
        let c = fields(f);
        assert!(boolean(&c, "NoDisplay"));
        assert!(!boolean(&c, "Hidden"), "`False` e falso");
        assert!(!boolean(&c, "Terminal"), "`1` nao e booleano da spec");
        assert!(!boolean(&c, "Ausente"));
    }

    #[test]
    fn the_list_ignores_the_empty_after_the_trailing_semicolon() {
        let f = "[Desktop Entry]\nCategories=Network;WebBrowser;\n";
        assert_eq!(list(&fields(f), "Categories"), ["Network", "WebBrowser"]);
    }

    #[test]
    fn a_comment_and_a_blank_line_do_not_become_a_field() {
        let f = "[Desktop Entry]\n# Name=Comentado\n\nName=Real\n";
        let c = fields(f);
        assert_eq!(c.len(), 1);
        assert_eq!(raw_bytes(&c, "Name"), Some("Real"));
    }
}
