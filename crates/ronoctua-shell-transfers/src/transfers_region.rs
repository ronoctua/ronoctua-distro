//! THE QUEUE'S PANEL IN THE FOOTER'S REGION (`LEADER t t`) — the transfers'
//! view as a [`Region`] the footer hosts by its id, and the eight commands
//! that put what was copied into the queue.
//!
//! # What the footer still does, and why
//!
//! It keeps the list: the queue's thread sends the footer every list
//! (`queue::listen`), the modeline draws its segment from it, and the panel
//! is LENT the list when it opens ([`crate::region::Lent`]) and TOLD each
//! new one ([`Region::heard`]). The panel keeps its cursor — a job's id — and
//! nothing else of the queue's.
//!
//! The other door of the queue is a WASM buffer (`LEADER t w`), not a native
//! window: what both share is the queue's own table of keys
//! (`ronoctua_protocol::TRANSFERS_KEYS`), and what this door does with a key
//! is [`carry_out`], written once.

use crate::region::{Closed, Doorway, Entry, Paging, Pair, Region, Row, Seen, Taken, Told};
use crate::transfers::{Step, Target, View};
use ronoctua_tokens as tokens;
use ronoctua_transfers::Job;
use ronoctua_transfers::where_to::Wish;

/// The id the footer ranks the panel under (`PRECEDENCE`).
pub const ID: &str = "transfers";

/// The panel, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The panel, open in the footer.
struct Panel {
    view: View,
    /// Whether the command that opened it has been handed to it — the slot
    /// host hands the opening command to the region it just opened, and the
    /// SAME command later is the person asking for a region already taken.
    shown: bool,
}

/// What the eight that put something in ask for: the command, and the wish.
fn wish_of(command: &str) -> Option<(&'static str, Wish)> {
    use ronoctua_transfers::Want;
    Some(match command {
        "transfers-add" => ("transfers-add", Wish::plain()),
        "transfers-video" => ("transfers-video", Wish::media(Want::Video)),
        "transfers-audio" => ("transfers-audio", Wish::media(Want::Audio)),
        "transfers-subtitles" => ("transfers-subtitles", Wish::media(Want::Subtitles)),
        "transfers-add-as-me" => ("transfers-add-as-me", Wish::plain().as_me()),
        "transfers-video-as-me" => ("transfers-video-as-me", Wish::media(Want::Video).as_me()),
        "transfers-audio-as-me" => ("transfers-audio-as-me", Wish::media(Want::Audio).as_me()),
        "transfers-subtitles-as-me" => (
            "transfers-subtitles-as-me",
            Wish::media(Want::Subtitles).as_me(),
        ),
        _ => return None,
    })
}

/// The commands that do the same with the panel open or closed: its window,
/// and what was copied into the queue. `true` when `command` was one.
fn anywhere(command: &str, way: &mut dyn Doorway) -> bool {
    // AND THE EIGHT THAT PUT SOMETHING IN. What they read is the clipboard,
    // because an address is copied and never typed; the footer's history is
    // where it is (`Doorway::queue_copied`).
    if let Some((command, wish)) = wish_of(command) {
        way.queue_copied(command, wish);
        return true;
    }
    if command != "transfers-window" {
        return false;
    }
    // THE SAME QUEUE IN A WINDOW OF ITS OWN, which is a wasm buffer: its
    // component comes by the environment, and one that is not there is said.
    match crate::launch::buffer_component("RONOCTUA_BUFFER_TRANSFERS") {
        Some(component) => way.open_window(vec!["buffer".to_owned(), component]),
        None => {
            tracing::error!(
                variable = "RONOCTUA_BUFFER_TRANSFERS",
                "the component's variable is not in the environment"
            );
            let said = way.texts().t(crate::key::ECHO_NO_BUFFER);
            way.say(said, crate::echo::Echo::Normal);
        }
    }
    true
}

/// What a command does with the panel CLOSED.
fn closed(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    if command == "transfers-open" {
        return Closed::Opens;
    }
    if anywhere(command, way) {
        Closed::Taken
    } else {
        Closed::NotMine
    }
}

/// The panel, opened — empty: the list it is lent comes with the command
/// that opened it.
fn open(_command: &str, _argument: Option<&str>, _way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(Panel {
        view: View::fresh_one(),
        shown: false,
    })
}

/// What a key or a click on the panel asked for, carried out — the panel's
/// ONE errand handler. The view never opens a socket: it says what to send,
/// and the door sends it; a queue that does not answer is said there.
fn carry_out(step: Step, view: &View, way: &mut dyn Doorway) {
    match step {
        Step::Nothing => {}
        Step::Changed => {
            // THE PAGE FOLLOWS A CURSOR THAT IS ON A LINE — the footer finds
            // it as drawn (`Region::cursor_in`).
            if view.cursor_line(way.columns(), way.texts()).is_some() {
                way.page(Paging::Follow);
            }
            way.repaint();
        }
        Step::Closed => way.close(),
        Step::Send(ask) => {
            way.queue(&ask);
            way.repaint();
        }
        // THE PANEL CLOSES FIRST, for the launcher's reason: a panel left
        // open over the program that just opened keeps its keyboard.
        Step::Launch(desktop_id) => {
            way.close();
            way.launch(crate::door::Launching::Entry(desktop_id));
        }
    }
}

impl Panel {
    /// Every line it shows: a job's row says what it is under the pointer,
    /// and the empty queue's line is in the footer's own two columns
    /// ([`Pair`]).
    fn every_line(&self, seen: &Seen<'_>) -> Vec<Told> {
        let lines = self.view.lines(seen.columns, seen.texts);
        if lines.is_empty() {
            return vec![Told {
                pair: Some(Pair {
                    text: seen.texts.t(crate::key::TRANSFERS_EMPTY),
                    text_tone: tokens::color::TEXT_DIM,
                    ..Pair::default()
                }),
                ..Told::default()
            }];
        }
        lines
            .into_iter()
            .map(|l| Told {
                row: Row {
                    takes: l.target != Target::Nothing,
                    fields: l.fields,
                },
                // A TRANSFER'S ROW SAYS WHAT WENT WRONG: a row that says only
                // "failed" sends somebody to the journal.
                about: match l.target {
                    Target::Job(i) => self
                        .view
                        .job_at(i)
                        .map(|job| crate::jobs::under_the_pointer(job, seen.texts)),
                    Target::Nothing | Target::Answer(_) => None,
                },
                ..Told::default()
            })
            .collect()
    }
}

impl Region for Panel {
    fn id(&self) -> &'static str {
        ID
    }

    /// One row per job, drawn by the view with the file manager's own
    /// `Field` — and "nothing under way" is still a line: an empty region
    /// would leave the person unable to tell an empty queue from a panel
    /// that failed to open.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.every_line(seen).into_iter().map(|t| t.row).collect()
    }

    /// A job's row says what it is under the pointer, and the empty queue's
    /// line is in the footer's own two columns ([`Pair`]).
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Told> {
        self.every_line(seen)
            .into_iter()
            .skip(skip)
            .take(take)
            .collect()
    }

    /// Its keys are chords nothing on the row spells out: a panel whose only
    /// way to find `C-c` is the manual is a panel where nobody ever stops
    /// anything.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    /// Asked with the width (`cursor_in`): the question's answers and a job's
    /// page are other lines than the list's.
    fn cursor(&self) -> Option<usize> {
        None
    }

    /// The panel draws no chrome above the list, so its row IS its line.
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        self.view.cursor_line(seen.columns, seen.texts)
    }

    /// The panel counts without building: laying every job out to answer
    /// "how many" is what the pointer would pay on every motion. "Nothing
    /// under way" is a line.
    fn row_count(&self, seen: &Seen<'_>) -> usize {
        self.view.line_count(seen.columns, seen.texts).max(1)
    }

    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let step = self.view.key(key);
        let took = step != Step::Nothing;
        carry_out(step, &self.view, way);
        Taken::of(took)
    }

    fn command(&mut self, id: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        if id == "transfers-open" {
            if self.shown {
                // THE REGION IS TAKEN — by this panel, or by a view above it:
                // the sentence every door says when it is refused the region.
                let said = way.texts().t(crate::key::ECHO_VIEW_BUSY);
                way.say(said, crate::echo::Echo::Normal);
                way.repaint();
                return Taken::Yes;
            }
            self.shown = true;
            // THE LIST IT ALREADY HAS, immediately. The daemon will send
            // another within the frame, but a panel that opens empty and fills
            // in a moment later reads as "nothing is running" at the exact
            // instant somebody opened it to check.
            let jobs = way.lent().get::<Vec<Job>>().cloned().unwrap_or_default();
            tracing::info!(jobs = jobs.len(), "transfers panel opened");
            self.view.refresh(jobs);
            return Taken::Yes;
        }
        Taken::of(anywhere(id, way))
    }

    /// THE FIRST CLICK SELECTS AND THE SECOND DOES WHAT `RET` DOES — the
    /// job's page, or the answer under the cursor (invariant 19).
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let step = self.view.clicked(row, way.columns(), way.texts());
        carry_out(step, &self.view, way);
    }

    /// A new list from the queue, the cursor kept on its job.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(jobs) = news.downcast_ref::<Vec<Job>>() {
            self.view.refresh(jobs.clone());
            way.repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Lent, Recorder};
    use ronoctua_transfers::{Engine, Stage, Work};
    use std::path::PathBuf;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language")
    }

    fn way(texts: &std::rc::Rc<ronoctua_i18n::Texts>) -> Recorder {
        Recorder::new(
            std::rc::Rc::clone(texts),
            120,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn a_job(id: u64, stage: Stage) -> Job {
        let mut j = Job::fresh_one(
            id,
            Engine::Local,
            Work::Paste {
                paths: vec![PathBuf::from("/home/film.mkv")],
                into: PathBuf::from("/backup"),
                moves: false,
            },
        );
        j.stage = stage;
        j.total = 1000;
        j.done = 620;
        j
    }

    fn seen<'a>(texts: &'a ronoctua_i18n::Texts, zone: &'a ronoctua_files::when::Zone) -> Seen<'a> {
        Seen {
            texts,
            columns: 120,
            lines: 12,
            zone,
            photographing: true,
            lent: Lent::none(),
        }
    }

    /// A panel opened over `jobs` — the footer lends them — and driven by
    /// `keys`, with the doorway it was opened through, its asks cleared.
    fn a_panel(
        t: &std::rc::Rc<ronoctua_i18n::Texts>,
        jobs: Vec<Job>,
        keys: &[&str],
    ) -> (Panel, Recorder) {
        let mut lending = way(t).lending(Lent::of(vec![&jobs]));
        let mut panel = Panel {
            view: View::fresh_one(),
            shown: false,
        };
        let _ = panel.command("transfers-open", None, &mut lending);
        for notation in keys {
            let key = ronoctua_keymap::Key::parse(notation).expect("a key");
            let _ = panel.key(key, &mut lending);
        }
        let mut w = lending.way;
        w.owed.clear();
        (panel, w)
    }

    /// FAILS IF: a resting pointer lights a line of the panel a click takes
    /// nothing on — the job the give-up question is about, its sentence, a
    /// fact of a job's page, the empty queue's line — or leaves unlit one it
    /// takes: a job of the list, an answer. THE LIGHT IS THE CLICK: every
    /// line clicked once, on a panel of its own, and lit exactly where
    /// something happened — on the list, under the give-up question, on a
    /// job's page and on the empty queue.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        fn two() -> Vec<Job> {
            vec![a_job(1, Stage::Running), a_job(2, Stage::Done)]
        }
        let t = std::rc::Rc::new(texts());
        let zone = ronoctua_files::when::Zone::utc();
        let at = seen(&t, &zone);
        let held = |jobs: fn() -> Vec<Job>, keys: &[&str]| {
            crate::region::held_to_the_click(
                || a_panel(&t, jobs(), keys),
                &at,
                |r: &Panel, w: &Recorder| crate::region::seen(r, &w.owed, &at),
            )
        };
        // THE LIST: every line a job, and each takes the click.
        let list = held(two, &[]);
        assert!(
            list.len() == 2 && list.iter().all(|h| h.lit),
            "the list: {list:?}"
        );
        // THE QUESTION, about the running job: its two answers take it, the
        // job it is about and its sentence do not.
        let question = held(two, &["C-c"]);
        assert!(
            question.iter().any(|h| h.lit) && question.iter().any(|h| !h.lit),
            "the question: {question:?}"
        );
        // A JOB'S PAGE is facts, and takes nothing.
        let page = held(two, &["RET"]);
        assert!(
            page.len() > 1 && page.iter().all(|h| !h.lit),
            "a job's page: {page:?}"
        );
        // THE EMPTY QUEUE is a line of words.
        let empty = held(Vec::new, &[]);
        assert!(
            empty.len() == 1 && !empty[0].lit,
            "the empty queue: {empty:?}"
        );
        for (what, held) in [
            ("the list", list),
            ("the question", question),
            ("a job's page", page),
            ("the empty queue", empty),
        ] {
            let unheld = crate::region::unheld(&held);
            assert!(unheld.is_empty(), "{what}: {unheld:?}");
        }
    }

    /// FAILS IF: the panel opens without the list the footer already holds
    /// — it would read "nothing is running" at the instant somebody looked.
    #[test]
    fn the_panel_opens_on_the_list_it_is_lent() {
        let t = std::rc::Rc::new(texts());
        let jobs = vec![a_job(1, Stage::Running), a_job(2, Stage::Done)];
        let mut w = way(&t).lending(Lent::of(vec![&jobs]));
        assert_eq!(closed("transfers-open", None, &mut w), Closed::Opens);
        let mut panel = open("transfers-open", None, &mut w);
        assert_eq!(panel.id(), ID);
        assert_eq!(panel.command("transfers-open", None, &mut w), Taken::Yes);
        let zone = ronoctua_files::when::Zone::utc();
        let rows = panel.told(&seen(&t, &zone), 0, 1000);
        assert_eq!(rows.len(), 2);
        assert_eq!(panel.rows(&seen(&t, &zone)).len(), 2);
        assert!(rows.iter().all(|r| r.row.takes && !r.row.fields.is_empty()));
        assert!(
            rows.iter().all(|r| r.about.is_some()),
            "a job's row says what it is"
        );
        assert_eq!(panel.cursor_in(&seen(&t, &zone)), Some(0));
        // AND OPENED AGAIN, the region is taken: said, and nothing reset.
        assert_eq!(panel.command("transfers-open", None, &mut w), Taken::Yes);
        assert!(w.owed.iter().any(|e| matches!(
            e,
            Effect::Say(said, _) if *said == t.t(crate::key::ECHO_VIEW_BUSY)
        )));
    }

    /// FAILS IF: an empty queue is an empty region — no line to tell it from
    /// a panel that never opened.
    #[test]
    fn an_empty_queue_is_a_line() {
        let t = std::rc::Rc::new(texts());
        let mut w = way(&t);
        let mut panel = open("transfers-open", None, &mut w);
        let _ = panel.command("transfers-open", None, &mut w);
        let zone = ronoctua_files::when::Zone::utc();
        let rows = panel.told(&seen(&t, &zone), 0, 1000);
        assert_eq!(rows.len(), 1);
        let pair = rows[0].pair.as_ref().expect("a line of words");
        assert_eq!(pair.text, t.t(crate::key::TRANSFERS_EMPTY));
        assert_eq!(pair.text_tone, tokens::color::TEXT_DIM);
        assert!(!rows[0].row.takes);
    }

    /// FAILS IF: a key that moves the cursor does not ask the page to follow
    /// it, `Esc` does not close, or a new list does not reach an open panel.
    #[test]
    fn keys_move_the_page_and_close_and_a_list_is_heard() {
        let t = std::rc::Rc::new(texts());
        let jobs = vec![a_job(1, Stage::Running), a_job(2, Stage::Running)];
        let mut w = way(&t).lending(Lent::of(vec![&jobs]));
        let mut panel = open("transfers-open", None, &mut w);
        let _ = panel.command("transfers-open", None, &mut w);
        w.owed.clear();
        let down = ronoctua_keymap::Key::parse("<down>").expect("a key");
        assert_eq!(panel.key(down, &mut w), Taken::Yes);
        assert!(w.owed.contains(&Effect::Page(Paging::Follow)));
        let zone = ronoctua_files::when::Zone::utc();
        assert_eq!(
            panel.cursor_in(&seen(&t, &zone)),
            Some(1),
            "the page follows a cursor that is not on the second line"
        );
        let more = vec![
            a_job(1, Stage::Running),
            a_job(2, Stage::Running),
            a_job(3, Stage::Done),
        ];
        panel.heard(&more, &mut w);
        let zone = ronoctua_files::when::Zone::utc();
        assert_eq!(panel.rows(&seen(&t, &zone)).len(), 3);
        w.owed.clear();
        let esc = ronoctua_keymap::Key::parse("ESC").expect("a key");
        let _ = panel.key(esc, &mut w);
        assert_eq!(w.owed, vec![Effect::Close]);
    }

    /// FAILS IF: one of the eight that put something in is not one, asks for
    /// another wish, or opens the panel — or the window's command is lost
    /// with the panel closed.
    #[test]
    fn what_was_copied_goes_in_open_or_closed() {
        let t = std::rc::Rc::new(texts());
        let mut w = way(&t);
        assert_eq!(closed("transfers-video-as-me", None, &mut w), Closed::Taken);
        assert_eq!(
            w.owed,
            vec![Effect::QueueCopied(
                "transfers-video-as-me",
                Wish::media(ronoctua_transfers::Want::Video).as_me()
            )]
        );
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
        let mut panel = open("transfers-open", None, &mut w);
        let _ = panel.command("transfers-open", None, &mut w);
        w.owed.clear();
        assert_eq!(panel.command("transfers-add", None, &mut w), Taken::Yes);
        assert_eq!(
            w.owed,
            vec![Effect::QueueCopied("transfers-add", Wish::plain())]
        );
        assert_eq!(panel.command("transfers-pause", None, &mut w), Taken::No);
        for (command, _) in [
            "transfers-add",
            "transfers-video",
            "transfers-audio",
            "transfers-subtitles",
            "transfers-add-as-me",
            "transfers-video-as-me",
            "transfers-audio-as-me",
            "transfers-subtitles-as-me",
        ]
        .iter()
        .filter_map(|c| wish_of(c))
        {
            assert!(
                crate::transfers::KEYS
                    .commands
                    .iter()
                    .any(|c| c.id == command),
                "{command}"
            );
        }
    }

    /// FAILS IF: launching what a finished install made leaves the panel
    /// open over the program, holding its keyboard.
    #[test]
    fn a_launch_closes_the_panel_first() {
        let t = std::rc::Rc::new(texts());
        let mut w = way(&t);
        let view = View::fresh_one();
        carry_out(Step::Launch("org.gimp.GIMP".to_owned()), &view, &mut w);
        assert_eq!(
            w.owed,
            vec![
                Effect::Close,
                Effect::Launch(crate::door::Launching::Entry("org.gimp.GIMP".to_owned()))
            ]
        );
    }
}
