//! Formatting a disk, in three questions — `LEADER s f`.
//!
//! # Why this is three lists and never a form
//!
//! Every other way of formatting a disk asks somebody to know things: what a
//! partition table is, which of nine filesystems, what `-F 32` means. This asks
//! three questions, each answered by moving a cursor and pressing `Enter`:
//! **which disk**, **what for**, and **are you sure**. There is exactly one
//! place to type, and it is the disk's name.
//!
//! The words are the point. A row says *"opens on every computer — pendrive,
//! camera, television"*, not *"FAT32"*. Somebody who knows it is FAT32 loses
//! nothing; somebody who does not gets an answer instead of a word to look up.
//!
//! # It reads nothing and it decides nothing
//!
//! Not the bus, not the disk, not a clock. The devices arrive through
//! `input::rack`, already read, already cut and already judged by the host —
//! including the judgement that matters, which is which of them the running
//! system needs. What leaves is `effect::format` naming an id this buffer was
//! GIVEN, and the host checks it again against the list it sent. Invariant 11
//! intact: the component has no parser and is not getting one.
//!
//! # Two things are drawn and not omitted
//!
//! A disk the system needs is **on the list**, greyed, with the reason beside
//! it. And XFS is **on the list of filesystems**, greyed, saying that this
//! distro does not carry the program that writes it. Both could have been left
//! out; both would then be somebody searching a screen for something that is
//! not there, which is the failure this repository refuses everywhere else.

#![allow(unsafe_code, missing_docs)]

use std::cell::RefCell;

wit_bindgen::generate!({
    world: "buffer",
    path: "../../crates/ronoctua-protocol/wit",
    generate_all,
    // The rack arrives whenever udisks says anything, and most of what udisks
    // says changes nothing that is drawn. Comparing needs `PartialEq`, which
    // `wit-bindgen` does not derive.
    additional_derives: [PartialEq],
});

use exports::ronoctua::buffer::view::Guest;
use ronoctua::buffer::types::{
    Capability, Color, Command, Context, Descriptor, Device, Effect, Icon, Input, LineCommand,
    Plan, Rack, Recipe, Scale, Text, TextCommand, Tone,
};

/// Every text key this buffer names.
///
/// A module, and every one of them a literal `pub const`: that is what
/// `buffer-keys-have-text` can see. A key built with `format!` is a key the
/// check cannot find, and the first sign of a missing one would be its own name
/// drawn on screen in front of somebody who was about to erase a disk.
mod key {
    pub const TITLE: &str = "format-title";
    pub const EMPTY: &str = "format-empty";
    pub const HINT_LIST: &str = "format-hint-list";
    pub const HINT_CHOOSE: &str = "format-hint-choose";
    pub const HINT_CONFIRM: &str = "format-hint-confirm";

    /// What a row IS, in the two words that decide everything downstream.
    pub const WHOLE: &str = "format-whole";
    pub const PART: &str = "format-part";
    pub const BLANK: &str = "format-blank";

    pub const CHOOSE_TITLE: &str = "format-choose-title";
    pub const RECIPE_UNIVERSAL: &str = "format-recipe-universal";
    pub const RECIPE_LARGE: &str = "format-recipe-large";
    pub const RECIPE_WINDOWS: &str = "format-recipe-windows";
    pub const RECIPE_LINUX: &str = "format-recipe-linux";
    pub const RECIPE_MODERN: &str = "format-recipe-modern";
    pub const RECIPE_MISSING: &str = "format-recipe-missing";
    pub const NAME: &str = "format-name";
    pub const NAME_LIMIT: &str = "format-name-limit";

    pub const CONFIRM_TITLE: &str = "format-confirm-title";
    pub const CONFIRM_WHOLE: &str = "format-confirm-whole";
    pub const CONFIRM_PART: &str = "format-confirm-part";
    pub const CONFIRM_NO: &str = "format-confirm-no";
    pub const CONFIRM_YES: &str = "format-confirm-yes";

    pub const WORKING: &str = "format-working";
    pub const DONE: &str = "format-done";

    /// THE DETAIL LINE, which is the same screen answering a second person.
    ///
    /// The rows above are written for somebody who does not know what FAT32 is
    /// and should not have to. These are for somebody who does: the real name
    /// of the filesystem, its limits, and what the gesture is actually going to
    /// do to the disk. One line, under the cursor, in the dimmest colour there
    /// is — a beginner never has to read it, and an expert never has to guess.
    pub const TECH_UNIVERSAL: &str = "format-tech-universal";
    pub const TECH_LARGE: &str = "format-tech-large";
    pub const TECH_WINDOWS: &str = "format-tech-windows";
    pub const TECH_LINUX: &str = "format-tech-linux";
    pub const TECH_MODERN: &str = "format-tech-modern";
    pub const TECH_MISSING: &str = "format-tech-missing";
    pub const TECH_WHOLE: &str = "format-tech-whole";
    pub const TECH_PART: &str = "format-tech-part";
    pub const TECH_NEEDED: &str = "format-tech-needed";
    pub const TECH_NOTHING: &str = "format-tech-nothing";
}

/// The states this buffer declares, and what the modeline shows for each.
///
/// # There is deliberately no `working`
///
/// Waiting for a disk to be written was a fourth screen in the first draft, and
/// it had to go: every state a buffer declares is offered under `SPC b s`, and
/// a `working` somebody could pick would put "writing to the disk" on the
/// modeline with nothing being written. `states-driven` is what said so, by
/// refusing a state that cannot be entered from the base.
///
/// So the wait happens on the LIST, with the message line saying what is going
/// on — which is also where it belongs: the row being formatted is about to
/// change under the cursor, and that is the thing worth looking at.
const STATES: &[(&str, char, Tone)] = &[
    ("normal", 'n', Tone::Standard),
    ("choose", 'c', Tone::Accent),
    ("confirm", 'y', Tone::Warning),
];

/// The longest label this buffer will send.
///
/// ELEVEN, WHICH IS vfat's LIMIT AND THE STRICTEST OF THE FIVE. One number for
/// every filesystem rather than five, and the reason is that the number is
/// SHOWN: "up to 11 letters" beside the field is a rule somebody can follow,
/// where a limit that changed as the cursor moved through the list above would
/// be a rule that moves. The cost is a btrfs volume that cannot be called
/// something long, which nobody has ever wanted from this screen.
const NAME_LIMIT: usize = 11;

/// What the line above the hint says, once something has happened.
///
/// TWO CASES AND NOT A STRING, because they come from different places: what
/// went well is a word this buffer owns and looks up by key, and what went
/// wrong was worded by the host — udisks' refusals are its to name, and a
/// component has no bundle to look one up in (invariant 11).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Said {
    /// It is being written right now.
    Working,
    /// It was written.
    Done,
    /// It was refused, in the host's words.
    Trouble(String),
}

/// Which screen is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// Every disk, one per row. Which one?
    List,
    /// Five filesystems and a name. What for?
    Choose,
    /// Two rows, and `no` is the first. Are you sure?
    Confirm,
}

/// The five, in the order they are drawn.
///
/// The order is not the enum's and is not alphabetical: it is most-likely
/// first. Somebody formatting a stick almost always wants the one that opens
/// everywhere, and it is the row the cursor starts on.
/// The five, in the order they are drawn, each with the two ways of saying it.
///
/// # Why the technical name is a KEY and not a string in this file
///
/// Because `vfat` is a fact about disks, and a component with an opinion about
/// disks is a component that has stopped being a drawing (invariant 11). It is
/// also CONTENT — what to call FAT32 in front of somebody, how much of its
/// limit is worth naming — and content lives in the `.ftl` with everything
/// else, in both languages, where the check can see it.
const RECIPES: [(Recipe, &str, &str); 5] = [
    (
        Recipe::Universal,
        key::RECIPE_UNIVERSAL,
        key::TECH_UNIVERSAL,
    ),
    (Recipe::LargeFiles, key::RECIPE_LARGE, key::TECH_LARGE),
    (Recipe::Windows, key::RECIPE_WINDOWS, key::TECH_WINDOWS),
    (Recipe::Linux, key::RECIPE_LINUX, key::TECH_LINUX),
    (Recipe::Modern, key::RECIPE_MODERN, key::TECH_MODERN),
];

/// Where the cursor sits when it is on the row that cannot be chosen.
///
/// # Why XFS is a POSITION and not a caption
///
/// It was drawn as a fixed line under the five at first, and the cursor could
/// not reach it. That made it the one thing on any of these screens that
/// explains itself only to somebody who happened to read it — where the whole
/// rule of this buffer is the opposite: the disk the running system needs is
/// ON the list, reachable, refusing when pressed, and saying why on the line
/// under the cursor. The row that names a filesystem this build cannot write
/// is the same case, and it gets the same shape.
const MISSING: usize = RECIPES.len();

/// Everything this buffer remembers between calls.
struct State {
    rack: Rack,
    screen: Screen,
    /// The cursor on the list, as a device ID and never a row number.
    ///
    /// The rack is re-read and re-sorted whenever udisks says anything — a
    /// stick plugged in reorders it — so a row number would point at whatever
    /// moved into that position between the draw and the key. On a screen whose
    /// next question is "erase this?", that is the whole ballgame.
    on: Option<String>,
    /// Which of [`RECIPES`] the cursor is on.
    recipe: usize,
    /// `false` for "no, go back", which is where the cursor starts.
    yes: bool,
    /// What to call it. Empty is ordinary and means no name.
    name: String,
    /// The device the question is about, once one has been chosen.
    ///
    /// A COPY AND NOT A LOOKUP. Between choosing and confirming the rack may
    /// arrive again with this device gone; the screen has to go on naming what
    /// was chosen, and the host has the last word on whether it is still there.
    target: Option<Device>,
    /// What became of the last format.
    said: Option<Said>,
    /// The window's height, as the last `render` saw it.
    lines: u16,
    /// The line the list starts on, so a click can be resolved.
    list_top: Option<u16>,
}

impl State {
    const fn fresh() -> Self {
        Self {
            rack: Rack {
                devices: Vec::new(),
            },
            screen: Screen::List,
            on: None,
            recipe: 0,
            yes: false,
            name: String::new(),
            target: None,
            said: None,
            lines: 24,
            list_top: None,
        }
    }

    fn state_id(&self) -> String {
        match self.screen {
            Screen::List => STATES[0].0,
            Screen::Choose => STATES[1].0,
            Screen::Confirm => STATES[2].0,
        }
        .to_owned()
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::fresh()) };
}

// ---------------------------------------------------------------------------
// Numbers a person reads.
// ---------------------------------------------------------------------------

/// Bytes as a person reads them.
///
/// The separator is a full stop because a component does not know the language
/// and does not translate (invariant 11) — a comma here would be a decision
/// about locale taken in the one place that cannot make it.
fn size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        let tenths = bytes * 10 / GIB;
        format!("{}.{} GiB", tenths / 10, tenths % 10)
    } else if bytes >= MIB {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// The size again, and this time without rounding.
///
/// `32.0 GiB` is what somebody reads; `34359738368 B` is what somebody
/// COMPARES — against what the vendor printed on the stick, against what
/// `lsblk` said, against the disk they meant to grab. Rounding is the right
/// answer on the row and the wrong one on this line, and that is the whole
/// reason both exist.
///
/// No thousands separator: the character depends on the language and a
/// component does not know one (invariant 11).
fn bytes_exactly(n: u64) -> String {
    format!("{n} B")
}

/// May this device be written over?
///
/// The host's answer, carried on the row. This buffer does not second-guess it
/// and could not: it has no bus and no `/etc/fstab`.
const fn spare(d: &Device) -> bool {
    d.needed.is_empty()
}

// ---------------------------------------------------------------------------
// Drawing.
// ---------------------------------------------------------------------------

/// Where the left edge of everything is.
///
/// TWO CELLS, AND NOT ZERO. A glyph in column 0 sits against the window's edge
/// and reads as a glyph that was cut off rather than one that starts there —
/// the defect this repository has now paid for three times, most recently in
/// the file manager's sidebar. Every column below is measured from here.
const GUTTER: u16 = 2;

/// The air between the last thing drawn and the window's right border.
///
/// The twin of [`GUTTER`], and it exists for the reason this repository has now
/// written down three times: a number that ends against the edge reads as a
/// number that was CUT OFF. `assets/tokens/space.nix` says so in
/// `modeline.edgeRight` and `files.edgeRight`; a component receives no token
/// values (invariant 11), so here it is the same decision spelled out again.
const EDGE_RIGHT: u16 = 1;

/// How little room the technical sentence may be left with before the number
/// beside it gives way.
///
/// Below this the sentence is elided to a stub and says nothing, which is worse
/// than a screen with no exact byte count: the count is precision added to
/// something the sentence already carries, and the sentence has no second place
/// to be.
const LEAST_SENTENCE: u16 = 24;

/// The gap between one column of a row and the next.
///
/// Also not zero, and for the same reason one step down: two words that touch
/// read as one word. The size and the reason beside it are the pair that would
/// run together first, because both are short.
const GAP: u16 = 2;

fn put(out: &mut Vec<Command>, line: u16, column: u16, content: Text, color: Color, width: u16) {
    out.push(Command::Text(TextCommand {
        line,
        column,
        content,
        color,
        scale: Scale::Normal,
        max_width: width,
    }));
}

/// The title, and the rule under it.
fn title(out: &mut Vec<Command>, icon: Icon, text: &str, tint: Color, ctx: &Context) -> u16 {
    out.push(Command::Text(TextCommand {
        line: 0,
        column: GUTTER,
        content: Text::Icon(icon),
        color: tint,
        scale: Scale::Large,
        max_width: 2,
    }));
    out.push(Command::Text(TextCommand {
        line: 0,
        column: GUTTER + 3,
        content: Text::Key(text.to_owned()),
        color: Color::TextPrimary,
        scale: Scale::Large,
        max_width: ctx.columns.saturating_sub(GUTTER + 3),
    }));
    out.push(Command::Rule(LineCommand {
        line: 1,
        color: Color::RuleDefault,
    }));
    3
}

/// The last line: what the keys do, and above it what just happened.
/// What the cursor is on, said twice — a label above, and the truth below.
///
/// A key for the vocabulary and a string for the machine's own identifiers: a
/// device node and a byte count are the same in every language, and a sentence
/// that had to interpolate them would need the host to word it. This way the
/// buffer draws two fields and neither of them is assembled prose.
struct Detail {
    /// The technical vocabulary, by key.
    said: &'static str,
    /// The node, the exact size, the name to be written — whatever the screen
    /// under the cursor has that is not already on it. Empty is ordinary.
    facts: String,
}

/// The last three lines: what happened, what this is, and what the keys do.
///
/// # Why the detail sits between the two and not beside the hint
///
/// Because it CHANGES as the cursor moves and the hint does not. Two things on
/// one line, one of them still and one of them moving, is a line whose motion
/// reads as a glitch — and the eye that learns to ignore the hint would learn
/// to ignore the detail with it.
fn foot(out: &mut Vec<Command>, hint: &str, said: Option<&Said>, detail: &Detail, ctx: &Context) {
    let last = ctx.lines.saturating_sub(1);

    // THE DETAIL, IN THE DIMMEST COLOUR THERE IS. That is the whole compromise:
    // somebody who does not know what FAT32 is never has to read this line, and
    // somebody who does never has to guess which of five rows means it.
    let at = last.saturating_sub(1);

    // THE RIGHT-HAND HALF IS PLACED FIRST, because it is what BOUNDS the other.
    //
    // THE FIRST VERSION DREW THE SENTENCE AT FULL WIDTH AND THE NUMBER OVER IT.
    // A screenshot caught it — `…dos até 2 §1BO88p4 B` — and it is the same
    // fault this project has now paid for four times: two fields placed by
    // arithmetic with no single place deciding the air between them. So the
    // number's column is computed FIRST, and the sentence is given the room
    // that is left, minus a gap. Neither can reach the other, in any language
    // and at any window width.
    //
    // `EDGE_RIGHT` in front of the number for the reason `space.nix` already
    // records twice: a count against the window's border reads as a number that
    // was CUT OFF, not as one that ended.
    let width = u16::try_from(detail.facts.chars().count()).unwrap_or(u16::MAX);
    let from = ctx.columns.saturating_sub(width + EDGE_RIGHT);
    let room = from.saturating_sub(GUTTER + GAP);
    // AND THE NUMBER GIVES WAY FIRST when the window cannot hold both. It adds
    // precision to something the sentence already says; the sentence has no
    // second place to be.
    let showing = !detail.facts.is_empty() && room >= LEAST_SENTENCE;
    put(
        out,
        at,
        GUTTER,
        Text::Key(detail.said.to_owned()),
        Color::TextDim,
        if showing {
            room
        } else {
            ctx.columns.saturating_sub(GUTTER)
        },
    );
    if showing {
        put(
            out,
            at,
            from,
            Text::Data(detail.facts.clone()),
            Color::TextDim,
            width,
        );
    }

    if let Some(what) = said {
        let (content, colour) = match what {
            Said::Working => (Text::Key(key::WORKING.to_owned()), Color::StateWarning),
            Said::Done => (Text::Key(key::DONE.to_owned()), Color::StateSuccess),
            Said::Trouble(why) => (Text::Data(why.clone()), Color::StateError),
        };
        put(
            out,
            last.saturating_sub(2),
            GUTTER,
            content,
            colour,
            ctx.columns.saturating_sub(GUTTER),
        );
    }
    put(
        out,
        last,
        GUTTER,
        Text::Key(hint.to_owned()),
        Color::TextDim,
        ctx.columns.saturating_sub(GUTTER),
    );
}

/// A row that is chosen, drawn as chosen.
fn mark(out: &mut Vec<Command>, line: u16, chosen: bool) {
    if chosen {
        out.push(Command::Selection(LineCommand {
            line,
            color: Color::SurfaceSelection,
        }));
    }
}

impl State {
    /// The disks, one per row. *Which one?*
    fn list(&mut self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let line = title(&mut out, Icon::Disk, key::TITLE, Color::StateAccent, ctx);

        if self.rack.devices.is_empty() {
            self.list_top = None;
            put(
                &mut out,
                line,
                GUTTER,
                Text::Key(key::EMPTY.to_owned()),
                Color::TextSecondary,
                ctx.columns.saturating_sub(GUTTER),
            );
            foot(&mut out, key::HINT_LIST, self.said(), &self.detail(), ctx);
            return out;
        }

        self.list_top = Some(line);
        // The last two lines belong to the message and the hint.
        let room = usize::from(self.lines.saturating_sub(line + 2)).max(1);

        // WHERE EACH COLUMN STARTS, worked out from the widest name on the
        // screen rather than from a constant. A fixed column would either
        // truncate `SanDisk Cruzer Blade` or leave a hand's width of nothing in
        // front of `/dev/sda`, and both look like a bug in the drawing.
        let widest = self
            .rack
            .devices
            .iter()
            .take(room)
            .map(|d| d.name.chars().count())
            .max()
            .unwrap_or(0);
        let name_width = u16::try_from(widest).unwrap_or(u16::MAX).clamp(8, 24);
        let node_at = GUTTER + 3 + name_width + GAP;
        let node_width = 14;
        let after_node = node_at + node_width + GAP;

        // AND WHAT GETS SACRIFICED WHEN THE WINDOW IS NARROW, which is a
        // decision and not an accident.
        //
        // The first draft laid five columns out at fixed offsets and let the
        // last one run off the right edge. On the screen that found it, the
        // machine's own root was a row whose ONLY signal that it could not be
        // touched was a grey glyph — the sentence saying why had fallen off the
        // window. A colour alone is a rule somebody has to already know, which
        // is exactly what this screen exists not to require.
        //
        // So the reason is the one column that is never dropped, and on a row
        // that HAS one it takes the space the filesystem and the size would
        // have used: neither of those matters about a disk nobody may write to.
        let kind_width = 8;
        let size_width = 10;
        let facts_fit = after_node + kind_width + GAP + size_width <= ctx.columns;

        for (n, d) in self.rack.devices.iter().take(room).enumerate() {
            let at = line + u16::try_from(n).unwrap_or(0);
            let chosen = self.on.as_deref() == Some(d.id.as_str());
            mark(&mut out, at, chosen);

            // MUTED MEANS "NOT ON OFFER", and it is never the only thing that
            // says so: the reason is spelled out on the same row.
            //
            // `TextSecondary` and NOT `TextDim`, which is what it was until a
            // VM read the screen back: dim text under the cursor's own band is
            // the lowest-contrast pair the palette has, and the row went
            // unreadable exactly when somebody had moved onto it to find out
            // why. A row nobody may use still has to be a row somebody can
            // READ.
            let tint = if spare(d) {
                Color::TextPrimary
            } else {
                Color::TextSecondary
            };
            let glyph = if !spare(d) {
                Icon::Readonly
            } else if d.removable {
                Icon::DiskRemovable
            } else {
                Icon::Disk
            };
            put(&mut out, at, GUTTER, Text::Icon(glyph), tint, 2);
            put(
                &mut out,
                at,
                GUTTER + 3,
                Text::Data(d.name.clone()),
                tint,
                name_width,
            );
            put(
                &mut out,
                at,
                node_at,
                Text::Data(d.node.clone()),
                Color::TextSecondary,
                node_width,
            );

            if !spare(d) {
                // THE REASON, TAKING THE WHOLE REST OF THE ROW.
                put(
                    &mut out,
                    at,
                    after_node,
                    Text::Data(d.needed.clone()),
                    Color::StateWarning,
                    ctx.columns.saturating_sub(after_node),
                );
                continue;
            }

            if !facts_fit {
                // TOO NARROW FOR THE FACTS, so what stays is what the next
                // question needs: whole disk or one part of it.
                let k = if d.whole { key::WHOLE } else { key::PART };
                put(
                    &mut out,
                    at,
                    after_node,
                    Text::Key(k.to_owned()),
                    Color::TextDim,
                    ctx.columns.saturating_sub(after_node),
                );
                continue;
            }

            // AN EMPTY `kind` IS A SENTENCE AND NOT A BLANK. A partition with
            // nothing written on it is the most ordinary thing this screen is
            // for, and a gap where the others have a word reads as a row that
            // failed to load.
            let kind = if d.kind.is_empty() {
                Text::Key(key::BLANK.to_owned())
            } else {
                Text::Data(d.kind.clone())
            };
            put(
                &mut out,
                at,
                after_node,
                kind,
                Color::TextSecondary,
                kind_width,
            );
            let size_at = after_node + kind_width + GAP;
            put(
                &mut out,
                at,
                size_at,
                Text::Data(size(d.bytes)),
                Color::TextSecondary,
                size_width,
            );

            // AND THE LAST COLUMN SAYS WHAT THE ROW IS. `whole` decides which
            // question comes next — a whole disk gets a new partition table and
            // a partition cannot — so it is on the row rather than discovered
            // one screen later. It is the first thing dropped when the window
            // cannot hold it, because the two screens after this one repeat it.
            let what_at = size_at + size_width + GAP;
            if what_at < ctx.columns {
                let k = if d.whole { key::WHOLE } else { key::PART };
                put(
                    &mut out,
                    at,
                    what_at,
                    Text::Key(k.to_owned()),
                    Color::TextDim,
                    ctx.columns.saturating_sub(what_at),
                );
            }
        }

        foot(&mut out, key::HINT_LIST, self.said(), &self.detail(), ctx);
        out
    }

    /// Five filesystems and a name. *What for?*
    fn choose(&self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let mut line = title(
            &mut out,
            Icon::Disk,
            key::CHOOSE_TITLE,
            Color::StateAccent,
            ctx,
        );

        // WHICH DISK, ON THE SCREEN THAT ASKS ABOUT IT. The title cannot say it
        // — a key is looked up whole, with no arguments (invariant 11) — so the
        // name goes on a line of its own, right under it, where somebody who
        // arrived here by pressing `Enter` twice can still see what they chose.
        if let Some(t) = self.target.as_ref() {
            put(
                &mut out,
                line,
                GUTTER,
                Text::Data(format!("{}  {}  {}", t.name, t.node, size(t.bytes))),
                Color::TextSecondary,
                ctx.columns.saturating_sub(GUTTER),
            );
            line += 2;
        }

        for (n, (_, text, _)) in RECIPES.iter().enumerate() {
            let at = line + u16::try_from(n).unwrap_or(0);
            mark(&mut out, at, n == self.recipe);
            put(
                &mut out,
                at,
                GUTTER,
                Text::Key((*text).to_owned()),
                Color::TextPrimary,
                ctx.columns.saturating_sub(GUTTER),
            );
        }
        let mut at = line + u16::try_from(RECIPES.len()).unwrap_or(0);

        // XFS, DRAWN AND REFUSED — and reachable, which is the part that took
        // a second pass. It could have been left off the list and nobody would
        // have written a bug report; what they would have done is look for it,
        // not find it, and wonder whether they were on the wrong screen.
        //
        // The row says the program that writes it is not installed, which is a
        // fact about this distro and not about their disk; the line under the
        // cursor names the program.
        mark(&mut out, at, self.recipe == MISSING);
        put(
            &mut out,
            at,
            GUTTER,
            Text::Icon(Icon::Readonly),
            Color::TextSecondary,
            2,
        );
        put(
            &mut out,
            at,
            GUTTER + 3,
            Text::Key(key::RECIPE_MISSING.to_owned()),
            Color::TextSecondary,
            ctx.columns.saturating_sub(GUTTER + 3),
        );
        at += 2;

        // THE ONE PLACE TO TYPE. The cursor is a glyph and not a colour, so it
        // is visible on a screenshot and to somebody who cannot tell the two
        // greys apart.
        put(
            &mut out,
            at,
            GUTTER,
            Text::Key(key::NAME.to_owned()),
            Color::TextSecondary,
            8,
        );
        put(
            &mut out,
            at,
            GUTTER + 8 + GAP,
            Text::Data(format!("{}_", self.name)),
            Color::TextPrimary,
            u16::try_from(NAME_LIMIT).unwrap_or(11) + 1,
        );
        put(
            &mut out,
            at,
            GUTTER + 8 + GAP + u16::try_from(NAME_LIMIT).unwrap_or(11) + 1 + GAP,
            Text::Key(key::NAME_LIMIT.to_owned()),
            Color::TextDim,
            ctx.columns.saturating_sub(GUTTER + 24),
        );

        foot(&mut out, key::HINT_CHOOSE, None, &self.detail(), ctx);
        out
    }

    /// Two rows, and `no` is the first. *Are you sure?*
    fn confirm(&self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let mut line = title(
            &mut out,
            Icon::Warning,
            key::CONFIRM_TITLE,
            Color::StateWarning,
            ctx,
        );

        if let Some(t) = self.target.as_ref() {
            put(
                &mut out,
                line,
                GUTTER,
                Text::Data(format!("{}  {}  {}", t.name, t.node, size(t.bytes))),
                Color::TextPrimary,
                ctx.columns.saturating_sub(GUTTER),
            );
            line += 1;
            // AND WHAT "EVERYTHING" MEANS HERE. On a whole disk it is every
            // partition on it, which is more than the row above shows and is
            // exactly the surprise this screen exists to prevent.
            let scope = if t.whole {
                key::CONFIRM_WHOLE
            } else {
                key::CONFIRM_PART
            };
            put(
                &mut out,
                line,
                GUTTER,
                Text::Key(scope.to_owned()),
                Color::StateWarning,
                ctx.columns.saturating_sub(GUTTER),
            );
            line += 2;
        }

        // NO IS FIRST, AND IT IS WHERE THE CURSOR STARTS. Somebody who arrived
        // here by pressing `Enter` three times and presses it a fourth out of
        // rhythm goes back, not ahead. It costs one deliberate keystroke to
        // erase a disk, which is the right price.
        for (n, (text, colour)) in [
            (key::CONFIRM_NO, Color::TextPrimary),
            (key::CONFIRM_YES, Color::StateError),
        ]
        .iter()
        .enumerate()
        {
            let at = line + u16::try_from(n).unwrap_or(0);
            mark(&mut out, at, self.yes == (n == 1));
            put(
                &mut out,
                at,
                GUTTER,
                Text::Key((*text).to_owned()),
                *colour,
                ctx.columns.saturating_sub(GUTTER),
            );
        }

        foot(&mut out, key::HINT_CONFIRM, None, &self.detail(), ctx);
        out
    }

    /// The technical line for whichever row the cursor is on.
    ///
    /// A lookup and never an index, so the row that refuses cannot be a panic:
    /// `MISSING` is deliberately one past the end of [`RECIPES`], and every
    /// reader of `self.recipe` has to cope with it.
    fn tech(&self) -> &'static str {
        RECIPES.get(self.recipe).map_or(key::TECH_MISSING, |r| r.2)
    }

    /// What the cursor is on, in the words of somebody who already knows.
    ///
    /// # One method for three screens, and that is the point
    ///
    /// The detail line is not a caption on the list — it is the answer to "what
    /// does the thing under the cursor DO", and every screen here has a thing
    /// under a cursor. Written per screen, the three would drift: the `choose`
    /// row would name a filesystem and the `confirm` row would name a different
    /// one, because the two would be reading the cursor from different places.
    fn detail(&self) -> Detail {
        match self.screen {
            Screen::List => match self.focused() {
                None => Detail {
                    said: key::TECH_NOTHING,
                    facts: String::new(),
                },
                // A DISK THE SYSTEM NEEDS GETS THE TECHNICAL ANSWER TOO. The
                // row already says the system lives there; this says what would
                // happen if the row were pressed anyway — udisks itself would
                // refuse, and this screen does not even get that far.
                Some(d) if !spare(d) => Detail {
                    said: key::TECH_NEEDED,
                    facts: bytes_exactly(d.bytes),
                },
                Some(d) => Detail {
                    said: if d.whole {
                        key::TECH_WHOLE
                    } else {
                        key::TECH_PART
                    },
                    facts: bytes_exactly(d.bytes),
                },
            },
            // THE ONE THE QUESTION WAS ASKED FOR. `abre em qualquer computador`
            // is the honest label and `vfat (FAT32), até 4 GiB por arquivo` is
            // the honest answer; neither replaces the other, and they are one
            // arrow key apart.
            Screen::Choose => Detail {
                said: self.tech(),
                facts: self
                    .target
                    .as_ref()
                    .map(|t| t.node.clone())
                    .unwrap_or_default(),
            },
            // AND ON THE LAST SCREEN IT SPELLS OUT THE WHOLE GESTURE: which
            // filesystem, onto which node, under which name. It is the closest
            // thing to the command somebody would have typed, and it is on the
            // screen where being sure is the entire question.
            Screen::Confirm => Detail {
                said: self.tech(),
                facts: match self.target.as_ref() {
                    None => String::new(),
                    Some(t) if self.name.is_empty() => t.node.clone(),
                    Some(t) => format!("{} · \"{}\"", t.node, self.name),
                },
            },
        }
    }

    /// The message under the hint, if anything has happened yet.
    const fn said(&self) -> Option<&Said> {
        self.said.as_ref()
    }

    // -----------------------------------------------------------------------
    // Keys.
    // -----------------------------------------------------------------------

    /// Which device the cursor is on, if the rack still has it.
    fn focused(&self) -> Option<&Device> {
        let on = self.on.as_deref()?;
        self.rack.devices.iter().find(|d| d.id == on)
    }

    fn move_by(&mut self, down: bool) {
        match self.screen {
            Screen::List => {
                if self.rack.devices.is_empty() {
                    return;
                }
                let at = self
                    .on
                    .as_deref()
                    .and_then(|on| self.rack.devices.iter().position(|d| d.id == on))
                    .unwrap_or(0);
                let last = self.rack.devices.len() - 1;
                let to = if down {
                    if at >= last { 0 } else { at + 1 }
                } else if at == 0 {
                    last
                } else {
                    at - 1
                };
                self.on = Some(self.rack.devices[to].id.clone());
            }
            Screen::Choose => {
                // `MISSING` AND NOT `len() - 1`: the walk covers the row that
                // refuses, exactly as the list's walk covers the disk that
                // refuses.
                let last = MISSING;
                self.recipe = if down {
                    if self.recipe >= last {
                        0
                    } else {
                        self.recipe + 1
                    }
                } else if self.recipe == 0 {
                    last
                } else {
                    self.recipe - 1
                };
            }
            // TWO ROWS, SO EITHER ARROW SWAPS THEM. Clamping instead would make
            // `up` on the first row do nothing, and a key that does nothing on
            // a two-row list reads as a key that did not arrive.
            Screen::Confirm => self.yes = !self.yes,
        }
    }

    /// `Enter`.
    fn forward(&mut self) -> Vec<Effect> {
        match self.screen {
            Screen::List => {
                let Some(d) = self.focused() else {
                    return vec![];
                };
                // A DISK THE SYSTEM NEEDS DOES NOT MOVE FORWARD, and it says
                // nothing new when pressed: the reason is already at the end of
                // the row it is sitting on. A message here would be the same
                // sentence twice, one of them somewhere it has to be dismissed.
                if !spare(d) {
                    return vec![];
                }
                self.target = Some(d.clone());
                self.recipe = 0;
                self.name.clear();
                self.said = None;
                self.screen = Screen::Choose;
                vec![Effect::Redraw]
            }
            Screen::Choose => {
                // THE THIRD REFUSAL, and the same one twice: a row that names
                // something this build cannot do does not move forward, and it
                // says nothing new when pressed — the reason is already on the
                // row and on the line under the cursor.
                if RECIPES.get(self.recipe).is_none() {
                    return vec![];
                }
                self.yes = false;
                self.screen = Screen::Confirm;
                vec![Effect::Redraw]
            }
            Screen::Confirm => {
                if !self.yes {
                    self.screen = Screen::Choose;
                    return vec![Effect::Redraw];
                }
                let Some(t) = self.target.as_ref() else {
                    self.screen = Screen::List;
                    return vec![Effect::Redraw];
                };
                let Some(recipe) = RECIPES.get(self.recipe) else {
                    // UNREACHABLE THROUGH THE KEYS, and refused anyway. `Enter`
                    // on the row that refuses never gets past `choose`, but a
                    // state entered by name from the modeline arrives here
                    // without having walked, and an index one past the end
                    // would be a panic in the component rather than a refusal.
                    self.screen = Screen::Choose;
                    return vec![Effect::Redraw];
                };
                let plan = Plan {
                    // THE ID THE HOST GAVE, passed back untouched. This buffer
                    // never builds one and could not: it has no idea what the
                    // string means, which is the point of it being opaque.
                    device: t.id.clone(),
                    recipe: recipe.0,
                    label: self.name.clone(),
                    // A WHOLE DISK GETS A NEW PARTITION TABLE, a partition does
                    // not — and the answer comes off the row rather than off a
                    // question, because it is a fact about the device and not a
                    // preference.
                    whole_disk: t.whole,
                };
                // BACK TO THE LIST, AND NOT TO A SCREEN OF ITS OWN. The row
                // being written is on that list and is about to change; a
                // waiting screen would hide the one thing worth watching. What
                // says a disk is being written is the message line.
                self.screen = Screen::List;
                self.said = Some(Said::Working);
                vec![Effect::Format(plan), Effect::Redraw]
            }
        }
    }

    /// `Esc`.
    fn back(&mut self) -> Vec<Effect> {
        match self.screen {
            // ONE LEVEL, AND THE BUFFER IS WHAT WALKS. `Esc` on the first
            // screen is the host's to act on, and it closes the window.
            Screen::List => vec![],
            Screen::Choose => {
                self.screen = Screen::List;
                self.target = None;
                vec![Effect::Redraw]
            }
            Screen::Confirm => {
                self.screen = Screen::Choose;
                vec![Effect::Redraw]
            }
        }
    }

    /// A letter, which only ever means the name.
    fn letter(&mut self, c: char) -> Vec<Effect> {
        if self.screen != Screen::Choose {
            return vec![];
        }
        // WHAT A LABEL MAY HOLD, decided here and not by udisks refusing later.
        // A space or an accent is legal on some of the five and not on others,
        // and a refusal arriving after the confirmation screen would be a
        // refusal for something typed three screens ago.
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            return vec![];
        }
        if self.name.chars().count() >= NAME_LIMIT {
            return vec![];
        }
        self.name.push(c.to_ascii_uppercase());
        vec![Effect::Redraw]
    }

    /// A fresh reading arrived.
    fn racked(&mut self, rack: Rack) -> Vec<Effect> {
        if self.rack == rack {
            return vec![];
        }
        self.rack = rack;
        // THE CURSOR SURVIVES BY ID, or lands on the first row somebody can
        // actually use. A stick pulled out while this screen is open takes its
        // row with it, and a cursor pointing at nothing would make `Enter` do
        // nothing with no reason given.
        //
        // FIRST SPARE, AND NOT SIMPLY FIRST. udisks sorts by device node, so
        // on an ordinary machine the first row is the internal disk the system
        // booted from — the one row on the screen that goes nowhere. Starting
        // there means the first key everybody presses does nothing.
        let still = self
            .on
            .as_deref()
            .is_some_and(|on| self.rack.devices.iter().any(|d| d.id == on));
        if !still {
            self.on = self
                .rack
                .devices
                .iter()
                .find(|d| spare(d))
                .or_else(|| self.rack.devices.first())
                .map(|d| d.id.clone());
        }
        vec![Effect::Redraw]
    }

    /// The host answered about the format.
    ///
    /// SUCCESS IS SAID OUT LOUD, even though the row is about to change under
    /// the cursor on its own. A screen that speaks only when something breaks
    /// teaches people that silence means failure, and this one has just spent
    /// three questions earning the opposite.
    fn formatted(&mut self, done: bool, trouble: String) -> Vec<Effect> {
        self.screen = Screen::List;
        self.target = None;
        self.said = Some(if done {
            Said::Done
        } else {
            Said::Trouble(trouble)
        });
        vec![Effect::Redraw]
    }

    /// The host asked for a screen by name.
    ///
    /// # Why `choose` and `confirm` take the cursor's disk when they have none
    ///
    /// Because a screen entered from the modeline has nothing behind it, and
    /// the two of them are ABOUT a disk. Falling back to the one under the
    /// cursor is what somebody meant; falling back to nothing would draw a
    /// question with no subject, and `Enter` on it would go back to the list
    /// having said nothing.
    fn enter(&mut self, id: &str) -> Vec<Effect> {
        let to = match id {
            "normal" => Screen::List,
            "choose" => Screen::Choose,
            "confirm" => Screen::Confirm,
            // AN ID THIS BUFFER DID NOT DECLARE. The host only ever sends one
            // it was told, so this is a defect on the other side rather than a
            // person's mistake — and guessing at the nearest screen would hide
            // it.
            _ => return vec![],
        };
        if to != Screen::List && self.target.is_none() {
            self.target = self.focused().filter(|d| spare(d)).cloned();
        }
        if to == Screen::Confirm {
            self.yes = false;
        }
        self.screen = to;
        vec![Effect::Redraw]
    }

    /// A click does what the key on that row does, and nothing else.
    fn clicked(&mut self, line: u16) -> Vec<Effect> {
        let Some(top) = self.list_top else {
            return vec![];
        };
        if self.screen != Screen::List || line < top {
            return vec![];
        }
        let n = usize::from(line - top);
        let Some(d) = self.rack.devices.get(n) else {
            return vec![];
        };
        self.on = Some(d.id.clone());
        self.forward()
    }

    fn act(&mut self, e: Input) -> Vec<Effect> {
        match e {
            Input::Up | Input::Left => {
                self.move_by(false);
                vec![Effect::Redraw]
            }
            Input::Down | Input::Right => {
                self.move_by(true);
                vec![Effect::Redraw]
            }
            Input::Confirm => self.forward(),
            Input::Cancel => self.back(),
            Input::Character(c) => self.letter(c),
            Input::Delete => {
                if self.screen == Screen::Choose && self.name.pop().is_some() {
                    vec![Effect::Redraw]
                } else {
                    vec![]
                }
            }
            Input::Click(p) => self.clicked(p.line),
            Input::Rack(r) => self.racked(r),
            Input::Formatted(o) => self.formatted(o.done, o.trouble),
            // THE REST OF THE CONTRACT, answered by ignoring it. The `match` is
            // exhaustive and the WIT is shared by seven components: an `input`
            // case added for one of them has to be answered by all, and here
            // "answered" is "this screen has no use for it".
            // THE HOST ASKS FOR A SCREEN BY NAME — `SPC b s`, or the modeline
            // clicked. Answering it is what makes a declared state a state
            // somebody can reach; a buffer that ignored it would leave the
            // modeline offering screens nobody enters, which is the failure
            // `states-driven` exists to catch.
            Input::EnterState(id) => self.enter(&id),
            Input::Command(_)
            | Input::Copied(_)
            | Input::Paste(_)
            | Input::Toggle
            | Input::Survey(_)
            | Input::Queue(_) => vec![],
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "format".into(),
            mode: "format".into(),
            states: STATES
                .iter()
                .map(|(id, letter, tone)| ronoctua::buffer::types::BufferState {
                    id: (*id).into(),
                    letter: *letter,
                    tone: *tone,
                })
                .collect(),
            // NO KEYMAP, and that is the design rather than an omission. Every
            // answer on every screen is an arrow and `Enter`; a command under
            // `m ` would be a second way to do what `Enter` already does, and a
            // second way is a second thing to keep in step.
            keymap: vec![],
            commands: vec![],
            segments: vec![],
            // THE ONE THAT MATTERS: without it the host neither subscribes to
            // udisks nor forwards a plan, and this is an empty screen.
            capabilities: vec![Capability::Disks],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| {
            s.lines = ctx.lines;
            match s.screen {
                Screen::List => s.list(&ctx),
                Screen::Choose => s.choose(&ctx),
                Screen::Confirm => s.confirm(&ctx),
            }
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        // THE STATE IS REPORTED FROM ONE PLACE, and not at each site that
        // assigns `screen`. There are nine of those; a report per site is a
        // report somebody forgets, and what it costs is the modeline saying
        // `normal` while a disk is being written.
        STATE.with_borrow_mut(|s| {
            let before = s.state_id();
            let mut out = s.act(e);
            let after = s.state_id();
            if before != after {
                out.push(Effect::StateChanged(after));
            }
            out
        })
    }
}

export!(Component);
