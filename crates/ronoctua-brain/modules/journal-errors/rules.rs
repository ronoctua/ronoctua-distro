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
//!
//! Only its first line counts ([`headline`]): what a crash writes under it is
//! a stack, different every time. The number in a unit's instance folds too
//! ([`fold_instance`]), or each crash would be a kind of its own named after
//! its pid, and so does one failure told in two lines ([`FAILED`]).
//!
//! # Two lists, so a warning never pushes out an error
//!
//! What is worth a person's look is in [`Book::groups`], on the mark. A plain
//! warning ([`Severity::Warning`]) is in [`Book::warnings`], a list of its own
//! with its own limit, off the mark: a noisy hour of warnings has a hundred
//! errors to push out of one list, and a list of errors has no room to lose.
//!
//! # What a filter counts is this boot
//!
//! A number that only grows says nothing after a month. So a filter counts the
//! lines of the boot in [`Book::boot`], and a line an older boot wrote, held
//! all the same, is not counted.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::module::Entry;
use crate::moment::Moment;

use super::classify;
use super::noise::{SHIPPED, Shipped};

/// The most kinds kept in the list. Past it, the kind heard longest ago goes —
/// a seen one before one not seen yet, a milder one before a graver.
pub const KINDS_KEPT: usize = 100;

/// The most kinds of plain warning kept, apart from the list. Past it, the
/// one heard longest ago goes.
pub const WARNINGS_KEPT: usize = 40;

/// The most kinds ignored.
pub const IGNORED_KEPT: usize = 200;

/// The most snippets the person filters by, past the shipped ones; over it,
/// the one of theirs added longest ago goes.
pub const FILTERS_KEPT: usize = 50;

/// The longest reason a person gives a filter, in bytes (cut on a character's
/// edge).
pub const REASON_MOST: usize = 200;

/// The longest message kept, in bytes (cut on a character's edge): the row
/// shows one line, and the footer the rest of it.
pub const MESSAGE_MOST: usize = 600;

/// The longest template, in bytes: past it, two messages that differ only at
/// the end are the same kind.
pub const TEMPLATE_MOST: usize = 240;

/// The most errors of this boot's past it asks the core's one pass for — which
/// runs on the brain's first start alone, and reaches it only if it is on then.
pub const BACKFILL_MOST: usize = 200;

/// The most warnings of it: a ring of their own, so they cannot push out an
/// error.
pub const BACKFILL_WARNINGS: usize = 100;

/// The most each source keeps of a pass over what it missed, the newest ones.
pub const PASS_MOST: usize = 1000;

/// What a digit or an id becomes in a template.
const FOLDED: char = '#';

/// What a unit's failure is as a template. A failing unit writes two lines,
/// `X: Failed with result 'exit-code'.` and `Failed to start X.`, and a person
/// reads one thing: they are one kind, whose row shows whichever came last.
const FAILED: &str = "#failed";

/// `MESSAGE_ID` of `X: Failed with result …`.
const UNIT_RESULT_MESSAGE: &str = "d9b373ed55a64feb8242e02dbe79a49c";

/// `MESSAGE_ID` of `Failed to start X`.
const UNIT_FAILED_MESSAGE: &str = "be02cf6855d2428ba40df7e9d022f03d";

/// What follows the unit in the first of a failing unit's lines, in a file
/// written before [`FAILED`].
const FAILED_RESULT_WORDS: &str = ": Failed with result '";

/// What the other line begins with there, whichever the unit's type: the
/// same `MESSAGE_ID` as `Failed to start X`, in systemd's own words.
const FAILED_JOB_WORDS: &[&str] = &[
    "Failed to start ",
    "Failed to mount ",
    "Failed to activate swap ",
    "Failed to set up automount ",
    "Timed out starting ",
    "Timed out mounting ",
    "Timed out activating swap ",
    "Timed out waiting for device ",
    "Dependency failed for ",
];

/// The types of unit whose failure is [`FAILED`]: all but a scope, which
/// [`who_of`] names by its program.
const UNIT_TYPES: &[&str] = &[
    "service",
    "socket",
    "target",
    "device",
    "mount",
    "automount",
    "swap",
    "timer",
    "path",
    "slice",
];

/// The alphabet of a Nix hash: no `e`, no `o`, no `t`, no `u`.
const NIX_BASE32: &str = "0123456789abcdfghijklmnpqrsvwxyz";

/// How long a Nix hash is.
const NIX_HASH: usize = 32;

/// How grave a kind is, the gravest first: [`Ord`] puts it below a milder one,
/// so the worst of two is their `min`.
///
/// Never in a [`Kind`]: a kind that gets worse is still that kind, with the same
/// id, the same row and the same place in `ignored.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Something was lost, or is about to be: a crash, or `crit` and above.
    Critical,
    /// An error.
    Error,
    /// A warning that says something ended badly ([`classify`]): on the list,
    /// on the mark.
    Notable,
    /// A warning and no more: in its own list, off the mark.
    Warning,
}

/// What an entry said: its `MESSAGE`, or a coredump's two short fields when
/// the message is gone. `journalctl --output=json` sends a field over 4096
/// bytes as `null`, and a crash with a long stack has no `MESSAGE` to read.
/// The program in brackets, as the message itself writes it, so what a filter
/// holds there it holds here.
#[must_use]
pub fn said(entry: &Entry) -> Cow<'_, str> {
    if !entry.message.is_empty() {
        return Cow::Borrowed(&entry.message);
    }
    let field = |name: &str| entry.fields.get(name).map_or("", String::as_str);
    match (field("COREDUMP_COMM"), field("COREDUMP_SIGNAL_NAME")) {
        ("", _) => Cow::Borrowed(""),
        (program, signal) => Cow::Owned(format!("({program}) {signal}").trim_end().to_owned()),
    }
}

/// What a row shows of a message: its first line that says anything. A crash
/// writes its one sentence and then a stack, line after line.
#[must_use]
pub fn headline(message: &str) -> &str {
    message
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

/// A unit whose instance is only numbers, folded: `systemd-coredump@3-1742-0.service`
/// is `systemd-coredump@#.service`, the same thing each time it happens. An
/// instance with a letter in it stays, because it says which one
/// (`a-unit@chrome.service`).
#[must_use]
pub fn fold_instance(unit: &str) -> Cow<'_, str> {
    let Some((name, rest)) = unit.split_once('@') else {
        return Cow::Borrowed(unit);
    };
    let Some((instance, suffix)) = rest.rsplit_once('.') else {
        return Cow::Borrowed(unit);
    };
    let is_a_number = |b: u8| b.is_ascii_digit() || b == b'-' || b == b'_';
    if instance.is_empty() || !instance.bytes().all(is_a_number) {
        return Cow::Borrowed(unit);
    }
    Cow::Owned(format!("{name}@{FOLDED}.{suffix}"))
}

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
        Some(unit) => fold_instance(unit).into_owned(),
        None => identifier.unwrap_or_default().to_owned(),
    }
}

/// A message with its numbers and ids folded: what two occurrences of one kind
/// have in common.
///
/// Word by word (a word is a run of letters and digits): a word that is an id
/// ([`is_id`]) folds whole; in any other, each run of digits folds. What is
/// between words stays. `sda1` is `sda#`, `0.0.0.0:80` is `#.#.#.#:#`, a uuid
/// is the same string whatever uuid.
#[must_use]
pub fn template(message: &str) -> String {
    folded(message, is_id)
}

/// The folding before this one, which the files written then keep their kinds
/// in: an id was only `0x` and hexadecimal, or eight hexadecimal characters or
/// more with a digit, and every other word had its digits folded run by run —
/// a Nix hash, a short rev and a name `mktemp` made among them, cut into
/// pieces no template can put back together. Read only to trust a message a
/// file kept ([`Kind::refolded_with`]).
fn legacy_template(message: &str) -> String {
    folded(message, |word, _| {
        let hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
        if let Some(rest) = word.strip_prefix("0x") {
            return hex(rest);
        }
        word.len() >= 8 && hex(word) && word.chars().any(|c| c.is_ascii_digit())
    })
}

/// A message with its numbers folded, and every word `is_id` names (given
/// whether a dot came right before it) folded whole.
fn folded(message: &str, is_id: impl Fn(&str, bool) -> bool) -> String {
    let message = &without_uuids(message);
    let mut out = String::with_capacity(message.len().min(TEMPLATE_MOST));
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String, after_dot: bool| {
        if word.is_empty() {
            return;
        }
        if is_id(word, after_dot) {
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
    let mut after_dot = false;
    for c in message.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out, after_dot);
            after_dot = c == '.';
            out.push(c);
        }
        if out.len() >= TEMPLATE_MOST {
            break;
        }
    }
    flush(&mut word, &mut out, after_dot);
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

/// Whether a word is an id and not a word: `0x` and hexadecimal; seven
/// hexadecimal characters or more with a digit among them, which is a short
/// rev too; a Nix hash; or, right after a dot, a name `mktemp` made
/// ([`is_made_up`]).
fn is_id(word: &str, after_dot: bool) -> bool {
    let hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
    if let Some(rest) = word.strip_prefix("0x") {
        return hex(rest);
    }
    let digit = word.chars().any(|c| c.is_ascii_digit());
    (word.len() >= 7 && hex(word) && digit)
        || (word.len() == NIX_HASH && digit && word.chars().all(|c| NIX_BASE32.contains(c)))
        || (after_dot && is_made_up(word))
}

/// Whether a word reads as `mktemp`'s: as long as its six or ten `X`, with a
/// capital, and a digit or a case that keeps changing. `nix-shell.6BABLQ` is
/// one and `org.freedesktop.UPower` is not.
fn is_made_up(word: &str) -> bool {
    let changes = word
        .as_bytes()
        .windows(2)
        .filter(|pair| {
            pair[0].is_ascii_alphabetic()
                && pair[1].is_ascii_alphabetic()
                && pair[0].is_ascii_uppercase() != pair[1].is_ascii_uppercase()
        })
        .count();
    matches!(word.len(), 6 | 10)
        && word.bytes().any(|b| b.is_ascii_uppercase())
        && (word.bytes().any(|b| b.is_ascii_digit()) || changes >= 3)
}

/// What one entry says, as its row and a filter read it: `<unit>: <the
/// message's first line>` — the line a burst's notification says.
#[must_use]
pub fn shown_of(entry: &Entry) -> String {
    line(&Kind::of(entry).who, headline(&said(entry)))
}

/// The line a row shows and a filter is matched against: `<who>: <message>`,
/// or the message alone when nothing said it — the journal's own line, as
/// `journalctl` prints it. The message alone too when it already opens with
/// `<who>: `, as systemd's lines about a unit do: said twice, the name took a
/// third of a row and pushed what happened off its end.
#[must_use]
pub fn line(who: &str, message: &str) -> String {
    let named = message
        .strip_prefix(who)
        .is_some_and(|rest| rest.starts_with(": "));
    if who.is_empty() || named {
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
            template: if is_a_units_failure(entry) {
                FAILED.to_owned()
            } else {
                template(headline(&said(entry)))
            },
        }
    }

    /// The same kind as today's folding writes it. A file written before a
    /// folding changed keeps the kind it had: read through this, what the
    /// person ignored then is still what they ignored, and two rows of one
    /// crash or one failure become the one row they are now.
    #[must_use]
    pub fn refolded(&self) -> Self {
        let text = template(headline(&self.template));
        Self {
            who: fold_instance(&self.who).into_owned(),
            template: if was_a_units_failure(&self.who, &text) {
                FAILED.to_owned()
            } else {
                text
            },
        }
    }

    /// [`Kind::refolded`], from a message of the kind when the file kept one.
    ///
    /// A word the folding before this one cut into pieces — a Nix hash, a
    /// short rev, a name `mktemp` made, each run of digits folded alone —
    /// stays in pieces when the template is folded again, and today's
    /// `Kind::of` folds it whole: a kind ignored with a store path in it would
    /// stop matching its own lines, and an old row would stand beside today's.
    /// The message still holds the word whole. Trusted only when it folds, the
    /// old way, to the very template it was kept with; any other is read from
    /// the template, as before.
    #[must_use]
    pub fn refolded_with(&self, message: &str) -> Self {
        let text = headline(message);
        if text.is_empty() || legacy_template(message) != self.template {
            return self.refolded();
        }
        Self {
            who: self.who.clone(),
            template: template(text),
        }
        .refolded()
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

/// Whether systemd is telling of a unit's failure, by the fields that say so
/// and whichever line of the two this is. Not of a scope: a scope is named by
/// the program in it ([`who_of`]), and a failure of each would fold into one.
fn is_a_units_failure(entry: &Entry) -> bool {
    let field = |name: &str| entry.fields.get(name).map(String::as_str);
    let about_a_unit = field("USER_UNIT")
        .or_else(|| field("UNIT"))
        .is_some_and(|unit| !unit.is_empty() && !unit.ends_with(".scope"));
    about_a_unit
        && matches!(
            field("MESSAGE_ID"),
            Some(UNIT_RESULT_MESSAGE | UNIT_FAILED_MESSAGE)
        )
}

/// [`is_a_units_failure`] of a kind in a file written before [`FAILED`], which
/// has its words and none of its fields. A unit's own line in those very words
/// folds with them; nothing in the file tells the two apart.
fn was_a_units_failure(who: &str, text: &str) -> bool {
    let a_unit = who
        .rsplit_once('.')
        .is_some_and(|(_, kind)| UNIT_TYPES.contains(&kind));
    let its_result = || {
        text.strip_prefix(template(who).as_str())
            .is_some_and(|rest| rest.starts_with(FAILED_RESULT_WORDS))
    };
    let its_job = || text.ends_with('.') && FAILED_JOB_WORDS.iter().any(|w| text.starts_with(w));
    a_unit && (its_result() || its_job())
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
    /// The worst it said since it was last seen. `None` in a file written
    /// before there were degrees ([`Group::severity`]).
    pub severity: Option<Severity>,
    /// How many times in [`Group::boot`]; `0` in a file older than this.
    pub boot_count: u64,
}

impl Group {
    /// How grave it is: an old file's rows were all errors.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.severity.unwrap_or(Severity::Error)
    }

    /// It happened again. Graver only while not seen: once seen, what it says
    /// next is all that counts.
    fn again(&mut self, message: String, at: Moment, boot: String, severity: Severity) {
        self.severity = Some(if self.unseen == 0 {
            severity
        } else {
            self.severity().min(severity)
        });
        self.boot_count = if self.boot == boot {
            self.boot_count + 1
        } else {
            1
        };
        self.message = message;
        self.last = self.last.max(at);
        self.first = self.first.min(at);
        self.count += 1;
        self.boot = boot;
    }

    /// Takes in another row of its kind, from a file that held two. The one
    /// heard last is the one that speaks.
    fn merge(&mut self, other: Self) {
        let (first, count, unseen) = (self.first, self.count, self.unseen);
        let worst = match (self.severity, other.severity) {
            (None, None) => None,
            _ => Some(self.severity().min(other.severity())),
        };
        let later = other.last >= self.last;
        self.boot_count = if self.boot == other.boot {
            self.boot_count + other.boot_count
        } else if later {
            other.boot_count
        } else {
            self.boot_count
        };
        if later {
            self.message = other.message;
            self.last = other.last;
            self.boot = other.boot;
        }
        self.first = first.min(other.first);
        self.count = count + other.count;
        self.unseen = unseen + other.unseen;
        self.severity = worst;
    }
}

/// A list as a file kept it, by kind as today's folding writes it
/// ([`Kind::refolded_with`] its message).
#[must_use]
pub fn by_kind(groups: Vec<Group>) -> BTreeMap<Kind, Group> {
    let mut out: BTreeMap<Kind, Group> = BTreeMap::new();
    for mut group in groups {
        group.kind = group.kind.refolded_with(&group.message);
        match out.get_mut(&group.kind) {
            Some(already) => already.merge(group),
            None => {
                out.insert(group.kind.clone(), group);
            }
        }
    }
    out
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
    /// Shipped with the brain ([`SHIPPED`]): a tap turns it off, and it stays
    /// on the page to turn on again.
    #[default]
    Default,
    /// The person typed it: a tap takes it off the page.
    Custom,
}

/// One snippet as the person's file keeps it. A shipped one left as it ships
/// is not in the file at all.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Choice {
    /// The snippet.
    pub text: String,
    /// Off: a shipped snippet the person turned off (never written for one of
    /// their own, which simply leaves the file).
    pub off: bool,
    /// Why it exists, in the person's words; empty when they gave none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// A snippet the person filters the journal's errors by. A line whose
/// `<who>: <message>` holds it — case folded, so `ERROR` and `error` are one —
/// is counted here and never reaches the list or the mark.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filter {
    /// The snippet, as it was written or shipped.
    pub text: String,
    /// It, case folded: the identity, and what a line is searched for.
    pub folded: String,
    /// Shipped, or the person's.
    pub origin: Origin,
    /// On the page, not filtering.
    pub off: bool,
    /// How many lines it has caught this boot. STATE, like [`Ignored::since`].
    pub since: u64,
    /// The last line it caught, to show what it is catching.
    pub message: String,
    /// Why it exists, in the person's words; they win over [`Filter::shipped`].
    pub reason: String,
    /// The key of the sentence it ships with, for one that ships with one.
    pub shipped: Option<&'static str>,
}

impl Filter {
    /// A shipped filter, on.
    #[must_use]
    pub fn shipped(shipped: &Shipped) -> Self {
        Self {
            shipped: Some(shipped.reason),
            ..Self::new(shipped.text, Origin::Default, false)
        }
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
            reason: String::new(),
            shipped: None,
        }
    }

    /// Whether a line, as `<who>: <message>` case folded, is one it holds.
    fn holds(&self, hay: &str) -> bool {
        !self.off && !self.folded.is_empty() && hay.contains(&self.folded)
    }

    /// Its id on the page: sixteen hexadecimal digits of the folded snippet
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

/// The book of kinds: the list, the warnings, what is ignored, and what is
/// filtered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Book {
    /// The list, by kind.
    pub groups: BTreeMap<Kind, Group>,
    /// The plain warnings, by kind: off the list and off the mark, and with a
    /// limit of their own, so one can never push out an error.
    pub warnings: BTreeMap<Kind, Group>,
    /// What is ignored, by kind.
    pub ignored: BTreeMap<Kind, Ignored>,
    /// What is filtered, by snippet: the shipped ones and the person's, each on
    /// or off. Built from [`SHIPPED`] and the person's file ([`Book::set_filters`]).
    pub filters: Vec<Filter>,
    /// The boot a filter counts, in the journal's spelling; empty when it
    /// could not be read, and then every line is this boot's.
    pub boot: String,
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
    /// A plain warning happened: counted on its row, its own or the list's,
    /// and no count on the mark moved.
    Quiet,
}

impl Book {
    /// One entry heard at `at` (the journal's second).
    pub fn hear(&mut self, entry: &Entry, at: Moment) -> Heard {
        let kind = Kind::of(entry);
        let said = said(entry);
        // A FILTER FIRST: a line a snippet holds is counted on the snippet and
        // is never a kind — off the list and off the mark. On its first line
        // alone, so a word in a stack frame holds nothing.
        let shown = line(&kind.who, headline(&said));
        let hay = shown.to_lowercase();
        let boot = entry
            .fields
            .get(ronoctua_journal::BOOT_ID)
            .cloned()
            .unwrap_or_default();
        if let Some(f) = self.filters.iter_mut().find(|f| f.holds(&hay)) {
            // A LINE OF ANOTHER BOOT is held all the same, and not counted: a
            // cursor reaching back would pay this boot's number with the past.
            if self.boot.is_empty() || boot == self.boot {
                f.since += 1;
                f.message = cut(&shown, MESSAGE_MOST).to_owned();
            }
            return Heard::Filtered;
        }
        if let Some(ignored) = self.ignored.get_mut(&kind) {
            ignored.since += 1;
            return Heard::Ignored;
        }
        let severity = classify::severity(entry, &hay);
        let message = cut(&said, MESSAGE_MOST).to_owned();
        if severity == Severity::Warning {
            // A PLAIN WARNING IS NEVER ON THE MARK, of a kind the list holds
            // too: its row counts it and shows its line, and it is no more
            // unseen and no milder than it was — a kind marked seen would
            // otherwise light the mark again on a warning, in grey.
            match self.groups.get_mut(&kind) {
                Some(group) => {
                    let as_it_was = group.severity();
                    group.again(message, at, boot, as_it_was);
                }
                None => self.warned(kind, message, at, boot),
            }
            return Heard::Quiet;
        }
        // A KIND THAT ROSE leaves the warnings, and what it counted there
        // comes with it.
        if let Some(risen) = self.warnings.remove(&kind) {
            self.groups.insert(kind.clone(), risen);
        }
        let spared = kind.clone();
        let heard = match self.groups.get_mut(&kind) {
            Some(group) => {
                group.again(message, at, boot, severity);
                group.unseen += 1;
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
                        severity: Some(severity),
                        boot_count: 1,
                    },
                );
                Heard::New
            }
        };
        self.bound(&spared);
        heard
    }

    /// A plain warning, in its own list.
    fn warned(&mut self, kind: Kind, message: String, at: Moment, boot: String) {
        match self.warnings.get_mut(&kind) {
            Some(group) => group.again(message, at, boot, Severity::Warning),
            None => {
                self.warnings.insert(
                    kind.clone(),
                    Group {
                        kind,
                        message,
                        first: at,
                        last: at,
                        count: 1,
                        unseen: 0,
                        boot,
                        severity: Some(Severity::Warning),
                        boot_count: 1,
                    },
                );
            }
        }
        while self.warnings.len() > WARNINGS_KEPT {
            let Some(oldest) = self
                .warnings
                .values()
                .min_by_key(|g| g.last)
                .map(|g| g.kind.clone())
            else {
                return;
            };
            self.warnings.remove(&oldest);
        }
    }

    /// Keeps the list to [`KINDS_KEPT`]: the kind heard longest ago goes,
    /// a seen one before one not seen yet, a milder one before a graver, and
    /// one of another boot before one of this boot.
    ///
    /// NEVER THE KIND JUST HEARD (`spared`): a list full of graver kinds would
    /// otherwise drop a milder one the moment it came, and it would never reach
    /// the list or the mark. The one it pushes out has been on the page.
    fn bound(&mut self, spared: &Kind) {
        let boot = self.boot.clone();
        while self.groups.len() > KINDS_KEPT {
            let Some(oldest) = self
                .groups
                .values()
                .filter(|g| g.kind != *spared)
                .min_by_key(|g| {
                    let this_boot = !boot.is_empty() && g.boot == boot;
                    (g.unseen > 0, Reverse(g.severity()), this_boot, g.last)
                })
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

    /// The gravest of the kinds not seen yet.
    #[must_use]
    pub fn worst_unseen(&self) -> Option<Severity> {
        self.groups
            .values()
            .filter(|g| g.unseen > 0)
            .map(Group::severity)
            .min()
    }

    /// The kind with this id, in the list or among the warnings.
    #[must_use]
    pub fn group(&self, id: &str) -> Option<&Kind> {
        self.groups
            .keys()
            .chain(self.warnings.keys())
            .find(|k| k.id() == id)
    }

    /// The ignored kind with this id.
    #[must_use]
    pub fn ignored_kind(&self, id: &str) -> Option<&Kind> {
        self.ignored.keys().find(|k| k.id() == id)
    }

    /// Marks a kind seen. `false` when it is not in the list: a warning is
    /// on no mark, and has nothing to be seen for.
    pub fn see(&mut self, id: &str) -> bool {
        let Some(g) = self.groups.values_mut().find(|g| g.kind.id() == id) else {
            return false;
        };
        g.unseen = 0;
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

    /// Ignores a kind from now on: out of the list or the warnings, into the
    /// ignored. `false` when it is in neither.
    pub fn ignore(&mut self, id: &str) -> bool {
        let Some(kind) = self.group(id).cloned() else {
            return false;
        };
        let held = self.groups.remove(&kind);
        let Some(group) = held.or_else(|| self.warnings.remove(&kind)) else {
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

    /// Rebuilds the filters from the [`SHIPPED`] ones and the person's
    /// choices, keeping the count the brain had for a snippet it still has. The
    /// person's choices are the base, so a snippet they added or took out by
    /// hand stays, and so does an `off` or a reason on a shipped one; this
    /// sweeps nothing ([`Book::apply_filters`] does, once the counts are back).
    pub fn set_filters(&mut self, choices: &[Choice]) {
        let kept: Vec<(String, u64, String)> = self
            .filters
            .iter()
            .map(|f| (f.folded.clone(), f.since, f.message.clone()))
            .collect();
        let mut out: Vec<Filter> = SHIPPED.iter().map(Filter::shipped).collect();
        for choice in choices {
            if choice.text.trim().is_empty() {
                continue;
            }
            let folded = choice.text.to_lowercase();
            if let Some(f) = out.iter_mut().find(|f| f.folded == folded) {
                f.off = choice.off;
                f.reason.clone_from(&choice.reason);
            } else {
                let mut own = Filter::own(&choice.text, choice.off);
                own.reason.clone_from(&choice.reason);
                out.push(own);
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

    /// Filters by a snippet from now on, and sweeps what was already heard. A
    /// snippet already there is turned on if it was off. The oldest of the
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

    /// Says why a filter exists, by id: the person's words, cut to
    /// [`REASON_MOST`], on any filter. Blank takes them away, and a shipped one
    /// says its own sentence again. `false` when there is none with this id.
    pub fn give_reason(&mut self, id: &str, text: &str) -> bool {
        let Some(f) = self.filters.iter_mut().find(|f| f.id() == id) else {
            return false;
        };
        cut(text.trim(), REASON_MOST).clone_into(&mut f.reason);
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

    /// Sweeps the list and the warnings of every kind an active filter now
    /// holds: it leaves (and the mark with it), what it counted this boot
    /// folded onto the filter. What a new or turned-on filter, or a start
    /// reading the kept list, does to what was already heard.
    pub fn apply_filters(&mut self) {
        let (filters, boot) = (&mut self.filters, self.boot.as_str());
        self.groups.retain(|kind, g| !swept(filters, boot, kind, g));
        self.warnings
            .retain(|kind, g| !swept(filters, boot, kind, g));
    }

    /// Forgets the kinds seen whose last occurrence was in another boot —
    /// what a start does: they were read, and they are over — and every
    /// warning of one, which nobody was asked to read.
    pub fn forget_seen_before(&mut self, boot: &str) {
        if boot.is_empty() {
            return;
        }
        self.groups.retain(|_, g| g.unseen > 0 || g.boot == boot);
        self.warnings.retain(|_, g| g.boot == boot);
    }

    /// The list as its page draws it: the kinds not seen yet of this boot,
    /// those of earlier boots, the seen ones, each the gravest first and then
    /// the newest — and the warnings, the newest first, and what is ignored.
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
            list.sort_by(|a, b| {
                a.severity()
                    .cmp(&b.severity())
                    .then(b.last.cmp(&a.last))
                    .then(a.kind.cmp(&b.kind))
            });
        }
        out.warnings = self.warnings.values().collect();
        out.warnings
            .sort_by(|a, b| b.last.cmp(&a.last).then(a.kind.cmp(&b.kind)));
        out.ignored = self.ignored.values().collect();
        out
    }
}

/// Whether an active filter holds a kind already heard, folding onto it what
/// the kind counted this boot.
fn swept(filters: &mut [Filter], boot: &str, kind: &Kind, g: &Group) -> bool {
    let shown = line(&kind.who, headline(&g.message));
    let hay = shown.to_lowercase();
    let Some(f) = filters.iter_mut().find(|f| f.holds(&hay)) else {
        return false;
    };
    if boot.is_empty() || g.boot == boot {
        f.since += g.boot_count;
        f.message = cut(&shown, MESSAGE_MOST).to_owned();
    }
    true
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
    /// The plain warnings.
    pub warnings: Vec<&'a Group>,
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
    use super::super::fixtures;
    use super::*;

    /// An error, as most of these are.
    fn entry(fields: &[(&str, &str)], message: &str) -> Entry {
        Entry {
            fields: fields
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            message: message.to_owned(),
            priority: Some(ronoctua_journal::ERR),
            ..Entry::default()
        }
    }

    /// The same at `warning`, the plain kind that stays off the list.
    fn warning(fields: &[(&str, &str)], message: &str) -> Entry {
        Entry {
            priority: Some(ronoctua_journal::WARNING),
            ..entry(fields, message)
        }
    }

    /// One of the tables' lines, as a unit's.
    fn sampled(who: &str, message: &str) -> Entry {
        entry(&[("_SYSTEMD_UNIT", who)], message)
    }

    fn parsed(line: &str) -> Entry {
        Entry::parse(line).expect("a fixture that is not a journal line")
    }

    /// A book with the shipped filters in it.
    fn shipped() -> Book {
        let mut book = Book::default();
        book.set_filters(&[]);
        book
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
    fn a_hash_a_short_rev_and_a_made_up_name_fold_whole() {
        // A NIX HASH, 32 of its alphabet with a digit: without this the same
        // store path of two builds is two kinds.
        assert_eq!(
            template("/nix/store/62yhnxbs4zbclpansd8vcpnwz5js40zs-system-path/share"),
            "/nix/store/#-system-path/share"
        );
        // A SHORT REV, seven hexadecimal characters.
        assert_eq!(template("nixos 26.11.20261027.31ecbac"), "nixos #.#.#.#");
        // WHAT `mktemp` WRITES, after its dot.
        assert_eq!(
            template("/tmp/nix-shell.6BABLQ/stdout"),
            "/tmp/nix-shell.#/stdout"
        );
        assert_eq!(template("/tmp/tmp.aKbMcRdS2x/y"), "/tmp/tmp.#/y");
        assert_eq!(template("/tmp/nix-shell.xKpZqR/y"), "/tmp/nix-shell.#/y");
        // AND NOT A WORD THAT ONLY LOOKS LIKE ONE: a name with a capital, after
        // a dot or not, and thirty-two letters of plain English.
        assert_eq!(
            template("org.freedesktop.UPower failed"),
            "org.freedesktop.UPower failed"
        );
        assert_eq!(template("UPower6 failed"), "UPower# failed");
        assert_eq!(template("nginx.Socket closed"), "nginx.Socket closed");
        let long_word = "backgroundprocessingpipelineword";
        assert_eq!(long_word.len(), NIX_HASH);
        assert_eq!(template(long_word), long_word);
    }

    #[test]
    fn a_crash_is_one_kind_whatever_its_pid() {
        assert_eq!(
            fold_instance("systemd-coredump@3-1742-0.service"),
            "systemd-coredump@#.service"
        );
        assert_eq!(fold_instance("user@1000.service"), "user@#.service");
        // AN INSTANCE THAT IS A WORD says which one, and stays.
        assert_eq!(
            fold_instance("a-unit@chrome.service"),
            "a-unit@chrome.service"
        );
        assert_eq!(fold_instance("getty@tty1.service"), "getty@tty1.service");
        // NOTHING TO FOLD.
        assert_eq!(fold_instance("nginx.service"), "nginx.service");
        assert_eq!(fold_instance("a@.service"), "a@.service");
        // AND THE ROW OF ONE CRASH AND THE ROW OF THE NEXT ARE ONE.
        let crash = |unit: &str, pid: &str| {
            entry(
                &[("_SYSTEMD_UNIT", unit)],
                &format!("Process {pid} (a) of user 1000 dumped core.\n\nStack trace of\n#0 a"),
            )
        };
        assert_eq!(
            Kind::of(&crash("systemd-coredump@3-1742-0.service", "1742")),
            Kind::of(&crash("systemd-coredump@9-8000-0.service", "8000")),
        );
    }

    #[test]
    fn a_row_shows_the_first_line_that_says_anything() {
        assert_eq!(headline("one\ntwo"), "one");
        assert_eq!(headline("\n\n  one  \ntwo"), "one");
        assert_eq!(headline(""), "");
        // A FRAME OF A STACK IS NOT WHAT A KIND IS, nor what a filter reads.
        let mut book = shipped();
        book.filter("frame_of_a_stack");
        let held = entry(
            &[("_SYSTEMD_UNIT", "a.service")],
            "It crashed.\n#0 frame_of_a_stack",
        );
        assert_eq!(
            book.hear(&held, 1),
            Heard::New,
            "a stack frame tripped a filter"
        );
    }

    #[test]
    fn a_coredump_with_no_message_is_named_by_its_fields() {
        let crash = parsed(fixtures::COREDUMP_NO_MESSAGE);
        assert_eq!(crash.message, "", "the fixture lost what it is here for");
        assert_eq!(said(&crash), "(pipewire) SIGSEGV");
        let kind = Kind::of(&crash);
        assert_eq!(kind.who, "systemd-coredump@#.service");
        assert_eq!(kind.template, "(pipewire) SIGSEGV");
        let mut book = shipped();
        book.hear(&crash, 1);
        assert_eq!(
            book.groups.values().next().map(|g| g.message.clone()),
            Some("(pipewire) SIGSEGV".to_owned()),
            "the row of a crash is `who: ` with nothing after it"
        );
        // NOTHING TO SAY IS NOTHING SAID, not a pair of brackets.
        assert_eq!(said(&entry(&[], "")), "");
        // AND THE BUILD'S OWN CRASH IS HELD WITH NO MESSAGE TOO, by the same
        // snippet that holds it with one.
        let test = entry(
            &[
                ("COREDUMP_COMM", "udisks-test-hel"),
                ("COREDUMP_SIGNAL_NAME", "SIGABRT"),
            ],
            "",
        );
        assert_eq!(book.hear(&test, 2), Heard::Filtered);
    }

    #[test]
    fn a_failing_unit_is_one_row_and_the_worst_of_its_lines() {
        let mut book = shipped();
        // THE WARNING, THEN THE ERROR: on the list once.
        assert_eq!(book.hear(&parsed(fixtures::UNIT_RESULT), 1), Heard::New);
        assert_eq!(
            book.hear(&parsed(fixtures::UNIT_START_FAILED), 1),
            Heard::Again
        );
        assert_eq!(book.groups.len(), 1, "one failure, two rows");
        let g = book.groups.values().next().expect("the row");
        assert_eq!(g.kind.who, "made-up.service");
        assert_eq!(g.kind.template, FAILED);
        assert_eq!(g.count, 2);
        assert_eq!(g.severity(), Severity::Error);
        assert_eq!(g.message, "Failed to start a made-up service.");
        // A SCOPE IS NAMED BY ITS PROGRAM: `systemd`, for all of them, and
        // folding those would make every scope one failure.
        let scope = Kind::of(&parsed(fixtures::SCOPE_RESULT));
        assert_eq!(scope.who, "systemd");
        assert_ne!(scope.template, FAILED);
    }

    #[test]
    fn a_warning_never_pushes_out_an_error() {
        let mut book = shipped();
        for n in 0..KINDS_KEPT {
            book.hear(
                &sampled(&format!("u{n}x.service"), "failed"),
                i64::try_from(n).unwrap_or(0),
            );
        }
        for n in 0..WARNINGS_KEPT * 3 {
            let heard = book.hear(
                &warning(&[("_SYSTEMD_UNIT", &format!("w{n}x.service"))], "slow"),
                10_000 + i64::try_from(n).unwrap_or(0),
            );
            assert_eq!(heard, Heard::Quiet);
        }
        assert_eq!(
            book.groups.len(),
            KINDS_KEPT,
            "a warning took an error's place"
        );
        assert_eq!(book.warnings.len(), WARNINGS_KEPT);
        assert_eq!(book.unseen(), KINDS_KEPT, "a warning reached the mark");
        // THE ONE HEARD LONGEST AGO went.
        let names: Vec<&str> = book.warnings.keys().map(|k| k.who.as_str()).collect();
        assert!(!names.contains(&"w0x.service"));
        assert!(names.contains(&"w119x.service"));
    }

    #[test]
    fn a_kind_just_heard_is_never_the_one_that_goes() {
        let mut book = shipped();
        book.boot = "this".into();
        for n in 0..KINDS_KEPT {
            let mut entry = sampled(&format!("u{n}x.service"), "failed");
            entry
                .fields
                .insert(ronoctua_journal::BOOT_ID.into(), "before".into());
            book.hear(&entry, 100 + i64::try_from(n).unwrap_or(0));
        }
        let mut notable = warning(
            &[
                ("_SYSTEMD_UNIT", "fresh.service"),
                ("UNIT_RESULT", "exit-code"),
            ],
            "ended badly",
        );
        notable
            .fields
            .insert(ronoctua_journal::BOOT_ID.into(), "this".into());
        assert_eq!(book.hear(&notable, 10_000), Heard::New);
        assert_eq!(book.groups.len(), KINDS_KEPT);
        let names: Vec<&str> = book.groups.keys().map(|k| k.who.as_str()).collect();
        assert!(names.contains(&"fresh.service"), "the kind just heard went");
        // THE ERROR OF ANOTHER BOOT HEARD LONGEST AGO made room.
        assert!(!names.contains(&"u0x.service"));
        assert_eq!(book.worst_unseen(), Some(Severity::Error));
    }

    #[test]
    fn a_milder_kind_goes_before_a_graver_one() {
        let mut book = shipped();
        for n in 0..KINDS_KEPT - 1 {
            book.hear(
                &sampled(&format!("u{n}x.service"), "failed"),
                100 + i64::try_from(n).unwrap_or(0),
            );
        }
        // THE NEWEST OF THEM ALL, so only how grave it is can make it go first.
        let notable = warning(&[("UNIT", "a.service"), ("UNIT_RESULT", "exit-code")], "x");
        book.hear(&notable, 1000);
        assert_eq!(book.groups.len(), KINDS_KEPT);
        book.hear(&sampled("last.service", "failed"), 1001);
        assert_eq!(book.groups.len(), KINDS_KEPT);
        assert!(
            book.groups.keys().all(|k| k.who != "a.service"),
            "an error went before what was milder"
        );
        assert!(
            book.groups.keys().any(|k| k.who == "u0x.service"),
            "the oldest error went while something milder stayed"
        );
    }

    #[test]
    fn a_kind_rises_out_of_the_warnings_with_what_it_counted() {
        let mut book = shipped();
        let plain = warning(&[("_SYSTEMD_UNIT", "a.service")], "the disk is slow");
        assert_eq!(book.hear(&plain, 1), Heard::Quiet);
        assert_eq!(book.hear(&plain, 2), Heard::Quiet);
        assert_eq!(book.unseen(), 0);
        // THE SAME LINE, AT `err`: the same kind, and now the list's.
        let error = sampled("a.service", "the disk is slow");
        assert_eq!(book.hear(&error, 3), Heard::Again);
        assert!(book.warnings.is_empty(), "it stayed in the warnings too");
        let g = book.groups.values().next().expect("the row");
        assert_eq!((g.first, g.count, g.unseen), (1, 3, 1));
        assert_eq!(book.unseen(), 1);
        // AND ONCE ON THE LIST, it stays there — counted, and never on the
        // mark: no more unseen than it was, and no milder.
        assert_eq!(book.hear(&plain, 4), Heard::Quiet);
        assert!(book.warnings.is_empty());
        let g = book.groups.values().next().expect("the row");
        assert_eq!((g.count, g.unseen, g.last), (4, 1, 4));
        assert_eq!(g.severity(), Severity::Error);
        let id = g.kind.id();
        assert!(book.see(&id));
        assert_eq!(book.hear(&plain, 5), Heard::Quiet);
        assert_eq!(book.unseen(), 0, "a warning lit the mark again");
        let g = book.groups.values().next().expect("the row");
        assert_eq!((g.count, g.unseen), (5, 0));
        assert_eq!(g.severity(), Severity::Error, "a warning made it milder");
    }

    #[test]
    fn the_worst_since_it_was_seen_is_what_a_row_says() {
        let mut book = shipped();
        let error = sampled("a.service", "the disk is slow");
        let crash = entry(
            &[("_SYSTEMD_UNIT", "a.service"), ("UNIT_RESULT", "core-dump")],
            "the disk is slow",
        );
        book.hear(&error, 1);
        book.hear(&crash, 2);
        book.hear(&error, 3);
        let id = book.groups.values().next().expect("row").kind.id();
        let severity = |book: &Book| book.groups.values().next().map(Group::severity);
        assert_eq!(severity(&book), Some(Severity::Critical));
        assert_eq!(book.worst_unseen(), Some(Severity::Critical));
        // SEEN, and it starts again from what it says next.
        assert!(book.see(&id));
        assert_eq!(book.worst_unseen(), None);
        book.hear(&error, 4);
        assert_eq!(severity(&book), Some(Severity::Error));
    }

    #[test]
    fn a_row_of_an_older_file_reads_as_an_error() {
        let g: Group = serde_json::from_str(r#"{"count": 3, "unseen": 1}"#).expect("an old row");
        assert_eq!(g.severity, None);
        assert_eq!(g.severity(), Severity::Error);
        assert_eq!(g.boot_count, 0);
    }

    #[test]
    fn a_filter_counts_this_boot_and_holds_the_others_uncounted() {
        let mut book = shipped();
        book.boot = "b2".to_owned();
        let noise = |boot: &str| {
            entry(
                &[("_SYSTEMD_UNIT", "dbus-broker.service"), ("_BOOT_ID", boot)],
                "Ignoring duplicate name 'x'",
            )
        };
        assert_eq!(book.hear(&noise("b2"), 1), Heard::Filtered);
        // A CURSOR REACHING BACK reads an older boot: held, and not counted.
        assert_eq!(book.hear(&noise("b1"), 2), Heard::Filtered);
        assert_eq!(book.filters()[0].since, 1);
        assert!(book.groups.is_empty());
    }

    #[test]
    fn a_sweep_folds_what_a_kind_counted_this_boot() {
        let mut book = shipped();
        book.boot = "b2".to_owned();
        let id = book.filters()[0].id();
        assert!(book.unfilter(&id));
        let noise = |boot: &str, unit: &str| {
            entry(
                &[("_SYSTEMD_UNIT", unit), ("_BOOT_ID", boot)],
                "Ignoring duplicate name 'x'",
            )
        };
        book.hear(&noise("b1", "old.service"), 1);
        book.hear(&noise("b1", "old.service"), 2);
        book.hear(&noise("b1", "both.service"), 3);
        book.hear(&noise("b2", "both.service"), 4);
        book.hear(&noise("b2", "both.service"), 5);
        let both = book
            .groups
            .values()
            .find(|g| g.kind.who == "both.service")
            .expect("the row");
        assert_eq!(both.boot_count, 2, "the count of its last boot alone");
        assert_eq!(both.count, 3);
        assert!(book.refilter(&id));
        assert!(book.groups.is_empty());
        assert_eq!(book.filters()[0].since, 2, "an older boot was counted");
    }

    #[test]
    fn a_warning_row_can_be_ignored_and_a_filter_sweeps_the_warnings() {
        let mut book = shipped();
        let plain = warning(&[("_SYSTEMD_UNIT", "a.service")], "the disk is slow");
        book.hear(&plain, 1);
        let id = Kind::of(&plain).id();
        assert_eq!(book.group(&id).cloned(), Some(Kind::of(&plain)));
        // NOTHING TO SEE: it is on no mark.
        assert!(!book.see(&id));
        assert!(book.ignore(&id));
        assert!(book.warnings.is_empty());
        assert_eq!(book.hear(&plain, 2), Heard::Ignored);
        assert!(book.unignore(&id));
        // AND A FILTER BY ITS LINE takes it from there.
        book.hear(&plain, 3);
        book.filter("the disk is slow");
        assert!(book.warnings.is_empty(), "the filter left a warning behind");
        assert_eq!(book.hear(&plain, 4), Heard::Filtered);
    }

    #[test]
    fn another_boots_warnings_are_forgotten_and_this_ones_kept() {
        let mut book = shipped();
        let line = |unit: &str, boot: &str| {
            warning(&[("_SYSTEMD_UNIT", unit), ("_BOOT_ID", boot)], "slow")
        };
        book.hear(&line("old.service", "b1"), 1);
        book.hear(&line("new.service", "b2"), 2);
        book.forget_seen_before("b2");
        let kept: Vec<&str> = book.warnings.keys().map(|k| k.who.as_str()).collect();
        assert_eq!(kept, ["new.service"]);
    }

    #[test]
    fn the_page_puts_the_gravest_first_and_the_warnings_newest_first() {
        let mut book = shipped();
        let crash = entry(
            &[
                ("_SYSTEMD_UNIT", "crash.service"),
                ("UNIT_RESULT", "core-dump"),
            ],
            "x",
        );
        let error = sampled("error.service", "x");
        book.hear(&crash, 1);
        book.hear(&error, 2);
        book.hear(&warning(&[("_SYSTEMD_UNIT", "w1.service")], "x"), 3);
        book.hear(&warning(&[("_SYSTEMD_UNIT", "w2.service")], "x"), 4);
        let sorted = book.sorted("");
        let who = |gs: &[&Group]| gs.iter().map(|g| g.kind.who.clone()).collect::<Vec<_>>();
        assert_eq!(who(&sorted.unseen), ["crash.service", "error.service"]);
        assert_eq!(who(&sorted.warnings), ["w2.service", "w1.service"]);
    }

    #[test]
    fn an_old_kind_reads_as_todays_and_two_old_rows_of_one_crash_are_one() {
        let old = Kind {
            who: "systemd-coredump@3-1742-0.service".to_owned(),
            template: "Process # (a) of user # dumped core.\n\nStack trace of thread #:".to_owned(),
        };
        let today = entry(
            &[("_SYSTEMD_UNIT", "systemd-coredump@9-8000-0.service")],
            "Process 8000 (a) of user 1000 dumped core.\n\nStack trace of thread 8000:\n#0 a",
        );
        assert_eq!(old.refolded(), Kind::of(&today));
        // FOLDING IT TWICE changes nothing.
        assert_eq!(old.refolded().refolded(), old.refolded());
        let row = |who: &str, first: i64, last: i64, count: u64, message: &str| Group {
            kind: Kind {
                who: who.to_owned(),
                template: "Process # (a) of user # dumped core.".to_owned(),
            },
            message: message.to_owned(),
            first,
            last,
            count,
            unseen: count,
            boot: "b1".to_owned(),
            ..Group::default()
        };
        let list = by_kind(vec![
            row("systemd-coredump@1-1-0.service", 10, 20, 1, "the later"),
            row("systemd-coredump@2-2-0.service", 5, 15, 2, "the earlier"),
        ]);
        assert_eq!(list.len(), 1, "two old rows of one crash stayed two");
        let g = list.values().next().expect("the row");
        assert_eq!(g.kind.who, "systemd-coredump@#.service");
        assert_eq!((g.first, g.last, g.count, g.unseen), (5, 20, 3, 3));
        assert_eq!(g.message, "the later");
    }

    #[test]
    fn an_old_failure_of_a_unit_reads_as_the_one_kind_it_is_now() {
        let old = |who: &str, template: &str| Kind {
            who: who.to_owned(),
            template: template.to_owned(),
        };
        let today = Kind::of(&parsed(fixtures::UNIT_RESULT));
        assert_eq!(today.template, FAILED);
        for template in [
            "made-up.service: Failed with result 'exit-code'.",
            "Failed to start a made-up service.",
            "Timed out starting a made-up service.",
            "Dependency failed for a made-up service.",
        ] {
            assert_eq!(
                old("made-up.service", template).refolded(),
                today,
                "`{template}` is not the failure it was"
            );
        }
        // THE UNIT'S NUMBER is folded in the file's template and not in its who.
        let crash = old(
            "systemd-coredump@3-1742-0.service",
            "systemd-coredump@#-#-#.service: Failed with result 'core-dump'.",
        );
        assert_eq!(crash.refolded().who, "systemd-coredump@#.service");
        assert_eq!(crash.refolded().template, FAILED);
        // FOLDING IT TWICE changes nothing.
        assert_eq!(crash.refolded().refolded(), crash.refolded());
        // WHAT TODAY'S `Kind::of` KEEPS AS IT IS, a file must not fold: a
        // scope, which is named by its program; a program in no unit; and a
        // unit saying something else.
        let scope = Kind::of(&parsed(fixtures::SCOPE_RESULT));
        assert_eq!(scope.refolded(), scope);
        for (who, template) in [
            ("a-program", "Failed to start the listener."),
            ("a.service", "Failed to start the listener"),
            ("a.service", "Failed to read the configuration."),
            ("a.service", "b.service: Failed with result 'exit-code'."),
        ] {
            let kind = old(who, template);
            assert_eq!(kind.refolded(), kind, "`{template}` is not a failure");
        }
    }

    #[test]
    fn every_shipped_snippet_catches_its_line_and_no_other_gets_there_first() {
        assert_eq!(
            SHIPPED.len(),
            fixtures::SHIPPED_SAMPLES.len(),
            "a snippet without a line, or a line without a snippet"
        );
        assert_eq!(SHIPPED.len(), 59, "the list lost a snippet");
        for (n, (shipped_snippet, (who, message))) in
            SHIPPED.iter().zip(fixtures::SHIPPED_SAMPLES).enumerate()
        {
            let mut book = shipped();
            let heard = book.hear(&sampled(who, message), 1);
            assert_eq!(
                heard,
                Heard::Filtered,
                "`{}` caught nothing",
                shipped_snippet.text
            );
            let caught: Vec<&str> = book
                .filters()
                .iter()
                .filter(|f| f.since > 0)
                .map(|f| f.text.as_str())
                .collect();
            assert_eq!(
                caught,
                [shipped_snippet.text],
                "the line of number {n} was taken by another snippet"
            );
        }
    }

    #[test]
    fn nothing_shipped_swallows_a_real_failure() {
        for (who, message) in fixtures::REAL_FAILURES {
            let mut book = shipped();
            let heard = book.hear(&sampled(who, message), 1);
            assert_ne!(
                heard,
                Heard::Filtered,
                "a real failure was held: `{who}: {message}`"
            );
        }
        assert!(fixtures::REAL_FAILURES.len() >= 28, "the table lost lines");
    }

    #[test]
    fn the_snippets_are_each_one_filter_and_the_first_stays_first() {
        // A PERSON'S FILE MAY ALREADY TURN THIS ONE OFF, by these words.
        assert_eq!(SHIPPED[0].text, "Ignoring duplicate name");
        let mut folded: Vec<String> = SHIPPED.iter().map(|s| s.text.to_lowercase()).collect();
        folded.sort();
        folded.dedup();
        assert_eq!(folded.len(), SHIPPED.len(), "a snippet twice");
        // A SNIPPET THAT HOLDS ANOTHER would leave one of them unreachable.
        for a in &folded {
            for b in &folded {
                assert!(a == b || !a.contains(b.as_str()), "`{a}` holds `{b}`");
            }
        }
    }

    #[test]
    fn a_person_gives_a_filter_a_reason_and_blank_takes_it_away() {
        let mut book = shipped();
        let id = book.filters()[0].id();
        assert_eq!(book.filters()[0].shipped, Some(SHIPPED[0].reason));
        assert!(book.give_reason(&id, "  mine  "));
        assert_eq!(book.filters()[0].reason, "mine");
        assert!(book.give_reason(&id, "   "));
        assert_eq!(book.filters()[0].reason, "");
        assert_eq!(book.filters()[0].shipped, Some(SHIPPED[0].reason));
        assert!(!book.give_reason("ffffffffffffffff", "x"));
        // CUT ON A CHARACTER'S EDGE.
        let long = "é".repeat(REASON_MOST);
        assert!(book.give_reason(&id, &long));
        assert!(book.filters()[0].reason.len() <= REASON_MOST);
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
        // WHAT SHIPS, on, and nothing of the person's.
        book.set_filters(&[]);
        assert_eq!(book.filters().len(), SHIPPED.len());
        assert!(book.filters().iter().all(|f| !f.off));
        // A dbus line at err, on the system bus and on the person's — one kind
        // each by `who`, both held by the one snippet, case folded.
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
        assert_eq!(book.filters()[0].since, 2, "both buses, one snippet");
        // A line the snippet does not hold is an error as ever.
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
            Choice {
                text: "Ignoring duplicate name".to_owned(),
                off: true,
                ..Choice::default()
            },
            Choice {
                text: "timed out".to_owned(),
                reason: "it always does".to_owned(),
                ..Choice::default()
            },
        ]);
        assert_eq!(book.filters().len(), SHIPPED.len() + 1);
        let def = &book.filters()[0];
        assert!(def.off, "the person's choice was not kept");
        assert_eq!(def.since, 1, "the count the brain had was lost");
        assert!(book.filters().iter().any(|f| f.origin == Origin::Custom
            && f.text == "timed out"
            && f.reason == "it always does"));
        // THE CAP keeps the person's own bounded.
        for n in 0..=FILTERS_KEPT {
            book.filter(&format!("snippet {n}"));
        }
        assert!(
            book.filters()
                .iter()
                .filter(|f| f.origin == Origin::Custom)
                .count()
                <= FILTERS_KEPT
        );
    }

    /// FAILS IF: a unit systemd names in its own line is said twice on the
    /// row — or the prefix is taken from a line that only begins like it, or
    /// a snippet that names the unit stops holding the line.
    #[test]
    fn a_line_says_its_unit_once() {
        let failed = "scroll.service: Failed with result 'core-dump'.";
        assert_eq!(line("scroll.service", failed), failed);
        assert_eq!(
            line("scroll", "scroll.service: it broke"),
            "scroll: scroll.service: it broke",
            "a name that only begins the line is not the line's"
        );
        assert_eq!(
            line("nginx.service", "bind() failed"),
            "nginx.service: bind() failed"
        );
        assert_eq!(line("", "a line"), "a line");
        let mut book = Book::default();
        book.filter("scroll.service: Failed with");
        assert_eq!(
            book.hear(&sampled("scroll.service", failed), 1),
            Heard::Filtered
        );
    }
}
