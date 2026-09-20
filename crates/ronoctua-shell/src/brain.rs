//! The brain's line — the one under the minibuffer, and the host half feeding it.
//!
//! # Why a line of its own, and why UNDER the pair
//!
//! The owner asked for it in those words: when it is time to update, a line
//! appears below everything, in a colour of its own, saying what is about to
//! happen with a countdown and two answers.
//!
//! It is not a popup and not a card (invariants 14 and 15): it is the footer's
//! OWN surface growing, exactly as the notification band grows upwards and the
//! expansion grows downwards. One surface, one more region.
//!
//! Glued under the minibuffer and NOT at the bottom of the surface, and the
//! third reason is the one that decides it:
//!
//! 1. it is what was asked for — *below the minibuffer region*;
//! 2. it keeps the footer symmetric — band · modeline · minibuffer · brain ·
//!    expansion — so the fixed rows sit above the list and this line moves on
//!    screen exactly when the modeline does;
//! 3. a truly-last line would live at `first_of_expansion() + expansion_rows()`
//!    and would have to be recomputed in the painter, in the hit test and in
//!    the background quad, for ever. That is the defect
//!    [`crate::surface::Footer::first_of_expansion`] was written to remove —
//!    "two calculations of the same thing, and the one that gets it wrong is
//!    the one nobody looks at". Glued under the minibuffer, that one function
//!    gains ONE term and every reader downstream follows for free.
//!
//! # Why the layout half has no Wayland in it
//!
//! Everything above [`lines`] is `(what is showing, width) -> pieces with
//! columns`. The same two consequences [`crate::band`] claims: it is testable
//! headless, and it fits the frame budget because it is integer arithmetic over
//! the cell.
//!
//! # And the host half
//!
//! Below the layout: the thread that watches the daemon, the conversion from
//! the daemon's [`ronoctua_brain::Mind`] into the WIT's, and the two asks the
//! band's controls send. The same shape [`crate::transfers`] already has for
//! the queue — one file per border, layout and wire together, because they are
//! two halves of one feature and splitting them would hide which is which.

use ronoctua_brain::{Announcement, Mind};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// What a click on a piece of the line does.
///
/// Exactly two, and each one has a key: `brain-abort` and `brain-proceed` are
/// registered commands with a sequence of their own, drawn on the line itself.
/// The mouse reaches nothing the keyboard does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Not now.
    ///
    /// # Why it postpones instead of aborting, while nothing is running
    ///
    /// Because taking a task out of the queue is not what stops it. The
    /// runner's own comment says so: `Runner::pump` enrols whatever is due on
    /// every beat, so a task unqueued while still due is back in the queue half
    /// a minute later, "and the person would answer the same question every
    /// half minute". What moves the next moment is the postponement, which the
    /// runner writes down as an event. See [`abort`].
    Abort,
    /// Do not wait the countdown out — start now.
    Proceed,
    /// Do whatever the standing notice offered.
    ///
    /// ONE variant and not one per notice, because what it runs is named by
    /// the notice itself — a command id, which the registry already resolves.
    /// A variant per offer would be this enum growing every time a watch
    /// learned to suggest something.
    Notice,
}

/// One drawn piece of the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// First cell.
    pub at: u16,
    /// What is drawn, already elided.
    pub text: String,
    /// Which token colours it.
    pub color: tokens::Color,
    /// What a click does. `None` = it is text, not a control.
    pub hit: Option<Hit>,
    /// What the message line says while the pointer rests here.
    ///
    /// Already translated, for the reason [`crate::band::Piece::explain`] gives:
    /// the alternative is the drawing carrying keys and arguments around.
    pub explain: Option<String>,
}

/// The line, laid out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    /// Left to right.
    pub pieces: Vec<Piece>,
}

/// What the line needs, resolved once.
///
/// A struct and not three arguments, the same reason [`crate::band::Words`] is
/// one: they are always wanted together, and a caller that forgot the bundle
/// would get `[brain-band-soon]` on screen rather than a compile error.
pub struct Words<'a> {
    /// Where the sentences come from.
    pub texts: &'a ronoctua_i18n::Texts,
    /// How `brain-abort` is typed, leader included — `SPC b a`.
    ///
    /// It comes from OUTSIDE because whoever knows it is whoever defines the
    /// bind (`crate::leader`) and the label of the leader itself is the
    /// environment's (`crate::leader()`): in the nested loop the trigger is not
    /// `SPC`, and a line that said so would be teaching a key that does not
    /// exist there.
    pub abort_keys: &'a str,
    /// The same, for `brain-proceed`.
    pub proceed_keys: &'a str,
    /// And how a standing notice's button is typed.
    ///
    /// The same shape as the two above and for the same reason: the band
    /// DRAWS it, and a key that lived in the table and again in the drawing
    /// would be two answers to "which key does this".
    pub notice_keys: &'a str,
}

/// How many lines this region takes for what is showing.
///
/// # Why it does not depend on the width
///
/// The same reason [`crate::band::rows`] gives, and it is not weaker here: the
/// footer's height is the same on every output (see `Footer::apply_geometry`),
/// so a line that wrapped on the laptop and not on the monitor would reflow
/// every buffer when focus changed screen, with nothing having changed in them.
///
/// So it is one line or none, and what does not fit ELIDES. There is no second
/// line to spill onto, which is what makes the sentence the thing that gives
/// way rather than the layout.
#[must_use]
pub const fn rows(showing: Option<&Announcement>, saying: Option<&ronoctua_brain::Notice>) -> u16 {
    if showing.is_some() || saying.is_some() {
        1
    } else {
        0
    }
}

/// How much of the warning is left, as a fraction (1.0 → 0.0).
///
/// `now` is epoch seconds, and it comes from the caller rather than from a
/// clock in here: the footer already beats a one-second timer and this is
/// computed on that beat. A clock of its own would be a second wake-up for one
/// effect — the trap `surface.rs`'s timer records for the blink.
#[must_use]
pub fn left_of(a: &Announcement, now: i64) -> f32 {
    if a.total_seconds == 0 {
        return 0.0;
    }
    let left = (a.deadline - now).max(0);
    // The ratio is clamped and not merely divided: a deadline further away than
    // the whole warning — a clock that stepped backwards, a daemon restarted
    // mid-countdown — would give a pie fuller than full.
    (left as f32 / f32::from(a.total_seconds)).clamp(0.0, 1.0)
}

/// How many seconds are left, for the sentence.
#[must_use]
pub fn seconds_left(a: &Announcement, now: i64) -> i64 {
    (a.deadline - now).max(0)
}

/// The line, laid out for `columns` cells.
///
/// `now` is epoch seconds; everything the countdown shows is derived from it
/// and from the announcement's own `deadline`, because the brain sends the
/// warning ONCE and never a ticking number.
#[must_use]
pub fn lines(
    showing: Option<&Announcement>,
    saying: Option<&ronoctua_brain::Notice>,
    columns: u16,
    now: i64,
    words: &Words<'_>,
) -> Option<Line> {
    // A COUNTDOWN WINS THE LINE. There is one line and it cannot hold two
    // things; between "this machine is about to rebuild itself in forty
    // seconds" and "your editor's daemon is not running", the first is the one
    // with a deadline on it. The second comes back the moment the first is
    // over — the watch runs every five minutes and its notice is not cleared
    // by anything else.
    let Some(a) = showing else {
        return saying.map(|n| notice_line(n, columns, words));
    };

    // THE MARGIN IS THE MODELINE'S, and not `chrome::INSET`.
    //
    // This line is peripheral reference — one glances at it, one does not read
    // it — which is the whole of the exception `chrome`'s header names. It is
    // the fourth site on that list, and it is named there.
    let left = tokens::space::MODELINE_EDGELEFT;
    let mut line = Line::default();

    // WHOSE LINE THIS IS, in the margin. It arrives without anybody having
    // asked for it, so the first thing on it says who is speaking — the same
    // argument `band::generic_glyph` makes for a notification's mark.
    line.pieces.push(Piece {
        at: left,
        text: tokens::icon::BRAIN.to_string(),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
    });
    let at = left + 2;

    // THE RIGHT-HAND CLUSTER FIRST, because it decides how much room the
    // sentence gets. Laying the text out first and then discovering the
    // controls do not fit is how a line comes to spill — `band.rs` records it,
    // and the answer is the same here.
    //
    // From the edge inwards: proceed, abort, the pie.
    let mut right = columns.saturating_sub(tokens::space::MODELINE_EDGERIGHT);

    right = control(
        &mut line,
        right,
        words.proceed_keys,
        &words.texts.t(ronoctua_i18n::key::BRAIN_BAND_PROCEED),
        Hit::Proceed,
        words.texts.t(ronoctua_i18n::key::CMD_BRAIN_PROCEED),
    );
    right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP);
    right = control(
        &mut line,
        right,
        words.abort_keys,
        &words.texts.t(ronoctua_i18n::key::BRAIN_BAND_ABORT),
        Hit::Abort,
        words.texts.t(ronoctua_i18n::key::CMD_BRAIN_ABORT),
    );

    // THE PIE, and it is `band::countdown_glyph` and not a second one.
    //
    // One function serves both places precisely so a mark cannot differ between
    // them: the notification band and this line are two countdowns on the same
    // screen, and a person who learned one has learned the other.
    right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP + 1);
    line.pieces.push(Piece {
        at: right,
        text: crate::band::countdown_glyph(ronoctua_notify::eighths(left_of(a, now))).to_string(),
        color: tokens::color::BRAIN_COUNTDOWN,
        hit: None,
        explain: Some(words.texts.t(ronoctua_i18n::key::HINT_BRAIN_COUNTDOWN)),
    });

    // AND THE SENTENCE, in whatever is left.
    //
    // The subject is the TASK'S OWN NAME, looked up as `brain-task-<id>`: a
    // task added tomorrow inherits this line instead of needing a sentence of
    // its own, which is the door `ronoctua-brain`'s header promises.
    let sentence = words.texts.t_args(
        ronoctua_i18n::key::BRAIN_BAND_SOON,
        &[
            (
                "task",
                ronoctua_i18n::Arg::Text(&words.texts.t(&format!("brain-task-{}", a.task))),
            ),
            ("seconds", ronoctua_i18n::Arg::Number(seconds_left(a, now))),
        ],
    );
    let room = right.saturating_sub(at).saturating_sub(1);
    line.pieces.push(Piece {
        at,
        text: layout::elide_end(&sentence, room),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
    });

    Some(line)
}

/// The same line, for something the brain wants said rather than a countdown.
///
/// # Why it is a second builder and not a branch in the first
///
/// Because almost nothing is shared. A countdown has a pie, two answers and a
/// deadline; a notice has a sentence and at most one button, and the button is
/// whatever the task named rather than one of a fixed pair. Folding them
/// together would be a function whose every line is an `if`.
///
/// What IS shared is the shape — the glyph in the margin, the cluster on the
/// right laid out before the sentence, the sentence eliding into what is left
/// — and that is shared by calling the same `control` and the same margins.
fn notice_line(n: &ronoctua_brain::Notice, columns: u16, words: &Words<'_>) -> Line {
    let left = tokens::space::MODELINE_EDGELEFT;
    let mut line = Line::default();
    line.pieces.push(Piece {
        at: left,
        text: tokens::icon::BRAIN.to_string(),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
    });
    let at = left + 2;
    let mut right = columns.saturating_sub(tokens::space::MODELINE_EDGERIGHT);
    // THE BUTTON, WHEN THERE IS ONE — and there is not always one. A notice
    // about a second copy of Emacs offers nothing, because what is open is
    // somebody's work and nothing here knows whether it is saved.
    if let Some(act) = n.act.as_deref() {
        let label = format!("cmd-{act}");
        right = control(
            &mut line,
            right,
            words.notice_keys,
            &words.texts.t(&label),
            Hit::Notice,
            words.texts.t(&label),
        );
        right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP);
    }
    let room = right.saturating_sub(at).saturating_sub(1);
    line.pieces.push(Piece {
        at,
        text: layout::elide_end(&words.texts.t(&n.says), room),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
    });
    line
}

/// One control — the bracketed key and the label — placed with its right edge
/// at `right`. Gives back the cell its left edge landed on.
///
/// TWO pieces and not one, because the two are coloured differently: the key is
/// `brain.dim` and the label is `brain.text`, which is what that token says it
/// is for. Both carry the same `hit` and the same explanation, so the pointer
/// answers the same thing over either half.
fn control(line: &mut Line, right: u16, keys: &str, label: &str, hit: Hit, explain: String) -> u16 {
    let bracketed = format!("[{keys}]");
    let width = layout::width(&bracketed) + 1 + layout::width(label);
    let start = right.saturating_sub(width);
    line.pieces.push(Piece {
        at: start,
        text: bracketed.clone(),
        color: tokens::color::BRAIN_DIM,
        hit: Some(hit),
        explain: Some(explain.clone()),
    });
    line.pieces.push(Piece {
        at: start + layout::width(&bracketed) + 1,
        text: label.to_owned(),
        color: tokens::color::BRAIN_TEXT,
        hit: Some(hit),
        explain: Some(explain),
    });
    start
}

/// Which piece of the line is at cell `column`.
///
/// From what was DRAWN, and not from a second calculation of where things would
/// be — the rule the modeline's segments and the notification band already
/// follow, for the reason both write down: a `Motion` arrives per pixel of
/// movement, and two calculations of one position are two chances to point at
/// the wrong thing.
#[must_use]
pub fn at(line: Option<&Line>, column: u16) -> Option<&Piece> {
    line?
        .pieces
        .iter()
        .find(|p| column >= p.at && column < p.at + layout::width(&p.text))
}

// ---------------------------------------------------------------------------
// The host half: the thread that listens, and what the two controls send
// ---------------------------------------------------------------------------

/// The `systemctl`, by path and never by name.
///
/// A unit's `PATH` is minimal (`AGENTS.md`), and a bare name fails far from its
/// cause. Outside Nix — `cargo run` — it falls back to the name and the shell's
/// own `PATH` resolves it, which is the shape `crate::keyboard` already uses.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// The unit `nix/modules/brain.nix` defines.
const UNIT: &str = "ronoctua-brain.service";

/// Brings up the thread that watches the brain and pushes every mind into
/// `calloop`.
///
/// It gives back `false` when there is nowhere to put a socket — and **that is
/// not fatal**, for the reason `transfers::listen` writes down: without it the
/// footer loses one line and goes on drawing everything else, and taking the
/// shell down over a missing `XDG_RUNTIME_DIR` would trade one feature for all
/// of them.
///
/// # Why a thread and not the socket in `calloop`
///
/// The two reasons the queue's thread already gives, and one of its own. A JSON
/// line can arrive split across two reads, which in the main loop is a state
/// machine and on a thread with a blocking `read_line` is one line of code. The
/// client RECONNECTS, which is a sleep and a retry, and a sleep in the loop that
/// draws is a frozen footer. And `client::watch` is explicitly the SLOW half of
/// that crate's two connects — `connect_quick` is what the drawing loop uses,
/// and it is what [`tell`] below calls.
pub fn listen(channel: smithay_client_toolkit::reexports::calloop::channel::Sender<Mind>) -> bool {
    if let Err(e) = ronoctua_brain::wire::socket_path() {
        tracing::warn!(error = %e, "no brain; the band does not appear");
        return false;
    }
    let built = std::thread::Builder::new()
        .name("brain".into())
        .spawn(move || {
            // THE UNIT FIRST, and on THIS thread — which is the whole reason
            // the start is here and not beside the call. `systemctl start`
            // returns when systemd has taken the job, not when the daemon
            // answers; the connect that follows does have to wait, and waiting
            // is what this thread is for.
            //
            // A failure is not worth a word above `debug`: in the nested loop
            // there is no unit at all, and `client::watch` starts the daemon
            // beside this binary instead. On the installed system it is already
            // running before the shell is.
            wake_the_unit();
            // The closure says "stop" only when the other end has gone, which
            // means the footer is already on its way out.
            let _ = ronoctua_brain::client::watch(|mind| channel.send(mind).is_err());
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the brain's thread");
            false
        }
    }
}

/// Asks systemd for the user unit. Failure is expected off the installed system.
fn wake_the_unit() {
    match std::process::Command::new(SYSTEMCTL)
        .args(["--user", "start", UNIT])
        .output()
    {
        Ok(out) if out.status.success() => tracing::info!(unit = UNIT, "brain asked of systemd"),
        Ok(out) => tracing::debug!(
            unit = UNIT,
            why = %String::from_utf8_lossy(&out.stderr).trim(),
            "systemd did not bring the brain up; carrying on through the socket"
        ),
        Err(e) => tracing::debug!(error = %e, "no systemctl; carrying on through the socket"),
    }
}

/// Sends one ask, and says in the log when there is nobody to send it to.
///
/// `tell` and not `ask`: none of these answer with anything the caller needs,
/// and waiting for a reply inside the loop that draws is the frozen footer this
/// project has paid for twice. What comes back is the whole mind, on the watch
/// stream, which is where the screen reads it anyway.
///
/// # Errors
///
/// Nobody listening.
pub fn tell(what: &ronoctua_brain::Ask) -> Result<(), anyhow::Error> {
    ronoctua_brain::client::tell(what)
}

/// What the band's `not now` sends.
///
/// # Why `postpone` and not `abort`, and why the two are one command
///
/// While the countdown is running, nothing is running yet: the task is QUEUED,
/// and `Ask::Abort` on a queued task only takes it out of the queue. The runner
/// enrols whatever is due on the next beat, so it would be back — with a fresh
/// countdown — half a minute later. Its own comment says exactly that about the
/// ordering of `put_off` against `enrol`.
///
/// `Ask::Postpone` is what moves the next moment: the runner writes it down as
/// an event, and the row comes back at its own hour instead of immediately.
///
/// So one command, `brain-abort`, means *stop it* in both situations and picks
/// the ask that actually stops it: postpone while it is only announced, abort
/// once something is running. Two commands for that would make the person
/// choose between two words for one intention, and get it wrong at the only
/// moment it matters.
#[must_use]
pub fn abort(mind: &Mind) -> ronoctua_brain::Ask {
    if mind.running.is_empty() {
        ronoctua_brain::Ask::Postpone
    } else {
        ronoctua_brain::Ask::Abort {
            task: mind.running.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// The daemon's mind, as the component draws it
// ---------------------------------------------------------------------------

/// The conversion to the WIT's records lives in [`crate::mind_wire`].
///
/// It moved out so `ronoctua-render-png` could link it without linking this
/// file, which reaches for smithay, `crate::band` and `crate::leader`. Re-
/// exported here because every caller in the footer already says
/// `crate::brain::mind_of`, and a move is not a reason to touch them.
pub(crate) use crate::mind_wire::{mind_of, part_back};

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn announced(total: u16, deadline: i64) -> Announcement {
        Announcement {
            task: "system-update".to_owned(),
            total_seconds: total,
            deadline,
            why: "due".to_owned(),
        }
    }

    // FROM THE CONSTANTS THE KEYMAP BINDS, so that the day the keys move the
    // fixture moves with them: written as literals, this test went on proving
    // that `SPC s a` was drawn for a fortnight after `s a` stopped existing.
    static ABORT: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_ABORT_KEYS.to_owned());
    static PROCEED: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_PROCEED_KEYS.to_owned());
    static NOTICE: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_NOTICE_KEYS.to_owned());

    fn bundle(t: &ronoctua_i18n::Texts) -> Words<'_> {
        Words {
            texts: t,
            abort_keys: &ABORT,
            proceed_keys: &PROCEED,
            notice_keys: &NOTICE,
        }
    }

    fn a_notice(act: Option<&str>) -> ronoctua_brain::Notice {
        ronoctua_brain::Notice {
            task: "emacs-daemon".to_owned(),
            says: "emacs-no-daemon".to_owned(),
            act: act.map(ToOwned::to_owned),
        }
    }

    /// A STANDING NOTICE GETS THE LINE WHEN NOTHING IS COUNTING DOWN.
    ///
    /// And loses it when something is: there is one line, and between "this
    /// machine rebuilds itself in forty seconds" and "your editor's daemon is
    /// not running", the one with a deadline on it wins. The second is not
    /// lost — the watch runs every five minutes and nothing else clears its
    /// notice, so it is back as soon as the countdown is over.
    #[test]
    fn a_notice_takes_the_line_only_when_nothing_is_counting_down() {
        let t = words();
        let w = bundle(&t);
        let n = a_notice(Some("brain-start-emacs"));
        assert_eq!(rows(None, Some(&n)), 1);
        assert!(lines(None, Some(&n), 100, 0, &w).is_some());

        let a = announced(60, 1_000);
        assert_eq!(rows(Some(&a), Some(&n)), 1, "two things, still one line");
        let both = lines(Some(&a), Some(&n), 100, 940, &w).expect("a line");
        let said: String = both.pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(
            !said.contains(&t.t("emacs-no-daemon")),
            "the notice took the countdown's line: {said}"
        );

        assert_eq!(rows(None, None), 0);
        assert!(lines(None, None, 100, 0, &w).is_none());
    }

    /// A NOTICE WITH NOTHING TO OFFER DRAWS NO BUTTON.
    ///
    /// The one about a second copy of Emacs has none on purpose: what is open
    /// is somebody's work, and nothing here knows whether it is saved. A
    /// button drawn anyway would be a button that closes it.
    #[test]
    fn a_notice_without_an_offer_has_no_control() {
        let t = words();
        let w = bundle(&t);
        let with =
            lines(None, Some(&a_notice(Some("brain-start-emacs"))), 100, 0, &w).expect("a line");
        assert!(
            with.pieces.iter().any(|p| p.hit == Some(Hit::Notice)),
            "a notice that offers something drew no control"
        );
        let without = lines(None, Some(&a_notice(None)), 100, 0, &w).expect("a line");
        assert!(
            without.pieces.iter().all(|p| p.hit.is_none()),
            "a notice with nothing to offer drew a control anyway"
        );
    }

    #[test]
    fn it_is_one_line_at_every_width_or_none_at_all() {
        // THE HEIGHT DOES NOT DEPEND ON THE WIDTH, and it may not: the footer
        // is the same height on every output, and a line that wrapped on the
        // laptop and not on the monitor would reflow every buffer when the
        // focus changed screen.
        assert_eq!(rows(None, None), 0);
        let a = announced(60, 1_000);
        assert_eq!(rows(Some(&a), None), 1);
        let t = words();
        let w = bundle(&t);
        for columns in [20_u16, 40, 80, 200] {
            assert!(
                lines(Some(&a), None, columns, 940, &w).is_some(),
                "it went away at {columns} columns"
            );
        }
        assert!(lines(None, None, 80, 940, &w).is_none());
    }

    #[test]
    fn nothing_spills_off_the_line() {
        // The same property the modeline and the band are tested for, and for
        // the same reason: a compositor does not clip, so what spills is
        // rubbish at the screen's edge.
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        for columns in [20_u16, 30, 40, 80, 120, 200] {
            let line = lines(Some(&a), None, columns, 940, &w).expect("the line went away");
            for p in &line.pieces {
                assert!(
                    p.at + layout::width(&p.text) <= columns,
                    "'{}' leaks at {columns} columns",
                    p.text
                );
            }
        }
    }

    #[test]
    fn the_two_controls_are_reachable_and_say_what_they_do() {
        // THE MOUSE REACHES NOTHING THE KEYBOARD DOES NOT. Both targets are
        // registered commands, both are drawn with the sequence that runs them,
        // and both explain themselves on the message line — never in a tooltip
        // (invariant 14).
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        let line = lines(Some(&a), None, 120, 940, &w).expect("the line went away");
        for hit in [Hit::Abort, Hit::Proceed] {
            let mine: Vec<&Piece> = line.pieces.iter().filter(|p| p.hit == Some(hit)).collect();
            assert_eq!(mine.len(), 2, "{hit:?} does not have the key and the label");
            assert!(
                mine.iter().all(|p| p.explain.is_some()),
                "{hit:?} does not say what it does"
            );
        }
        // AND THE SEQUENCES ARE ON THE LINE, in brackets: a control whose key
        // is only in the manual is a control nobody presses.
        let drawn: String = line.pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(drawn.contains(&format!("[{}]", *ABORT)), "{drawn}");
        assert!(drawn.contains(&format!("[{}]", *PROCEED)), "{drawn}");
        // The margin carries the mark, and it is not a control.
        let first = line.pieces.first().unwrap();
        assert_eq!(first.at, tokens::space::MODELINE_EDGELEFT);
        assert_eq!(first.hit, None);
        assert_eq!(first.text, tokens::icon::BRAIN.to_string());
    }

    #[test]
    fn the_pointer_finds_what_was_drawn() {
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        let line = lines(Some(&a), None, 120, 940, &w).expect("the line went away");
        let proceed = line
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Proceed))
            .expect("no proceed control");
        assert_eq!(
            at(Some(&line), proceed.at).and_then(|p| p.hit),
            Some(Hit::Proceed)
        );
        // A cell with nothing in it is nothing, and not the nearest thing.
        let gap = tokens::space::MODELINE_EDGELEFT + 1;
        assert!(at(Some(&line), gap).is_none());
        assert!(at(None, 0).is_none());
    }

    #[test]
    fn the_pie_is_the_bands_and_empties_as_the_time_goes() {
        // ONE FUNCTION FOR TWO PLACES. A mark that differed between the
        // notification's countdown and this one would make the person look
        // twice to decide they are the same thing.
        let a = announced(80, 1_000);
        for (now, fraction) in [(920_i64, 1.0_f32), (960, 0.5), (1_000, 0.0)] {
            assert!((left_of(&a, now) - fraction).abs() < 0.001);
            let mine = crate::band::countdown_glyph(ronoctua_notify::eighths(left_of(&a, now)));
            let theirs = crate::band::countdown_glyph(ronoctua_notify::eighths(fraction));
            assert_eq!(mine, theirs);
        }
        // A deadline already past does not go negative, and a clock that
        // stepped backwards does not make the pie fuller than full.
        assert_eq!(seconds_left(&a, 2_000), 0);
        assert!((left_of(&a, 0) - 1.0).abs() < 0.001);
        // A warning with no length at all is not a division by zero.
        assert!((left_of(&announced(0, 10), 0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn the_sentence_says_the_task_and_the_seconds() {
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        let line = lines(Some(&a), None, 200, 958, &w).expect("the line went away");
        // BY ITS COLUMN, and not by "the longest piece": the mark in the margin
        // is one character and four bytes, and a test that went looking for a
        // long `String` found the glyph.
        let sentence = at(Some(&line), tokens::space::MODELINE_EDGELEFT + 2).expect("no phrase");
        assert!(sentence.text.contains("42"), "{}", sentence.text);
        assert!(
            !sentence.text.starts_with('['),
            "the sentence became a raw key: {}",
            sentence.text
        );
    }

    #[test]
    fn not_now_postpones_while_nothing_is_running() {
        // THE DEFECT THIS CATCHES: `Ask::Abort` on a task that is only QUEUED
        // takes it out of the queue, and the runner enrols it again on the next
        // beat — the countdown comes back half a minute later and the person
        // answers the same question for ever. What moves the next moment is the
        // postponement.
        let mut mind = Mind::default();
        assert_eq!(abort(&mind), ronoctua_brain::Ask::Postpone);
        mind.running = "system-update".to_owned();
        assert_eq!(
            abort(&mind),
            ronoctua_brain::Ask::Abort {
                task: "system-update".to_owned()
            }
        );
    }

    #[test]
    fn what_was_never_measured_does_not_report_a_zero() {
        // The one thing the conversion must not do: a kernel without
        // `/proc/pressure/cpu` has no reading, and a zero there is a number
        // somebody would act on.
        let mind = Mind {
            pulse: ronoctua_brain::Pulse {
                at: 10,
                ram_used_pct: Some(0),
                memory_stall: None,
                cpu_stall: None,
                on_battery: Some(false),
            },
            ..Mind::default()
        };
        let over = mind_of(&mind, &ronoctua_files::when::Zone::utc(), &words());
        let of: Vec<ronoctua_protocol::Vital> = over.pulse.signs.iter().map(|s| s.of).collect();
        assert_eq!(
            of,
            vec![
                ronoctua_protocol::Vital::RamUsed,
                ronoctua_protocol::Vital::OnBattery
            ]
        );
        assert_eq!(
            over.pulse.signs[0].amount, 0,
            "a measured zero became an absence"
        );
    }

    #[test]
    fn a_task_that_never_ran_says_so_instead_of_dating_1970() {
        let mind = Mind {
            tasks: vec![ronoctua_brain::TaskState {
                kind: ronoctua_brain::Kind::Routine,
                id: "system-update".to_owned(),
                standing: ronoctua_brain::Standing::Idle,
                enabled: true,
                interval_days: 7,
                part_of_day: ronoctua_brain::PartOfDay::Afternoon,
                next: 0,
                hold: None,
                why_not: "off".to_owned(),
                history: Vec::new(),
                last: None,
                steps: Vec::new(),
            }],
            ..Mind::default()
        };
        let over = mind_of(&mind, &ronoctua_files::when::Zone::utc(), &words());
        let row = &over.tasks[0];
        assert!(!row.ran, "a run that never happened became a run");
        assert!(row.next_text.is_empty(), "no next hour, and it dated one");
        assert_eq!(row.hold, ronoctua_protocol::HoldKind::None);
        // And a moment that EXISTS is formatted, in ISO, because the component
        // has no calendar (invariant 11).
        let mut with = mind;
        with.tasks[0].next = 1_700_000_000;
        let over = mind_of(&with, &ronoctua_files::when::Zone::utc(), &words());
        assert_eq!(over.tasks[0].next_text, "2023-11-14 22:13");
    }
}
