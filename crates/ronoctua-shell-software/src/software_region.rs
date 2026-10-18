//! THE PERSON'S PROGRAMS IN THE FOOTER'S REGION (`LEADER i i`) — the view as
//! a [`Region`] of this crate, which the footer hosts by its id and names
//! nothing else of.
//!
//! # What is here, and what is the footer's
//!
//! The view, what it reads and the one handler of what it asks are the two
//! doors' (`software_reading`); this is the footer's door around them: which
//! of the view's commands open it, the launcher's sheet that waits for the
//! first reading, the rows with their pictures, and the keyboard given back
//! before a program opens elsewhere. What the footer keeps about the
//! person's programs with the view CLOSED — the modeline's two marks, the
//! ends of the queue's software jobs, the event that opens what was just
//! installed — is the footer's, and stays there.
//!
//! # What outlives the view
//!
//! The view and its threads are born with the region and go with it:
//! nothing is read and nobody is asked while nobody looks. The READINGS stay
//! (`KEPT`): the next opening draws what was last read at once — the
//! programs, the history, a sheet's details — while the machine is read
//! again, as the footer kept them when the view was a field of its own.

use std::cell::{Cell, RefCell};

use crate::programs_words::{SHEET, SHEET_TO_REMOVE, sheet_of};
use crate::region::{Closed, Doorway, Framing, Region, Row, RowPicture, Seen, Taken, Told};
use crate::software::{Ask, Errand, Opening, Picking, View};
use crate::software_feed::{Readings, Taught, Want};
use crate::software_reading::Reading;

/// The region's id, as the footer's precedence ranks it.
pub const ID: &str = "software";

thread_local! {
    /// THE READINGS BETWEEN TWO OPENINGS — see the module. The footer's
    /// loop is one thread, and so is each test: one footer's, never shared.
    static KEPT: RefCell<Option<Readings>> = const { RefCell::new(None) };
}

/// Readings planted for the next opening — a photograph's, where its feed's
/// thread would put one (`photograph --door footer --software <host>`).
pub fn keep_for_the_next_opening(readings: Readings) {
    KEPT.with(|k| *k.borrow_mut() = Some(readings));
}

/// The region the binary registers (`regions.rs`).
pub const ENTRY: crate::region::Entry = crate::region::Entry {
    id: ID,
    closed,
    open,
};

/// What a command does while the view is closed: those that open it, those
/// done without it — the window, a question asked first, a word that names
/// no page — and the view's own keys, which find nothing to press.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "software-open" => match argument.map(str::trim).filter(|w| !w.is_empty()) {
            Some(words) if crate::software_window::opening_of(words).is_none() => {
                no_such_level(words, way);
                Closed::Taken
            }
            _ => Closed::Opens,
        },
        "software-window" => {
            open_the_window(argument, way);
            Closed::Taken
        }
        "software-trials" | "software-maintenance" | "software-android" => Closed::Opens,
        // THE LAUNCHER'S DOOR INTO A SHEET, with the entry it names — and
        // words it did not write open nothing: an empty view held open over
        // the keyboard would be a panel that says nothing.
        SHEET | SHEET_TO_REMOVE => {
            if sheet_of(argument).is_some() {
                Closed::Opens
            } else {
                tracing::warn!(
                    command,
                    "a sheet asked for with no launcher entry; nothing opens"
                );
                Closed::Taken
            }
        }
        // ASKED AND THEN TOLD: with nothing typed the minibuffer asks for the
        // address, and the view opens on the answer.
        "software-web-new" => match argument {
            Some(said) if !said.trim().is_empty() => Closed::Opens,
            // AN EMPTY ANSWER IS `Esc`, not a question to ask again.
            Some(_) => Closed::Taken,
            None => {
                way.ask(crate::line::Line::new("software-web-new", false, None));
                Closed::Taken
            }
        },
        // A QUESTION OF THE VIEW'S, run by name: the view opens first, since
        // it is what knows which question the answer answers.
        id if Ask::of_command(id).is_some() => Closed::Opens,
        // ONE OF THE VIEW'S KEYS with no view: nothing to press, and nothing
        // that says "does nothing yet" either — the key exists.
        id if crate::software::press_of(id).is_some() => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The region, born with its threads — none for a photograph's door, whose
/// view draws what was planted (`keep_for_the_next_opening`) and reaches
/// nothing of the machine.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(SoftwareRegion::born(way))
}

impl SoftwareRegion {
    /// The region, before the command that opened it — see [`open`].
    fn born(way: &mut dyn Doorway) -> Self {
        let photographing = way.photographing();
        let context = if photographing {
            None
        } else {
            ronoctua_software::Context::of_session()
            .map_err(
                |e| tracing::warn!(error = %e, "no software context; the view will say it is reading"),
            )
            .ok()
        };
        let kept = KEPT.with(|k| k.borrow_mut().take());
        let mut readings = kept.unwrap_or_else(|| {
            if photographing {
                Readings::default()
            } else {
                let mut r = Readings::fresh(
                    context
                        .as_ref()
                        .map(|c| c.prefs.clone())
                        .unwrap_or_default(),
                    Taught::of_this_leader(),
                );
                r.belongings = ronoctua_belongings::places::Places::from_env();
                r
            }
        });
        // THE FILE AS IT IS NOW: what another door wrote while the view was
        // closed is what this one opens on.
        if let Some(c) = context.as_ref() {
            readings.took_preferences(c.prefs.clone());
        }
        let reading = Reading::new(readings, context, way.texts().language(), way.wake());
        Self {
            view: None,
            reading,
            waiting: None,
            first: true,
            cursor: Cell::new(0),
            scale: 1.0,
            photographing,
        }
    }
}

/// "No such page", for the words after `software-open`.
fn no_such_level(words: &str, way: &mut dyn Doorway) {
    let said = way
        .texts()
        .t_with(crate::key::SOFTWARE_NO_SUCH_LEVEL, &[("words", words)]);
    way.say(said, crate::echo::Echo::Normal);
}

/// THE WINDOW, on the level an argument names (`software-window trials`,
/// `software-window search gimp`, `software-window web <address>`) — started
/// by the footer, so it inherits the footer's environment, its tools among
/// it, which a window started by hand would not.
fn open_the_window(argument: Option<&str>, way: &mut dyn Doorway) {
    let mut args = vec!["software".to_owned()];
    args.extend(
        argument
            .map(str::trim)
            .filter(|w| crate::software_window::opening_of(w).is_some())
            .map(str::to_owned),
    );
    way.open_window(args);
}

/// The view in the footer's region — see the module.
pub struct SoftwareRegion {
    /// The view, from the command that opened the region on.
    view: Option<View>,
    /// What it reads, and the one handler of what it asks.
    reading: Reading,
    /// A launcher entry whose sheet was asked for before a reading this view
    /// keeps fresh: its desktop id, its name, and whether with the remove
    /// question up. Which program it is — the person's or the system's — is
    /// the reading's to say.
    waiting: Option<(String, String, bool)>,
    /// The command being handled is the one that opened the region.
    first: bool,
    /// Which drawn row the cursor is on, as last counted — at the door's
    /// width, after every call, and again whenever the rows are drawn.
    cursor: Cell<usize>,
    /// The sharpest screen's scale, as the last frame was told it
    /// (`Region::before_frame`) — what a row's picture is decoded for: the
    /// footer decodes a picture once and draws it on every screen.
    scale: f32,
    /// A photograph's region: nothing of the machine is read.
    photographing: bool,
}

impl std::fmt::Debug for SoftwareRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareRegion")
            .field("open", &self.view.is_some())
            .finish_non_exhaustive()
    }
}

impl SoftwareRegion {
    /// What the door fills in the readings on every opening — the leader's
    /// sequences, the zone, the clock and the home — when it reads the
    /// machine at all: a photograph's draws what it was handed.
    fn awake(&mut self) {
        if self.photographing {
            return;
        }
        let home = self.reading.home();
        let r = &mut self.reading.readings;
        r.taught = Taught::of_this_leader();
        r.zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        r.now = crate::when::seconds_now();
        if home.is_some() {
            r.home = home;
        }
    }

    /// The view opened on `opening` — or turned to it when it is up, the
    /// level it leaves being the way back. EVERY OPENING READS THE MACHINE
    /// AGAIN: what the view says about what is installed is never older than
    /// the view — nor what the person let (a catalogue let in another door
    /// draws no row here).
    fn open_on(&mut self, opening: Opening, way: &mut dyn Doorway) {
        self.awake();
        self.reading.speak(way.texts().language());
        let errand = match self.view.as_mut() {
            Some(v) => v.reopen(opening, &self.reading.readings, way.texts()),
            None => {
                let (mut v, e) = View::fresh(opening, &self.reading.readings, way.texts());
                // THE FILE PICKER'S FOLDER, watched through the door's wake.
                if !self.photographing {
                    v.watch_with(way.wake());
                }
                self.view = Some(v);
                e
            }
        };
        self.reading.readings.reread_consents();
        self.reading.want(Want::Reread);
        // ANOTHER PAGE OF THE VIEW: at its top, and the cursor followed from
        // there.
        way.first_page();
        way.follow();
        self.carry(errand, way);
    }

    /// What the view asked, done — after giving the keyboard back when it
    /// opens something elsewhere: a view left open over the program that just
    /// opened keeps the keyboard the footer holds (the launcher's rule).
    fn carry(&mut self, errand: Errand, way: &mut dyn Doorway) {
        if errand.opens_elsewhere() {
            way.close();
        }
        self.reading.carry_out(errand, way);
    }

    /// The opening for a launcher entry, from the reading in hand.
    fn opening_for(&self, desktop_id: &str, name: String, remove: bool) -> Opening {
        let key =
            crate::software::key_of_desktop(self.reading.readings.snapshot.as_ref(), desktop_id);
        if remove {
            Opening::Remove { key, name }
        } else {
            Opening::Sheet { key, name }
        }
    }

    /// The launcher's `C-i` and `C-d`: the sheet of the program behind an
    /// entry, with the remove question up for `C-d`.
    ///
    /// WHICH PROGRAM IT IS — the person's, or the system's — is a reading's to
    /// say, and only a reading the open view keeps fresh will do: a program
    /// installed from a terminal since, called "the system's", would be a
    /// refusal that lies. Without one the view opens on its first page and
    /// the sheet follows the reading the opening asks for (`woken`).
    fn sheet(&mut self, argument: Option<&str>, remove: bool, first: bool, way: &mut dyn Doorway) {
        let Some((desktop_id, name)) = sheet_of(argument) else {
            return;
        };
        let fresh = !first && self.reading.reads() && self.reading.readings.snapshot.is_some();
        if fresh {
            let opening = self.opening_for(desktop_id, name.to_owned(), remove);
            self.open_on(opening, way);
            return;
        }
        self.waiting = Some((desktop_id.to_owned(), name.to_owned(), remove));
        self.open_on(Opening::Home, way);
    }

    /// A question of the view's, by its command: what the minibuffer
    /// answered, or — run by name with nothing typed — the question asked.
    fn typed_command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) {
        let Some(ask) = Ask::of_command(id) else {
            return;
        };
        if self.view.is_none() {
            self.open_on(Opening::Home, way);
        }
        // A FILE IS CHOSEN, not typed: with no argument, the view's picker up
        // over it — what its rows open (`software::Picking`) — as
        // `syncthing-add-folder` does. With one, the path is the answer.
        let picking = match ask {
            Ask::IconPath => Some(Picking::Icon),
            Ask::FilePath => Some(Picking::Package),
            _ => None,
        };
        let Some(view) = self.view.as_mut() else {
            return;
        };
        if argument.is_none()
            && let Some(asked) = picking
        {
            let errand = view.choose(asked);
            self.carry(errand, way);
            return;
        }
        let Some(said) = argument else {
            way.ask(crate::line::Line::new(ask.command(), ask.secret(), None));
            return;
        };
        let errand = self.reading.typed(view, id, said, way);
        self.carry(errand, way);
    }

    /// Count the cursor's row again, at the door's width.
    fn recount(&self, way: &dyn Doorway) {
        if let Some(v) = self.view.as_ref() {
            self.cursor.set(View::cursor(&self.reading.rows(
                v,
                way.columns(),
                way.texts(),
            )));
        }
    }

    /// The side a row's picture is decoded at, on the sharpest screen.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn side(&self) -> u32 {
        (crate::launch::pixmap_side() * self.scale.max(1.0)).ceil() as u32
    }

    /// THE PERSON'S OWN PROGRAMS as drawn: rows of fields, and a program's
    /// picture where its glyph is once the picture is here — the glyph until
    /// then, never both.
    fn told_rows(&self, seen: &Seen<'_>) -> Vec<Told> {
        let Some(v) = self.view.as_ref() else {
            return Vec::new();
        };
        let side = self.side();
        let rows = self.reading.rows(v, seen.columns, seen.texts);
        self.cursor.set(View::cursor(&rows));
        rows.into_iter()
            .map(|r| {
                let ready = r.picture.as_ref().and_then(|p| {
                    self.reading
                        .pictures()
                        .and_then(|pictures| pictures.ready(p, side))
                });
                let mut fields = r.fields;
                // THE GLYPH'S CELL IS WHERE THE PICTURE GOES, and the glyph
                // leaves the row the moment the picture takes it.
                let picture = ready.and_then(|key| {
                    (!fields.is_empty()).then(|| RowPicture {
                        key,
                        at: fields.remove(0).at,
                    })
                });
                Told {
                    row: Row {
                        fields,
                        takes: !r.heading,
                    },
                    picture,
                    ..Told::default()
                }
            })
            .collect()
    }
}

impl Region for SoftwareRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// The rows as drawn ([`SoftwareRegion::told_rows`]), without their
    /// pictures.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.told_rows(seen).into_iter().map(|t| t.row).collect()
    }

    /// The rows as drawn, each with its picture once it is here.
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Told> {
        self.told_rows(seen)
            .into_iter()
            .skip(skip)
            .take(take)
            .collect()
    }

    /// EVERY COMMAND IS A `C-` KEY, and only the ones that work on the row
    /// are taught.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.as_ref().map_or_else(Vec::new, |v| {
            v.hint(&self.reading.readings, seen.columns, seen.texts)
        })
    }

    /// THE DRAWN ROW, which is the view's own answer: a note is as many rows
    /// as the width allows, and the cursor is found by identity.
    fn cursor(&self) -> Option<usize> {
        self.view.as_ref().map(|_| self.cursor.get())
    }

    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        if let Some(v) = self.view.as_mut() {
            let errand = self.reading.stroke(v, key, way);
            self.carry(errand, way);
        }
        self.recount(way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        let first = std::mem::take(&mut self.first);
        match id {
            // THE PERSON'S OWN PROGRAMS — `LEADER i`. The footer's view on
            // the level each key names, and ON THE LEVEL ITS WORDS NAME — the
            // window's own words (`software_window::opening_of`): `advanced`,
            // `search <words>`, `web <address>`.
            "software-open" => match argument.map(str::trim).filter(|w| !w.is_empty()) {
                None => self.open_on(Opening::Home, way),
                Some(words) => match crate::software_window::opening_of(words) {
                    Some(opening) => self.open_on(opening, way),
                    None => no_such_level(words, way),
                },
            },
            "software-window" => open_the_window(argument, way),
            "software-trials" => self.open_on(Opening::Trials, way),
            "software-maintenance" => self.open_on(Opening::Maintenance, way),
            "software-android" => self.open_on(Opening::Android, way),
            "software-web-new" => match argument {
                Some(said) if !said.trim().is_empty() => {
                    self.open_on(Opening::WebNew(said.trim().to_owned()), way);
                }
                Some(_) => {}
                None => way.ask(crate::line::Line::new("software-web-new", false, None)),
            },
            SHEET => self.sheet(argument, false, first, way),
            SHEET_TO_REMOVE => self.sheet(argument, true, first, way),
            // WHAT WAS TYPED comes back as the command's argument, and the
            // view is what knows which question it answers.
            id if Ask::of_command(id).is_some() => self.typed_command(id, argument, way),
            // THE VIEW'S OWN KEYS, by their command ids — from `LEADER :`, a
            // hint clicked, the manual — through the reader a key goes
            // through.
            id => {
                let Some(press) = crate::software::press_of(id) else {
                    return Taken::No;
                };
                if let Some(v) = self.view.as_mut() {
                    let errand = self.reading.pressed(v, press, way);
                    self.carry(errand, way);
                }
            }
        }
        self.recount(way);
        // A PERSON'S GESTURE THAT IS NO KEY — a command by name, a hint
        // clicked, an answer: the page follows the cursor, as after a key.
        way.follow();
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        if let Some(v) = self.view.as_mut() {
            let errand = self.reading.clicked(v, row, way);
            self.carry(errand, way);
        }
        self.recount(way);
    }

    /// The minibuffer answered a question the view asked: the same command,
    /// with what was typed.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let _ = self.command(id, Some(text), way);
    }

    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        if let Some(v) = self.view.as_mut() {
            let errand = self.reading.pasted(v, text);
            self.carry(errand, way);
        }
        self.recount(way);
        way.follow();
    }

    /// ANYWHERE ON IT: a package to install, or an image for a web app's
    /// icon.
    fn takes_drops(&self) -> bool {
        true
    }

    fn dropped(&mut self, paths: &[std::path::PathBuf], _: bool, way: &mut dyn Doorway) -> bool {
        tracing::info!(
            dropped = paths.len(),
            "files were let go over the software view"
        );
        if let Some(v) = self.view.as_mut() {
            let errand = self.reading.dropped(v, paths, way);
            self.carry(errand, way);
        }
        self.recount(way);
        // A DROP IS A PERSON'S GESTURE: the page follows the cursor after it,
        // as after a key.
        way.follow();
        true
    }

    /// The view's threads brought something: take all of it, keep the cursor
    /// on its row, and draw — a reading never moves the page. And THE
    /// LAUNCHER'S SHEET THAT WAITED for a reading.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let Some(v) = self.view.as_mut() else {
            return;
        };
        let woke = self.reading.woken(v, way);
        for errand in woke.errands {
            self.carry(errand, way);
        }
        if woke.read
            && let Some((id, name, remove)) = self.waiting.take()
        {
            // THE SHEET THE PERSON ASKED FOR, now that a reading says which
            // program it is — a page that opens, at its top.
            let opening = self.opening_for(&id, name, remove);
            if let Some(v) = self.view.as_mut() {
                let errand = v.reopen(opening, &self.reading.readings, way.texts());
                way.first_page();
                way.follow();
                self.carry(errand, way);
            }
            self.recount(way);
            return;
        }
        if woke.changed {
            if let Some(v) = self.view.as_mut() {
                self.reading.settle(v, way.columns(), way.texts());
            }
            way.repaint();
        }
        self.recount(way);
    }

    /// THE QUEUE'S LIST: the view draws a job's progress on its program's
    /// row, and a job of ours that ended changed the machine.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        let Some(jobs) = news.downcast_ref::<Vec<ronoctua_transfers::Job>>() else {
            return;
        };
        // TOLD AT OPENING, before the view: the list is taken, and the
        // opening reads the machine again anyway — one read, not two.
        if self.view.is_none() {
            let _ = self.reading.readings.took_jobs(jobs);
            return;
        }
        if self.reading.jobs(self.view.as_ref(), jobs) {
            if let Some(v) = self.view.as_mut() {
                self.reading.settle(v, way.columns(), way.texts());
            }
            way.repaint();
        }
        self.recount(way);
    }

    /// The pictures of the rows on screen, asked for — the launcher's rule:
    /// the visible page only, before the frame's borrows, at the scale the
    /// frame is drawn for. And the file picker, up over the view, told how
    /// many rows a page is (its `PgUp` and `PgDn` walk half of them).
    fn before_frame(&mut self, framing: &Framing<'_>) {
        self.scale = framing.scale;
        let Some(v) = self.view.as_ref() else {
            return;
        };
        v.page_is(framing.page);
        let side = self.side();
        let rows = self.reading.rows(v, framing.columns, framing.texts);
        let shown = rows.iter().skip(framing.first).take(framing.page);
        self.reading.ask_pictures(shown, side);
    }

    fn held_picture(&self, key: &str) -> Option<&ronoctua_render::image::Rgba> {
        self.reading.pictures().and_then(|p| p.pixels(key))
    }

    /// IT SAYS WHAT A WAKE BROUGHT: a frame when a reading changed what it
    /// shows (`Doorway::repaint`), a page that opened, and nothing for a wake
    /// whose readings the first wake already took.
    fn says_what_a_wake_brought(&self) -> bool {
        true
    }

    /// THE VIEW'S SEARCH, where the launcher draws its query — and only while
    /// there is one: an empty search leaves the line to the keys the view
    /// teaches.
    fn prompt(&self) -> Option<String> {
        self.view
            .as_ref()
            .map(View::query)
            .filter(|q| !q.is_empty())
            .map(ToOwned::to_owned)
    }

    /// The view goes, and its threads with it; what it read is kept for the
    /// next opening (`KEPT`).
    fn closing(&mut self, _: &mut dyn Doorway) {
        let readings = std::mem::take(&mut self.reading.readings);
        KEPT.with(|k| *k.borrow_mut() = Some(readings));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::programs_words::sheet_argument;
    use crate::region::{Effect, Recorder};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// A photograph's doorway: nothing of the machine is read, and what the
    /// region asks is written down.
    fn way() -> Recorder {
        Recorder::new(
            std::rc::Rc::new(texts()),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// The region as the footer opens it: born, then handed the command.
    fn opened(command: &str, argument: Option<&str>) -> (SoftwareRegion, Recorder) {
        let mut w = way();
        assert_eq!(
            closed(command, argument, &mut w),
            Closed::Opens,
            "{command}"
        );
        let mut r = SoftwareRegion::born(&mut w);
        assert_eq!(r.command(command, argument, &mut w), Taken::Yes);
        (r, w)
    }

    fn seen<'a>(t: &'a ronoctua_i18n::Texts, zone: &'a ronoctua_files::when::Zone) -> Seen<'a> {
        Seen {
            texts: t,
            columns: 100,
            lines: 30,
            zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        }
    }

    fn drawn(r: &SoftwareRegion) -> String {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        r.rows(&seen(&t, &zone))
            .iter()
            .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|")
    }

    /// FAILS IF: a command of the view, run while the view is closed, opens
    /// it when it should not — the window, a question asked first, a word
    /// that names no page, a key with nothing to press — or does not open it
    /// when it should; or a command that is the footer's is taken.
    #[test]
    fn what_a_command_does_with_the_view_closed() {
        let mut w = way();
        assert_eq!(closed("software-open", None, &mut w), Closed::Opens);
        assert_eq!(
            closed("software-open", Some("advanced"), &mut w),
            Closed::Opens
        );
        assert_eq!(closed("software-trials", None, &mut w), Closed::Opens);
        assert_eq!(closed(SHEET, Some("a.desktop\nA"), &mut w), Closed::Opens);
        assert_eq!(
            closed(SHEET_TO_REMOVE, Some("a.desktop\nA"), &mut w),
            Closed::Opens
        );
        assert_eq!(closed("software-icon-path", None, &mut w), Closed::Opens);
        assert_eq!(
            closed("software-web-new", Some("example.org"), &mut w),
            Closed::Opens
        );
        assert!(w.owed.is_empty(), "{:?}", w.owed);
        // A WORD THAT NAMES NO PAGE is said, and nothing opens.
        assert_eq!(
            closed("software-open", Some("nowhere"), &mut w),
            Closed::Taken
        );
        assert!(
            matches!(w.owed.as_slice(), [Effect::Say(..)]),
            "{:?}",
            w.owed
        );
        // THE WINDOW, on the level its words name — and on the first page
        // for words that name none.
        let mut w = way();
        assert_eq!(
            closed("software-window", Some("trials"), &mut w),
            Closed::Taken
        );
        assert_eq!(
            closed("software-window", Some("bogus"), &mut w),
            Closed::Taken
        );
        assert_eq!(
            w.owed,
            vec![
                Effect::OpenWindow(vec!["software".to_owned(), "trials".to_owned()]),
                Effect::OpenWindow(vec!["software".to_owned()]),
            ]
        );
        // ASKED FIRST, and an empty answer is `Esc`.
        let mut w = way();
        assert_eq!(closed("software-web-new", None, &mut w), Closed::Taken);
        assert_eq!(
            closed("software-web-new", Some("  "), &mut w),
            Closed::Taken
        );
        assert_eq!(
            w.owed,
            vec![Effect::Ask(crate::line::Line::new(
                "software-web-new",
                false,
                None
            ))]
        );
        // A SHEET OF NO ENTRY — the command typed by hand — opens nothing,
        // and holds no empty view over the keyboard.
        let mut w = way();
        assert_eq!(closed(SHEET, Some("a.desktop"), &mut w), Closed::Taken);
        assert_eq!(closed(SHEET_TO_REMOVE, None, &mut w), Closed::Taken);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
        // A KEY OF THE VIEW, with no view: taken, and nothing done.
        let mut w = way();
        assert_eq!(closed("software-down", None, &mut w), Closed::Taken);
        assert!(w.owed.is_empty());
        // THE FOOTER'S OWN, and nobody's.
        assert_eq!(
            closed("software-launch-latest", None, &mut w),
            Closed::NotMine
        );
        assert_eq!(closed("files-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: the region opens on another page than its command names, or
    /// its page does not start at its top, or it reaches the machine in a
    /// photograph.
    #[test]
    fn a_command_opens_the_view_on_its_page() {
        let (r, w) = opened("software-trials", None);
        assert_eq!(r.id(), ID);
        assert!(w.owed.contains(&Effect::FirstPage), "{:?}", w.owed);
        assert!(r.cursor().is_some());
        assert_eq!(
            r.view.as_ref().map(View::level),
            Some(&crate::software::Level::Trials)
        );
    }

    /// FAILS IF: what the view read is lost when it closes — the next opening
    /// would draw "reading…" where the programs were — or a photograph's
    /// planted reading is not what its view draws.
    #[test]
    fn the_readings_outlive_the_view() {
        let planted = Readings {
            snapshot: Some(ronoctua_software::state::Snapshot::default()),
            ..Readings::default()
        };
        keep_for_the_next_opening(planted);
        let (mut r, mut w) = opened("software-open", None);
        let reading = drawn(&r);
        r.closing(&mut w);
        drop(r);
        assert!(KEPT.with(|k| k.borrow().is_some()), "nothing kept");
        let (r, _) = opened("software-open", None);
        assert_eq!(drawn(&r), reading);
        assert!(KEPT.with(|k| k.borrow().is_none()), "kept twice");
    }

    /// FAILS IF: the launcher's sheet trusts a reading the open view did not
    /// make — opened by the sheet's own command, the region has read nothing
    /// yet, and the sheet must wait for the reading on the first page.
    #[test]
    fn a_launcher_sheet_waits_for_a_fresh_reading() {
        let argument = sheet_argument("org.gimp.GIMP.desktop", "GIMP");
        assert_eq!(argument, "org.gimp.GIMP.desktop\nGIMP");
        let (r, _) = opened(SHEET, Some(&argument));
        assert_eq!(
            r.waiting,
            Some(("org.gimp.GIMP.desktop".to_owned(), "GIMP".to_owned(), false))
        );
        assert_eq!(
            r.view.as_ref().map(View::level),
            Some(&crate::software::Level::Home)
        );
        // AND `C-d`'s, with the remove question to come.
        let (r, _) = opened(SHEET_TO_REMOVE, Some(&argument));
        assert!(r.waiting.as_ref().is_some_and(|(_, _, remove)| *remove));
    }

    /// FAILS IF: a key that starts a program leaves the view open over it —
    /// the footer would keep the keyboard the new window needs.
    #[test]
    fn a_program_started_gives_the_keyboard_back_first() {
        let (mut me, _) = opened("software-open", None);
        let mut w = way();
        me.carry(Errand::Launch("org.gimp.GIMP.desktop".to_owned()), &mut w);
        me.carry(Errand::Run("files-window"), &mut w);
        assert_eq!(
            w.owed,
            vec![
                Effect::Close,
                Effect::Launch(crate::door::Launching::Entry(
                    "org.gimp.GIMP.desktop".to_owned()
                )),
                // THE FILE MANAGER BESIDE THE VIEW, which stays: an image
                // dragged out of it lands on the view.
                Effect::OpenWindow(vec!["files".to_owned()]),
            ]
        );
        // THE BRAIN'S PAGE is a window elsewhere: the keyboard first — and
        // then the page, or, with no brain buffer in this environment, why not.
        let mut w = way();
        me.carry(Errand::Run("brain-open"), &mut w);
        assert!(
            matches!(
                w.owed.as_slice(),
                [Effect::Close, Effect::OpenWindow(args)] if args[0] == "buffer"
            ) || matches!(w.owed.as_slice(), [Effect::Close, Effect::Say(..)]),
            "{:?}",
            w.owed
        );
    }

    /// FAILS IF: a question of the view's run by name with nothing typed is
    /// not asked on the door's line, or a file to choose is typed instead of
    /// chosen, or the answer does not reach the view.
    #[test]
    fn a_question_is_asked_chosen_or_answered() {
        let (mut r, _) = opened("software-open", None);
        let mut w = way();
        assert_eq!(r.command("software-web-name", None, &mut w), Taken::Yes);
        assert!(
            matches!(
                w.owed.as_slice(),
                [Effect::Ask(l), Effect::Follow] if l.id == "software-web-name"
            ),
            "{:?}",
            w.owed
        );
        // A FILE IS CHOSEN in the view's picker, not typed.
        let mut w = way();
        assert_eq!(r.command("software-icon-path", None, &mut w), Taken::Yes);
        assert!(
            w.owed.iter().all(|e| !matches!(e, Effect::Ask(_))),
            "{:?}",
            w.owed
        );
        // AND AN ANSWER — a new web app's address — opens the editor on it.
        let (mut r, _) = opened("software-open", None);
        let mut w = way();
        r.answered("software-web-new", "example.org/mail", &mut w);
        assert!(drawn(&r).contains("example.org"), "{}", drawn(&r));
    }

    /// FAILS IF: a letter typed is not the search, or the minibuffer's line
    /// does not say what was typed — and says it when nothing was.
    #[test]
    fn what_is_typed_is_the_search_and_the_line_says_it() {
        let (mut r, _) = opened("software-open", None);
        assert_eq!(r.prompt(), None, "nothing typed");
        let mut w = way();
        for c in ['g', 'i'] {
            let key = ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Char(c));
            assert_eq!(r.key(key, &mut w), Taken::Yes);
        }
        assert_eq!(r.prompt().as_deref(), Some("gi"));
    }

    /// FAILS IF: a wake that brought nothing asks anything of the door — the
    /// footer would draw and lay itself out again for every empty wake, and
    /// the feed posts three readings with a wake each, the first of which
    /// takes all three.
    #[test]
    fn an_empty_wake_asks_nothing() {
        let (mut r, _) = opened("software-open", None);
        let mut w = way();
        r.woken(&mut w);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
    }

    /// FAILS IF: a job of ours running is not drawn on its row — the queue's
    /// list, heard by the region, does not ask for a frame.
    #[test]
    fn the_queues_list_is_heard() {
        use ronoctua_transfers::{Engine, Job, Stage, Work};
        let (mut r, _) = opened("software-open", None);
        let mut job = Job::fresh_one(
            3,
            Engine::Software,
            Work::Software {
                order: Box::new(ronoctua_software::Order::RefreshPins),
                label: "x".to_owned(),
            },
        );
        job.stage = Stage::Running;
        let mut w = way();
        r.heard(&vec![job], &mut w);
        assert!(w.owed.contains(&Effect::Repaint), "{:?}", w.owed);
        // AND ANY OTHER NEWS IS NOBODY'S HERE.
        let mut w = way();
        r.heard(&42_u8, &mut w);
        assert!(w.owed.is_empty());
        // TOLD AT OPENING, before the view: kept, and no frame for nothing.
        let mut w = way();
        let mut born = SoftwareRegion::born(&mut w);
        born.heard(&vec![job_running()], &mut w);
        assert_eq!(born.reading.readings.jobs.len(), 1);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
    }

    fn job_running() -> ronoctua_transfers::Job {
        let mut job = ronoctua_transfers::Job::fresh_one(
            4,
            ronoctua_transfers::Engine::Software,
            ronoctua_transfers::Work::Software {
                order: Box::new(ronoctua_software::Order::RefreshPins),
                label: "y".to_owned(),
            },
        );
        job.stage = ronoctua_transfers::Stage::Running;
        job
    }
}
