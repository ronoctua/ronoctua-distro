//! THE BELONGINGS IN EITHER DOOR: what the view keeps while a door shows it
//! ([`Open`]), the one handler of what it asks a door to do ([`carry_out`]),
//! and the footer's region ([`BelongingsRegion`], registered as [`ENTRY`]).
//!
//! # One handler, two doors
//!
//! The footer's region and the window (`crate::belongings_window`) hold the
//! same [`Open`] and hand every errand of the view to [`carry_out`], over
//! the door they are (`crate::region::Doorway`): the window's `Ui`, or the
//! footer's recorder. So a refusal said in one door is said in the other, a
//! root gesture goes through each door's own sudo gate, and what a
//! photograph's door reaches is nothing in both ([`Reach::Nowhere`]).
//!
//! # Its threads, in either door
//!
//! The three places' watches, the listing's ears and the count's thread post
//! what they found to the door's loop through one inbox
//! (`crate::region::Inbox`), which wakes the door — `Region::woken` in the
//! footer, `Content::woken` in the window. A photograph's door has no loop:
//! nothing is watched, and the listing is counted in place.

use std::cell::RefCell;
use std::rc::Rc;

use ronoctua_belongings::order::Op;
use ronoctua_belongings::places::Places;
use ronoctua_belongings::status::Offer;

use crate::belongings::{
    Came, Counting, Ears, Errand, OPEN_COMMAND, Press, Readings, View, WINDOW_COMMAND,
};
use crate::region::{Closed, Doorway, Entry, Inbox, Paging, Region, Row, Seen, Taken};

/// The region's id, as the footer's precedence ranks it.
pub const ID: &str = "belongings";

/// The words that open the belongings' window, after the binary's name.
const WINDOW_WORDS: &str = "belongings";

/// Where the orders of a door go.
#[derive(Debug)]
pub enum Reach {
    /// The transfer queue — and a "yes" to an offer, turned on here.
    Queue,
    /// Nowhere: what would have been sent is written down — a photograph's.
    Nowhere(Vec<Op>),
}

/// THE VIEW WHILE A DOOR SHOWS IT: the view, what it reads and where from,
/// the clock it reads at, the count, and the watches and ears that keep it
/// current — the same in both doors.
pub struct Open {
    /// The view.
    pub view: View,
    /// What it draws.
    pub readings: Readings,
    /// Where it reads from.
    pub places: Option<Places>,
    /// The clock the readings are taken at — a photograph's is fixed.
    now: fn() -> i64,
    /// The zone its dates are drawn in.
    zone: ronoctua_files::when::Zone,
    /// Where the orders go.
    pub reach: Reach,
    /// The count of what goes to the repository.
    counting: Counting,
    /// The offers a photograph said yes to, instead of turning them on.
    accepted: Vec<Offer>,
    /// The keys a photograph asked to pin (their fingerprints), instead of
    /// asking the sudo door.
    pinned: Vec<String>,
    /// The three places' notices, gone with the view.
    _watches: Vec<ronoctua_files::notice::Notice>,
    /// What the listing's files are heard by.
    ears: Ears,
    /// What the threads hand back; `None`: a photograph's door, which hears
    /// nothing and counts in place.
    inbox: Option<Inbox<Came>>,
}

impl std::fmt::Debug for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Open")
            .field("view", &self.view)
            .field("reach", &self.reach)
            .finish_non_exhaustive()
    }
}

impl Open {
    /// The view over the files under `places`, read at `now`'s clock, its
    /// dates in `zone`, its orders going to `reach` — first settled at
    /// `columns` in `texts` (a window settles again at its own width; with
    /// no words at all, on the first row). With a `wake`, the places are
    /// watched and the listing counted on a thread, both handed back through
    /// it; without one, counted in place.
    #[must_use]
    pub fn new(
        places: Option<Places>,
        now: fn() -> i64,
        zone: ronoctua_files::when::Zone,
        reach: Reach,
        wake: Option<crate::wake::Wake>,
        columns: u16,
        texts: Option<&ronoctua_i18n::Texts>,
    ) -> Self {
        let inbox = wake.map(Inbox::new);
        // THE THREE PLACES, watched: a sync that ended, a decision the other
        // door made, a setting another door changed — each moves this view
        // without it being opened again.
        let watches = match (&inbox, places.as_ref()) {
            (Some(inbox), Some(p)) => {
                let post = inbox.post();
                crate::belongings::watch(p, move || post.send(Came::Rang))
            }
            _ => Vec::new(),
        };
        let mut counting = inbox
            .as_ref()
            .map_or_else(Counting::in_place, |i| Counting::through(i.post()));
        let mut readings = Readings::read(places.clone(), now());
        counting.ask(&mut readings);
        let view = texts.map_or_else(View::default, |t| View::fresh(&readings, &zone, columns, t));
        let mut me = Self {
            view,
            readings,
            places,
            now,
            zone,
            reach,
            counting,
            accepted: Vec::new(),
            pinned: Vec::new(),
            _watches: watches,
            ears: Ears::default(),
            inbox,
        };
        me.listen();
        me
    }

    /// Hears the folders of the count that is in.
    fn listen(&mut self) {
        if let Some(inbox) = self.inbox.as_ref() {
            let post = inbox.post();
            self.ears
                .follow(&self.readings, move || post.send(Came::Rang));
        }
    }

    /// The files as they are now — the listing kept until the next count
    /// replaces it, and counted again at once only in place (a thread's
    /// count waits for a watch to ring: a key that only moved the cursor
    /// walks nothing).
    pub fn reread(&mut self) {
        self.readings = Readings::read(self.places.clone(), (self.now)()).keeping(&self.readings);
        self.counting.reread(&mut self.readings);
    }

    /// A watch rang, or the clock: read again, and count again.
    pub fn heard(&mut self) {
        self.readings = Readings::read(self.places.clone(), (self.now)()).keeping(&self.readings);
        self.counting.ask(&mut self.readings);
        self.listen();
    }

    /// What the threads handed back, taken — a watch that rang, a count —
    /// and whether anything came.
    pub fn woken(&mut self) -> bool {
        let came = self.inbox.as_ref().map(Inbox::take).unwrap_or_default();
        let any = !came.is_empty();
        for c in came {
            match c {
                Came::Rang => self.heard(),
                Came::Counted(counted) => {
                    self.counting.came(&mut self.readings, counted);
                    self.listen();
                }
            }
        }
        any
    }

    /// The cursor back on its row, at `columns` in `texts`.
    pub fn settle(&mut self, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.view
            .settle_in(&self.readings, &self.zone, columns, texts);
    }

    /// The rows, at `columns` in `texts`.
    #[must_use]
    pub fn rows(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<crate::belongings::Row> {
        self.view.rows(&self.readings, &self.zone, columns, texts)
    }

    /// The keys that work on the row under the cursor.
    #[must_use]
    pub fn hint(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        self.view.hint(&self.readings, &self.zone, columns, texts)
    }

    /// What a key asks — over THE FILES AS THEY ARE NOW: a decision the other
    /// door made a moment ago is the one this key lays its own over.
    pub fn press(&mut self, press: Press, columns: u16, texts: &ronoctua_i18n::Texts) -> Errand {
        self.reread();
        self.view
            .press(press, &self.readings, &self.zone, columns, texts)
    }

    /// A command of the view's by its name — a leader key, `LEADER :`, the
    /// brain's mark — or, with an `argument`, the answer to the view's own
    /// question under that name.
    pub fn command(
        &mut self,
        id: &str,
        argument: Option<&str>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.reread();
        // AN ANSWER ONLY TO THE VIEW'S OWN QUESTION: the brain's mark runs
        // `belongings-sync` with an empty argument, and that is a command,
        // not a passphrase.
        let answering = self.view.asking().is_some_and(|a| a.command() == id);
        match argument.filter(|_| answering) {
            Some(said) => self.view.answered(id, said, &self.readings, texts),
            None => self
                .view
                .command(id, &self.readings, &self.zone, columns, texts),
        }
    }

    /// What was typed on the door's line for the question `id`.
    pub fn answered(&mut self, id: &str, text: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        self.reread();
        self.view.answered(id, text, &self.readings, texts)
    }

    /// Row `n` was clicked: whether the cursor was already on it — the
    /// second click, which is `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, columns: u16, texts: &ronoctua_i18n::Texts) -> bool {
        let rows = self.rows(columns, texts);
        self.view.clicked(n, &rows)
    }

    /// The moment the session forgets the passphrase it remembers — when
    /// "remembered until" stops being true and the view reads again.
    #[must_use]
    pub const fn kept_until(&self) -> Option<i64> {
        self.readings.kept_until
    }

    /// Whether that moment has passed (a door's second, or its clock).
    #[must_use]
    pub fn forgotten(&self) -> bool {
        self.readings.kept_until.is_some_and(|at| (self.now)() > at)
    }

    /// What would have been sent — a photograph's.
    #[must_use]
    pub fn sent(&self) -> &[Op] {
        match &self.reach {
            Reach::Queue => &[],
            Reach::Nowhere(sent) => sent,
        }
    }

    /// The offers a photograph said yes to.
    #[must_use]
    pub fn accepted(&self) -> &[Offer] {
        &self.accepted
    }

    /// The keys a photograph asked to pin.
    #[must_use]
    pub fn pinned(&self) -> &[String] {
        &self.pinned
    }
}

/// CARRIES OUT what the view asked for, in the door `way` is — the ONE
/// handler both doors call. What it wrote is read back at once (the watch
/// rings for it too, and finding nothing new then is free), and the page
/// follows the cursor, whatever brought the gesture: a question's page is a
/// few rows, and the page the list was scrolled to would draw none of them.
pub fn carry_out(errand: Errand, open: &mut Open, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Ask(line) => way.ask(line),
        Errand::Say(said) => way.say(said, crate::echo::Echo::Long),
        // A WORD THAT CONFIRMS WHAT THE HAND DID: a glance (`Echo::Brief`).
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, crate::echo::Echo::Brief);
        }
        // THE KEYS' YES is a root gesture: asked in the footer's sudo bar,
        // through this door's gate.
        Errand::Accept(at_s, offer) => match (&open.reach, open.places.clone()) {
            (Reach::Queue, Some(places)) => {
                crate::belongings::accept_in_background(places, at_s, offer, way.sudo());
            }
            _ => open.accepted.push(offer),
        },
        // THE PIN is a root gesture: asked in the footer's sudo bar, through
        // this door's gate. A photograph asks no door: it writes it down.
        Errand::Pin { key, fingerprint } => match (&open.reach, open.places.clone()) {
            (Reach::Queue, Some(places)) => {
                crate::belongings::pin_in_background(places, key, fingerprint, way.sudo());
            }
            _ => open.pinned.push(fingerprint),
        },
        Errand::Queue(op) => {
            let sent = match &mut open.reach {
                Reach::Queue => crate::belongings::queue(op, open.places.as_ref()),
                Reach::Nowhere(sent) => {
                    sent.push(op);
                    Ok(())
                }
            };
            match sent {
                Ok(()) => {
                    let said = crate::belongings::queued_said(op, way.texts());
                    way.say(said, crate::echo::Echo::Normal);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "the transfer queue did not take the order");
                    let said = way.texts().t(crate::key::TRANSFERS_OFFLINE);
                    way.say(said, crate::echo::Echo::Long);
                }
            }
        }
    }
    // WHAT THIS DOOR WROTE, read back at once — the watch rings for it too.
    open.reread();
    open.settle(way.columns(), way.texts());
    way.page(Paging::Follow);
    way.repaint();
}

/// THE BELONGINGS IN THE FOOTER'S REGION (`LEADER s v v`).
pub struct BelongingsRegion {
    open: Open,
    /// Moves with every change of the view — what [`BelongingsRegion::drawn`]
    /// is kept against.
    generation: u64,
    /// The rows last built, at their width and generation: the footer asks
    /// each row what it does (`Region::about`) right after the rows, and
    /// building the whole list again per row would be the listing squared.
    drawn: RefCell<Option<Drawn>>,
}

/// Rows built at a width (`.0`) for a generation of the view (`.1`).
type Drawn = (u16, u64, Rc<Vec<crate::belongings::Row>>);

impl BelongingsRegion {
    /// The view, to be changed — and the rows kept for it no longer its own.
    const fn open(&mut self) -> &mut Open {
        self.generation = self.generation.wrapping_add(1);
        &mut self.open
    }

    /// The rows at `seen`'s width, built now and kept for the questions that
    /// follow them.
    fn rows_at(&self, seen: &Seen<'_>) -> Rc<Vec<crate::belongings::Row>> {
        if let Some((columns, generation, rows)) = self.drawn.borrow().as_ref()
            && *columns == seen.columns
            && *generation == self.generation
        {
            return Rc::clone(rows);
        }
        let rows = Rc::new(self.open.rows(seen.columns, seen.texts));
        *self.drawn.borrow_mut() = Some((seen.columns, self.generation, Rc::clone(&rows)));
        rows
    }

    /// An errand of the view's, carried out — the window's handler.
    fn carry(&mut self, errand: Errand, way: &mut dyn Doorway) {
        carry_out(errand, self.open(), way);
    }

    /// A key of the view — from the keyboard, the hint or a second click.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let columns = way.columns();
        let errand = self.open().press(press, columns, way.texts());
        self.carry(errand, way);
    }

    /// A command of the view's by its name, or the answer to its question
    /// under that name.
    fn by_name(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        let columns = way.columns();
        let errand = self.open().command(id, argument, columns, way.texts());
        self.carry(errand, way);
        Taken::Yes
    }
}

impl Region for BelongingsRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// Every row a click that selects and a second that acts — a heading
    /// none.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        // FRESH ON EVERY FRAME: the rows say times, which move with no
        // change of the view's.
        *self.drawn.borrow_mut() = None;
        self.rows_at(seen)
            .iter()
            .map(|r| Row {
                fields: r.fields.clone(),
                takes: !r.heading(),
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.hint(seen.columns, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.open.view.cursor())
    }

    /// What its `RET` does, said under the pointer.
    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        View::about(&self.rows_at(seen), row, seen.texts)
    }

    /// Read by the whole notation through the reader the window calls too
    /// (`belongings::press_of_key`).
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match crate::belongings::press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// The view, its window, and every command of the view — a key, a row,
    /// the brain's mark.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // A SECOND OPENING keeps the live view and its cursor.
            OPEN_COMMAND => Taken::Yes,
            WINDOW_COMMAND => {
                way.open_window(vec![WINDOW_WORDS.to_owned()]);
                Taken::Yes
            }
            // THE ONE THE LEADER BINDS (`LEADER s v s`), by its name: the rest
            // of the view's commands come from its rows and its keys.
            crate::belongings::SYNC_COMMAND => self.by_name(id, argument, way),
            other if crate::belongings::is_command(other) => self.by_name(other, argument, way),
            _ => Taken::No,
        }
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let columns = way.columns();
        if self.open().clicked(row, columns, way.texts()) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let errand = self.open().answered(id, text, way.texts());
        self.carry(errand, way);
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.open().view.let_go();
        way.repaint();
    }

    /// A watch rang or a count came back: drawn where the page was — data
    /// arriving never moves it to the cursor.
    fn woken(&mut self, way: &mut dyn Doorway) {
        if self.open().woken() {
            let columns = way.columns();
            self.open().settle(columns, way.texts());
            way.repaint();
        }
    }

    /// THE SESSION FORGOT THE PASSPHRASE: on the footer's second, the view
    /// reads again and "remembered until" goes — no file says the marker ran
    /// out. (The window, which has no second, arms a clock at the moment.)
    fn tick(&mut self, _: std::time::Instant, way: &mut dyn Doorway) -> bool {
        if !self.open.forgotten() {
            return false;
        }
        self.open().heard();
        let columns = way.columns();
        self.open().settle(columns, way.texts());
        true
    }

    /// THE CURSOR MAY BE BORN BELOW THE FIRST PAGE — on the "no" of a
    /// question that came by itself — and is shown where it is.
    fn shown(&mut self, way: &mut dyn Doorway) {
        way.page(Paging::Follow);
    }
}

/// What a command does to the belongings while their region is closed:
/// their window opens on its own; every other command of the view opens the
/// region first and is then the open view's.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        WINDOW_COMMAND => {
            way.open_window(vec![WINDOW_WORDS.to_owned()]);
            Closed::Taken
        }
        OPEN_COMMAND | crate::belongings::SYNC_COMMAND => Closed::Opens,
        other if crate::belongings::is_command(other) => Closed::Opens,
        _ => Closed::NotMine,
    }
}

/// Opens the belongings in the footer's region: their files read, watched
/// while it is open, and the listing counted on a thread — a photograph's
/// footer watches nothing, counts in place and sends nothing past the door.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let photographing = way.photographing();
    let reach = if photographing {
        Reach::Nowhere(Vec::new())
    } else {
        Reach::Queue
    };
    let open = Open::new(
        Places::from_env(),
        crate::belongings::now_s,
        ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        reach,
        (!photographing).then(|| way.wake()),
        way.columns(),
        Some(way.texts()),
    );
    Box::new(BelongingsRegion {
        open,
        generation: 0,
        drawn: RefCell::new(None),
    })
}

/// The belongings, as the binary registers them in the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed() -> i64 {
        1_700_000_000
    }

    fn places(name: &str) -> Places {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-region-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::under(&root);
        std::fs::create_dir_all(&places.config).expect("config");
        crate::keep::store_of(&places)
            .write(&ronoctua_belongings::prefs::Belongings::with_repository(
                "git@example.org:someone/b.git",
            ))
            .expect("prefs");
        ronoctua_consent::let_hosts_under(&places.config, &["example.org"], true).expect("consent");
        std::fs::create_dir_all(places.keys()).expect("keys");
        std::fs::write(places.deploy_key(), "k").expect("key");
        places
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn region(places: &Places) -> BelongingsRegion {
        let texts = texts();
        BelongingsRegion {
            open: Open::new(
                Some(places.clone()),
                fixed,
                ronoctua_files::when::Zone::utc(),
                Reach::Nowhere(Vec::new()),
                None,
                80,
                Some(&texts),
            ),
            generation: 0,
            drawn: RefCell::new(None),
        }
    }

    fn way() -> crate::region::Recorder {
        crate::region::Recorder::new(
            Rc::new(texts()),
            80,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: the footer's region does not ask the passphrase on the
    /// door's line, or its answer does not reach the view — or the order
    /// that follows does not reach the door's queue (a photograph's: written
    /// down). The same flow the window's test walks: one handler.
    #[test]
    fn the_region_asks_and_the_order_leaves_from_it() {
        let places = places("sync");
        let mut r = region(&places);
        let mut w = way();
        assert!(
            r.command(crate::belongings::SYNC_COMMAND, None, &mut w)
                .took()
        );
        let line = w
            .owed
            .iter()
            .find_map(|e| match e {
                crate::region::Effect::Ask(line) => Some(line.clone()),
                _ => None,
            })
            .expect("the passphrase is not asked");
        assert!(line.secret, "the passphrase is shown as it is typed");
        assert!(
            w.owed
                .contains(&crate::region::Effect::Page(Paging::Follow)),
            "the page does not follow the gesture"
        );
        let mut w = way();
        r.answered(line.id, "a passphrase", &mut w);
        let again = w
            .owed
            .iter()
            .find_map(|e| match e {
                crate::region::Effect::Ask(line) => Some(line.clone()),
                _ => None,
            })
            .expect("a first sync did not ask it again");
        r.answered(again.id, "a passphrase", &mut way());
        assert_eq!(r.open.sent(), [Op::Prepare]);
        assert!(places.passphrase().is_file());
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the rows the footer draws and the row under the pointer's
    /// sentence disagree with the view's, a heading takes a click, or a
    /// change of the view leaves the rows kept for the old one.
    #[test]
    fn the_region_draws_the_views_rows() {
        let places = places("rows");
        let mut r = region(&places);
        let texts = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 80,
            lines: 20,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let rows = r.rows(&seen);
        let views = r.open.rows(80, &texts);
        assert_eq!(rows.len(), views.len());
        for (n, (row, view)) in rows.iter().zip(&views).enumerate() {
            assert_eq!(row.takes, !view.heading(), "row {n}");
            assert_eq!(r.about(n, &seen), View::about(&views, n, &texts));
        }
        let before = r.generation;
        let _ = r.key(
            ronoctua_keymap::Key::parse("<down>").expect("a key"),
            &mut way(),
        );
        assert_ne!(r.generation, before, "a key kept the old rows");
        assert_eq!(r.cursor(), Some(r.open.view.cursor()));
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the window's command is not its window, opened from either
    /// state, or a command that is not the belongings' is taken.
    #[test]
    fn the_window_and_the_view_are_its_commands() {
        let mut w = way();
        assert_eq!(closed(WINDOW_COMMAND, None, &mut w), Closed::Taken);
        assert_eq!(
            w.owed,
            vec![crate::region::Effect::OpenWindow(vec![
                WINDOW_WORDS.to_owned()
            ])]
        );
        assert_eq!(closed(OPEN_COMMAND, None, &mut way()), Closed::Opens);
        assert_eq!(
            closed(crate::belongings::SYNC_COMMAND, None, &mut way()),
            Closed::Opens
        );
        assert_eq!(closed("stick-open", None, &mut way()), Closed::NotMine);
    }
}
