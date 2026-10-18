//! THE CLIPBOARD'S HISTORY OPEN IN A DOOR — what both of its doors keep and
//! do ([`Clip`], [`carry_out`]) — and its region of the footer
//! ([`ClipboardRegion`], [`ENTRY`]).
//!
//! # The history is the footer's; the view is the door's
//!
//! One watcher of the clipboard, one history: the footer keeps it, and both
//! doors FOLLOW it — the window over the control socket, the region through
//! what the footer tells the regions it hosts (the history [`Held`], shared,
//! in [`Region::heard`]) — and ACT on it the same way, by an [`Act`] aimed at
//! the history they were shown ([`Doorway::clipboard`]): the window's goes
//! down the socket, the region's is done by the footer once the region is
//! back in its slot. What a door keeps of its own is what it SHOWS: the
//! view's cursor, the code of an entry, its picture decoded, a login half
//! typed.
//!
//! # One errand handler, two doors
//!
//! What the view asks is carried out by [`carry_out`] in both — the window
//! and the region had written it twice, and had drifted: a click on a row
//! of the footer's history copied it at once, where the window's first
//! click selects and the second acts (invariant 19).

use std::sync::Arc;

use crate::clipboard_view::{Errand, Facts, Press, Shows, Target, View};
use crate::clipboard_wire::{Act, Held};
use crate::region::{
    Closed, Doorway, Entry, Painted, Region, RegionEasel, Row, Seen, Source, Taken,
};
use ronoctua_tokens as tokens;

/// The region's id, as the footer ranks it.
pub const ID: &str = "clipboard";

/// The key entry `n`'s picture goes up under, in history `generation`: a
/// new history is a new picture.
#[must_use]
pub fn picture_key(generation: u64, n: usize) -> String {
    format!("clipboard:{generation}:{n}")
}

/// THE CLIPBOARD'S HISTORY OPEN IN A DOOR: the last history the footer
/// showed it and what the door shows of it — the same in the window and in
/// the footer's region.
#[derive(Debug, Default)]
pub struct Clip {
    /// The view: its cursor, the code it shows.
    pub view: View,
    /// The history, for the view — the footer's own, shared, in the region
    /// (`clipboard_wire::Held`); the one the socket brought, in the window.
    queue: std::rc::Rc<ronoctua_clipboard::Queue>,
    /// ClipCascade's reading, for the view.
    sync: ronoctua_clipcascade::Snapshot,
    /// Which history it is — what an act is aimed at.
    generation: u64,
    /// Whether a history has arrived at all.
    heard: bool,
    /// Whether the footer went away (or was never there).
    gone: bool,
    /// The shown entry's code, while `q` shows one — by the entry it is of.
    qr: Option<(usize, ronoctua_clipboard::qr::Matrix)>,
    /// The shown entry's picture, decoded — by the entry it is of.
    picture: Option<(usize, Arc<ronoctua_render::image::Rgba>)>,
    /// A login being typed: the server, then the account.
    login: Option<(String, Option<String>)>,
}

impl Clip {
    /// What the rows are drawn from.
    #[must_use]
    pub fn facts(&self) -> Facts<'_> {
        Facts {
            queue: &self.queue,
            sync: &self.sync,
        }
    }

    /// Whether a history has arrived.
    #[must_use]
    pub const fn heard(&self) -> bool {
        self.heard
    }

    /// Whether the footer went away.
    #[must_use]
    pub const fn gone(&self) -> bool {
        self.gone
    }

    /// The history it was shown.
    #[must_use]
    pub fn queue(&self) -> &ronoctua_clipboard::Queue {
        &self.queue
    }

    /// Which history it was shown.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The code shown, by the entry it is of.
    #[must_use]
    pub const fn qr(&self) -> Option<&(usize, ronoctua_clipboard::qr::Matrix)> {
        self.qr.as_ref()
    }

    /// A login is being typed.
    #[must_use]
    pub const fn logging_in(&self) -> bool {
        self.login.is_some()
    }

    /// A whole history from the footer: the entries, ClipCascade's reading
    /// and which history it is. A NEW HISTORY is a new list under the same
    /// cursor: what the code and the picture were of may have moved; a
    /// ClipCascade reading moves no entry, and leaves them.
    pub fn took(
        &mut self,
        queue: std::rc::Rc<ronoctua_clipboard::Queue>,
        sync: ronoctua_clipcascade::Snapshot,
        generation: u64,
    ) {
        let moved = generation != self.generation || !self.heard;
        self.queue = queue;
        self.sync = sync;
        self.generation = generation;
        self.heard = true;
        self.gone = false;
        if moved {
            self.qr = None;
            self.view.no_qr();
            self.picture = None;
        }
    }

    /// The footer is not there to follow.
    pub const fn lost(&mut self) {
        self.gone = true;
    }

    /// The entry the cursor is on, when it is on one.
    fn entry_here(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        match self.view.target_at(&self.facts(), columns, texts) {
            Target::Entry(n) => Some(n),
            _ => None,
        }
    }

    /// One press — from the keyboard, the hint, the name of its command, or
    /// `RET` from a second click.
    pub fn pressed(&mut self, press: Press, way: &mut dyn Doorway) {
        let columns = way.columns();
        let before = self.entry_here(columns, way.texts());
        let errand = {
            let facts = Facts {
                queue: &self.queue,
                sync: &self.sync,
            };
            self.view.key(press, &facts, columns, way.texts())
        };
        // THE CODE BELONGED TO THE ENTRY THE CURSOR LEFT.
        if self.entry_here(columns, way.texts()) != before && self.view.qr().is_none() {
            self.qr = None;
        }
        carry_out(errand, self, way);
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click on
    /// the row DRAWN as the cursor is `RET` (`View::here`, the row `RET`
    /// acts on — never the raw cursor, which a fresh view keeps on the
    /// heading: compared with that, a click on the heading acted on the row
    /// drawn below it, and one on the row drawn as the cursor only selected
    /// it). A row that answers no key — a heading, a fact, the air between —
    /// takes nothing, as a row past the list does, or a list not heard yet.
    pub fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        if !self.heard {
            return;
        }
        let facts = self.facts();
        if !self.view.acts_at(&facts, way.columns(), way.texts(), row) {
            return;
        }
        let here = self.view.here(&facts, way.columns(), way.texts());
        if row == here {
            self.pressed(Press::Enter, way);
        } else {
            self.view.clicked(row);
            if self.view.qr().is_none() {
                self.qr = None;
            }
            self.decode_shown(way.columns(), way.texts());
            way.repaint();
        }
    }

    /// What was typed on the door's line, under the footer's own ids: a
    /// login's three answers, one at a time, and the typed settings.
    pub fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let said = text.trim();
        match id {
            "clipboard-server" if !said.is_empty() => {
                self.login = Some((said.to_owned(), None));
                way.ask(crate::line::Line::new("clipboard-account", false, None));
            }
            "clipboard-account" if !said.is_empty() => {
                if let Some((server, _)) = self.login.take() {
                    self.login = Some((server, Some(said.to_owned())));
                    // THE ONLY MASKED ONE IN THIS FLOW. A server address and
                    // an account are typed to be READ BACK and corrected; a
                    // password is not.
                    way.ask(crate::line::Line::new("clipboard-password", true, None));
                }
            }
            // THE PASSWORD IS NOT KEPT: it goes into the act, which the
            // footer hands to ClipCascade, and leaves this door with it.
            "clipboard-password" => {
                if let Some((server, Some(username))) = self.login.take() {
                    way.clipboard(
                        Act::LogIn {
                            server,
                            username,
                            password: text.to_owned(),
                        },
                        self.generation,
                    );
                }
            }
            "clipboard-salt" | "clipboard-rounds" | "clipboard-cap" => way.clipboard(
                Act::Typed {
                    id: id.to_owned(),
                    text: said.to_owned(),
                },
                self.generation,
            ),
            _ => {}
        }
        way.repaint();
    }

    /// The line was let go: a login half typed is forgotten.
    pub fn unanswered(&mut self, way: &mut dyn Doorway) {
        self.login = None;
        way.repaint();
    }

    /// The rows the view draws — and, before the first history, above
    /// them, the sentence that says why there is none: the footer's history
    /// has not come yet, or there is no footer to follow.
    #[must_use]
    pub fn rows(
        &self,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Vec<crate::fields::Field>> {
        let mut rows: Vec<Vec<crate::fields::Field>> = self
            .view
            .rows(&self.facts(), columns, texts)
            .into_iter()
            .map(|r| r.fields)
            .collect();
        if !self.heard {
            rows.insert(
                0,
                vec![crate::fields::Field {
                    at: crate::chrome::INSET,
                    text: texts.t(if self.gone {
                        crate::key::CLIPBOARD_WINDOW_NO_FOOTER
                    } else {
                        crate::key::CLIPBOARD_WINDOW_WAITING
                    }),
                    color: tokens::color::TEXT_DIM,
                    hit: crate::fields::Hit::Nothing,
                    explain: None,
                    ink: crate::fields::Ink::Grid,
                }],
            );
        }
        rows
    }

    /// The row the cursor is on, among [`Clip::rows`].
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.view.cursor() + if self.heard { 0 } else { 1 }
    }

    /// Whether the view's picture box has rows on the door's page now.
    fn picture_fits(&self, way: &dyn Doorway) -> bool {
        let page = way.page_shown();
        self.view
            .picture(&self.facts(), way.columns(), way.texts())
            .is_none_or(|p| {
                crate::clipboard_view::picture_rows(p, page.offset, page.lines).is_some()
            })
    }

    /// The shown entry's picture, decoded once per entry — when the box the
    /// view leaves holds an image, and not on every frame: a 4K capture is
    /// 33 MB of RGBA.
    pub fn decode_shown(&mut self, columns: u16, texts: &ronoctua_i18n::Texts) {
        let Some(picture) = self.view.picture(&self.facts(), columns, texts) else {
            return;
        };
        let Shows::Image(n) = picture.shows else {
            return;
        };
        if self.picture.as_ref().is_some_and(|(at, _)| *at == n) {
            return;
        }
        let Some(bytes) = self.queue.at(n).and_then(ronoctua_clipboard::Entry::image) else {
            return;
        };
        let start = std::time::Instant::now();
        let Some(r) = ronoctua_clipboard::image::decode(bytes) else {
            // A format we cannot read, or too big. The entry stays in the
            // history — what is lost is the preview.
            tracing::debug!(bytes = bytes.len(), "the copied image did not decode");
            return;
        };
        tracing::debug!(
            width = r.width,
            height = r.height,
            ms = start.elapsed().as_secs_f64() * 1000.0,
            "preview decoded"
        );
        self.picture = Some((
            n,
            Arc::new(ronoctua_render::image::Rgba {
                width: r.width,
                height: r.height,
                pixels: r.pixels,
            }),
        ));
    }

    /// Entry `n`'s picture, when it is the one decoded: its key, its size
    /// and its pixels.
    #[must_use]
    pub fn picture_of(&self, n: usize) -> Option<(String, Arc<ronoctua_render::image::Rgba>)> {
        let (at, rgba) = self.picture.as_ref().filter(|(at, _)| *at == n)?;
        Some((picture_key(self.generation, *at), Arc::clone(rgba)))
    }
}

/// WHAT THE VIEW ASKED, carried out in either door — the one errand handler
/// the window and the footer's region share: what touches the history is an
/// [`Act`] for the footer, aimed at the history the door was shown; the code
/// is drawn here, from the entry the door holds; the typed ones are asked on
/// the door's own line, under the footer's ids.
pub fn carry_out(errand: Errand, clip: &mut Clip, way: &mut dyn Doorway) {
    let generation = clip.generation;
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Copy(n) => way.clipboard(Act::Copy { n }, generation),
        Errand::Remove(n) => way.clipboard(Act::Remove { n }, generation),
        Errand::Pin(n) => way.clipboard(Act::Pin { n }, generation),
        Errand::Clear => way.clipboard(Act::Clear, generation),
        Errand::Sync(on) => way.clipboard(Act::Sync { on }, generation),
        Errand::LogOut => way.clipboard(Act::LogOut, generation),
        Errand::Keep(settings) => way.clipboard(Act::of_settings(&settings), generation),
        // THE CODE IS DRAWN HERE, from the entry this door holds: it is a
        // picture of text, and nothing about it is the footer's.
        Errand::Qr(n) => {
            clip.qr = if clip.view.qr().is_some() {
                let m = clip
                    .queue
                    .at(n)
                    .and_then(ronoctua_clipboard::Entry::text)
                    .and_then(ronoctua_clipboard::qr::generate);
                if m.is_none() {
                    let said = way.texts().t(crate::key::ECHO_QR_LARGE);
                    way.say(said, crate::echo::Echo::Normal);
                } else if !clip.picture_fits(way) {
                    // A CODE NOBODY CAN SEE IS SAID, never drawn off screen
                    // in silence.
                    let said = way.texts().t(crate::key::CLIPBOARD_NO_ROOM);
                    way.say(said, crate::echo::Echo::Normal);
                }
                m.map(|m| (n, m))
            } else {
                None
            };
        }
        Errand::AskServer => way.ask(crate::line::Line::new("clipboard-server", false, None)),
        Errand::AskAccount => way.ask(crate::line::Line::new("clipboard-account", false, None)),
        Errand::AskSalt => way.ask(crate::line::Line::new("clipboard-salt", false, None)),
        Errand::AskRounds => way.ask(crate::line::Line::new("clipboard-rounds", false, None)),
        Errand::AskCap => way.ask(crate::line::Line::new("clipboard-cap", false, None)),
    }
    clip.decode_shown(way.columns(), way.texts());
    way.repaint();
}

/// THE CLIPBOARD'S HISTORY IN THE FOOTER'S REGION (`LEADER c c`).
#[derive(Debug, Default)]
pub struct ClipboardRegion {
    clip: Clip,
}

impl Region for ClipboardRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// Its rows carry BOTH halves — the history on the left and the item's
    /// facts on the right, as fields at known columns — and the click is the
    /// row, which the view turns into a target.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.clip
            .rows(seen.columns, seen.texts)
            .into_iter()
            .map(|fields| Row {
                fields,
                takes: self.clip.heard,
            })
            .collect()
    }

    /// On the minibuffer's line, elided with an ellipsis, the rest reachable
    /// through the fold, like every other view's.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.clip.view.hint(seen.texts)
    }

    /// THE ROW THE VIEW DREW — entries, then the sync's rows.
    fn cursor(&self) -> Option<usize> {
        Some(self.clip.cursor())
    }

    /// A key, read by its whole notation in the view's own table — the
    /// window's reader — with the shift kept (`S-<left>` turns a ring) and
    /// `Esc` through the view, so a code on screen is put away before the
    /// view closes. A key that is not in the table does nothing, and does
    /// not end the view: ending on a wrong key would lose the place.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let press = if key.sym == ronoctua_keymap::Sym::Esc {
            Some(Press::Escape)
        } else {
            let notation = ronoctua_keymap::Key {
                mods: ronoctua_keymap::Mods {
                    shift: key.mods.shift,
                    ..ronoctua_keymap::Mods::default()
                },
                sym: key.sym,
            }
            .notation();
            crate::clipboard_view::OF_THE_VIEW
                .iter()
                .find(|(t, _)| *t == notation)
                .and_then(|(_, id)| crate::clipboard_view::press_of(id))
        };
        match press {
            Some(press) => {
                self.clip.pressed(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// Its keys' commands, by their names — the same presses the keys make.
    /// Its OPENING, run again while it is open, is not taken: the host
    /// refuses it as every opening is refused while the region is held, with
    /// the same sentence (`region_is_free`), as the footer did when it held
    /// the history itself.
    fn command(&mut self, id: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // `LEADER c c` — the command that opened it, handed over as it
            // opens, with nothing left to do. AGAIN, with the history open,
            // it is the footer's to refuse, as every opening is while the
            // region is held (`Footer::region_is_free`) — so it is never
            // taken here.
            "clipboard-open" => return Taken::No,
            // ITS WINDOW: the second screen of this history.
            "clipboard-window" => way.open_window(vec!["clipboard".to_owned()]),
            _ => match crate::clipboard_view::press_of(id) {
                Some(press) => self.clip.pressed(press, way),
                None => return Taken::No,
            },
        }
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clip.clicked(row, way);
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.clip.answered(id, text, way);
    }

    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.clip.unanswered(way);
    }

    /// THE FOOTER'S HISTORY, whole and shared, every time it moves — and
    /// when the region opens. What the footer says of an act is said on its
    /// own line, which is this door's.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        let Some(held) = news.downcast_ref::<Held>() else {
            return;
        };
        // THE OPENING, SAID WITH WHAT IT OPENED ON: the footer tells the
        // region its history as it opens (`standing_readings`), and the
        // journal's line carries how many entries it holds — what the
        // clipboard's VM test reads (`clipboard-vm`, `inputs=2`).
        if !self.clip.heard() {
            tracing::info!(inputs = held.queue.how_many(), "clipboard opened");
        }
        self.clip.took(
            std::rc::Rc::clone(&held.queue),
            held.sync.clone(),
            held.generation,
        );
        self.clip.decode_shown(way.columns(), way.texts());
        // THE ROWS MAY HAVE COME OR GONE — an entry copied, one removed — so
        // the height follows and the page stays inside them; data that
        // arrives does not move the page to the cursor.
        way.rows_changed();
    }

    /// The vertical rule between its two columns, the code's modules, and
    /// the entry's picture — in the box the view left for them, at the rows
    /// it left (`clipboard_view::picture_area`, which the window asks too).
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        let first = easel.area.first;
        let columns = tokens::metrics::columns(easel.area.width, size);
        let x = f32::from(crate::clipboard_view::history_columns(columns).saturating_add(1)) * cell;
        easel.under.push(crate::chrome::divider(
            x,
            f32::from(first) * line,
            easel.area.bottom,
        ));
        if !self.clip.heard {
            return;
        }
        let Some(box_) =
            self.clip
                .view
                .picture(&self.clip.facts(), easel.area.columns, easel.texts)
        else {
            return;
        };
        let Some(area) = crate::clipboard_view::picture_area(
            box_,
            easel.offset,
            usize::from(easel.area.lines),
            crate::chrome::FIELDS_LEAD,
            cell,
            |l| f32::from(first + l) * line,
        ) else {
            return;
        };
        match box_.shows {
            Shows::Qr(n) => {
                if let Some((_, m)) = self.clip.qr.as_ref().filter(|(at, _)| *at == n) {
                    easel.under.extend(crate::clipboard_view::qr_quads(m, area));
                }
            }
            // THE PICTURE OF THE ENTRY THE BOX IS FOR, and no other: one
            // decoded for another entry is not drawn.
            Shows::Image(n) => {
                if let Some((key, rgba)) = self.clip.picture_of(n) {
                    let at = crate::sizes::picture_in(area, rgba.width, rgba.height);
                    easel.pictures.push(Painted {
                        key,
                        area: at,
                        // SHARED, never cloned: up only when it is not on
                        // the GPU yet.
                        source: Source::Still(rgba),
                    });
                }
            }
        }
    }
}

/// What a command does with the history's view closed: it opens it, or its
/// window; a key's command reached by name opens the view and is pressed in
/// it.
fn closed(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "clipboard-open" => Closed::Opens,
        "clipboard-window" => {
            way.open_window(vec!["clipboard".to_owned()]);
            Closed::Taken
        }
        _ if crate::clipboard_view::press_of(command).is_some() => Closed::Opens,
        _ => Closed::NotMine,
    }
}

/// The view, fresh: the footer tells it its history as it opens — and the
/// opening is said then, with the history's size (`Region::heard`).
fn open(_command: &str, _argument: Option<&str>, _way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(ClipboardRegion::default())
}

/// The clipboard's history, as the binary registers it with the footer.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        Recorder::new(
            std::rc::Rc::new(texts),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
        .with_page(crate::region::Page {
            offset: 0,
            lines: 20,
        })
    }

    fn told(r: &mut ClipboardRegion, entries: &[&str], generation: u64, w: &mut Recorder) {
        let held = Held {
            queue: std::rc::Rc::new(ronoctua_clipboard::Queue::of_entries(
                entries
                    .iter()
                    .map(|e| ronoctua_clipboard::Entry::of_text(*e)),
            )),
            sync: ronoctua_clipcascade::Snapshot::default(),
            generation,
        };
        r.heard(&held, w);
    }

    /// FAILS IF: a key in the footer's history does something to a list of
    /// its own instead of asking the footer's — the act names the entry and
    /// the history it was shown — or the history it is told says anything
    /// on the footer's line by itself.
    #[test]
    fn a_key_in_the_region_asks_the_footers_history() {
        let mut w = way();
        let mut r = ClipboardRegion::default();
        told(&mut r, &["one", "two"], 7, &mut w);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Say(_, _))),
            "the history it was told said something: {:?}",
            w.owed
        );
        let down = ronoctua_keymap::Key::parse("<down>").expect("a key");
        let enter = ronoctua_keymap::Key::parse("RET").expect("a key");
        assert_eq!(r.key(down, &mut w), Taken::Yes);
        assert_eq!(r.key(enter, &mut w), Taken::Yes);
        assert!(
            w.owed.contains(&Effect::Clipboard(Act::Copy { n: 1 }, 7)),
            "{:?}",
            w.owed
        );
    }

    /// The row the cursor is drawn on.
    fn drawn(r: &ClipboardRegion, w: &Recorder) -> usize {
        r.clip.view.here(&r.clip.facts(), w.columns(), w.texts())
    }

    /// Whether anything was asked of the footer's history.
    fn acted(w: &Recorder) -> bool {
        w.owed.iter().any(|e| matches!(e, Effect::Clipboard(..)))
    }

    /// FAILS IF: the first click on a row acts — invariant 19: it selects,
    /// and the second click is `RET`, in both doors (the window's click is
    /// this one) — a click on the row DRAWN as the cursor is not `RET`, or a
    /// click on the `History` heading, which answers no key, acts on the row
    /// drawn below it or moves the cursor.
    #[test]
    fn the_first_click_selects_and_the_second_acts() {
        let mut w = way();
        let mut r = ClipboardRegion::default();
        told(&mut r, &["one", "two", "three"], 2, &mut w);
        w.owed.clear();
        let here = drawn(&r, &w);
        assert_ne!(here, 0, "the cursor is drawn on the heading");
        r.clicked(0, &mut w);
        r.clicked(0, &mut w);
        assert!(!acted(&w), "a click on the heading acted: {:?}", w.owed);
        assert_eq!(
            drawn(&r, &w),
            here,
            "a click on the heading moved the cursor"
        );
        let other = here + 1;
        r.clicked(other, &mut w);
        assert_eq!(drawn(&r, &w), other, "the click did not select");
        assert!(!acted(&w), "the first click acted: {:?}", w.owed);
        r.clicked(other, &mut w);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Clipboard(Act::Copy { .. }, 2))),
            "the second click did not act: {:?}",
            w.owed
        );

        // THE ROW DRAWN AS THE CURSOR, on a history just shown: `RET`.
        let mut w = way();
        let mut r = ClipboardRegion::default();
        told(&mut r, &["one", "two"], 3, &mut w);
        w.owed.clear();
        let here = drawn(&r, &w);
        r.clicked(here, &mut w);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Clipboard(Act::Copy { .. }, 3))),
            "a click on the row drawn as the cursor did not act: {:?}",
            w.owed
        );

        // AN EMPTY HISTORY: the cursor is drawn on the server's row, and a
        // click there asks for the server; one on the heading asks nothing.
        let mut w = way();
        let mut r = ClipboardRegion::default();
        told(&mut r, &[], 4, &mut w);
        w.owed.clear();
        r.clicked(0, &mut w);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Ask(_))),
            "a click on the heading asked: {:?}",
            w.owed
        );
        let here = drawn(&r, &w);
        r.clicked(here, &mut w);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Ask(l) if l.id == "clipboard-server")),
            "a click on the server's row, drawn as the cursor, did not ask for it: {:?}",
            w.owed
        );
    }

    /// FAILS IF: a login typed in the footer leaves the password anywhere
    /// but the one act that carries it, or asks on another line than the
    /// door's.
    #[test]
    fn the_login_is_asked_on_the_doors_line_and_sent_once() {
        let mut w = way();
        let mut r = ClipboardRegion::default();
        told(&mut r, &[], 3, &mut w);
        carry_out(Errand::AskServer, &mut r.clip, &mut w);
        for said in ["https://clip.example.org", "someone", "s3nh4"] {
            let line = w
                .owed
                .iter()
                .rev()
                .find_map(|e| match e {
                    Effect::Ask(l) => Some(l.id),
                    _ => None,
                })
                .expect("a question");
            w.owed.clear();
            r.answered(line, said, &mut w);
        }
        assert!(
            w.owed.contains(&Effect::Clipboard(
                Act::LogIn {
                    server: "https://clip.example.org".to_owned(),
                    username: "someone".to_owned(),
                    password: "s3nh4".to_owned(),
                },
                3
            )),
            "{:?}",
            w.owed
        );
        assert!(!r.clip.logging_in(), "the login was kept");
    }

    /// FAILS IF: a key's command run by name with the view shut is no one's,
    /// opens the window instead of the view, or the OPEN view takes its own
    /// opening — the host would not refuse it with the sentence every other
    /// view's second opening says.
    #[test]
    fn the_commands_open_the_view_or_the_window() {
        let mut w = way();
        assert_eq!(closed("clipboard-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("clipboard-remove", None, &mut w), Closed::Opens);
        assert_eq!(closed("clipboard-window", None, &mut w), Closed::Taken);
        assert_eq!(closed("calc-open", None, &mut w), Closed::NotMine);
        assert_eq!(
            w.owed,
            vec![Effect::OpenWindow(vec!["clipboard".to_owned()])]
        );
        let mut r = ClipboardRegion::default();
        let mut w = way();
        told(&mut r, &["one"], 1, &mut w);
        w.owed.clear();
        assert_eq!(r.command("clipboard-open", None, &mut w), Taken::No);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
    }
}
