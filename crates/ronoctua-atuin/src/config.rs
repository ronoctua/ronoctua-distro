//! Atuin's `config.toml`, edited a line at a time.
//!
//! # Why this is not a TOML parser
//!
//! Because what it has to do is change ONE line and leave every other byte
//! alone. A parse-and-reserialise would rewrite the whole file: it would lose
//! the comments the person put there, reorder the sections, and turn a
//! hand-written `# search_mode = "fuzzy"  # tried this once` into nothing. The
//! file is theirs.
//!
//! Atuin's settings are scalars — a bool, an integer, a string, a word out of
//! a small set — under FLAT sections (`[sync]`, `[keys]`, `[preview]`,
//! `[daemon]`, `[stats]`). Nothing nests, so a line is a whole setting, which
//! is what makes this shape safe. It is the torrc fragment's shape one file
//! over, and it refuses the same things for the same reasons.
//!
//! # What it refuses
//!
//! A value carrying a newline. It is the same hole `tor::fragment::a_value`
//! names: a pasted value that ends a line and starts another writes a setting
//! nobody asked for, and `history_filter` is a list of regular expressions
//! somebody pastes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where Atuin keeps it, unless the environment says otherwise.
///
/// `ATUIN_CONFIG_DIR` first, then `$XDG_CONFIG_HOME/atuin`. Atuin's own order,
/// and reading it any other way would edit a file the binary does not read.
#[must_use]
pub fn path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ATUIN_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("config.toml"));
    }
    let config = std::env::var_os("XDG_CONFIG_HOME").map_or_else(
        || std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")),
        |c| Some(PathBuf::from(c)),
    )?;
    Some(config.join("atuin").join("config.toml"))
}

/// One setting's place in the file: which section, and which key.
///
/// `None` for the section means the top level, which is where most of Atuin's
/// settings are.
pub type Where<'a> = (Option<&'a str>, &'a str);

/// Is this a value a TOML line could carry, written plainly?
///
/// THE ONE THAT MATTERS. `history_filter` is a list of regular expressions
/// that somebody pastes, and a paste with a newline in it ends this setting
/// and starts whatever came after — in a file that decides what is recorded.
#[must_use]
pub fn a_value(value: &str) -> bool {
    !value.contains(['\n', '\r', '\0'])
}

/// Is this a key TOML would accept bare?
#[must_use]
pub fn a_key(key: &str) -> bool {
    !key.is_empty()
        && key.starts_with(|c: char| c.is_ascii_alphabetic())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Everything the file says, by section and key.
///
/// Values come back EXACTLY as written, quotes included: the view knows what
/// kind each setting is from the declared table, and stripping quotes here
/// would mean guessing.
#[must_use]
pub fn read(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    let mut section: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            section = Some(name.trim().to_owned());
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        out.insert(at(section.as_deref(), key.trim()), value.trim().to_owned());
    }
    out
}

/// The map's key for one setting.
#[must_use]
pub fn at(section: Option<&str>, key: &str) -> String {
    match section {
        Some(s) => format!("{s}.{key}"),
        None => key.to_owned(),
    }
}

/// Writes one setting, changing nothing else.
///
/// It rewrites the line where the setting already is, or appends it under its
/// section — creating the section header when the file has none. Every other
/// byte of the file survives, comments included.
///
/// # Errors
///
/// The key or the value could not go in a TOML line, or the file could not be
/// written.
pub fn put(path: &Path, place: Where<'_>, value: &str) -> anyhow::Result<()> {
    let (section, key) = place;
    anyhow::ensure!(a_key(key), "`{key}` is not a key TOML would accept");
    anyhow::ensure!(
        a_value(value),
        "`{key}` was given a value with a line break in it"
    );
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let written = replaced(&text, section, key, value);
    // ATOMIC, AND THE DRAFT CARRIES THE PID. Two doors are two processes, and
    // two opens of one name with truncate interleave into a file that is
    // neither — the lesson `ronoctua-store` records, which cost a
    // `display.json` that would not parse.
    let draft = path.with_extension(format!("{}.new", std::process::id()));
    std::fs::write(&draft, written.as_bytes())
        .map_err(|e| anyhow::anyhow!("{}: {e}", draft.display()))?;
    std::fs::rename(&draft, path).map_err(|e| {
        let _ = std::fs::remove_file(&draft);
        anyhow::anyhow!("{}: {e}", path.display())
    })
}

/// The file, with one setting changed. Pure, so every rule above is testable.
#[must_use]
pub fn replaced(text: &str, section: Option<&str>, key: &str, value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut here: Option<String> = None;
    let mut done = false;
    let mut section_seen = section.is_none();
    let mut last_of_section: Option<usize> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            here = Some(name.trim().to_owned());
            if here.as_deref() == section {
                section_seen = true;
            }
            out.push(line.to_owned());
            continue;
        }
        let in_place = here.as_deref() == section;
        if in_place {
            last_of_section = Some(out.len());
        }
        if !done
            && in_place
            && !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == key)
        {
            out.push(format!("{key} = {value}"));
            done = true;
            continue;
        }
        out.push(line.to_owned());
    }

    if !done {
        if !section_seen {
            // THE SECTION IS CREATED AT THE END, with a blank line before it:
            // a `[sync]` header glued to the last top-level setting reads as
            // part of it.
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.push(format!("[{}]", section.unwrap_or_default()));
            out.push(format!("{key} = {value}"));
        } else {
            // AFTER THE LAST LINE OF THE SECTION, and not at the end of the
            // file: appended below a LATER section header, the setting would
            // belong to that one instead — which is a setting that appears to
            // do nothing.
            let at = last_of_section.map_or(out.len(), |n| n + 1);
            out.insert(at, format!("{key} = {value}"));
        }
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"## a comment the person wrote
auto_sync = false
search_mode = "fuzzy"

[sync]
records = true

[keys]
scroll_exits = true
"#;

    #[test]
    fn one_line_changes_and_every_other_byte_survives() {
        // THE WHOLE REASON THIS IS NOT A TOML PARSER: a
        // parse-and-reserialise would lose the comment, reorder the sections
        // and rewrite what the person typed. The file is theirs.
        let after = replaced(SAMPLE, None, "auto_sync", "true");
        assert!(after.contains("## a comment the person wrote"), "{after}");
        assert!(after.contains("auto_sync = true"), "{after}");
        assert!(!after.contains("auto_sync = false"), "{after}");
        assert!(after.contains(r#"search_mode = "fuzzy""#), "{after}");
        assert!(after.contains("[sync]"), "{after}");
        assert!(after.contains("records = true"), "{after}");
    }

    #[test]
    fn a_setting_in_a_section_finds_its_own_section() {
        let after = replaced(SAMPLE, Some("sync"), "records", "false");
        assert!(after.contains("records = false"), "{after}");
        // AND THE TOP-LEVEL ONE OF THE SAME NAME WOULD NOT HAVE BEEN TOUCHED.
        let both = format!("records = 1\n{SAMPLE}");
        let after = replaced(&both, Some("sync"), "records", "false");
        assert!(after.starts_with("records = 1\n"), "{after}");
        assert!(after.contains("records = false"), "{after}");
    }

    #[test]
    fn a_setting_that_is_not_there_lands_inside_its_own_section() {
        // THE DEFECT THIS PREVENTS: appended at the end of the FILE, the
        // setting would fall under whatever section header came last — so a
        // top-level `auto_sync` written after `[keys]` becomes `keys.auto_sync`
        // and appears to do nothing at all.
        let after = replaced(SAMPLE, None, "update_check", "false");
        let lines: Vec<&str> = after.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.starts_with("update_check"))
            .unwrap();
        let sync = lines.iter().position(|l| *l == "[sync]").unwrap();
        assert!(at < sync, "it landed under a section header:\n{after}");
    }

    #[test]
    fn a_section_that_does_not_exist_is_created_at_the_end() {
        let after = replaced(SAMPLE, Some("daemon"), "enabled", "true");
        assert!(after.contains("[daemon]\nenabled = true"), "{after}");
        // WITH AIR BEFORE IT: a header glued to the previous setting reads as
        // part of it.
        assert!(!after.contains("scroll_exits = true\n[daemon]"), "{after}");
    }

    #[test]
    fn a_pasted_filter_cannot_smuggle_a_second_setting() {
        // `history_filter` IS A LIST OF REGULAR EXPRESSIONS somebody pastes,
        // in the file that decides what is recorded. A paste with a newline
        // ends this setting and starts whatever came after it.
        assert!(!a_value("[\"^secret\"]\nauto_sync = true"));
        assert!(!a_value("a\rb"));
        assert!(a_value(r#"["^secret", "^export"]"#));
        for bad in ["", "1key", "a key", "a-key"] {
            assert!(!a_key(bad), "`{bad}` was accepted");
        }
        assert!(a_key("search_mode"));
        assert!(a_key("inline_height"));
    }

    #[test]
    fn what_is_written_reads_back_under_the_same_name() {
        let after = replaced(SAMPLE, Some("sync"), "records", "false");
        let dir = std::env::temp_dir().join(format!("ronoctua-atuin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.toml");
        std::fs::write(&file, after).unwrap();
        let held = read(&file);
        assert_eq!(held.get("sync.records").map(String::as_str), Some("false"));
        assert_eq!(held.get("auto_sync").map(String::as_str), Some("false"));
        assert_eq!(
            held.get("search_mode").map(String::as_str),
            Some("\"fuzzy\"")
        );
        // A COMMENTED-OUT SETTING IS NOT A SETTING.
        assert!(!held.contains_key("## a comment the person wrote"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_empty_file_still_takes_a_setting() {
        let after = replaced("", None, "auto_sync", "true");
        assert_eq!(after.trim(), "auto_sync = true");
    }
}
