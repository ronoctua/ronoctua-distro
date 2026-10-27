//! The arithmetic of a list of errors: who said it, what KIND of thing it
//! said, and the book of kinds — seen, not seen, ignored. Pure: entries in,
//! rows out.
//!
//! # A kind is a unit and a template
//!
//! `nginx.service` saying `bind() to 0.0.0.0:80 failed (98: Address in use)`
//! at nine and at ten is one thing going wrong twice, and a list with a row
//! for each would bury the second thing under the first. So the numbers and
//! the ids in a message are folded ([`template`]) — a pid, a port, an address,
//! a uuid, a hex id — and the unit and the folded message together are the
//! kind ([`Kind`]). The row shows the LAST message as it was written; the
//! template is only how two rows are told to be one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::module::Entry;
use crate::moment::Moment;

/// The most kinds kept in the list. Past it, the kind heard longest ago goes —
/// a seen one before one not seen yet.
pub const KINDS_KEPT: usize = 100;

/// The most kinds ignored.
pub const IGNORED_KEPT: usize = 200;

/// The most trechos the person filters by, past the shipped ones; over it,
/// the one of theirs added longest ago goes.
pub const FILTERS_KEPT: usize = 50;

/// The trechos a fresh install filters, each shipped ON — the person opts out
/// of them the way they opt into the module (the manifest's `starts_off`): the
/// row is on its page from the first boot, and a tap turns it off. `dbus-broker`
/// writes `Ignoring duplicate name` at `err` once per name a service file
/// clashes on — dozens on a NixOS boot, one noise — so it is off the mark by
/// default.
pub const DEFAULTS: &[&str] = &["Ignoring duplicate name"];

/// The longest message kept, in bytes (cut on a character's edge): the row
/// shows one line, and the footer the rest of it.
pub const MESSAGE_MOST: usize = 600;

/// The longest template, in bytes: past it, two messages that differ only at
/// the end are the same kind.
pub const TEMPLATE_MOST: usize = 240;

/// How many entries of this boot it reads on its first start — the most of
/// the past it asks the core's one pass for.
pub const BACKFILL_MOST: usize = 200;

/// What a digit or an id becomes in a template.
const FOLDED: char = '#';

/// Who said an entry: the unit it is about, else the unit that wrote it (the
/// person's own first), else the program's name.
///
/// `USER_UNIT` and `UNIT` first — systemd writes about a unit from its own
/// manager (`init.scope`, `user@1000.service`), and the row is about the unit.
/// A scope's name carries a login's number or a random id, so a program in a
/// scope is named by its own identifier: the same program's errors from two
/// logins are one kind.
#[must_use]
pub fn who_of(entry: &Entry) -> String {
    let field = |name: &str| {
        entry
            .fields
            .get(name)
            .filter(|v| !v.is_empty())
            .map(String::as_str)
    };
    let identifier = field("SYSLOG_IDENTIFIER").or_else(|| field("_COMM"));
    let named = field("USER_UNIT")
        .or_else(|| field("UNIT"))
        .or_else(|| field("_SYSTEMD_USER_UNIT"))
        .or_else(|| field("_SYSTEMD_UNIT"));
    match named {
        Some(unit) if unit.ends_with(".scope") && identifier.is_some() => {
            identifier.unwrap_or(unit).to_owned()
        }
        Some(unit) => unit.to_owned(),
        None => identifier.unwrap_or_default().to_owned(),
    }
}

/// A message with its numbers and ids folded: what two occurrences of one kind
/// have in common.
///
/// Word by word (a word is a run of letters and digits): a word that is an id
/// — hexadecimal, eight long or more, or `0x…` — folds whole; in any other,
/// each run of digits folds. What is between words stays. `sda1` is `sda#`,
/// `0.0.0.0:80` is `#.#.#.#:#`, a uuid is the same string whatever uuid.
#[must_use]
pub fn template(message: &str) -> String {
    let message = &without_uuids(message);
    let mut out = String::with_capacity(message.len().min(TEMPLATE_MOST));
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        if is_id(word) {
            out.push(FOLDED);
        } else {
            let mut in_digits = false;
            for c in word.chars() {
                if c.is_ascii_digit() {
                    if !in_digits {
                        out.push(FOLDED);
                    }
                    in_digits = true;
                } else {
                    out.push(c);
                    in_digits = false;
                }
            }
        }
        word.clear();
    };
    for c in message.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
        if out.len() >= TEMPLATE_MOST {
            break;
        }
    }
    flush(&mut word, &mut out);
    cut(&out, TEMPLATE_MOST).to_owned()
}

/// The message with every uuid (`8-4-4-4-12` hexadecimal) folded whole: its
/// five parts are words of five different shapes, and two uuids would
/// otherwise fold into two templates.
fn without_uuids(message: &str) -> String {
    const SHAPE: [usize; 5] = [8, 4, 4, 4, 12];
    const LENGTH: usize = 36;
    let bytes = message.as_bytes();
    let is_uuid_at = |at: usize| {
        let Some(chunk) = bytes.get(at..at + LENGTH) else {
            return false;
        };
        let edge_before = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let edge_after = bytes
            .get(at + LENGTH)
            .is_none_or(|b| !b.is_ascii_alphanumeric());
        let mut i = 0;
        for (n, len) in SHAPE.iter().enumerate() {
            if !chunk[i..i + len].iter().all(u8::is_ascii_hexdigit) {
                return false;
            }
            i += len;
            if n < SHAPE.len() - 1 {
                if chunk[i] != b'-' {
                    return false;
                }
                i += 1;
            }
        }
        edge_before && edge_after
    };
    let mut out = String::with_capacity(message.len());
    let mut at = 0;
    while at < message.len() {
        if is_uuid_at(at) {
            out.push(FOLDED);
            at += LENGTH;
            continue;
        }
        let Some(c) = message[at..].chars().next() else {
            break;
        };
        out.push(c);
        at += c.len_utf8();
    }
    out
}

/// Whether a word is an id and not a word: `0x` and hexadecimal, or eight
/// hexadecimal characters or more with a digit among them.
fn is_id(word: &str) -> bool {
    let hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
    if let Some(rest) = word.strip_prefix("0x") {
        return hex(rest);
    }
    word.len() >= 8 && hex(word) && word.chars().any(|c| c.is_ascii_digit())
}

/// The line a row shows and a filter is matched against: `<who>: <message>`,
/// or the message alone when nothing said it — the journal's own line, as
/// `journalctl` prints it.
#[must_use]
pub fn line(who: &str, message: &str) -> String {
    if who.is_empty() {
        message.to_owned()
    } else {
        format!("{who}: {message}")
    }
}

/// `text` cut to at most `most` bytes, on a character's edge.
#[must_use]
pub fn cut(text: &str, most: usize) -> &str {
    if text.len() <= most {
        return text;
    }
    let mut end = most;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// A kind of error: who said it, and the shape of what it said.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Kind {
    /// The unit, or the program.
    pub who: String,
    /// The message, folded.
    pub template: String,
}

impl Kind {
    /// The kind of an entry.
    #[must_use]
    pub fn of(entry: &Entry) -> Self {
        Self {
            who: who_of(entry),
            template: template(&entry.message),
        }
    }

    /// Its id on the page: sixteen hexadecimal digits, the same for the same
    /// kind in every process and every run (FNV-1a over both fields). DATA —
    /// a row's id the page hands back.
    #[must_use]
    pub fn id(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self
            .who
            .bytes()
            .chain(std::iter::once(0))
            .chain(self.template.bytes())
        {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }
}

/// One kind in the list.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Group {
    /// Which.
    pub kind: Kind,
    /// Its last message, as written.
    pub message: String,
    /// When it was first heard.
    pub first: Moment,
    /// When it was last heard.
    pub last: Moment,
    /// How many times, in all.
    pub count: u64,
    /// How many times since it was last marked seen; `0` is seen.
    pub unseen: u64,
    /// The boot of its last occurrence (`_BOOT_ID`, 32 hexadecimal digits).
    pub boot: String,
}

/// A kind ignored, with a message of it to recognise it by.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Ignored {
    /// Which.
    pub kind: Kind,
    /// Its last message, when it was ignored.
    pub message: String,
    /// How many times it has been heard since. STATE, not the person's
    /// choice: kept in the list's file, never in theirs, so their file
    /// changes only when they act.
    #[serde(skip)]
    pub since: u64,
}

/// Where a filter came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    /// Shipped with the brain ([`DEFAULTS`]): a tap turns it off, and it stays
    /// on the page to turn on again.
    #[default]
    Default,
    /// The person typed it: a tap takes it off the page.
    Custom,
}

/// A trecho the person filters the journal's errors by. A line whose
/// `<who>: <message>` holds it — case folded, so `ERROR` and `error` are one —
/// is counted here and never reaches the list or the mark.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filter {
    /// The trecho, as it was written or shipped.
    pub text: String,
    /// It, case folded: the identity, and what a line is searched for.
    pub folded: String,
    /// Shipped, or the person's.
    pub origin: Origin,
    /// On the page, not filtering.
    pub off: bool,
    /// How many lines it has caught. STATE, like [`Ignored::since`].
    pub since: u64,
    /// The last line it caught, to show what it is catching.
    pub message: String,
}

impl Filter {
    /// A shipped filter, on.
    #[must_use]
    pub fn shipped(text: &str) -> Self {
        Self::new(text, Origin::Default, false)
    }

    /// The person's own, on or off.
    #[must_use]
    pub fn own(text: &str, off: bool) -> Self {
        Self::new(text, Origin::Custom, off)
    }

    fn new(text: &str, origin: Origin, off: bool) -> Self {
        Self {
            text: text.to_owned(),
            folded: text.to_lowercase(),
            origin,
            off,
            since: 0,
            message: String::new(),
        }
    }

    /// Its id on the page: sixteen hexadecimal digits of the folded trecho
    /// (FNV-1a), the same across runs — DATA, the row's id the page hands back.
    #[must_use]
    pub fn id(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.folded.bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }
}

/// The book of kinds: the list, what is ignored, and what is filtered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Book {
    /// The list, by kind.
    pub groups: BTreeMap<Kind, Group>,
    /// What is ignored, by kind.
    pub ignored: BTreeMap<Kind, Ignored>,
    /// What is filtered, by trecho: the shipped ones and the person's, each on
    /// or off. Built from [`DEFAULTS`] and the person's file ([`Book::set_filters`]).
    pub filters: Vec<Filter>,
}

/// What hearing one entry did to the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// A kind not seen before is in the list, not seen.
    New,
    /// A kind in the list happened again.
    Again,
    /// A kind that is ignored happened; counted, and nothing else.
    Ignored,
    /// A line a filter holds happened; counted on the filter, and nothing else.
    Filtered,
}

impl Book {
    /// One entry heard at `at` (the journal's second).
    pub fn hear(&mut self, entry: &Entry, at: Moment) -> Heard {
        let kind = Kind::of(entry);
        // A FILTER FIRST: a line a trecho holds is counted on the trecho and
        // is never a kind — off the list and off the mark.
        let shown = line(&kind.who, &entry.message);
        let hay = shown.to_lowercase();
        if let Some(f) = self
            .filters
            .iter_mut()
            .find(|f| !f.off && !f.folded.is_empty() && hay.contains(&f.folded))
        {
            f.since += 1;
            f.message = cut(&shown, MESSAGE_MOST).to_owned();
            return Heard::Filtered;
        }
        if let Some(ignored) = self.ignored.get_mut(&kind) {
            ignored.since += 1;
            return Heard::Ignored;
        }
        let message = cut(&entry.message, MESSAGE_MOST).to_owned();
        let boot = entry
            .fields
            .get(ronoctua_journal::BOOT_ID)
            .cloned()
            .unwrap_or_default();
        let heard = match self.groups.get_mut(&kind) {
            Some(group) => {
                group.message = message;
                group.last = group.last.max(at);
                group.first = group.first.min(at);
                group.count += 1;
                group.unseen += 1;
                group.boot = boot;
                Heard::Again
            }
            None => {
                self.groups.insert(
                    kind.clone(),
                    Group {
                        kind,
                        message,
                        first: at,
                        last: at,
                        count: 1,
                        unseen: 1,
                        boot,
                    },
                );
                Heard::New
            }
        };
        self.bound();
        heard
    }

    /// Keeps the list to [`KINDS_KEPT`]: the kind heard longest ago goes,
    /// a seen one before one not seen yet.
    fn bound(&mut self) {
        while self.groups.len() > KINDS_KEPT {
            let Some(oldest) = self
                .groups
                .values()
                .min_by_key(|g| (g.unseen > 0, g.last))
                .map(|g| g.kind.clone())
            else {
                return;
            };
            self.groups.remove(&oldest);
        }
    }

    /// How many kinds are not seen yet — the mark's number.
    #[must_use]
    pub fn unseen(&self) -> usize {
        self.groups.values().filter(|g| g.unseen > 0).count()
    }

    /// The kind in the list with this id.
    #[must_use]
    pub fn group(&self, id: &str) -> Option<&Kind> {
        self.groups.keys().find(|k| k.id() == id)
    }

    /// The ignored kind with this id.
    #[must_use]
    pub fn ignored_kind(&self, id: &str) -> Option<&Kind> {
        self.ignored.keys().find(|k| k.id() == id)
    }

    /// Marks a kind seen. `false` when it is not in the list.
    pub fn see(&mut self, id: &str) -> bool {
        let Some(kind) = self.group(id).cloned() else {
            return false;
        };
        if let Some(g) = self.groups.get_mut(&kind) {
            g.unseen = 0;
        }
        true
    }

    /// Marks every kind seen; how many were not.
    pub fn see_all(&mut self) -> usize {
        let mut seen = 0;
        for g in self.groups.values_mut().filter(|g| g.unseen > 0) {
            g.unseen = 0;
            seen += 1;
        }
        seen
    }

    /// Ignores a kind from now on: out of the list, into the ignored.
    /// `false` when it is not in the list.
    pub fn ignore(&mut self, id: &str) -> bool {
        let Some(kind) = self.group(id).cloned() else {
            return false;
        };
        let Some(group) = self.groups.remove(&kind) else {
            return false;
        };
        self.ignored.insert(
            kind.clone(),
            Ignored {
                kind,
                message: group.message,
                since: 0,
            },
        );
        while self.ignored.len() > IGNORED_KEPT {
            let Some(first) = self.ignored.keys().next().cloned() else {
                break;
            };
            self.ignored.remove(&first);
        }
        true
    }

    /// Hears a kind again. `false` when it was not ignored.
    pub fn unignore(&mut self, id: &str) -> bool {
        let Some(kind) = self.ignored_kind(id).cloned() else {
            return false;
        };
        self.ignored.remove(&kind).is_some()
    }

    /// The filters, for the page.
    #[must_use]
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Rebuilds the filters from the shipped [`DEFAULTS`] and the person's
    /// choices — each a trecho and whether it is off — keeping the count the
    /// brain had for a trecho it still has. The person's choices are the base,
    /// so a trecho they added or took out by hand stays; this sweeps nothing
    /// ([`Book::apply_filters`] does, once the counts are back).
    pub fn set_filters(&mut self, choices: &[(String, bool)]) {
        let kept: Vec<(String, u64, String)> = self
            .filters
            .iter()
            .map(|f| (f.folded.clone(), f.since, f.message.clone()))
            .collect();
        let mut out: Vec<Filter> = DEFAULTS.iter().map(|t| Filter::shipped(t)).collect();
        for (text, off) in choices {
            if text.trim().is_empty() {
                continue;
            }
            let folded = text.to_lowercase();
            if let Some(f) = out.iter_mut().find(|f| f.folded == folded) {
                f.off = *off;
            } else {
                out.push(Filter::own(text, *off));
            }
        }
        for f in &mut out {
            if let Some((_, since, message)) = kept.iter().find(|(folded, ..)| *folded == f.folded)
            {
                f.since = *since;
                f.message = message.clone();
            }
        }
        self.filters = out;
    }

    /// Filters by a trecho from now on, and sweeps what was already heard. A
    /// trecho already there is turned on if it was off. The oldest of the
    /// person's own goes once there are more than [`FILTERS_KEPT`] of them.
    pub fn filter(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let folded = text.to_lowercase();
        if let Some(f) = self.filters.iter_mut().find(|f| f.folded == folded) {
            f.off = false;
        } else {
            self.filters.push(Filter::own(text, false));
            while self
                .filters
                .iter()
                .filter(|f| f.origin == Origin::Custom)
                .count()
                > FILTERS_KEPT
            {
                let Some(pos) = self.filters.iter().position(|f| f.origin == Origin::Custom) else {
                    break;
                };
                self.filters.remove(pos);
            }
        }
        self.apply_filters();
    }

    /// Takes a filter off, by id: the person's own leaves the page; a shipped
    /// one stays, off, to turn on again. `false` when there is none with this id.
    pub fn unfilter(&mut self, id: &str) -> bool {
        let Some(pos) = self.filters.iter().position(|f| f.id() == id) else {
            return false;
        };
        match self.filters[pos].origin {
            Origin::Custom => {
                self.filters.remove(pos);
            }
            Origin::Default => self.filters[pos].off = true,
        }
        true
    }

    /// Turns a filter back on, by id, and sweeps what was already heard.
    /// `false` when there is none with this id.
    pub fn refilter(&mut self, id: &str) -> bool {
        let Some(f) = self.filters.iter_mut().find(|f| f.id() == id) else {
            return false;
        };
        if f.off {
            f.off = false;
            self.apply_filters();
        }
        true
    }

    /// Sweeps the list of every kind an active filter now holds: it leaves the
    /// list (and the mark), its count folded onto the filter. What a new or
    /// turned-on filter, or a start reading the kept list, does to what was
    /// already heard.
    pub fn apply_filters(&mut self) {
        let filters = &mut self.filters;
        self.groups.retain(|kind, g| {
            let shown = line(&kind.who, &g.message);
            let hay = shown.to_lowercase();
            match filters
                .iter_mut()
                .find(|f| !f.off && !f.folded.is_empty() && hay.contains(&f.folded))
            {
                Some(f) => {
                    f.since += g.count;
                    f.message = cut(&shown, MESSAGE_MOST).to_owned();
                    false
                }
                None => true,
            }
        });
    }

    /// Forgets the kinds seen whose last occurrence was in another boot —
    /// what a start does: they were read, and they are over.
    pub fn forget_seen_before(&mut self, boot: &str) {
        if boot.is_empty() {
            return;
        }
        self.groups.retain(|_, g| g.unseen > 0 || g.boot == boot);
    }

    /// The list as its page draws it: the kinds not seen yet of this boot,
    /// those of earlier boots, the seen ones, each newest first — and what is
    /// ignored.
    #[must_use]
    pub fn sorted(&self, boot: &str) -> Sorted<'_> {
        let mut out = Sorted::default();
        for g in self.groups.values() {
            let this_boot = boot.is_empty() || g.boot == boot;
            match (g.unseen > 0, this_boot) {
                (true, true) => out.unseen.push(g),
                (true, false) => out.earlier.push(g),
                (false, _) => out.seen.push(g),
            }
        }
        for list in [&mut out.unseen, &mut out.earlier, &mut out.seen] {
            list.sort_by(|a, b| b.last.cmp(&a.last).then(a.kind.cmp(&b.kind)));
        }
        out.ignored = self.ignored.values().collect();
        out
    }
}

/// The list, in the page's order.
#[derive(Debug, Default)]
pub struct Sorted<'a> {
    /// Not seen yet, this boot.
    pub unseen: Vec<&'a Group>,
    /// Not seen yet, from an earlier boot.
    pub earlier: Vec<&'a Group>,
    /// Seen.
    pub seen: Vec<&'a Group>,
    /// Ignored.
    pub ignored: Vec<&'a Ignored>,
}

/// The kernel's boot id in the journal's spelling: 32 hexadecimal digits,
/// lower case, no dashes — `/proc` writes it as a uuid.
#[must_use]
pub fn journal_boot(proc_boot_id: &str) -> String {
    proc_boot_id
        .trim()
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(fields: &[(&str, &str)], message: &str) -> Entry {
        Entry {
            fields: fields
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            message: message.to_owned(),
            ..Entry::default()
        }
    }

    #[test]
    fn numbers_and_ids_fold_and_words_stay() {
        assert_eq!(
            template("bind() to 0.0.0.0:80 failed (98: Address in use)"),
            "bind() to #.#.#.#:# failed (#: Address in use)"
        );
        assert_eq!(
            template("I/O error, dev sda1, sector 123456 op 0x1:(WRITE)"),
            "I/O error, dev sda#, sector # op #:(WRITE)"
        );
        // TWO UUIDS ARE ONE SHAPE, and a hex id folds whole.
        let a = template("session 123e4567-e89b-12d3-a456-426614174000 failed");
        let b = template("session 9f8e7d6c-5b4a-3921-8a7b-6c5d4e3f2a10 failed");
        assert_eq!(a, b);
        assert_eq!(a, "session # failed");
        assert_eq!(template("commit deadbeef12 lost"), "commit # lost");
        // A WORD IS NOT AN ID because it could be hexadecimal: `added` stays.
        assert_eq!(template("added feed cafe"), "added feed cafe");
        // AND A MESSAGE LONGER THAN THE LIMIT is cut on a character's edge.
        let long = "é".repeat(TEMPLATE_MOST);
        assert!(template(&long).len() <= TEMPLATE_MOST);
        assert_eq!(cut("aé", 2), "a");
    }

    #[test]
    fn who_is_the_unit_it_is_about_and_a_scope_is_its_program() {
        assert_eq!(
            who_of(&entry(
                &[("UNIT", "nginx.service"), ("_SYSTEMD_UNIT", "init.scope")],
                "x"
            )),
            "nginx.service"
        );
        assert_eq!(
            who_of(&entry(&[("USER_UNIT", "syncthing.service")], "x")),
            "syncthing.service"
        );
        let in_scope = entry(
            &[
                ("SYSLOG_IDENTIFIER", "sudo"),
                ("_SYSTEMD_UNIT", "session-2.scope"),
            ],
            "x",
        );
        assert_eq!(who_of(&in_scope), "sudo");
        assert_eq!(
            who_of(&entry(
                &[("SYSLOG_IDENTIFIER", "kernel"), ("_TRANSPORT", "kernel")],
                "x"
            )),
            "kernel"
        );
        assert_eq!(who_of(&entry(&[], "x")), "");
    }

    #[test]
    fn one_kind_is_one_row_and_its_count_and_unseen_grow() {
        let mut book = Book::default();
        let at = |n: &str| {
            entry(
                &[("_SYSTEMD_UNIT", "a.service"), ("_BOOT_ID", "b1")],
                &format!("port {n} refused"),
            )
        };
        assert_eq!(book.hear(&at("80"), 10), Heard::New);
        assert_eq!(book.hear(&at("443"), 20), Heard::Again);
        assert_eq!(book.groups.len(), 1);
        let g = book.groups.values().next().expect("one group");
        assert_eq!((g.count, g.unseen, g.first, g.last), (2, 2, 10, 20));
        assert_eq!(
            g.message, "port 443 refused",
            "the row shows the last message"
        );
        assert_eq!(book.unseen(), 1);
        // SEEN: the mark goes, the row stays.
        let id = g.kind.id();
        assert!(book.see(&id));
        assert_eq!(book.unseen(), 0);
        assert_eq!(book.groups.len(), 1);
        // AGAIN AFTER SEEN is news again.
        assert_eq!(book.hear(&at("8080"), 30), Heard::Again);
        assert_eq!(book.unseen(), 1);
        assert_eq!(book.groups.values().next().map(|g| g.count), Some(3));
    }

    #[test]
    fn an_ignored_kind_is_counted_and_never_listed_until_heard_again() {
        let mut book = Book::default();
        let e = entry(&[("_SYSTEMD_UNIT", "noisy.service")], "tick 1 failed");
        book.hear(&e, 1);
        let id = Kind::of(&e).id();
        assert!(book.ignore(&id));
        assert!(!book.ignore(&id), "ignored twice");
        assert!(book.groups.is_empty());
        assert_eq!(book.hear(&e, 2), Heard::Ignored);
        assert_eq!(book.unseen(), 0, "an ignored kind marked the modeline");
        assert_eq!(book.ignored.values().next().map(|i| i.since), Some(1));
        assert!(book.unignore(&id));
        assert!(!book.unignore(&id));
        assert_eq!(book.hear(&e, 3), Heard::New);
        assert!(!book.see("0000000000000000"), "a row nobody has was seen");
    }

    #[test]
    fn the_list_is_bounded_and_a_seen_kind_goes_first() {
        let mut book = Book::default();
        for n in 0..KINDS_KEPT {
            book.hear(
                &entry(&[("_SYSTEMD_UNIT", &format!("u{n}x.service"))], "x"),
                i64::try_from(n).unwrap_or(0) + 100,
            );
        }
        // The NEWEST is seen, the oldest not: the seen one goes first.
        let newest = book
            .groups
            .values()
            .max_by_key(|g| g.last)
            .map(|g| g.kind.id())
            .expect("a group");
        assert!(book.see(&newest));
        book.hear(&entry(&[("_SYSTEMD_UNIT", "late.service")], "x"), 1000);
        assert_eq!(book.groups.len(), KINDS_KEPT);
        assert!(
            book.group(&newest).is_none(),
            "an unseen kind went before a seen one"
        );
    }

    #[test]
    fn the_page_splits_this_boot_from_earlier_ones_and_seen_from_not() {
        let mut book = Book::default();
        let e = |unit: &str, boot: &str| {
            entry(&[("_SYSTEMD_UNIT", unit), ("_BOOT_ID", boot)], "failed")
        };
        book.hear(&e("now.service", "b2"), 50);
        book.hear(&e("then.service", "b1"), 40);
        book.hear(&e("read.service", "b1"), 30);
        let read = Kind::of(&e("read.service", "b1")).id();
        book.see(&read);
        let sorted = book.sorted("b2");
        let who = |gs: &[&Group]| gs.iter().map(|g| g.kind.who.clone()).collect::<Vec<_>>();
        assert_eq!(who(&sorted.unseen), ["now.service"]);
        assert_eq!(who(&sorted.earlier), ["then.service"]);
        assert_eq!(who(&sorted.seen), ["read.service"]);
        // A START FORGETS WHAT WAS SEEN IN ANOTHER BOOT, and keeps the rest.
        book.forget_seen_before("b2");
        assert_eq!(book.groups.len(), 2);
        assert!(book.group(&read).is_none());
    }

    #[test]
    fn the_ids_are_stable_and_the_boot_is_the_journals_spelling() {
        let k = Kind {
            who: "a".to_owned(),
            template: "b".to_owned(),
        };
        let same = Kind {
            who: "a".to_owned(),
            template: "b".to_owned(),
        };
        assert_eq!(k.id(), same.id());
        assert_eq!(k.id().len(), 16);
        assert_ne!(
            k.id(),
            Kind {
                who: "ab".to_owned(),
                template: String::new()
            }
            .id(),
            "the separator did not keep two kinds apart"
        );
        assert_eq!(
            journal_boot("0123ABCD-4567-89ab-cdef-0123456789ab\n"),
            "0123abcd456789abcdef0123456789ab"
        );
    }

    #[test]
    fn a_filter_counts_a_line_and_keeps_it_off_the_list_and_the_mark() {
        let mut book = Book::default();
        // THE SHIPPED DEFAULT, on, and nothing of the person's.
        book.set_filters(&[]);
        assert_eq!(book.filters().len(), 1);
        assert!(!book.filters()[0].off);
        // A dbus line at err, on the system bus and on the person's — one kind
        // each by `who`, both held by the one trecho, case folded.
        let sys = entry(
            &[("_SYSTEMD_UNIT", "dbus-broker.service")],
            "Ignoring duplicate name 'org.freedesktop.UPower' in service file '/nix/store/x'",
        );
        let usr = entry(
            &[("_SYSTEMD_UNIT", "user@1000.service")],
            "ignoring DUPLICATE name 'org.freedesktop.portal.Flatpak' in service file '/y'",
        );
        assert_eq!(book.hear(&sys, 10), Heard::Filtered);
        assert_eq!(book.hear(&usr, 20), Heard::Filtered);
        assert!(book.groups.is_empty(), "a filtered line became a kind");
        assert_eq!(book.unseen(), 0, "a filtered line marked the modeline");
        assert_eq!(book.filters()[0].since, 2, "both buses, one trecho");
        // A line the trecho does not hold is an error as ever.
        let other = entry(&[("_SYSTEMD_UNIT", "nginx.service")], "bind() failed");
        assert_eq!(book.hear(&other, 30), Heard::New);
        assert_eq!(book.unseen(), 1);
    }

    #[test]
    fn a_turned_on_filter_sweeps_what_was_heard_and_off_is_not_retroactive() {
        let mut book = Book::default();
        book.set_filters(&[]);
        // Turn the default off: the dbus lines count as errors.
        let id = book.filters()[0].id();
        assert!(book.unfilter(&id));
        let dbus = |n: &str| {
            entry(
                &[("_SYSTEMD_UNIT", "dbus-broker.service")],
                &format!("Ignoring duplicate name '{n}' in service file '/x'"),
            )
        };
        // Each name is its own kind.
        assert_eq!(book.hear(&dbus("a"), 10), Heard::New);
        assert_eq!(book.hear(&dbus("b"), 20), Heard::New);
        assert_eq!(book.unseen(), 2);
        // Turn it back on: the kinds leave the list and the mark, their counts
        // folded onto the filter.
        assert!(book.refilter(&id));
        assert!(book.groups.is_empty());
        assert_eq!(book.unseen(), 0);
        assert_eq!(book.filters()[0].since, 2);
        // OFF AGAIN IS NOT RETROACTIVE: what it caught is gone, and a line of
        // that kind is news again.
        assert!(book.unfilter(&id));
        assert_eq!(book.hear(&dbus("a"), 30), Heard::New);
        assert_eq!(book.unseen(), 1);
    }

    #[test]
    fn the_persons_choices_rebuild_the_filters_and_keep_the_counts() {
        let mut book = Book::default();
        book.set_filters(&[]);
        book.hear(
            &entry(
                &[("_SYSTEMD_UNIT", "dbus-broker.service")],
                "Ignoring duplicate name 'x'",
            ),
            1,
        );
        assert_eq!(book.filters()[0].since, 1);
        // A person's file: the default turned off, and one of their own.
        book.set_filters(&[
            ("Ignoring duplicate name".to_owned(), true),
            ("timed out".to_owned(), false),
        ]);
        assert_eq!(book.filters().len(), 2);
        let def = book
            .filters()
            .iter()
            .find(|f| f.origin == Origin::Default)
            .expect("the default");
        assert!(def.off, "the person's choice was not kept");
        assert_eq!(def.since, 1, "the count the brain had was lost");
        assert!(
            book.filters()
                .iter()
                .any(|f| f.origin == Origin::Custom && f.text == "timed out")
        );
        // THE CAP keeps the person's own bounded.
        for n in 0..=FILTERS_KEPT {
            book.filter(&format!("trecho {n}"));
        }
        assert!(
            book.filters()
                .iter()
                .filter(|f| f.origin == Origin::Custom)
                .count()
                <= FILTERS_KEPT
        );
    }
}
