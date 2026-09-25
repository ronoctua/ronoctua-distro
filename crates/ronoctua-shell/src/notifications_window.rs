//! The notification manager's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14) and it is right that it does: it is a footer. Twelve rows are
//! enough to see what just happened and act on it, and not enough to read a
//! fortnight of history or to go through every filter the distro ships with.
//!
//! So this exists — and everything it knows lives in
//! [`crate::notifications::View`], which the footer draws from too. What this
//! module holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the same arrangement, and the same header, as
//! `files_window.rs`.
//!
//! # Where its data comes from
//!
//! From the same two files the footer reads: the history under
//! `$XDG_STATE_HOME` and the person's filters under `$XDG_CONFIG_HOME` (see
//! `ronoctua_notify::store`). It re-reads them on a beat rather than sharing
//! memory with the footer — they are two processes, and a socket for a list
//! that changes a few times an hour would be a mechanism to keep in step for
//! nothing.

use crate::door::{Canvas, Content, Door, Hand, Ui};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::timer,
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

/// The window's `app_id`.
///
/// Its own, like the file manager's and the terminal's: it is by this that the
/// modeline and the VM test tell one window from another.
pub const APP_ID: &str = "br.ronoctua.notifications";

/// How often the window re-reads what the footer wrote.
///
/// Two seconds. The history changes when something happens, which on a desktop
/// is a few times an hour; polling faster would spend a wake-up to find the
/// same file, and polling slower would make the window feel stale next to the
/// band that raised the notification.
const REREAD: std::time::Duration = std::time::Duration::from_secs(2);

/// Opens the manager in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        w.handle
            .insert_source(
                timer::Timer::from_duration(REREAD),
                |_, (), door: &mut Door<Notifications>| {
                    if door.content.reread() {
                        door.ui.ask_paint();
                    }
                    timer::TimeoutAction::ToDuration(REREAD)
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the clock: {e}"))?;
        let mut content = Notifications {
            // THE MACHINE'S CLOCK, read once and carried — an open record
            // shows when it arrived, and a `View` that read `/etc/localtime`
            // itself would be a `View` no test could pin to a date.
            view: crate::notifications::View::fresh().in_zone(
                ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            ),
            records: Vec::new(),
            book: ronoctua_notify::Book::distro(),
            notes: ronoctua_notify::store::Where::of_env(),
            pointed: None,
            pointers: std::collections::HashMap::new(),
            hands: 0,
            click_cell: None,
        };
        content.reread();
        Ok(content)
    })
}

/// The window: the view, and what it reads.
struct Notifications {
    view: crate::notifications::View,
    /// The history, as it was last read off disk.
    records: Vec<ronoctua_notify::Record>,
    /// The catalogue, the distro's filters and the person's.
    book: ronoctua_notify::Book,
    /// Where this process persists — the two directories, found ONCE.
    ///
    /// A SEPARATE PROCESS from the footer, with its own copy of both files;
    /// carrying the roots rather than finding them per write is what keeps a
    /// test from reaching the real home. See `sidebar::Sidebar`'s `store` for
    /// the incident that made it a rule.
    notes: ronoctua_notify::store::Where,
    /// The row and cell under the pointer that spoke LAST — what the message
    /// line says about the row under the pointer (`View::about_at`).
    pointed: Option<(usize, Option<u16>)>,
    /// Every pointer's row and cell, by the pointer's own number — for the
    /// hover bands, which light only the COLUMN a pointer is in.
    pointers: std::collections::HashMap<u32, (Option<usize>, Option<u16>)>,
    /// The last number handed to a pointer.
    hands: u32,
    /// The cell of the last press — `View::clicked` needs it and the row
    /// alone cannot say it.
    click_cell: Option<u16>,
}

/// What this window keeps for ONE pointer.
#[derive(Debug, Default)]
struct Pointed {
    /// Which pointer this is, given on its first frame — `0` is "not yet".
    number: u32,
    /// Which CELL along the row under it, which the message line needs and
    /// the click already asked for.
    ///
    /// Side by side one drawn row is two records, one per column, so the row
    /// alone cannot say what the pointer is over — see
    /// `notifications::View::about_at`.
    cell: Option<u16>,
}

impl Notifications {
    /// Re-reads what the footer wrote; `true` when it changed.
    ///
    /// Compares before drawing: repainting on a beat that found nothing new is
    /// a frame spent to put the same pixels back.
    fn reread(&mut self) -> bool {
        let fresh = self.notes.history();
        let prefs = self.notes.preferences();
        // COMPARED AFTER `took`, not before it. A filter that has come back as
        // one of the distro's leaves `mine` — so the file and the book DIFFER
        // by design from that moment on, and comparing the raw file against
        // the book would say "changed" on every one of these ticks, for ever.
        let mut candidate = ronoctua_notify::Book::distro();
        candidate.took(prefs.rules, prefs.shared);
        candidate.adjustments = prefs.adjustments;
        if fresh == self.records
            && candidate.mine == self.book.mine
            && candidate.shared == self.book.shared
            && candidate.adjustments == self.book.adjustments
        {
            return false;
        }
        self.records = fresh;
        self.book = candidate;
        true
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::notifications::Row> {
        let lists = crate::notifications::Lists::of(&self.records, &self.book);
        self.view.rows(&lists, &self.book, ui.columns, &ui.texts)
    }

    /// Keeps the cursor on screen.
    ///
    /// NOT THE DOOR'S OWN `follow`, which follows an index into the rows:
    /// this view's cursor is not always one.
    fn follow(&self, ui: &mut Ui) {
        // THE SHEET IS NOT A LIST, and its rows are not the cursor's.
        //
        // With an entry open the drawn rows are the KNOBS — eight of them —
        // while `View::cursor()` is still the CATALOGUE's index, which can be
        // thirty. In a short window that pinned the sheet to its last rows,
        // and moving between knobs scrolled nothing at all. The sheet is
        // short by construction, so there is nothing to follow: the offset
        // goes home and stays there.
        if self.view.in_sheet() {
            ui.offset = 0;
            return;
        }
        // AN OPEN RECORD IS NOT A LIST EITHER, but it can be taller than the
        // window — that is why the level exists — so it DOES follow: what it
        // follows is the line the arrows moved, not a cursor.
        //
        // THE DRAWN LINE OF THE CURSOR, asked of the view — which is what
        // knows how many titles are above it. Adding one for "the heading" was
        // the arrangement written down as a constant, and narrow stacks the
        // two columns, so there are two.
        let (cursor, rows) = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            (
                self.view
                    .visual_cursor(&lists, &self.book, ui.columns, &ui.texts),
                self.view
                    .rows(&lists, &self.book, ui.columns, &ui.texts)
                    .len(),
            )
        };
        ui.follow(cursor, rows);
    }

    /// Which cell of the row is at `x`.
    ///
    /// The content's margin comes off first: the row's own cells start after
    /// it, and counting from the window's edge would put every target one
    /// margin to the right of where it is drawn.
    fn cell_at(ui: &Ui, x: f64) -> Option<u16> {
        let cell = f64::from(tokens::metrics::cell_width(tokens::text::size()));
        if cell <= 0.0 || x < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (x / cell) as u16;
        which.checked_sub(ui.margin)
    }

    /// Whether cell `c` is in the RIGHT column, when there are two.
    fn on_the_right(ui: &Ui, c: Option<u16>) -> bool {
        crate::notifications::two_columns(ui.columns)
            && c.is_some_and(|c| c >= crate::notifications::split_at(ui.columns))
    }

    fn apply(&mut self, ui: &mut Ui, errand: crate::notifications::Errand) {
        use crate::notifications::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            // THE WINDOW WRITES THE SAME FILES THE FOOTER READS. It is not a
            // second store: `store::write_history` is one function, and the
            // footer WATCHES both files by inotify — the same "the file is
            // the broadcast" the theme uses — so a record forgotten here is
            // forgotten there within the event loop's next turn.
            //
            // It used to say the footer picked the change up "on its next
            // write or restart", and that was the opposite of what happened:
            // the footer read the history once, at start-up, and wrote its
            // whole vector over the file on every change, so this door's `d`
            // came undone the moment any notification arrived.
            Errand::Read(key) => {
                self.set_read(key, true);
            }
            // THE OTHER WAY, and it writes the file just the same. The footer
            // reconciles `read` in BOTH directions now, so a row put back to
            // unread here stays unread there — it used to take only `true`,
            // which would have undone this on the next write.
            Errand::Unread(key) => {
                self.set_read(key, false);
            }
            Errand::ReadAll => {
                // NOT THE SUPPRESSED ONES, for the footer's reason: they are
                // already off the count, `SPC` refuses them one at a time,
                // and marking them read here would be the two doors
                // disagreeing about one word.
                for r in &mut self.records {
                    if !matches!(r.presentation.show, ronoctua_notify::Show::Hidden) {
                        r.read = true;
                    }
                }
                self.save_history();
            }
            Errand::Forget(key) => {
                self.records.retain(|r| r.key() != key);
                self.save_history();
            }
            // AN ACTION IS THE FOOTER'S TO RUN — and this door SENDS it there
            // rather than telling somebody to go and do it themselves.
            //
            // The owner pressed one here and was told the action runs from the
            // footer, which is true and useless: the control socket exists for
            // exactly this, the buffer host already writes to it, and a
            // command of ours is a command id the footer knows how to run.
            //
            // The other three kinds stay the footer's alone and say so: a bus
            // reply belongs to the process that owns the name, the clipboard
            // is the footer's, and spawning into a scope of its own is section
            // 9.1. A second implementation of any of them here is the second
            // door disagreeing with the first.
            Errand::Act(key, which) => {
                let sent = self
                    .records
                    .iter()
                    .find(|r| r.key() == key)
                    .and_then(|r| r.notification.actions.get(which))
                    .and_then(|a| match &a.doing {
                        ronoctua_notify::Doing::Command(id) => Some(id.clone()),
                        _ => None,
                    })
                    .map(|id| crate::leader::send_it(&format!("command {id}")));
                match sent {
                    Some(Ok(())) => {}
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "the footer did not take the action");
                        ui.message = Some(ui.texts.t(ronoctua_i18n::key::NOTIF_ACT_NO_FOOTER));
                    }
                    None => Self::act_in_the_footer(ui),
                }
            }

            // TYPING HAPPENS IN THE FOOTER, and this door says so rather
            // than growing a minibuffer of its own — the same answer it gives
            // about running an action, and the same reason: a second place to
            // type is a second thing that can disagree.
            Errand::Ask(_) => {
                let keys = format!("{} {}", crate::leader(), crate::leader::NOTIF_OPEN_KEYS);
                ui.message = Some(ui.texts.t_with(
                    ronoctua_i18n::key::NOTIF_TYPE_IN_THE_FOOTER,
                    &[("keys", &keys)],
                ));
            }
            // THE PERSON'S OWN FILTERS. This door writes the same file the
            // footer does, and the footer is watching it — the arrangement
            // `Adjusted` below already has.
            Errand::Filter { was, rule } => {
                if let Some(old) = was {
                    self.book.mine.retain(|r| r.id != old);
                }
                self.book.mine.retain(|r| r.id != rule.id);
                self.book.mine.push(rule);
                self.save_the_book();
            }
            Errand::Unfilter(id) => {
                self.book.mine.retain(|r| r.id != id);
                self.book.adjustments.remove(&id);
                self.save_the_book();
            }
            // INTO THE REPOSITORY, one filter at a time, by name, after a
            // screen that said what it means. This writes SOURCE — what makes
            // it part of the distro is the next build, and what publishes it
            // is the owner's own commit, which needs them in front of the
            // machine anyway.
            Errand::Share(id) => {
                let said = self.share_one(&id);
                ui.message = Some(ui.texts.t(said));
            }
            Errand::Adjusted(id, verdict) => {
                match verdict {
                    Some(v) => self.book.adjustments.insert(id, v),
                    None => self.book.adjustments.remove(&id),
                };
                self.save_the_book();
            }
        }
        ui.ask_paint();
    }

    /// Puts one filter of the person's into the repository.
    ///
    /// The file is a LIST and this replaces by id, so sharing the same filter
    /// twice leaves one line rather than two. It is written in the table's own
    /// shape (`ronoctua_notify::as_table`) — the shape `assets/notifications.nix`
    /// uses and the generator already validates — because somebody will open
    /// that file, and serde's `{"Contains": "backup"}` is exactly the thing the
    /// owner said was not an interface.
    ///
    /// It returns the text key to put on the message line.
    fn share_one(&mut self, id: &str) -> &'static str {
        let Some(rule) = self.book.mine.iter().find(|r| r.id == id).cloned() else {
            return ronoctua_i18n::key::NOTIF_SHARE_FAILED;
        };
        let shared = ronoctua_store::Shared::of(ronoctua_store::repo_dir());
        if !shared.is_open() {
            return ronoctua_i18n::key::NOTIF_SHARE_NOWHERE;
        }
        let mut held: Vec<serde_json::Value> =
            shared.read(crate::notifications::SHARED_NOTIFICATIONS);
        held.retain(|it| it.get("id").and_then(serde_json::Value::as_str) != Some(id));
        held.push(ronoctua_notify::as_table(&rule));
        if let Err(e) = shared.write(crate::notifications::SHARED_NOTIFICATIONS, &held) {
            tracing::warn!(error = %e, id, "the filter could not be shared");
            return ronoctua_i18n::key::NOTIF_SHARE_FAILED;
        }
        // AND THE SCREEN REMEMBERS IT IS TRAVELLING. The copy on this machine
        // keeps working until a build brings it back as one of the distro's —
        // see `Book::took`, which is where it retires.
        self.book.shared.insert(id.to_owned());
        self.save_the_book();
        tracing::info!(id, "a filter was written into the repository");
        ronoctua_i18n::key::NOTIF_SHARED_WRITTEN
    }

    /// Says the action is the footer's to run, and where the footer is.
    ///
    /// THE KEYS ARE AN ARGUMENT, read from the same constant the keymap binds:
    /// a sentence that carried `n h` as text went stale the day the key moved.
    /// `leader()` and not `leader_chord()`: the sequence is typed after the
    /// LEADER (`SPC`, or `CTRL ALT SPC` in the nested loop).
    fn act_in_the_footer(ui: &mut Ui) {
        let keys = format!("{} {}", crate::leader(), crate::leader::NOTIF_OPEN_KEYS);
        ui.message = Some(ui.texts.t_with(
            ronoctua_i18n::key::NOTIF_ACT_IN_THE_FOOTER,
            &[("keys", &keys)],
        ));
    }

    /// Writes the person's half of the filter book: their filters and their
    /// adjustments.
    fn save_the_book(&mut self) {
        let prefs = ronoctua_notify::Preferences {
            rules: self.book.mine.clone(),
            shared: self.book.shared.clone(),
            adjustments: self.book.adjustments.clone(),
        };
        if let Err(e) = self.notes.save_preferences(&prefs) {
            tracing::warn!(error = %e, "could not write the preferences");
        }
    }

    /// Turns one record's `read` on or off, and writes the file.
    ///
    /// BY THE RECORD'S NAME and not by the protocol's id: the id repeats across
    /// sessions and inside one, so `id ==` acted on every record that had ever
    /// been given that number — and the list being newest first, the extra ones
    /// were the rows BELOW the one under the cursor.
    fn set_read(&mut self, key: ronoctua_notify::RecordKey, read: bool) {
        for r in self.records.iter_mut().filter(|r| r.key() == key) {
            r.read = read;
        }
        self.save_history();
    }

    fn save_history(&mut self) {
        if let Err(e) = self.notes.save_history(&mut self.records) {
            tracing::warn!(error = %e, "could not write the history");
        }
    }
}

impl Content for Notifications {
    type Hand = Pointed;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    /// `*name*` is Emacs's convention for a buffer that is not a file, the
    /// same one the modeline already uses for `*scratch*`.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::NOTIF_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        let Some(press) = press_of(event) else {
            return;
        };
        // A key clears the last message: it answered something that is no
        // longer what is being done.
        ui.message = None;
        let errand = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view.key(press, &lists, &self.book, ui.columns)
        };
        // THE CURSOR MOVED, so the list follows it — here, on the EVENT, and
        // never in the paint.
        self.follow(ui);
        self.apply(ui, errand);
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A CLICK IS THE `RET` OF THE ROW IT LANDS ON — `View::clicked` answers
    /// with the same errand the key would (`open_here`), invariant 19.
    ///
    /// THE DRAWN ROW, headings and all: which of them is an item of which
    /// column is `View::place_of`, the one plan the drawing walked. And THE
    /// CELL TRAVELS WITH THE ROW: a row has two targets on it — the envelope
    /// and the bin — so the row alone cannot say what was clicked.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let errand = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view
                .clicked(row, self.click_cell, &lists, &self.book, ui.columns)
        };
        self.apply(ui, errand);
    }

    // THE RIGHT BUTTON DOES NOTHING HERE. There is no menu in this view:
    // every action it offers is one key, and a second meaning for the same
    // target would be chrome nobody asked for.

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Pointed>, events: &[PointerEvent]) {
        if hand.extra.number == 0 {
            self.hands = self.hands.wrapping_add(1).max(1);
            hand.extra.number = self.hands;
        }
        // THE CELL, from the same events the standard list reads for the row.
        let side_before = Self::on_the_right(ui, hand.extra.cell);
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    hand.extra.cell = Self::cell_at(ui, e.position.0);
                }
                PointerEventKind::Press { .. } => {
                    hand.extra.cell = Self::cell_at(ui, e.position.0);
                    self.click_cell = hand.extra.cell;
                }
                PointerEventKind::Leave { .. } => hand.extra.cell = None,
                _ => {}
            }
        }
        crate::door::list::pointer(self, ui, hand, events);
        let now = (hand.row_under_pointer, hand.extra.cell);
        self.pointers.insert(hand.extra.number, now);
        self.pointed = hand.row_under_pointer.map(|r| (r, hand.extra.cell));
        // AND A NEW COLUMN IS A NEW PICTURE: side by side the band lights the
        // half the pointer is in, and the row alone did not change.
        if side_before != Self::on_the_right(ui, hand.extra.cell) {
            ui.ask_paint();
        }
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        // LINE 0: what the last action said, the focused row's own note, or
        // the keys — the footer's minibuffer, through the other door.
        //
        // The message and the note win over the keys for the reason the footer
        // gives on its own line: they are what the manager has to SAY about
        // something just done, and the keys are always there to come back to.
        // The note is the same source as the footer's — the reason a switch
        // does nothing — so the two doors say the same sentence in the same
        // place.
        let said = ui
            .message
            .clone()
            .or_else(|| self.view.note(&ui.texts))
            // AND WHAT THE ROW UNDER THE POINTER HAS TO ADD: when the
            // notification arrived, and the whole of a sentence the row had to
            // cut. The footer's own line takes it from the same method, so the
            // two doors cannot say different things about the same row.
            .or_else(|| {
                let (row, cell) = self.pointed?;
                let lists = crate::notifications::Lists::of(&self.records, &self.book);
                self.view.about_at(&lists, ui.columns, row, cell, &ui.texts)
            });
        // A MESSAGE TAKES THIS LINE, and then there is no shortcut on it to
        // point at. Asked BEFORE the list is measured, because the
        // explanation's row comes out of that measurement.
        if said.is_some() {
            ui.hinted.say(None);
        }
        let (pieces, hinting) = said.map_or_else(
            || {
                // THE KEYS, in two registers, from the SAME table the footer's
                // minibuffer reads. AND WHERE IN THE LIST THE CURSOR IS: this
                // window draws no rectangle for the cursor, so with a
                // fortnight of records there was no sign that anything lay
                // below the last drawn row.
                let lists = crate::notifications::Lists::of(&self.records, &self.book);
                let place = Some((self.view.cursor(), lists.len(self.view.pane())));
                (self.view.hint_of(place, &lists, &ui.texts), true)
            },
            |text| (vec![crate::chrome::Piece::said(text)], false),
        );
        let (margin, columns) = (ui.margin, ui.columns);
        let texts = ui.texts.clone();
        canvas
            .spans
            .extend(ui.hinted.head(&pieces, hinting, margin, columns, &texts));
        canvas.over.extend(ui.hinted.bar(ui.frame(0)));

        // THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
        // draws for these rows, and the one `record_line` argues for: with
        // two columns on screen a band across the row would say the two
        // items on it were one thing.
        //
        // THE LIST WITHOUT ITS HOVER, which is drawn below by column.
        let rows = self.rows(ui);
        let lit = canvas.lit;
        canvas.lit = &[];
        crate::door::list::body(ui, canvas, &rows);
        canvas.lit = lit;

        // EVERY CURSOR'S HOVER, in its own colour — see `crate::seats`. Side
        // by side a drawn line is TWO records, one per column, so a band
        // lights only the column the pointer is in: a band across the whole
        // line would say both records are under it. And to the BAR, never
        // under it.
        let cell = ui.frame(0).cell();
        let split = f32::from(margin + crate::notifications::split_at(columns)) * cell;
        let two = crate::notifications::two_columns(columns);
        let width = ui.room.band_width();
        // ONE BAND PER ROW AND HALF, so two cursors on one line but in
        // different columns are two bands, and on the same half the first
        // wins.
        let mut seen: Vec<(usize, bool)> = Vec::new();
        for &(color, row) in lit {
            let Some(row) = row else {
                continue;
            };
            let mut halves: Vec<bool> = self
                .pointers
                .values()
                .filter(|(r, _)| *r == Some(row))
                .map(|(_, c)| Self::on_the_right(ui, *c))
                .collect();
            if halves.is_empty() {
                halves.push(false);
            }
            for right in halves {
                if seen.contains(&(row, right)) {
                    continue;
                }
                seen.push((row, right));
                for mut band in crate::chrome::hover_bands(
                    std::iter::once((color, Some(row))),
                    ui.offset,
                    ui.room_for_rows(),
                    rows.len(),
                    ui.first_of_list(),
                    width,
                ) {
                    if two {
                        if right {
                            band.x = split;
                            band.width = (width - split).max(0.0);
                        } else {
                            band.width = split;
                        }
                    }
                    canvas.under.push(band);
                }
            }
        }
    }
}

/// A `KeyEvent` as the manager sees it.
///
/// The same mapping the footer does — see `Footer::manager_key`. Two doors
/// answering one key differently is the defect this whole arrangement exists to
/// prevent, so the table is the same table.
fn press_of(event: &KeyEvent) -> Option<crate::notifications::Press> {
    use crate::notifications::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Tab => Press::Tab,
        // `ISO_Left_Tab` IS `S-TAB`: xkb has already applied the shift, and
        // this is the one keysym it gives back for it.
        Keysym::ISO_Left_Tab => Press::BackTab,
        Keysym::space => Press::Space,
        Keysym::Left => Press::Left,
        Keysym::Right => Press::Right,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        otherwise => Press::Letter(otherwise.key_char()?),
    })
}
