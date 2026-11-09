//! The two halves' output: one journal entry as the view draws it
//! ([`OutputLine`]), the lines kept ([`Scrollback`]) and what journalctl is
//! asked for them ([`journal_arguments`]).
//!
//! # What this file may not know
//!
//! A step, a unit, the update's module. The units are handed in by the window
//! (`update_window::HALVES`, the manifest's), so the module's seal never has
//! to list this file — it is the output of ANY two journals.
//!
//! # Where a line's colour comes from
//!
//! Two places, and neither is a colour of this file's. What a program
//! coloured stays its colour ([`ronoctua_shell_kit::ansi`]), as one of the
//! terminal's sixteen roles. Everything else takes the line's BASE, from the
//! priority journald gave it ([`base_of`]): the runner's failure lines carry
//! `<3>` and the machine's program its `<3>`/`<4>`, so an error is red even
//! where no program coloured it — and under systemd nix colours nothing
//! (`update_window::follow` says why).

use std::collections::VecDeque;

use ronoctua_render::layout;
use ronoctua_shell_kit::ansi;
use ronoctua_tokens as tokens;

/// How many lines of output the view keeps.
///
/// # Why two thousand, and why a ceiling at all
///
/// A `nixos-rebuild switch` of this workspace prints on the order of a few
/// thousand lines, and a window that kept all of them would be a memory leak
/// with a user interface — twenty minutes of rebuild, growing. Two thousand
/// lines at the length machine output actually has is a few hundred kibibytes,
/// which is the size of ONE of the file manager's preview thumbnails, and it is
/// some fifty screenfuls to page back through.
///
/// A ring and not a truncation, because what matters after a failed rebuild is
/// the TAIL: nix prints the last lines of the failing log, and those are the
/// last thing written.
pub const KEPT: usize = 2000;

/// The longest line kept, in grapheme clusters. A line is drawn on ONE row
/// and cut to the window's width, so past a few hundred nothing more of it is
/// ever seen — and a journal line may be 256 KiB (`ronoctua_journal`'s
/// ceiling), two thousand of which would be half a gigabyte.
pub const LINE_MOST_CLUSTERS: usize = 2048;

/// The most bytes a line keeps, beside [`LINE_MOST_CLUSTERS`]: one cluster
/// may hold any number of combining marks, so a few clusters can still be a
/// journal line's whole 48 KiB — and a row of them is shaped every frame.
/// Four bytes a cluster is what text of any script needs.
pub const LINE_MOST_BYTES: usize = 4 * LINE_MOST_CLUSTERS;

/// The most pieces a line keeps: the 32nd holds the rest of the line, in the
/// line's own colour. A program that changes colour on every character is
/// read, never kept as a piece per character.
pub const LINE_MOST_PIECES: usize = 32;

/// The fields journalctl is asked for, and only these: the text, its
/// priority, and the run it belongs to — `_SYSTEMD_INVOCATION_ID` for what
/// the unit itself wrote, `INVOCATION_ID` (the system's manager) and
/// `USER_INVOCATION_ID` (the person's) for what the manager said ABOUT it,
/// its `Starting…` and `Failed…`. The cursor and the time come with every
/// entry whatever is asked.
pub const FIELDS: &str = "MESSAGE,PRIORITY,_SYSTEMD_INVOCATION_ID,INVOCATION_ID,USER_INVOCATION_ID";

/// The arguments of one half's follower: `--user` for the person's half, JSON
/// with [`FIELDS`], the last [`KEPT`] entries, then every new one, of these
/// units — `-u` given twice matches either, in the order they were written.
/// Pure.
#[must_use]
pub fn journal_arguments(units: &[&str], of_the_person: bool) -> Vec<String> {
    let mut args = Vec::new();
    if of_the_person {
        args.push("--user".to_owned());
    }
    args.extend([
        "--no-pager".to_owned(),
        "--output=json".to_owned(),
        format!("--output-fields={FIELDS}"),
        // A LONG MESSAGE IS A MESSAGE, not `[N bytes blob data]`: without
        // `--all` journalctl hides a message it judges unprintable, and an
        // escape is what makes it judge so.
        "--all".to_owned(),
        format!("--lines={KEPT}"),
        "--follow".to_owned(),
    ]);
    for unit in units {
        args.push(format!("--unit={unit}"));
    }
    args
}

/// One line of either journal, as the view draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    /// When journald wrote it, in microseconds since the epoch — what puts the
    /// two halves' lines in one order ([`Scrollback::push`]).
    pub at_us: u64,
    /// Whether it is the root half's (the system journal's).
    pub root: bool,
    /// The run it belongs to: the unit's invocation (empty when the journal
    /// named none) — what tells the last run from the ones before
    /// ([`Scrollback::last_run`]).
    pub invocation: String,
    /// The colour of what no program coloured, from its priority
    /// ([`base_of`]).
    pub base: tokens::Color,
    /// The text, in runs of one colour — bounded ([`LINE_MOST_CLUSTERS`],
    /// [`LINE_MOST_BYTES`], [`LINE_MOST_PIECES`]).
    pub pieces: Vec<ansi::Piece>,
}

impl OutputLine {
    /// The line of one entry of `journalctl --output=json` — its `MESSAGE`
    /// already read as text when journald kept it as bytes
    /// ([`ronoctua_journal::Entry`]), which is how an escape arrives.
    #[must_use]
    pub fn of(entry: &ronoctua_journal::Entry, root: bool) -> Self {
        let invocation = [
            "_SYSTEMD_INVOCATION_ID",
            "USER_INVOCATION_ID",
            "INVOCATION_ID",
        ]
        .iter()
        .find_map(|field| entry.fields.get(*field))
        .cloned()
        .unwrap_or_default();
        Self::new(
            entry.realtime_us,
            root,
            invocation,
            entry.priority,
            &entry.message,
        )
    }

    /// A line as a unit WRITES it, read the way journald reads a unit's
    /// output: a leading `<N>` (0–7, the form `sd-daemon(3)` documents) is its
    /// priority and is not text, and a line with none is `info`. What a
    /// photograph's scene hands in.
    #[must_use]
    pub fn written(at_us: u64, root: bool, invocation: &str, raw: &str) -> Self {
        let (priority, message) = match raw.as_bytes() {
            [b'<', p @ b'0'..=b'7', b'>', ..] => (p - b'0', &raw[3..]),
            _ => (ronoctua_journal::INFO, raw),
        };
        Self::new(at_us, root, invocation.to_owned(), Some(priority), message)
    }

    fn new(
        at_us: u64,
        root: bool,
        invocation: String,
        priority: Option<ronoctua_journal::Priority>,
        message: &str,
    ) -> Self {
        Self {
            at_us,
            root,
            invocation,
            base: base_of(priority),
            pieces: bounded(ansi::pieces(message)),
        }
    }

    /// The text, with no colour.
    #[must_use]
    pub fn text(&self) -> String {
        self.pieces.iter().map(|p| p.text.as_str()).collect()
    }

    /// Each piece with the colour it is drawn in: its own, or the line's.
    pub fn inked(&self) -> impl Iterator<Item = (&str, tokens::Color)> {
        self.pieces
            .iter()
            .map(|p| (p.text.as_str(), p.color.unwrap_or(self.base)))
    }
}

/// The colour of a line's uncoloured text, by the priority journald gave it:
/// an error (3) or worse in the error's colour, a warning (4) in the
/// warning's, anything else — no priority at all included — as output.
#[must_use]
pub const fn base_of(priority: Option<ronoctua_journal::Priority>) -> tokens::Color {
    match priority {
        Some(p) if p <= ronoctua_journal::ERR => tokens::color::STATE_ERROR,
        Some(ronoctua_journal::WARNING) => tokens::color::STATE_WARNING,
        _ => tokens::color::TEXT_SECONDARY,
    }
}

/// A line's colour as a TERMINAL draws it — on the card a window lays the
/// output on (`terminal.background`), whose inks are measured against that
/// card: the line's own colour is the terminal's foreground, an error's and a
/// warning's are its red and its yellow, and a program's own colours — the
/// terminal's sixteen already ([`ansi`]) — stay as they are.
#[must_use]
pub fn on_a_card(color: tokens::Color) -> tokens::Color {
    if color == tokens::color::STATE_ERROR {
        tokens::color::TERMINAL_ANSI_RED
    } else if color == tokens::color::STATE_WARNING {
        tokens::color::TERMINAL_ANSI_YELLOW
    } else if color == tokens::color::TEXT_SECONDARY {
        tokens::color::TERMINAL_FOREGROUND
    } else {
        color
    }
}

/// A line's colour where NO CARD is under it — the footer's ground, whose
/// inks are the chrome's: the sixteen are measured against
/// `terminal.background` alone, so a program's red is the error's, its yellow
/// the warning's, its green the success's, and any other of them the line's
/// quiet ink. A colour that is none of the sixteen stays as it is.
#[must_use]
pub fn on_the_chrome(color: tokens::Color) -> tokens::Color {
    match ansi::SIXTEEN.iter().position(|c| *c == color) {
        Some(1 | 9) => tokens::color::STATE_ERROR,
        Some(3 | 11) => tokens::color::STATE_WARNING,
        Some(2 | 10) => tokens::color::STATE_SUCCESS,
        Some(_) => tokens::color::TEXT_SECONDARY,
        None => color,
    }
}

/// `pieces` within the ceilings: cut at [`LINE_MOST_CLUSTERS`] or
/// [`LINE_MOST_BYTES`], whichever comes first — at a cluster's edge, never
/// inside one, which would draw something else — and, past
/// [`LINE_MOST_PIECES`], the rest joined into the last piece, which then
/// takes the line's colour: one colour of a program's would be a lie about
/// the others.
fn bounded(pieces: Vec<ansi::Piece>) -> Vec<ansi::Piece> {
    let mut out: Vec<ansi::Piece> = Vec::with_capacity(pieces.len().min(LINE_MOST_PIECES));
    let mut left = LINE_MOST_CLUSTERS;
    let mut bytes = LINE_MOST_BYTES;
    for mut piece in pieces {
        let (mut clusters, mut end, mut cut) = (0, 0, false);
        for c in layout::cells_of(&piece.text) {
            if clusters == left || end + c.len() > bytes {
                cut = true;
                break;
            }
            clusters += 1;
            end += c.len();
        }
        piece.text.truncate(end);
        left -= clusters;
        bytes -= end;
        if piece.text.is_empty() {
            break;
        }
        if out.len() < LINE_MOST_PIECES {
            out.push(piece);
        } else if let Some(last) = out.last_mut() {
            last.text.push_str(&piece.text);
            last.color = None;
        }
        if cut {
            break;
        }
    }
    out
}

/// The output, bounded, in the order it was written, with one flag saying
/// whether the screen owes a frame.
///
/// # Why the flag, and why a redraw per line is a frozen window
///
/// Because a rebuild prints thousands of lines in a burst, and painting once
/// per line is the defect `docs/ARCHITECTURE.md` records under *"the loop
/// waited past the version from BEFORE its own pass"*: the brain's daemon
/// published on every pass, the loop that draws was woken 248 088 times in
/// 300 ms, and what the owner saw was a window that would not resize. Same
/// failure, different producer.
///
/// So arrival and painting are separated: every line that has arrived is drained
/// into here, and the frame is owed ONCE, after the event queue is empty — the
/// shape `Footer::owes_paint` already has.
#[derive(Debug, Clone, Default)]
pub struct Scrollback {
    said: VecDeque<OutputLine>,
    owes_a_frame: bool,
}

impl Scrollback {
    /// Takes one line WHERE IT WAS WRITTEN, by its time: the two halves are
    /// two followers, and each starts with its own backlog, so arrival is not
    /// the order things happened in. A line written at the same moment as
    /// one kept goes after it. At the ceiling the oldest goes — the new line
    /// itself, when it is older than everything kept.
    pub fn push(&mut self, line: OutputLine) {
        let at = self.said.partition_point(|l| l.at_us <= line.at_us);
        self.said.insert(at, line);
        if self.said.len() > KEPT {
            self.said.pop_front();
        }
        self.owes_a_frame = true;
    }

    /// Whether a frame is owed, and clears the debt.
    pub const fn owes_a_frame(&mut self) -> bool {
        let owed = self.owes_a_frame;
        self.owes_a_frame = false;
        owed
    }

    /// How many lines are kept.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.said.len()
    }

    /// Whether anything has arrived at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }

    /// Line `n`, counted from the oldest kept.
    #[must_use]
    pub fn line(&self, n: usize) -> Option<&OutputLine> {
        self.said.get(n)
    }

    /// `room` lines ending `up` lines above the tail.
    #[must_use]
    pub fn window(&self, up: usize, room: usize) -> Vec<&OutputLine> {
        let end = self.said.len().saturating_sub(up);
        let start = end.saturating_sub(room);
        self.said.range(start..end).collect()
    }

    /// The lines of the LAST RUN: from the first line of the newest
    /// invocation of the person's half onward — the half that starts a run,
    /// and that waits while the root half it started writes, so the root's
    /// lines of that run come after it. With no line of the person's half,
    /// the newest invocation of any; with no invocation named at all, every
    /// line kept.
    #[must_use]
    pub fn last_run(&self) -> std::collections::vec_deque::Iter<'_, OutputLine> {
        self.said.range(self.last_run_from()..)
    }

    /// Where [`Scrollback::last_run`] begins.
    fn last_run_from(&self) -> usize {
        let named = |l: &&OutputLine| !l.invocation.is_empty();
        let newest = self
            .said
            .iter()
            .rev()
            .filter(named)
            .find(|l| !l.root)
            .or_else(|| self.said.iter().rev().find(named));
        newest.map_or(0, |newest| {
            self.said
                .iter()
                .position(|l| l.root == newest.root && l.invocation == newest.invocation)
                .unwrap_or(0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(at_us: u64, root: bool, invocation: &str, text: &str) -> OutputLine {
        OutputLine::written(at_us, root, invocation, text)
    }

    fn texts<'a>(lines: impl IntoIterator<Item = &'a OutputLine>) -> Vec<String> {
        lines.into_iter().map(OutputLine::text).collect()
    }

    /// FAILS IF: an error's or a warning's priority does not colour what no
    /// program coloured, any other priority (or none) is not output's
    /// colour, or a program's own colour gives way to the priority's.
    #[test]
    fn the_priority_colours_what_no_program_did() {
        assert_eq!(base_of(Some(0)), tokens::color::STATE_ERROR);
        assert_eq!(base_of(Some(3)), tokens::color::STATE_ERROR);
        assert_eq!(base_of(Some(4)), tokens::color::STATE_WARNING);
        assert_eq!(base_of(Some(5)), tokens::color::TEXT_SECONDARY);
        assert_eq!(base_of(Some(6)), tokens::color::TEXT_SECONDARY);
        assert_eq!(base_of(None), tokens::color::TEXT_SECONDARY);
        let failed = said(1, true, "a", "<3>!! a step: failed \u{1b}[32mkept\u{1b}[0m");
        assert_eq!(
            failed.inked().collect::<Vec<_>>(),
            vec![
                ("!! a step: failed ", tokens::color::STATE_ERROR),
                ("kept", tokens::color::TERMINAL_ANSI_GREEN),
            ]
        );
        assert_eq!(
            failed.text(),
            "!! a step: failed kept",
            "the `<3>` is no text"
        );
        assert_eq!(
            said(1, true, "a", "<4>held").base,
            tokens::color::STATE_WARNING
        );
        assert_eq!(
            said(1, true, "a", "plain").base,
            tokens::color::TEXT_SECONDARY,
            "a line with no prefix is info"
        );
        assert_eq!(
            said(1, true, "a", "<9>odd").text(),
            "<9>odd",
            "not a priority"
        );
    }

    /// FAILS IF: on a terminal's card a line's own colour, an error's or a
    /// warning's is not the terminal's ink for it — a colour measured
    /// against the chrome laid on the terminal's ground — or a program's own
    /// colour is changed.
    #[test]
    fn a_line_on_the_card_wears_the_terminals_inks() {
        let line = said(1, false, "a", "<3>broke \u{1b}[32mkept\u{1b}[0m");
        let on_the_card: Vec<_> = line.inked().map(|(_, c)| on_a_card(c)).collect();
        assert_eq!(
            on_the_card,
            vec![
                tokens::color::TERMINAL_ANSI_RED,
                tokens::color::TERMINAL_ANSI_GREEN
            ]
        );
        assert_eq!(
            on_a_card(said(1, false, "a", "<4>held").base),
            tokens::color::TERMINAL_ANSI_YELLOW
        );
        assert_eq!(
            on_a_card(said(1, false, "a", "plain").base),
            tokens::color::TERMINAL_FOREGROUND
        );
    }

    /// FAILS IF: an entry whose MESSAGE journald kept as BYTES — which is how a
    /// line with an escape arrives — is not read, coloured, given its
    /// priority, its time and its run.
    #[test]
    fn a_message_of_bytes_is_coloured() {
        let message: Vec<String> = "\u{1b}[31merror:\u{1b}[0m boom"
            .bytes()
            .map(|b| b.to_string())
            .collect();
        let json = format!(
            r#"{{"__CURSOR":"s=a;i=1","__REALTIME_TIMESTAMP":"1790000000000042","PRIORITY":"6","_SYSTEMD_INVOCATION_ID":"run-1","MESSAGE":[{}]}}"#,
            message.join(",")
        );
        let entry = ronoctua_journal::read_entry(&mut std::io::Cursor::new(format!("{json}\n")))
            .expect("read")
            .expect("an entry");
        let line = OutputLine::of(&entry, false);
        assert_eq!(line.at_us, 1_790_000_000_000_042);
        assert_eq!(line.invocation, "run-1");
        assert!(!line.root);
        assert_eq!(
            line.inked().collect::<Vec<_>>(),
            vec![
                ("error:", tokens::color::TERMINAL_ANSI_RED),
                (" boom", tokens::color::TEXT_SECONDARY),
            ]
        );
        // WHAT THE MANAGER SAID ABOUT A RUN belongs to it.
        let started = ronoctua_journal::Entry::parse(
            r#"{"__CURSOR":"s=a;i=2","__REALTIME_TIMESTAMP":"1","PRIORITY":"6","USER_INVOCATION_ID":"run-2","MESSAGE":"Starting the update..."}"#,
        )
        .expect("an entry");
        assert_eq!(OutputLine::of(&started, false).invocation, "run-2");
    }

    /// FAILS IF: the arguments lose the person's journal, the JSON, the fields
    /// a line's run and priority are read from, the backlog, the follow or a
    /// unit.
    #[test]
    fn a_half_is_followed_as_json_with_its_fields() {
        let args = journal_arguments(&["one.service", "two.service"], true);
        for wanted in [
            "--user",
            "--output=json",
            "--all",
            "--follow",
            "--unit=one.service",
            "--unit=two.service",
        ] {
            assert!(args.iter().any(|a| a == wanted), "{wanted} in {args:?}");
        }
        assert!(args.contains(&format!("--lines={KEPT}")), "{args:?}");
        let fields = args
            .iter()
            .find_map(|a| a.strip_prefix("--output-fields="))
            .expect("the fields are asked for");
        for field in [
            "MESSAGE",
            "PRIORITY",
            "_SYSTEMD_INVOCATION_ID",
            "USER_INVOCATION_ID",
        ] {
            assert!(fields.split(',').any(|f| f == field), "{field} in {fields}");
        }
        assert!(
            !journal_arguments(&["one.service"], false).contains(&"--user".to_owned()),
            "root's half read from the person's journal"
        );
    }

    /// FAILS IF: two halves' lines are kept in the order they arrived rather
    /// than the order they were written, or two lines of one moment swap.
    #[test]
    fn the_halves_interleave_by_time() {
        let mut back = Scrollback::default();
        back.push(said(10, false, "p", "person 10"));
        back.push(said(30, false, "p", "person 30"));
        back.push(said(20, true, "r", "root 20"));
        back.push(said(5, true, "r", "root 5"));
        back.push(said(20, false, "p", "person 20, after"));
        assert_eq!(
            texts(back.window(0, 10)),
            [
                "root 5",
                "person 10",
                "root 20",
                "person 20, after",
                "person 30"
            ]
        );
    }

    /// FAILS IF: a line past the ceiling is kept whole, is cut inside a
    /// cluster, or a line of many colours keeps a piece per colour.
    #[test]
    fn a_line_is_bounded() {
        let long = "é".repeat(LINE_MOST_CLUSTERS + 100);
        let line = said(1, false, "", &long);
        assert_eq!(line.text().chars().count(), LINE_MOST_CLUSTERS);
        // A CLUSTER OF TWO CODEPOINTS, cut at the ceiling: whole or not at all.
        let mut clusters = "a".repeat(LINE_MOST_CLUSTERS - 1);
        clusters.push_str("e\u{301}tail");
        let line = said(1, false, "", &clusters);
        assert!(line.text().ends_with("e\u{301}"), "cut inside a cluster");
        assert_eq!(layout::cells_of(&line.text()).count(), LINE_MOST_CLUSTERS);
        // MANY COLOURS: the last piece holds the rest, in the line's colour.
        let rainbow: String = (0..100)
            .map(|n| format!("\u{1b}[3{}m{}", 1 + n % 6, n % 10))
            .collect();
        let line = said(1, false, "", &rainbow);
        assert_eq!(line.pieces.len(), LINE_MOST_PIECES);
        assert_eq!(line.pieces.last().and_then(|p| p.color), None);
        let digits: String = (0..100).map(|n| char::from(b'0' + n % 10)).collect();
        assert_eq!(line.text(), digits, "text was lost to the piece ceiling");
        // A FEW CLUSTERS OF THOUSANDS OF MARKS each are kept by bytes, never
        // whole: a cluster past the ceiling goes, never cut inside.
        let heavy = format!("a{}", "\u{301}".repeat(3000)).repeat(8);
        let line = said(1, false, "", &heavy);
        assert!(
            line.text().len() <= LINE_MOST_BYTES,
            "{}",
            line.text().len()
        );
        assert_eq!(layout::cells_of(&line.text()).count(), 1);
    }

    #[test]
    fn a_thousand_lines_owe_exactly_one_frame() {
        // THE DEFECT THIS CATCHES, and it has a name in `ARCHITECTURE.md`: a
        // producer that publishes faster than the loop draws. A rebuild prints
        // thousands of lines in a burst; painting per line is the frozen window
        // the brain's daemon already caused once.
        let mut back = Scrollback::default();
        for n in 0..1000 {
            back.push(said(n, false, "", &format!("line {n}")));
        }
        assert!(
            back.owes_a_frame(),
            "a thousand rows asked for no frame at all"
        );
        assert!(
            !back.owes_a_frame(),
            "the frame was charged twice for the same burst"
        );
    }

    #[test]
    fn the_scrollback_is_bounded_and_keeps_the_tail() {
        let mut back = Scrollback::default();
        let all = u64::try_from(KEPT * 3).expect("a count");
        for n in 0..all {
            back.push(said(n, false, "", &n.to_string()));
        }
        assert_eq!(back.amount(), KEPT, "the window grew with no ceiling");
        // AND WHAT SURVIVED IS THE END, which is where the error is: nix prints
        // the last lines of the failing log, and those are written last.
        assert_eq!(
            texts(back.window(0, 2)),
            [(all - 2).to_string(), (all - 1).to_string()]
        );
        // A LINE OLDER THAN EVERYTHING KEPT is the one that goes.
        back.push(said(0, true, "", "too old"));
        assert_eq!(back.amount(), KEPT);
        assert!(!texts(back.window(0, KEPT)).contains(&"too old".to_owned()));
    }

    #[test]
    fn the_output_window_never_asks_for_what_is_not_there() {
        let mut back = Scrollback::default();
        for n in 0..5 {
            back.push(said(n, false, "", &n.to_string()));
        }
        assert_eq!(
            back.window(0, 100).len(),
            5,
            "it asked for more than exists"
        );
        assert_eq!(back.window(100, 2).len(), 0, "it went above the start");
        assert_eq!(texts(back.window(2, 2)), ["1", "2"]);
    }

    /// FAILS IF: the last run does not begin at the first line of the newest
    /// run of the person's half, leaves out the root half's lines written
    /// during it, takes in a run before it, or — with no run named — leaves
    /// lines out.
    #[test]
    fn the_last_run_is_the_newest_run_of_the_person() {
        let mut back = Scrollback::default();
        assert_eq!(back.last_run().len(), 0);
        for (at, root, run, text) in [
            (1, false, "p1", "an earlier run"),
            (2, true, "r1", "its root half"),
            (3, false, "p1", "it ended"),
            (4, true, "r0", "a root half alone"),
            (5, false, "p2", "this run"),
            (6, true, "r2", "this root half"),
            (7, false, "", "a line of no run"),
            (8, false, "p2", "this run ended"),
        ] {
            back.push(said(at, root, run, text));
        }
        assert_eq!(
            texts(back.last_run()),
            [
                "this run",
                "this root half",
                "a line of no run",
                "this run ended"
            ]
        );
        // ONLY THE ROOT HALF has run: its newest run.
        let mut root = Scrollback::default();
        root.push(said(1, true, "r1", "first"));
        root.push(said(2, true, "r2", "second"));
        assert_eq!(texts(root.last_run()), ["second"]);
        // NO RUN NAMED (a photograph's lines): all of them.
        let mut unnamed = Scrollback::default();
        unnamed.push(said(1, false, "", "one"));
        unnamed.push(said(2, true, "", "two"));
        assert_eq!(texts(unnamed.last_run()), ["one", "two"]);
    }
}
