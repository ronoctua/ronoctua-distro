//! THE CALCULATOR OPEN IN A DOOR — what both of its doors keep and do
//! ([`Calc`], [`carry_out`]) — and its region of the footer
//! ([`CalcRegion`], [`ENTRY`]).
//!
//! # One errand handler, two doors
//!
//! A press of the calculator answers an [`Errand`]: keep an answer, copy it,
//! read the clipboard, fetch the rates, let their host. The window and the
//! footer's region used to carry each errand out in code of their own, and
//! the two had drifted — a refusal to keep an answer read to the end in one
//! door and gone in the other. Now both hand the errand to [`carry_out`],
//! over their [`Doorway`], and what differs is only what a door IS: the
//! window floats (`C-f`), and the footer, which has no place of its own to
//! float, opens the window ([`Errand::Float`] is handed back to the door).
//!
//! # Threads, and the door's wake
//!
//! The rates are fetched on a thread and `calc.json` is watched on another;
//! both leave what they found on the calculator's own channel and wake the
//! door ([`Doorway::wake`]), whose loop then hands it in ([`Calc::woken`]).
//! A door that closed takes the channel with it.

use std::sync::mpsc;

use crate::calc::{Errand, Fetched, Press, Target, View};
use crate::region::{Closed, Doorway, Entry, Lights, Lit, Region, Row, Seen, Taken};

/// The region's id, as the footer ranks it.
pub const ID: &str = "calc";

/// What a thread of the calculator leaves for its door.
#[derive(Debug)]
enum Back {
    /// The rates fetch ended.
    Rates(Fetched),
    /// `calc.json` changed: another door kept an answer.
    Changed,
}

/// THE CALCULATOR OPEN IN A DOOR: the view, the file its history is kept in
/// and the channel its threads answer on — the same in the window and in the
/// footer's region.
pub struct Calc {
    /// The view.
    pub view: View,
    /// Where the history is read from and written to: the person's
    /// `calc.json`, or a photograph's copy.
    store: ronoctua_store::Store<crate::calc::Kept>,
    /// A photograph's window: nothing leaves the door, and what a press
    /// would have done past it is printed instead (`would have …`).
    kept: bool,
    /// Where the threads leave what they found.
    back: mpsc::Sender<Back>,
    /// And where the door takes it ([`Calc::woken`]).
    heard: mpsc::Receiver<Back>,
}

impl std::fmt::Debug for Calc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Calc")
            .field("kept", &self.kept)
            .finish_non_exhaustive()
    }
}

impl Calc {
    /// The calculator over `view`, keeping its answers in `store`; `kept`
    /// for a photograph's, which reaches nothing past its door.
    #[must_use]
    pub fn over(view: View, store: ronoctua_store::Store<crate::calc::Kept>, kept: bool) -> Self {
        let (back, heard) = mpsc::channel();
        Self {
            view,
            store,
            kept,
            back,
            heard,
        }
    }

    /// Whether this is a photograph's calculator.
    #[must_use]
    pub const fn is_kept(&self) -> bool {
        self.kept
    }

    /// Now, for the view: the machine's clock — or, for a photograph, the
    /// same moment on every machine, one answer after the other.
    #[must_use]
    pub fn now_ms(&self) -> u64 {
        if self.kept {
            return 1_790_769_600_000 + u64::try_from(self.view.kept()).unwrap_or(0);
        }
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0))
    }

    /// One press — a key, a card, a key of the hint — and its errand,
    /// carried out. What is left is the door's own ([`carry_out`]).
    pub fn press(&mut self, press: Press, way: &mut dyn Doorway) -> Option<Errand> {
        let now = self.now_ms();
        self.view.at(now);
        let errand = self.view.press(press, way.texts());
        carry_out(errand, self, way)
    }

    /// A click on what a line holds — a key, a mode, an answer, a unit.
    pub fn click(&mut self, target: Target, way: &mut dyn Doorway) -> Option<Errand> {
        let now = self.now_ms();
        self.view.at(now);
        let errand = self.view.clicked(target, way.texts());
        carry_out(errand, self, way)
    }

    /// The clipboard's text it asked for ([`Errand::Paste`]), onto the line.
    pub fn pasted(&mut self, text: &str, way: &mut dyn Doorway) -> Option<Errand> {
        self.view.pasted(text, way.texts());
        carry_out(Errand::Nothing, self, way)
    }

    /// The file changed: the history is what it says — read with `parsed`,
    /// and a file that will not parse keeps the copy held. Whether it moved.
    pub fn reread(&mut self) -> bool {
        let Some(kept) = self.store.parsed() else {
            return false;
        };
        if kept.history.as_slice() == self.view.history() {
            return false;
        }
        self.view.history_is(kept.history);
        true
    }

    /// A thread woke the door: what it left, handed in. Whether anything
    /// changed.
    pub fn woken(&mut self, way: &mut dyn Doorway) -> bool {
        let mut changed = false;
        while let Ok(back) = self.heard.try_recv() {
            match back {
                Back::Rates(got) => {
                    self.view.fetched(got, way.texts());
                    changed = true;
                }
                Back::Changed => changed |= self.reread(),
            }
        }
        changed
    }

    /// A watch on `calc.json` in `dir` — the directory the door found and
    /// HANDS in (`persistence-takes-its-root-as-a-parameter`), made first: a
    /// watch on a folder that does not exist never fires. An answer the
    /// other door keeps is in this history at once: the file is the
    /// broadcast.
    #[must_use]
    pub fn watch(
        &self,
        dir: &std::path::Path,
        wake: crate::wake::Wake,
    ) -> Option<ronoctua_files::notice::Notice> {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(error = %e, "no configuration directory to watch calc.json in");
            return None;
        }
        let back = self.back.clone();
        ronoctua_files::notice::Notice::on_file(
            dir,
            crate::calc::FILE,
            ronoctua_files::notice::Pace::Coalesced,
            move || {
                let sent = back.send(Back::Changed).is_ok();
                if sent {
                    wake.call();
                }
                sent
            },
        )
        .ok()
    }

    /// An answer kept: merged into the file, and the history is the file.
    /// What could not be written is SAID on the door where `RET` was
    /// pressed — the LONG class: it is read to the end.
    fn keep(&mut self, entry: crate::calc::Entry, way: &mut dyn Doorway) {
        match crate::calc::keep_in(&self.store, entry) {
            Ok(history) => self.view.history_is(history),
            Err(crate::calc::Unkept::Unparsed) => {
                let said = way.texts().t_with(
                    crate::key::CALC_UNKEPT_UNPARSED,
                    &[("file", crate::calc::FILE)],
                );
                way.say(said, crate::echo::Echo::Long);
            }
            Err(crate::calc::Unkept::NotWritten(why)) => {
                tracing::warn!(error = %why, "an answer could not be kept");
                let said = way.texts().t_with(
                    crate::key::CALC_UNKEPT,
                    &[("file", crate::calc::FILE), ("why", &why.to_string())],
                );
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }

    /// The rates are wanted and nobody looked: the file and the consent, read
    /// now — and a fetch when the host may be asked.
    fn look_for_rates(&mut self, way: &mut dyn Doorway) {
        if !self.view.to_look(way.texts().decimal_mark()) {
            return;
        }
        let probe = if self.kept {
            crate::calc::Probe {
                cached: None,
                allowed: false,
            }
        } else {
            crate::calc::probe()
        };
        if self.view.looked(probe, way.texts()) == Errand::Fetch {
            self.fetch(way);
        }
    }

    /// The rates, asked for on a thread; what comes back wakes the door.
    /// A photograph's footer has no loop to hear it on, and reaches nothing
    /// of the machine: it asks nothing.
    fn fetch(&self, way: &mut dyn Doorway) {
        if self.kept {
            println!(
                "would have asked {} for the rates",
                ronoctua_calc::rates::HOST
            );
            return;
        }
        if way.photographing() {
            return;
        }
        let back = self.back.clone();
        let wake = way.wake();
        crate::calc::fetch_then(move |got| {
            if back.send(Back::Rates(got)).is_ok() {
                wake.call();
            }
        });
    }

    /// The row that names the host was pressed: that is the consent, written
    /// merged — and the rates are asked for.
    fn allow(&mut self, way: &mut dyn Doorway) {
        let host = ronoctua_calc::rates::HOST;
        if self.kept {
            println!("would have let {host}");
            self.view.allowed();
            return;
        }
        match crate::consent_rows::allow(&[host], way.texts()) {
            Ok(()) => {
                self.view.allowed();
                self.fetch(way);
            }
            // EVERY APP'S SENTENCE for a press that could not be kept
            // (`consent_rows::unkept`), on this door's line.
            Err(said) => way.say(said, crate::echo::Echo::Long),
        }
    }
}

/// WHAT THE CALCULATOR ASKED, carried out in either door — the one errand
/// handler the window and the footer's region share. What it hands back is
/// the door's own: [`Errand::Float`], which only a window can do.
pub fn carry_out(errand: Errand, calc: &mut Calc, way: &mut dyn Doorway) -> Option<Errand> {
    let left = match errand {
        Errand::Nothing => None,
        Errand::Close => {
            way.close();
            return None;
        }
        Errand::Keep(entry) => {
            calc.keep(entry, way);
            None
        }
        Errand::Copy(text) => {
            if calc.kept {
                println!("would have copied: {text}");
            } else {
                let said = crate::chrome::copied(&text, way.texts());
                way.say(said, crate::echo::Echo::Normal);
            }
            None
        }
        Errand::Paste => {
            way.paste();
            None
        }
        Errand::Fetch => {
            calc.fetch(way);
            None
        }
        Errand::Allow => {
            calc.allow(way);
            None
        }
        Errand::Float => Some(Errand::Float),
    };
    // A MODE, AN ANSWER KEPT OR A LIST FOUND changes what the door holds.
    calc.look_for_rates(way);
    way.repaint();
    left
}

/// THE CALCULATOR IN THE FOOTER'S REGION (`LEADER = =`).
#[derive(Debug)]
pub struct CalcRegion {
    calc: Calc,
    /// The watch on `calc.json`, while the region is open: it goes with it.
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl CalcRegion {
    /// The footer has no place of its own to float: the window is the door
    /// that floats, and `C-f` opens it — compact, floating.
    fn left(left: Option<Errand>, way: &mut dyn Doorway) {
        if left == Some(Errand::Float) {
            way.run("calc-window", None);
        }
    }
}

impl Region for CalcRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// The view's lines, each a click on what it holds — a key, a mode, an
    /// answer kept, a unit — found by the cell under the pointer
    /// (`calc::Line::target_at`), the ruler the lit field uses.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.calc
            .view
            .footer_lines(seen.columns, seen.texts)
            .into_iter()
            .map(|l| Row {
                takes: l.targets.iter().any(Option::is_some) || !l.bands.is_empty(),
                fields: l.fields,
            })
            .collect()
    }

    /// Its mode's keys, the converter's in Convert (`calc::hint`, the table
    /// the window's header reads too).
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        crate::calc::hint(
            self.calc.view.taught(seen.texts.decimal_mark()),
            false,
            seen.texts,
        )
    }

    fn cursor(&self) -> Option<usize> {
        None
    }

    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        self.calc.view.footer_cursor(seen.columns, seen.texts)
    }

    fn caret(&self, row: usize, seen: &Seen<'_>) -> Option<u16> {
        self.calc
            .view
            .footer_lines(seen.columns, seen.texts)
            .get(row)
            .and_then(|l| l.caret)
    }

    /// Every key is its own, read by the whole notation through the reader
    /// the window calls too (`calc::press_of_key`); one it does not take is
    /// swallowed, as every modal swallows the keyboard it holds.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = crate::calc::press_of_key(key) else {
            return Taken::No;
        };
        let left = self.calc.press(press, way);
        Self::left(left, way);
        Taken::Yes
    }

    /// Its window's door. Its OPENING, run again while it is open, is not
    /// taken: the host refuses it as every opening is refused while the
    /// region is held, with the same sentence (`region_is_free`) — the
    /// gesture another view answers that way too, as the footer did when it
    /// held the calculator itself. What is typed stays, and the window's
    /// answers arrive by the history's watch (`Calc::reread`).
    fn command(&mut self, id: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // THE CALCULATOR IN A WINDOW OF ITS OWN (`LEADER = w`), compact
            // and floating — the same view, in the door that has a place.
            "calc-window" => {
                way.open_window(vec!["calc".to_owned()]);
                Taken::Yes
            }
            // `LEADER = =` — the command that opened it, handed over as it
            // opens, with nothing left to do. AGAIN, with the calculator
            // open, it is the footer's to refuse, as every opening is while
            // the region is held (`Footer::region_is_free`) — so it is never
            // taken here.
            "calc-open" => Taken::No,
            _ => Taken::No,
        }
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// The cell under the pointer says what the line's press reaches — a
    /// key, a mode, an answer kept, a unit (the line's first cell, with no
    /// pointer to say).
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let target = self
            .calc
            .view
            .footer_lines(way.columns(), way.texts())
            .get(row)
            .and_then(|l| l.target_at(cell.unwrap_or(0)));
        match target {
            Some(t) => {
                let left = self.calc.click(t, way);
                Self::left(left, way);
            }
            None => way.repaint(),
        }
    }

    /// THE BAND IS WHAT A PRESS THERE TAKES: a row of its lists whole, a key
    /// or a mode alone — and nothing where a press takes nothing
    /// (`calc::Line::band_at`, which the click reads too through
    /// `target_at`).
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Lit {
        cell.and_then(|c| {
            self.calc
                .view
                .footer_lines(seen.columns, seen.texts)
                .get(row)
                .and_then(|l| l.band_at(c))
        })
        .map_or(Lit::Nothing, |(from, to)| Lit::Cells(from, to))
    }

    /// THE ACCENT IS THE MODE IN USE AND THE RING'S CATEGORY, and the
    /// pointer's mark is the band over what a press takes: the field under
    /// it is not lit apart.
    fn lights(&self) -> Lights {
        Lights::Kin
    }

    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let left = self.calc.pasted(text, way);
        Self::left(left, way);
    }

    fn woken(&mut self, way: &mut dyn Doorway) {
        if self.calc.woken(way) {
            way.repaint();
        }
    }
}

/// What a command does with the calculator closed.
fn closed(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "calc-open" => Closed::Opens,
        "calc-window" => {
            way.open_window(vec!["calc".to_owned()]);
            Closed::Taken
        }
        _ => Closed::NotMine,
    }
}

/// `calc-open`: the calculator, over the history `calc.json` keeps, and a
/// watch on the file while it is open.
fn open(_command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let store = crate::calc::store();
    let history = store.read().history;
    let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
    let zoneinfo = std::env::var_os("RONOCTUA_ZONEINFO").map(std::path::PathBuf::from);
    let calc = Calc::over(View::fresh(history, zone, zoneinfo), store, false);
    // THE DIRECTORY FOUND HERE AND HANDED IN
    // (`persistence-takes-its-root-as-a-parameter`) — and none in a
    // photograph's footer, which has no loop to hear a watch on.
    let watch = if way.photographing() {
        None
    } else {
        ronoctua_store::config_dir().and_then(|d| calc.watch(&d, way.wake()))
    };
    Box::new(CalcRegion {
        calc,
        _watch: watch,
    })
}

/// The calculator, as the binary registers it with the footer.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn texts() -> std::rc::Rc<ronoctua_i18n::Texts> {
        std::rc::Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"))
    }

    fn way() -> Recorder {
        Recorder::new(
            texts(),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn a_room(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-calc-region-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a room");
        dir
    }

    fn region(dir: &std::path::Path) -> CalcRegion {
        let store = ronoctua_store::Store::at(Some(dir.to_path_buf()), crate::calc::FILE);
        let view = View::fresh(Vec::new(), ronoctua_files::when::Zone::utc(), None);
        CalcRegion {
            calc: Calc::over(view, store, false),
            _watch: None,
        }
    }

    fn typed(r: &mut CalcRegion, keys: &[&str], w: &mut Recorder) {
        for k in keys {
            let key = ronoctua_keymap::Key::parse(k).expect("a key");
            assert_eq!(r.key(key, w), Taken::Yes, "{k} was not the calculator's");
        }
    }

    /// FAILS IF: an answer `RET` keeps in the footer's region is not in the
    /// file the window reads — the two doors would keep two histories.
    #[test]
    fn an_answer_kept_in_the_region_is_in_the_file() {
        let dir = a_room("kept");
        let mut r = region(&dir);
        let mut w = way();
        typed(&mut r, &["2", "+", "3", "RET"], &mut w);
        let store: ronoctua_store::Store<crate::calc::Kept> =
            ronoctua_store::Store::at(Some(dir.clone()), crate::calc::FILE);
        assert_eq!(store.read().history.len(), 1, "nothing was kept");
        assert_eq!(r.calc.view.history().len(), 1);
        assert!(w.owed.contains(&Effect::Repaint));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the footer's `C-f` does what only a window can do, or
    /// nothing at all — it is the calculator's window, opened by its command.
    #[test]
    fn the_footers_float_opens_the_window() {
        let dir = a_room("float");
        let mut r = region(&dir);
        let mut w = way();
        typed(&mut r, &["C-f"], &mut w);
        assert!(
            w.owed.contains(&Effect::Run("calc-window", None)),
            "{:?}",
            w.owed
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the region's command opens a second window, the closed
    /// calculator does not open on its command, or the OPEN one takes its
    /// own opening — the host would not refuse it with the sentence every
    /// other view's second opening says.
    #[test]
    fn the_commands_open_the_region_and_the_window() {
        let mut w = way();
        assert_eq!(closed("calc-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("calc-window", None, &mut w), Closed::Taken);
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["calc".to_owned()])]);
        let dir = a_room("again");
        let mut r = region(&dir);
        let mut w = way();
        assert_eq!(r.command("calc-open", None, &mut w), Taken::No);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the band is not the stretch a press takes — a key alone,
    /// nothing where nothing is taken — or the field under the pointer is
    /// lit apart from it.
    #[test]
    fn the_band_is_what_a_press_takes() {
        let dir = a_room("band");
        let r = region(&dir);
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 80,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let lines = r.calc.view.footer_lines(seen.columns, seen.texts);
        let (n, cell) = lines
            .iter()
            .enumerate()
            .find_map(|(n, l)| (0..80).find(|c| l.band_at(*c).is_some()).map(|c| (n, c)))
            .expect("a line that takes a press");
        let (from, to) = lines[n].band_at(cell).expect("a band");
        assert_eq!(r.lit(n, Some(cell), &seen), Lit::Cells(from, to));
        assert_eq!(r.lit(n, None, &seen), Lit::Nothing);
        assert_eq!(r.lit(lines.len(), Some(cell), &seen), Lit::Nothing);
        assert_eq!(r.lights(), Lights::Kin);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
