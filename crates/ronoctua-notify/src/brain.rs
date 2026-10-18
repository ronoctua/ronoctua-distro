//! THE BRAIN MODULES' NOTIFICATIONS, merged into the book at run time.
//!
//! # Why they are not in the generated table
//!
//! A brain module's event that declared a notification is an entry of this
//! catalogue like any other — recognised by its own tag (`<module>-<event>`),
//! written by the same rules (`gen-notifications.py`) — but its module may be
//! the PERSON's, and the generated table is compiled into this crate and every
//! one above it: the whole footer. So the build writes them into the brain's
//! catalogue (`nix/brain-catalogue.nix`), which the footer reads at run time
//! ([`VARIABLE`], the same file `ronoctua_brain::catalogue` reads), and the
//! book takes them in when it is made ([`crate::Book::distro`]).
//!
//! This crate reads ONLY its half of that file: each event's `notification`,
//! already in this crate's own serde shape. What the rest of it means is the
//! brain's.
//!
//! # Read back as it was written, or refused
//!
//! An entry is read into [`Entry`] and written out again, and the two must be
//! the same object: the generator writes every field (`null` for one left to
//! the default), so a field spelt in a way this crate does not know — which
//! `serde`, on a `#[serde(default)]` verdict, would leave at its default in
//! silence — is a refusal with the entry's id, never a notification shown the
//! wrong way.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::catalogue::Entry;

/// The variable naming the brain's catalogue — `ronoctua_brain::catalogue::ENV`,
/// spelt here because this crate does not link the brain.
pub const VARIABLE: &str = "RONOCTUA_BRAIN_CATALOGUE";

/// The catalogue's format this crate reads its half of —
/// `ronoctua_brain::catalogue::VERSION` (the shell binary's test holds the
/// two equal).
pub const VERSION: u32 = 1;

/// The half of the file this crate reads.
#[derive(serde::Deserialize)]
struct File {
    version: u32,
    modules: Vec<Module>,
}

/// A module, as far as its notifications.
#[derive(serde::Deserialize)]
struct Module {
    events: Vec<Event>,
}

/// An event, as far as its notification.
#[derive(serde::Deserialize)]
struct Event {
    notification: Option<serde_json::Value>,
}

/// The entries of a catalogue's text, each read back as it was written.
///
/// # Errors
///
/// A sentence: not JSON of the brain's catalogue, another format, an entry
/// this crate cannot read, or one that does not read back as it was written.
pub fn of_json(text: &str) -> Result<Vec<Entry>, String> {
    let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if file.version != VERSION {
        return Err(format!(
            "it is format {}, and this program reads format {VERSION}",
            file.version
        ));
    }
    let mut out = Vec::new();
    for written in file
        .modules
        .iter()
        .flat_map(|m| m.events.iter())
        .filter_map(|e| e.notification.as_ref())
    {
        // BY ITS ID when it has one, so the sentence names what to fix.
        let named = written
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let entry: Entry = serde_json::from_value(written.clone())
            .map_err(|e| format!("the notification `{named}` cannot be read: {e}"))?;
        let again = serde_json::to_value(&entry).map_err(|e| e.to_string())?;
        if &again != written {
            return Err(format!(
                "the notification `{}` does not read back as it was written: {written} is read as {again}",
                entry.id
            ));
        }
        if entry.when.is_empty() {
            return Err(format!(
                "the notification `{}` recognises everything: an entry is one notification",
                entry.id
            ));
        }
        out.push(entry);
    }
    Ok(out)
}

/// The entries the variable's file holds, read the first time anything asks
/// and kept: the file is a store path.
static READ: OnceLock<Result<Vec<Entry>, String>> = OnceLock::new();

/// The brain modules' entries, or why they cannot be had — naming the
/// variable and the file.
///
/// # Errors
///
/// The sentence: the variable unset, the file unreadable, or not a
/// catalogue whose notifications this crate reads.
pub fn checked() -> Result<&'static [Entry], String> {
    READ.get_or_init(|| {
        let file = std::env::var_os(VARIABLE)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                format!(
                    "{VARIABLE} is not set: the brain modules' notifications are data the build \
                     writes beside the program, and the notification book is not whole without them"
                )
            })?;
        let text = std::fs::read_to_string(&file).map_err(|e| {
            format!(
                "{VARIABLE} points at {}, which cannot be read: {e}",
                file.display()
            )
        })?;
        of_json(&text).map_err(|why| {
            format!(
                "{VARIABLE} points at {}, whose notifications this program cannot read: {why}",
                file.display()
            )
        })
    })
    .as_deref()
    .map_err(Clone::clone)
}

/// The brain modules' entries, for a program past its start guard.
///
/// # Panics
///
/// When they cannot be had, with [`checked`]'s sentence: the footer's start
/// has already refused that case, so only a program run without its wrapper
/// — or a test without the variable — meets it.
#[must_use]
pub fn entries() -> &'static [Entry] {
    checked().unwrap_or_else(|why| panic!("{why}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One entry as the generator writes it: every verdict field, the
    /// variants by their Rust names.
    fn written() -> serde_json::Value {
        serde_json::json!({
            "id": "made-up-done",
            "when": [{ "field": "Tag", "test": { "Is": "made-up-done" } }],
            "apply": {
                "show": "Full", "life": "Quick", "loud": false, "level": 30, "glyph": "\u{f0e16}",
                "blink": null, "vibrate": null, "audio": null, "auto_read": true
            },
            "tags": ["made-up-done", "made-up", "made-up-old"],
            "actions": [{ "id": "open", "label": { "Key": "notif-action-open" }, "doing": { "Command": "made-up-open" } }],
            "rewrite": false
        })
    }

    fn file(notification: serde_json::Value) -> String {
        serde_json::json!({
            "version": VERSION,
            "modules": [{ "id": "made-up", "events": [
                { "name": "done", "notification": notification },
                { "name": "quiet", "notification": null }
            ] }]
        })
        .to_string()
    }

    /// FAILS IF: an entry the generator writes is not read whole — its
    /// recognition, its verdict, its tags and its action — or an event with
    /// no notification becomes one.
    #[test]
    fn an_entry_the_generator_writes_is_read_whole() {
        let entries = of_json(&file(written())).expect("the entry reads");
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.id, "made-up-done");
        assert_eq!(e.summary_key(), "notif-made-up-done-summary");
        assert_eq!(e.apply.level, Some(30));
        assert_eq!(e.apply.auto_read, Some(true));
        assert_eq!(e.apply.glyph, Some('\u{f0e16}'));
        assert_eq!(e.tags.len(), 3);
        assert_eq!(
            e.actions[0].doing,
            crate::Doing::Command("made-up-open".to_owned())
        );
    }

    /// FAILS IF: a field this crate does not know — kebab where serde reads
    /// snake, a variant spelt the generator's way — is left at its default
    /// in silence, or an entry recognising everything, another format or a
    /// file that is not JSON is read.
    #[test]
    fn an_entry_that_does_not_read_back_is_refused() {
        let mut kebab = written();
        let apply = kebab["apply"].as_object_mut().expect("an object");
        apply.remove("auto_read");
        apply.insert("auto-read".into(), serde_json::json!(true));
        assert!(of_json(&file(kebab)).unwrap_err().contains("made-up-done"));
        let mut lower = written();
        lower["apply"]["show"] = serde_json::json!("full");
        assert!(of_json(&file(lower)).unwrap_err().contains("made-up-done"));
        let mut everything = written();
        everything["when"] = serde_json::json!([]);
        assert!(
            of_json(&file(everything))
                .unwrap_err()
                .contains("everything")
        );
        let newer = file(written()).replace(
            &format!("\"version\":{VERSION}"),
            &format!("\"version\":{}", VERSION + 1),
        );
        assert!(of_json(&newer).unwrap_err().contains("format"));
        assert!(of_json("not json").is_err());
    }

    /// FAILS IF: the build's own catalogue (`RONOCTUA_BRAIN_CATALOGUE`, in
    /// the tests' environment) holds a notification this crate cannot read
    /// back as it was written, or holds none — the brain's modules declare
    /// some, and a reader that found nothing would prove nothing.
    #[test]
    fn the_builds_notifications_are_read_back_as_written() {
        let entries = entries();
        assert!(
            !entries.is_empty(),
            "the build's catalogue holds no notification"
        );
        for e in entries {
            assert!(!e.when.is_empty(), "`{}` recognises everything", e.id);
        }
    }
}
