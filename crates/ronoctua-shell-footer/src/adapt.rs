//! The modeline that adapts to a program — one adapter per program.
//!
//! # What an adapter is
//!
//! The modeline says the same things about every window: its title, its
//! program, its state. Some programs have a whole toolbar of their own that
//! this system hides — the Chrome kiosk hides Chrome's tab strip and its
//! toolbar — and for those the modeline becomes that toolbar: the title goes
//! (the tab in front says it better), the program's name comes first, and the
//! adapter's own segments follow it.
//!
//! An adapter CLAIMS a window, draws segments about it, and answers the
//! pointer on them. It never runs anything itself: every answer is a command
//! of the registry ([`Act`]), which the adapter's view and the `,` group reach
//! too — the three come out of one table (`kiosk_view::ROOT` for the kiosk),
//! and `footer-mouse-reaches-nothing-new` holds them to that.
//!
//! # A closed enum, and not a trait object
//!
//! [`Adapted`] names every adapter there is. The footer asks it while it is
//! also mutating itself, and a `&dyn` borrowed out of the footer would fight
//! every one of those borrows; an enum over plain references is dropped the
//! moment the answer is in hand. A second adapter is one variant here and one
//! arm in each method below.
//!
//! # What was DRAWN answers the pointer
//!
//! The same rule as the rest of the modeline (`Footer::modeline_placed`): the
//! footer keeps, beside what it placed, which [`Target`] each placed segment
//! was about ([`resolve`] hands it back), and a click, a wheel, a drag and a
//! hover all read that table — never a second layout.
//!
//! # On and off
//!
//! `modeline.json` keeps one switch per adapter (absent is ON) and whether
//! the kiosk's address line is up (absent is OFF). The file is the broadcast:
//! the footer watches it and recognises its own write.

use std::collections::BTreeMap;

use ronoctua_compositor::Window;
use ronoctua_i18n::Texts;
use ronoctua_render::layout::{self, Placed, Segment, Side};
use serde::{Deserialize, Serialize};

/// What an adapter's segment does when it is pointed at: a command of the
/// registry, with its argument when it takes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Run this command.
    Run(&'static str),
    /// Run this command with this argument.
    RunWith(&'static str, String),
}

/// Which button a press came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary one.
    Left,
    /// The secondary one.
    Right,
    /// The wheel, pressed — which closes a tab, as it does in every browser.
    Middle,
}

/// A run of segments whose members are counted when the line is too narrow
/// for all of them — the kiosk's tabs, with `+N` on either side.
///
/// A POOL, like the workspace numbers' (`workspaces::POOL`): a segment's id is
/// `&'static str`, so the members are named by their place and the adapter
/// says what each place is about in [`Line::targets`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strip {
    /// The counter drawn before the first member that survives.
    pub before: &'static str,
    /// The counter drawn after the last one.
    pub after: &'static str,
    /// Every member that has a segment, left to right.
    pub members: Vec<&'static str>,
    /// Members that exist and have no segment at all — past the pool — before
    /// the first one and after the last.
    pub beyond: (usize, usize),
    /// The counter's words, `+{ $count }`, resolved with a NUMBER.
    pub more: &'static str,
}

/// One adapter: a program's modeline.
pub trait ModelineAdapter {
    /// What a segment of this adapter is about.
    type Target: Clone;
    /// Its id, kebab-case: the key of its switch in `modeline.json`.
    fn id(&self) -> &'static str;
    /// Whether it adapts the modeline to this window.
    fn claims(&self, w: &Window) -> bool;
    /// Its segments for this window, each with what it is about.
    fn segments(&self, w: &Window, texts: &Texts) -> Vec<(Segment, Self::Target)>;
    /// The run of its segments that is counted rather than lost.
    fn strip(&self, w: &Window) -> Option<Strip>;
    /// A segment's colour, when it has one of its own.
    fn colour(&self, t: &Self::Target) -> Option<ronoctua_tokens::Color>;
    /// What a press on a segment does.
    fn clicked(&self, t: &Self::Target, b: Button) -> Option<Act>;
    /// What the wheel does on a segment, `lines` down (negative is up).
    fn wheeled(&self, t: &Self::Target, lines: i64) -> Option<Act>;
    /// Whether a segment can be taken by the pointer and dropped elsewhere.
    fn draggable(&self, t: &Self::Target) -> bool;
    /// What dropping one segment on another does.
    fn dropped(&self, from: &Self::Target, onto: &Self::Target) -> Option<Act>;
    /// What the message line says while the pointer rests on a segment.
    fn hint(&self, t: &Self::Target, texts: &Texts) -> Option<String>;
    /// A counter's target, told how many it counts.
    fn counted(&self, t: &Self::Target, n: usize) -> Self::Target;
    /// The `,` group while its window is focused: `(sequence, command,
    /// argument)`, generated from its view's table.
    fn local_bindings(&self) -> &'static [crate::window_menu::Generated];
}

/// Every adapter there is, over what it reads.
pub enum Adapted<'a> {
    /// The Chrome kiosk.
    Kiosk(crate::kiosk::Adapter<'a>),
}

/// What a segment of any adapter is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A kiosk's.
    Kiosk(crate::kiosk::Target),
}

impl Adapted<'_> {
    /// Its id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Kiosk(a) => a.id(),
        }
    }

    /// Whether it claims this window.
    #[must_use]
    pub fn claims(&self, w: &Window) -> bool {
        match self {
            Self::Kiosk(a) => a.claims(w),
        }
    }

    /// Its segments, each with its target.
    #[must_use]
    pub fn segments(&self, w: &Window, texts: &Texts) -> Vec<(Segment, Target)> {
        match self {
            Self::Kiosk(a) => a
                .segments(w, texts)
                .into_iter()
                .map(|(s, t)| (s, Target::Kiosk(t)))
                .collect(),
        }
    }

    /// Its strip.
    #[must_use]
    pub fn strip(&self, w: &Window) -> Option<Strip> {
        match self {
            Self::Kiosk(a) => a.strip(w),
        }
    }

    /// A segment's colour.
    #[must_use]
    pub fn colour(&self, t: &Target) -> Option<ronoctua_tokens::Color> {
        let (Self::Kiosk(a), Target::Kiosk(t)) = (self, t);
        a.colour(t)
    }

    /// What a press does.
    #[must_use]
    pub fn clicked(&self, t: &Target, b: Button) -> Option<Act> {
        let (Self::Kiosk(a), Target::Kiosk(t)) = (self, t);
        a.clicked(t, b)
    }

    /// What the wheel does.
    #[must_use]
    pub fn wheeled(&self, t: &Target, lines: i64) -> Option<Act> {
        let (Self::Kiosk(a), Target::Kiosk(t)) = (self, t);
        a.wheeled(t, lines)
    }

    /// Whether it can be dragged.
    #[must_use]
    pub fn draggable(&self, t: &Target) -> bool {
        let (Self::Kiosk(a), Target::Kiosk(t)) = (self, t);
        a.draggable(t)
    }

    /// What a drop does.
    #[must_use]
    pub fn dropped(&self, from: &Target, onto: &Target) -> Option<Act> {
        let (Self::Kiosk(a), Target::Kiosk(from), Target::Kiosk(onto)) = (self, from, onto);
        a.dropped(from, onto)
    }

    /// The hover's sentence.
    #[must_use]
    pub fn hint(&self, t: &Target, texts: &Texts) -> Option<String> {
        let (Self::Kiosk(a), Target::Kiosk(t)) = (self, t);
        a.hint(t, texts)
    }

    /// A counter's target, with its count.
    #[must_use]
    pub fn counted(&self, t: &Target, n: usize) -> Target {
        let (Self::Kiosk(a), Target::Kiosk(t)) = (self, t);
        Target::Kiosk(a.counted(t, n))
    }

    /// The `,` group.
    #[must_use]
    pub fn local_bindings(&self) -> &'static [crate::window_menu::Generated] {
        match self {
            Self::Kiosk(a) => a.local_bindings(),
        }
    }

    /// The modeline for a window it claims: `base` — what every window gets —
    /// with the title gone, the program's name promoted, and this adapter's
    /// segments right after it.
    #[must_use]
    pub fn line(&self, base: Vec<Segment>, w: &Window, texts: &Texts) -> Line {
        let own = self.segments(w, texts);
        let targets: Vec<(&'static str, Target)> =
            own.iter().map(|(s, t)| (s.id, t.clone())).collect();
        Line {
            segments: graft(base, own.into_iter().map(|(s, _)| s).collect()),
            strip: self.strip(w),
            targets,
        }
    }
}

/// A modeline as it is about to be laid out.
#[derive(Debug, Clone, Default)]
pub struct Line {
    /// Every segment, the adapter's included.
    pub segments: Vec<Segment>,
    /// The adapter's counted run, when there is one.
    pub strip: Option<Strip>,
    /// What each of the adapter's segments is about, by id.
    pub targets: Vec<(&'static str, Target)>,
}

impl Line {
    /// A line no adapter touched.
    #[must_use]
    pub fn plain(segments: Vec<Segment>) -> Self {
        Self {
            segments,
            strip: None,
            targets: Vec::new(),
        }
    }
}

/// The program's name, made the line's first item: the title is gone (the
/// adapter says it better), so the name is what says which program this is,
/// and it may not go before the adapter's floor does.
///
/// PRIORITY 4 AND UNREMOVABLE — the title's own rung — with a floor of four
/// cells: `goo…` still tells Chrome from anything else on this line.
pub fn promote_mode(segments: &mut [Segment]) {
    if let Some(mode) = segments.iter_mut().find(|s| s.id == "mode") {
        mode.priority = 4;
        mode.min_cols = 4;
        mode.removable = false;
    }
}

/// `base` with the program's name promoted and the adapter's segments put
/// right after it. The left side is laid out in the vector's order
/// (`layout::resolve`), so AFTER is a place in the vector; a title that was
/// there is taken out, whatever the caller passed.
#[must_use]
pub fn graft(mut base: Vec<Segment>, own: Vec<Segment>) -> Vec<Segment> {
    base.retain(|s| s.id != "buffer");
    promote_mode(&mut base);
    let at = base.iter().position(|s| s.id == "mode").map_or_else(
        || {
            base.iter()
                .position(|s| s.side == Side::Left)
                .unwrap_or(base.len())
        },
        |i| i + 1,
    );
    let tail = base.split_off(at);
    base.extend(own);
    base.extend(tail);
    base
}

/// Lays out a line, counters and all, in two passes that cannot disagree.
///
/// # Why two
///
/// A counter says how many members of the strip were NOT drawn, and which
/// were drawn is what the layout decides — so the count cannot be known
/// before the layout, and its width is part of what the layout decides with.
///
/// PASS ONE lays the line out with both counters as wide as the widest they
/// could ever be (the whole strip counted). PASS TWO keeps only what survived
/// pass one, writes the real counts — or drops a counter whose count is zero —
/// and lays that out. Every survivor is at most as wide as it was, so nothing
/// that survived is lost and nothing that was dropped comes back: the answer
/// is the same at every frame for the same input.
///
/// The survivors of the strip are CONTIGUOUS around its unremovable member,
/// because the adapter gives the members rungs that grow with the distance
/// from it; the counts are what lies outside the first and the last survivor.
///
/// Gives back what to draw and, for each of the adapter's segments that was
/// drawn, what it is about — counters told their count.
#[must_use]
pub fn resolve(
    columns: u16,
    line: &Line,
    adapter: Option<&Adapted<'_>>,
    texts: &Texts,
) -> (Vec<Placed>, Vec<(&'static str, Target)>) {
    let drawn_of = |placed: &[Placed], counts: (usize, usize), strip: Option<&Strip>| {
        line.targets
            .iter()
            .filter(|(id, _)| placed.iter().any(|p| p.id == *id))
            .map(|(id, t)| {
                let n = match strip {
                    Some(s) if *id == s.before => Some(counts.0),
                    Some(s) if *id == s.after => Some(counts.1),
                    _ => None,
                };
                match (n, adapter) {
                    (Some(n), Some(a)) => (*id, a.counted(t, n)),
                    _ => (*id, t.clone()),
                }
            })
            .collect::<Vec<_>>()
    };
    let Some(strip) = line.strip.as_ref() else {
        let placed = layout::resolve(columns, &line.segments);
        let drawn = drawn_of(&placed, (0, 0), None);
        return (placed, drawn);
    };
    // PASS ONE, with the counters at their widest.
    let widest = strip.members.len() + strip.beyond.0 + strip.beyond.1;
    let mut first: Vec<Segment> = line.segments.clone();
    for s in &mut first {
        if s.id == strip.before || s.id == strip.after {
            s.text = counter_text(strip.more, widest, texts);
        }
    }
    let placed = layout::resolve(columns, &first);
    let (before, after) = hidden(strip, &placed);
    // PASS TWO: the survivors, with the counts written or the counter gone.
    let second: Vec<Segment> = first
        .into_iter()
        .filter(|s| placed.iter().any(|p| p.id == s.id))
        .filter_map(|mut s| {
            let n = if s.id == strip.before {
                Some(before)
            } else if s.id == strip.after {
                Some(after)
            } else {
                None
            };
            match n {
                Some(0) => None,
                Some(n) => {
                    s.text = counter_text(strip.more, n, texts);
                    Some(s)
                }
                None => Some(s),
            }
        })
        .collect();
    let placed = layout::resolve(columns, &second);
    let drawn = drawn_of(&placed, (before, after), Some(strip));
    (placed, drawn)
}

/// How many members of the strip are not drawn, before the first survivor
/// and after the last — those past the pool included.
fn hidden(strip: &Strip, placed: &[Placed]) -> (usize, usize) {
    let alive: Vec<usize> = strip
        .members
        .iter()
        .enumerate()
        .filter(|(_, id)| placed.iter().any(|p| p.id == **id))
        .map(|(i, _)| i)
        .collect();
    match (alive.first(), alive.last()) {
        (Some(&first), Some(&last)) => (
            strip.beyond.0 + first,
            strip.beyond.1 + strip.members.len() - 1 - last,
        ),
        // NOTHING OF THE STRIP SURVIVED — only on a line narrower than its
        // one unremovable member. Every member is behind the first counter.
        _ => (strip.beyond.0 + strip.members.len(), strip.beyond.1),
    }
}

/// A counter's text, from a NUMBER (`numeric-selectors-get-numbers`).
fn counter_text(more: &str, n: usize, texts: &Texts) -> String {
    texts.t_args(
        more,
        &[(
            "count",
            ronoctua_i18n::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)),
        )],
    )
}

/// The program's name while an adapter holds the line and the footer holds
/// the keyboard: the accent, which is what the title wore — the one blue
/// thing on screen says which window the next key is about.
#[must_use]
pub const fn mode_colour(footer_with_keyboard: bool) -> ronoctua_tokens::Color {
    if footer_with_keyboard {
        ronoctua_tokens::color::STATE_ACCENT
    } else {
        ronoctua_tokens::color::TEXT_SECONDARY
    }
}

/// The file the switches live in.
pub const FILE: &str = "modeline.json";

/// What `modeline.json` keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// One switch per adapter, by its id — ABSENT IS ON: an adapter is what
    /// a person gets until they say otherwise.
    pub adapt: BTreeMap<String, bool>,
    /// Whether the kiosk's address line is up. Absent is off.
    pub address_line: bool,
    /// Whether the modeline and the minibuffer are hidden, and how — see
    /// `crate::hidden`. ABSENT IS VISIBLE: the bars are what a person gets
    /// until they say otherwise.
    ///
    /// AN UNKNOWN VALUE IS A FILE THAT DOES NOT PARSE, and not a default: a
    /// word misspelt by hand is refused with the sentence every switch of
    /// this file already says (`echo-modeline-will-not-parse`), rather than
    /// quietly turning the bars back on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<Hidden>,
    /// Whether the FREE minibuffer — nothing said on it, nothing open —
    /// shows the hours at its right end (`LEADER s r`, `crate::footer_settings`).
    /// ABSENT IS OFF: the owner's word (2026-09-30), a clock the person asks
    /// for and not one the footer puts up.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hours: bool,
}

/// The two hidden modes of the modeline and the minibuffer.
///
/// In both the bars leave the screen at rest and come back at its bottom edge,
/// and the minibuffer shows itself whenever it has something to say. They
/// differ in what the footer does to the windows while it shows anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hidden {
    /// One tap of `Ctrl` at the leader's root: what opens PUSHES the windows
    /// away and stands on the desk they moved off — the desk's colour,
    /// opaque, worn as the desk's page (`crate::hidden::Shape::page`).
    Push,
    /// The second tap: everything lies OVER the windows, on a veil.
    Cover,
}

impl Prefs {
    /// Whether the adapter with this id adapts the modeline.
    #[must_use]
    pub fn adapts(&self, id: &str) -> bool {
        self.adapt.get(id).copied().unwrap_or(true)
    }
}

/// The store the switches live in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Prefs> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// Why `modeline.json` was not taken as it is written — each with its
/// sentence, said at the start and on a change alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// Nothing of it reads: not JSON, or nothing like the switches. Writing
    /// would replace it, so nothing is written (`write_the_modeline`).
    Whole,
    /// A value of it does not read — a word misspelt by hand, a newer
    /// machine's mode: the store repaired it in the value read, and the file
    /// keeps it as written (a write of another switch puts it back).
    Part,
}

impl Unread {
    /// What a read owes the person, if anything.
    #[must_use]
    pub fn of(reading: &ronoctua_store::Reading<Prefs>) -> Option<Self> {
        if reading.lost_everything() {
            Some(Self::Whole)
        } else if reading.part_not_read() {
            Some(Self::Part)
        } else {
            None
        }
    }

    /// Its sentence.
    #[must_use]
    pub fn said(self, texts: &Texts) -> String {
        match self {
            Self::Whole => texts.t(crate::key::ECHO_MODELINE_WILL_NOT_PARSE),
            Self::Part => texts.t_with(
                crate::key::BELONGINGS_REPAIR_PART_NOT_READ,
                &[("file", FILE)],
            ),
        }
    }
}

/// What the footer does with `modeline.json` as it reads now, against the
/// copy it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reread {
    /// Nothing new — this footer's own write, or nothing changed.
    Same,
    /// Another hand changed it: this is the file now.
    Taken(Prefs),
    /// The file did not read as written: the copy held is kept, and the
    /// footer says why ([`Unread::said`]).
    Refused(Unread),
}

/// The file read again, for the watcher — never as a plain `read`.
///
/// A lenient read keeps the rest of a file and REPAIRS the value it cannot
/// take, and the repair of `hidden` is the bars visible: a word misspelt by
/// hand (`"Cover"`) would turn the bars back on with nothing on the line. So
/// a file with a repair is refused here, as one that does not parse is — a
/// refusal is a value with a sentence (`AGENTS.md`), and the copy held stays
/// what it was. The repair is in the ledger, and a write of another switch
/// keeps the person's word in the file.
#[must_use]
pub fn reread(store: &ronoctua_store::Store<Prefs>, held: &Prefs) -> Reread {
    let reading = store.reading();
    if let Some(unread) = Unread::of(&reading) {
        return Reread::Refused(unread);
    }
    if reading.value == *held {
        Reread::Same
    } else {
        Reread::Taken(reading.value)
    }
}

/// The file as the footer STARTS with it: the switches read (repaired where
/// a value did not read — there is no copy to keep yet), and what the read
/// owes the person — the same [`Unread`] a change of the same bytes gives
/// ([`reread`]), so one content never behaves two ways.
#[must_use]
pub fn at_start(store: &ronoctua_store::Store<Prefs>) -> (Prefs, Option<Unread>) {
    let reading = store.reading();
    let unread = Unread::of(&reading);
    (reading.value, unread)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_render::layout::Shrink;

    fn seg(id: &'static str, text: &str, priority: u8, removable: bool) -> Segment {
        Segment {
            id,
            text: text.to_owned(),
            priority,
            min_cols: 1,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable,
        }
    }

    /// FAILS IF: the title stays, the program's name can be dropped, or the
    /// adapter's segments land anywhere but right after it.
    #[test]
    fn the_title_goes_and_the_name_leads() {
        let mut mode = seg("mode", "google-chrome", 6, true);
        mode.min_cols = 4;
        let base = vec![
            seg("notif", "x", 2, true),
            seg("buffer", "a title", 4, false),
            mode,
            seg("state", "s", 7, true),
        ];
        let out = graft(base, vec![seg("a", "1", 9, true), seg("b", "2", 9, true)]);
        let ids: Vec<&str> = out.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["notif", "mode", "a", "b", "state"]);
        let mode = out.iter().find(|s| s.id == "mode").expect("mode");
        assert_eq!((mode.priority, mode.removable), (4, false));
    }

    /// FAILS IF: the program's name stops taking the accent while the footer
    /// has the keyboard, or takes it while it does not.
    #[test]
    fn the_name_takes_the_accent_with_the_keyboard() {
        assert_eq!(mode_colour(true), ronoctua_tokens::color::STATE_ACCENT);
        assert_ne!(mode_colour(false), ronoctua_tokens::color::STATE_ACCENT);
    }

    /// FAILS IF: an absent `hidden` reads as anything but visible, one of the
    /// two modes does not survive a write and a read, or a word nobody wrote
    /// parses into a mode instead of refusing the file.
    #[test]
    fn the_hidden_mode_is_a_word_or_nothing() {
        let absent: Prefs = serde_json::from_str("{}").expect("parses");
        assert_eq!(absent.hidden, None);
        for mode in [Hidden::Push, Hidden::Cover] {
            let p = Prefs {
                hidden: Some(mode),
                ..Prefs::default()
            };
            let text = serde_json::to_string(&p).expect("writes");
            assert_eq!(serde_json::from_str::<Prefs>(&text).expect("reads"), p);
        }
        assert_eq!(
            serde_json::from_str::<Prefs>(r#"{"hidden":"cover"}"#)
                .expect("parses")
                .hidden,
            Some(Hidden::Cover)
        );
        // VISIBLE IS WRITTEN AS NOTHING, so a file that never hid stays the
        // file it was.
        assert!(
            !serde_json::to_string(&Prefs::default())
                .expect("writes")
                .contains("hidden")
        );
        assert!(serde_json::from_str::<Prefs>(r#"{"hidden":"sideways"}"#).is_err());
        assert!(serde_json::from_str::<Prefs>(r#"{"hidden":"Push"}"#).is_err());
    }

    /// FAILS IF: the store reads a file with an unknown mode as anything but
    /// the rest of the file with the mode REPAIRED and listed — what makes
    /// the watcher refuse it with the sentence (`reread`) — or a write of
    /// another switch erases the person's word from the file.
    #[test]
    fn an_unknown_mode_is_repaired_by_the_store_and_kept_in_the_file() {
        let dir = std::env::temp_dir().join(format!("ronoctua-hidden-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a directory");
        std::fs::write(
            dir.join(FILE),
            r#"{"adapt":{"kiosk":false},"hidden":"sideways"}"#,
        )
        .expect("written");
        let store: ronoctua_store::Store<Prefs> =
            ronoctua_store::Store::at(Some(dir.clone()), FILE);
        let reading = store.reading();
        assert_eq!(reading.value.hidden, None);
        assert!(
            !reading.value.adapts("kiosk"),
            "the rest of the file was lost"
        );
        let repaired: Vec<&str> = reading.repairs.iter().map(|r| r.pointer.as_str()).collect();
        assert_eq!(repaired, ["/hidden"]);
        // ANOTHER SWITCH IS WRITTEN, and the person's word stays.
        let mut p = reading.value;
        p.address_line = true;
        store.write(&p).expect("written");
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(FILE)).expect("read")).expect("json");
        assert_eq!(raw["hidden"], "sideways");
        assert_eq!(raw["address_line"], true);
        std::fs::write(dir.join(FILE), r#"{"hidden":"push"}"#).expect("written");
        assert_eq!(store.parsed().and_then(|p| p.hidden), Some(Hidden::Push));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the WATCHER takes a file that does not parse as the default
    /// — the bars back on and every switch reset, in silence — instead of
    /// keeping the copy it holds and refusing; or stops taking a file another
    /// hand changed, or takes its own write as news; or the footer STARTING
    /// over the same bytes owes the person a different sentence, or none —
    /// one content behaving two ways.
    #[test]
    fn the_watcher_refuses_an_unknown_mode_and_keeps_its_copy() {
        let dir = std::env::temp_dir().join(format!("ronoctua-reread-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a directory");
        let store: ronoctua_store::Store<Prefs> =
            ronoctua_store::Store::at(Some(dir.clone()), FILE);
        let held = Prefs {
            hidden: Some(Hidden::Push),
            address_line: true,
            ..Prefs::default()
        };
        // A WORD MISSPELT BY HAND: capital C.
        std::fs::write(dir.join(FILE), r#"{"address_line":true,"hidden":"Cover"}"#)
            .expect("written");
        assert_eq!(reread(&store, &held), Reread::Refused(Unread::Part));
        // AT THE START, the same bytes: the rest of the switches, and the
        // same sentence owed.
        let (start, owed) = at_start(&store);
        assert!(
            start.address_line,
            "the rest of the file was lost at the start"
        );
        assert_eq!(start.hidden, None);
        assert_eq!(owed, Some(Unread::Part));
        // NOT JSON: the same refusal both ways, the other sentence.
        std::fs::write(dir.join(FILE), "{ not json").expect("written");
        assert_eq!(reread(&store, &held), Reread::Refused(Unread::Whole));
        assert_eq!(at_start(&store), (Prefs::default(), Some(Unread::Whole)));
        std::fs::write(dir.join(FILE), r#"{"address_line":true,"hidden":"push"}"#)
            .expect("written");
        assert_eq!(reread(&store, &held), Reread::Same);
        std::fs::write(dir.join(FILE), r#"{"hidden":"cover"}"#).expect("written");
        assert_eq!(
            reread(&store, &held),
            Reread::Taken(Prefs {
                hidden: Some(Hidden::Cover),
                ..Prefs::default()
            })
        );
        // NO FILE is a fresh machine, and the default is right there.
        std::fs::remove_file(dir.join(FILE)).expect("removed");
        assert_eq!(reread(&store, &held), Reread::Taken(Prefs::default()));
        assert_eq!(at_start(&store), (Prefs::default(), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an absent switch reads as off, one adapter's switch turns
    /// another's, or the address line is up before anybody asked.
    #[test]
    fn an_absent_switch_is_on() {
        let p = Prefs::default();
        assert!(p.adapts("kiosk"));
        assert!(!p.address_line);
        let read: Prefs = serde_json::from_str(r#"{"adapt":{"kiosk":false}}"#).expect("parses");
        assert!(!read.adapts("kiosk"));
        assert!(read.adapts("another"));
        assert!(!read.address_line);
    }
}
