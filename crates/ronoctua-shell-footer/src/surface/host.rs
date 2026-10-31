//! THE SLOT HOST'S HALF IN THE FOOTER: every region the binary registered
//! (`Composition::regions`, kept in `crate::region::Slots`), asked by the
//! footer's dispatchers by its id — one line of each — and what a region asks
//! of the footer's door ([`crate::region::Effect`]) done once it is back in
//! its slot.
//!
//! # Why nothing is done inside a call
//!
//! A region is lent out of its slot for the call (`Slots::call`), and the
//! doorway it is handed is a [`crate::region::Recorder`]: it answers what it
//! was given — the words, the width, the gate, the wake — and writes down the
//! rest. The footer does it after the call, with the region back: nothing
//! re-enters the footer from inside a region, and `view()` is never asked in
//! the middle of one.
//!
//! # What the geometry may read
//!
//! Whether a region is open and the shape it had when it last came back
//! (`Slots::is_open`, `Slots::kept_shape`) are stored facts, which the rows'
//! arithmetic under `Footer::metric` may read. Its rows, hint, cursor and
//! paint are its CONTENT, and only the methods `metric` never reaches ask
//! for them (`the-footer-does-not-ask-itself`).
//!
//! # A region that lives hidden
//!
//! A command that acts for a view without its page (`Closed::Behind`)
//! brings the region to life HIDDEN, and a region that closes with work in
//! flight stays so: live — its threads heard, its question answered, what
//! the footer hears told to it — but neither ranked nor drawn, until it no
//! longer lingers (`Region::lingers`) or asks for the region
//! (`Doorway::show`), which it gets when the region is free. A hidden one
//! that does not take a command of its own — a closed view with only its
//! work's answers left to say — is as closed as a closed one: its entry
//! answers the command (`Entry::closed`), and an opening steps it aside for a
//! fresh one (`Slots::linger`), told only what its threads bring until it has
//! nothing out.
//!
//! # What a region reads of the machine
//!
//! What the footer keeps of the machine for its own sake — its last
//! readings, the lines it asks the daemons through, the wishes it follows —
//! is lent to every call by type (`Footer::kept_for_regions`,
//! `region::Kept`), and each new reading is told to the open regions as news
//! (`hosted_heard::<T>`, the type named where it is told, so
//! `regions-hear-what-the-footer-tells` can pair it with the type a region's
//! `heard` reads). A line the footer raises only once somebody needs it is
//! raised when a region asks for it (`Effect::Want`,
//! `Footer::raise_for_a_region`). A region that comes to life is also told,
//! as news, what the footer already heard (`hosted_replay`,
//! `Footer::standing`).
//!
//! # What a region reads of the footer's
//!
//! What the footer holds and a region reads without owning it — the queue's
//! jobs, the studio's state, the notification desk's queue and book, the
//! clock's zone — is LENT for each call, the questions and the
//! painting included (`region::Lent`, the list `held_by_the_footer!` writes
//! once), and what changes is told (`Region::heard`). A region keeps no
//! second copy of it. (What it keeps of the MACHINE is the other loan,
//! above: shared, by `Rc`, for the gestures.)
//!
//! # Who moves the page
//!
//! The REGION, after a gesture that may have moved its cursor — a press, the
//! click that acts, the command it was opened with or its open command given
//! to it already open (`Effect::Follow`, the surface first and then the
//! cursor, as every view's press did) — and the footer after a key
//! (`Footer::typed`) and when it opens one (the first page, before the
//! region is told anything). A click that only selects, an answer typed, a
//! reading that arrives or a thread of the region's that woke do not move
//! the page — a click moves it only to bring back a cursor it left off the
//! page (`Footer::after_a_click`), and a reading only to keep a followed tail
//! in sight (`Effect::FollowIfSeen`): a reading whose rows came or went only
//! clamps it to the rows the region holds now, and the height follows them —
//! when the region says so (`Effect::RowsChanged`), never by the host
//! counting its rows. A region that does not say so
//! (`Region::says_when_its_rows_change`) has the height measured again after
//! any frame it asks for on news.
//!
//! It names no feature: what it knows of a region is its id.

use super::*;
use crate::region::{Closed, Effect, Lit, Paging, Recorder, Shape};

/// Whether the region the footer SHOWS (`shown`, what `Footer::view`
/// answers) types text where Caps Lock means capitals — what a window
/// answers with `Content::types_capitals`: a text being edited in it. The
/// shown one is asked, never a region open BEHIND another view, whose
/// letters are that view's keys (the owner's rule of 2026-09-27: both doors
/// type the same).
pub(super) fn region_types_capitals(shown: ViewState, regions: &crate::region::Slots) -> bool {
    match shown {
        ViewState::Region(id) => regions
            .get(id)
            .is_some_and(crate::region::Region::types_capitals),
        _ => false,
    }
}

/// A region's wake: the loop's channel it calls, and the source that hears
/// it — removed when the region closes and no closed one of it lingers, or
/// kept for the session when the feature has a resident
/// ([`crate::region::Resident`]), whose threads wake it with the region
/// closed.
pub(super) struct RegionWake {
    /// The region it wakes — whose wakes are its slot's
    /// (`region::Slots::wakes`): what a thread of the region calls, and what
    /// one that brings a new picture calls (`Region::framed`).
    id: &'static str,
    /// The loop's source, removed with the wake.
    token: Option<calloop::RegistrationToken>,
}

impl Footer {
    /// Whether the footer is a photograph's (`crate::footer_photograph`): a
    /// region it opens reaches nothing of the machine.
    fn photographs(&self) -> bool {
        self.screens.iter().any(|s| s.photographing)
    }

    /// The doorway for ONE call into region `id`: the words, the width, the
    /// sudo bar's gate and the region's own wake (one that wakes nothing for
    /// a region not open, whose threads have nowhere to be heard), the loop's
    /// way of running a command a thread of it leaves — and, for the region
    /// SHOWN, the page of its rows the footer shows. What the footer lends
    /// the call of its own fields is added where the call is made, by the
    /// fields it borrows (`held_by_the_footer!`).
    fn region_way(&mut self, id: &'static str) -> Recorder {
        let page = if self.shown_region() == Some(id) {
            crate::region::Page {
                offset: self.current_offset(),
                lines: self.grid().lines,
            }
        } else {
            crate::region::Page::default()
        };
        self.region_way_for_news(id).with_page(page)
    }

    /// The same doorway for a call that is no person's gesture — what the
    /// footer heard, its second, a thread of the region's, its closing: no
    /// page, which only a gesture is measured against, so the rows are not
    /// built for every reading of the machine.
    fn region_way_for_news(&mut self, id: &'static str) -> Recorder {
        let (wake, frame) = self
            .regions
            .wakes(id)
            .unwrap_or_else(|| (crate::wake::Wake::new(|| {}), crate::wake::Wake::new(|| {})));
        let orders = self.region_orders();
        let mut way = Recorder::new(
            std::rc::Rc::clone(&self.texts),
            self.screen_columns(),
            self.photographs(),
            self.sudo_gate(),
            wake,
        )
        .with_orders(orders)
        .keeping(self.kept_for_regions())
        // WHERE IT IS on the screen the pointer is on — what a region that
        // lays out its own cells measures a key against; measured for those
        // alone (a stored fact, `Slots::kept_shape`), since the measure asks
        // the footer's grid and a list's doorway needs none of it.
        .at((self.regions.kept_shape(id) == Shape::Cells)
            .then(|| self.pointer_screen().map(|(w, h)| self.region_area(w, h)))
            .flatten());
        // WHAT THE FEATURE KEEPS for the session, and the footer's facts a
        // region may need: the workspace it is on, the screens it knows.
        way.residence = self.regions.residence(id);
        way.frame_wake = Some(frame);
        way.workspace = self.current_workspace_name();
        way.screens = self.known_screens();
        // AND WHETHER A WAKE IS HEARD: a photograph's footer, and a test's,
        // runs no loop.
        way.has_a_loop = self.repeat.is_some();
        way
    }

    /// The screens the footer knows by name, each with what the compositor
    /// says it is, and the one the pointer last entered.
    fn known_screens(&self) -> (Vec<(String, Option<String>)>, Option<String>) {
        let known = self
            .screens
            .iter()
            .filter_map(|t| {
                Some((
                    t.connector.clone()?,
                    self.outputs.info(&t.output).and_then(|i| i.description),
                ))
            })
            .collect();
        (known, self.pointer_on.clone())
    }

    /// The loop's ear for a command a region's thread leaves
    /// (`region::Orders`): a channel hung the first time, which runs the
    /// command with its argument as `ronoctua-shell command` would — and on a
    /// footer with no loop (a photograph's), orders that say they went
    /// nowhere.
    fn region_orders(&mut self) -> crate::region::Orders {
        if self.region_orders.is_none()
            && let Some(handle) = self.repeat.clone()
        {
            let (sender, orders) = channel::channel::<(&'static str, String)>();
            match handle.insert_source(orders, |event, (), state: &mut Self| {
                if let channel::Event::Msg((command, argument)) = event {
                    state.run_with(command, Some(&argument));
                    state.apply_geometry(state.interactivity());
                    state.draw_and_paint();
                }
            }) {
                Ok(_) => self.region_orders = Some(sender),
                Err(e) => tracing::warn!(error = %e, "the regions' orders have no ear"),
            }
        }
        match self.region_orders.clone() {
            Some(sender) => {
                let sender = std::sync::Mutex::new(sender);
                crate::region::Orders::new(move |command, argument| {
                    if let Ok(s) = sender.lock()
                        && s.send((command, argument)).is_err()
                    {
                        tracing::warn!(command, "an order for a footer that is gone");
                    }
                })
            }
            None => crate::region::Orders::nowhere(),
        }
    }

    /// What a region is drawn at, from the footer's own facts: the lines it
    /// may hold are the ceiling, the most it can be given — and, for one laid
    /// out WHOLE (`Shape::Whole`, a repository's history: one page it lays
    /// out itself), the page it is drawn on, the ceiling once the
    /// explanation's row is paid out of it (`region_lines`, the grid's own
    /// arithmetic): ONE answer for its rows, its paint, its hover, its click
    /// and its keys. A list is paged by the footer, and keeps the ceiling.
    fn region_seen(&self) -> crate::region::Seen<'_> {
        let ceiling = self.expansion_ceiling();
        let lines = if self.hosted_shape() == Some(Shape::Whole) {
            region_lines(ceiling, self.explain_rows())
        } else {
            ceiling
        };
        crate::region::Seen {
            texts: &self.texts,
            columns: self.screen_columns(),
            lines: u16::try_from(lines).unwrap_or(u16::MAX),
            zone: &self.zone,
            photographing: self.photographs(),
            lent: crate::region::Lent::of(held_by_the_footer!(self)),
        }
    }

    /// CALLS the open region `id` with a doorway of its own, and does what it
    /// asked once it is back in its slot. `None` when it is not open.
    pub(super) fn with_region<T>(
        &mut self,
        id: &'static str,
        f: impl FnOnce(&mut dyn crate::region::Region, &mut dyn crate::region::Doorway) -> T,
    ) -> Option<T> {
        let way = self.region_way(id);
        let (out, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let out = self.regions.call(id, &mut way, f);
            (out, std::mem::take(&mut way.owed))
        };
        self.region_owed(id, owed);
        self.let_a_hidden_region_go(id);
        out
    }

    /// [`Self::with_region`], for a call that is no person's gesture
    /// ([`Footer::region_way_for_news`]).
    fn with_region_for_news<T>(
        &mut self,
        id: &'static str,
        f: impl FnOnce(&mut dyn crate::region::Region, &mut dyn crate::region::Doorway) -> T,
    ) -> Option<T> {
        let way = self.region_way_for_news(id);
        let (out, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let out = self.regions.call(id, &mut way, f);
            (out, std::mem::take(&mut way.owed))
        };
        self.region_owed(id, owed);
        self.let_a_hidden_region_go(id);
        out
    }

    /// [`Self::with_region`], a hidden region KEPT whatever it says after the
    /// call — for the calls that bring one to life, before its command
    /// reaches it: told what the footer heard first, it waits for nothing
    /// yet, and letting it go there would drop the command it came for.
    fn with_region_held<T>(
        &mut self,
        id: &'static str,
        f: impl FnOnce(&mut dyn crate::region::Region, &mut dyn crate::region::Doorway) -> T,
    ) -> Option<T> {
        let way = self.region_way(id);
        let (out, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let out = self.regions.call(id, &mut way, f);
            (out, std::mem::take(&mut way.owed))
        };
        self.region_owed(id, owed);
        out
    }

    /// A HIDDEN region that no longer lingers is let go: nothing it waited
    /// for is left to hear, and nothing ranks or draws it.
    fn let_a_hidden_region_go(&mut self, id: &'static str) {
        let done = self.regions.is_hidden(id)
            && !self
                .regions
                .get(id)
                .is_some_and(crate::region::Region::lingers);
        if !done {
            return;
        }
        drop(self.regions.take(id));
        self.let_go_of_a_region_wake(id);
        if self.region_asking.is_some_and(|(asker, _)| asker == id) {
            self.region_asking = None;
        }
        tracing::info!(region = %id, "a hidden region let go");
    }

    /// The HIDDEN region `id`, which did not take a command that opens it,
    /// STEPS ASIDE for a fresh one (`Slots::set_aside`): kept beside its slot
    /// while it lingers, its wake kept with it, and its question let go — a
    /// closed view answers none.
    fn set_a_hidden_region_aside(&mut self, id: &'static str) {
        if !self.regions.set_aside(id) {
            return;
        }
        if self.region_asking.is_some_and(|(asker, _)| asker == id) {
            self.region_asking = None;
        }
        self.let_go_of_a_region_wake(id);
        tracing::info!(region = %id, "a hidden region stepped aside for a fresh one");
    }

    /// What region `id` asked of the footer's door, done in the order it
    /// asked — the window's `Ui` does the same things in its own door.
    ///
    /// AND RIGHT AFTER AN ASK OF THE NOTIFICATION DESK, the desk has moved
    /// under the region (`d` forgets the row its cursor is on): it is told so
    /// at once ([`crate::desk::Settle`], the page left where it is), BEFORE
    /// what it asked after the ask — so a page it asked to follow its cursor
    /// follows a cursor back on a row that exists. Whether the page follows
    /// is the region's to ask, as the view's errand did before it was one:
    /// after a gesture over its rows, and never for the order of a record
    /// named (the band's button, the window's). What the settle asks is done
    /// in its place; an ask of the desk from inside a settle is not done —
    /// one turn, no ring.
    fn region_owed(&mut self, id: &'static str, owed: Vec<Effect>) {
        // `true`: asked by the region's call itself; `false`: by its settle.
        let mut owed: std::collections::VecDeque<(Effect, bool)> =
            owed.into_iter().map(|e| (e, true)).collect();
        while let Some((effect, first_hand)) = owed.pop_front() {
            match effect {
                Effect::Say(text, class) => self.echo_for(text, class),
                // HELD, AS IN THE WINDOW (`Ui::hold`): no deadline takes a
                // state that stays true off the line; the next message does.
                Effect::Hold(text) => self.echo_held(text),
                Effect::Ask(line) => {
                    // ANOTHER QUESTION TAKES THE LINE: the one on it is over.
                    // The same region asking the same again replaced its own.
                    if self.region_asking != Some((id, line.id)) {
                        self.the_line_moves_on();
                    }
                    self.region_asking = Some((id, line.id));
                    self.mx.request_line(line);
                    self.apply_geometry(KeyboardInteractivity::Exclusive);
                }
                Effect::Paste => self.read_the_clipboard(PasteFor::Region(id)),
                Effect::Launch(crate::door::Launching::Program { program, args, cwd }) => {
                    self.open_foreign(&program, &args, cwd.as_deref());
                }
                Effect::Launch(crate::door::Launching::Entry(desktop_id)) => {
                    self.launch_desktop(&desktop_id);
                }
                Effect::OpenWindow(args) => {
                    if let Ok(me) = std::env::current_exe() {
                        self.open_foreign(&me.to_string_lossy(), &args, None);
                    }
                }
                Effect::Queue(ask) => self.tell_the_queue(&ask),
                Effect::Close => self.close_region(id),
                Effect::Repaint => self.owes_paint = true,
                // A COMMAND THE SHOWN REGION ASKED FOR: a region it opens that
                // is ranked above it opens over it (`region_free_for`).
                Effect::Run(command, argument) => {
                    let asker = (self.shown_region() == Some(id)).then_some(id);
                    let was = std::mem::replace(&mut self.region_opener, asker);
                    self.run_with(command, argument.as_deref());
                    self.region_opener = was;
                }
                Effect::Show => self.show_region(id),
                Effect::Page(paging) => self.page_region(id, paging),
                // THE REGION IS LEFT FIRST, so it is free for the view the
                // command opens — as `Esc` leaves it.
                Effect::GiveWay(command) => {
                    if self.regions.is_open(id) {
                        self.close_region(id);
                    }
                    self.run_with(command, None);
                }
                // THE MINIBUFFER'S LINE IS THE FOOTER'S, opened and walked as
                // its own keys do; the geometry follows after the call.
                Effect::Fold(crate::fold::Ask::Toggle) => self.fold.toggle(),
                Effect::Fold(crate::fold::Ask::By(by)) => {
                    self.scroll_fold(by);
                }
                // AS AFTER A PERSON'S KEY: the page follows the cursor — the
                // surface first, then the cursor, which is measured against
                // the region about to be drawn (`region_resized`).
                Effect::Follow => {
                    self.apply_geometry(self.interactivity());
                    self.follow_cursor();
                }
                // THE FILE WAS WRITTEN BY THE VIEW: this door takes it now,
                // the way its watch will when it rings.
                Effect::Written(crate::region::Written::Language) => self.adopt_the_language(),
                Effect::Written(crate::region::Written::Places) => self.adopt_the_places(),
                Effect::LetGoOf(question) => {
                    if self.mx.let_go_of(question) {
                        if self.region_asking.is_some_and(|(_, q)| q == question) {
                            self.region_asking = None;
                        }
                        self.apply_geometry(self.interactivity());
                    }
                }
                Effect::Want(line) => self.raise_for_a_region(line),
                // THE PAGE FROM THE TOP, the shown region's — the region
                // follows its cursor from there, as when it opens.
                Effect::FirstPage => {
                    if self.shown_region() == Some(id) {
                        self.first_page();
                    }
                }
                // ROWS THAT CAME OR WENT — a device unplugged with its row, a
                // folder added: the page clamps to what is left and the
                // height follows, as an arrival does (`settle_region`), and
                // only for the region shown; the cursor is a person's to
                // follow, never a reading's.
                Effect::RowsChanged => {
                    if self.shown_region() == Some(id) {
                        self.settle_region();
                        self.region_resized();
                    }
                    self.owes_paint = true;
                }
                // WHAT WAS COPIED LAST, into the queue: the clipboard's
                // history is the footer's.
                Effect::QueueCopied(_, wish) => self.queue_the_clipboard(wish),
                Effect::Desk(ask) if !first_hand => {
                    tracing::error!(
                        region = id,
                        ?ask,
                        "a region asked the desk while it settled; not done"
                    );
                }
                Effect::Desk(ask) => {
                    self.desk_asked(ask);
                    for settled in self.desk_settled(id).into_iter().rev() {
                        owed.push_front((settled, false));
                    }
                }
                // WHAT ONLY THE FOOTER DOES ANYTHING WITH — the modeline's
                // marks following what a view read — by its type.
                Effect::News(news) => self.region_news(&news),
                // A FOLLOWED TAIL, kept in sight: the page follows the
                // region's cursor only if row `was` is on the page — which
                // nothing moved during the call — and only while the region
                // is the one shown, whose cursor `follow_cursor` reads.
                Effect::FollowIfSeen(was) => {
                    if self.shown_region() == Some(id)
                        && on_the_page(was, self.current_offset(), self.expansion_ceiling())
                        && self.follow_cursor()
                    {
                        self.owes_paint = true;
                    }
                }
                // ANOTHER DOOR TAKES THE KEYBOARD: the region is given back
                // first, and what it asks after this is still done.
                Effect::StepAside => self.close_region(id),
                // THE CLIPBOARD'S HISTORY IS THIS FOOTER'S: the region's act
                // is carried out on it, and said on this door's line.
                Effect::Clipboard(act, generation) => self.clipboard_acted(generation, act),
                // ITS LINES CAME OR WENT: the footer measures itself again.
                Effect::Resized => self.region_resized(),
                // WHAT THE ORDER PUT ON THE LINE is taken back: it was not
                // done, and its question is on the region.
                Effect::TakeBack => self.echo = None,
            }
        }
    }

    /// Region `id`, told the notification desk moved under it — the page
    /// left where it is — and what it asked when told; nothing for a region
    /// not open (a command of the closed one, or one that closed itself),
    /// and no doorway built for it, whose gate would open the sudo bar's
    /// channel before any gesture needs it.
    fn desk_settled(&mut self, id: &'static str) -> Vec<Effect> {
        if !self.regions.is_open(id) {
            return Vec::new();
        }
        let way = self.region_way(id);
        let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
        let settle = crate::desk::Settle { follow: false };
        let _ = self.regions.call(id, &mut way, |r, w| r.heard(&settle, w));
        std::mem::take(&mut way.owed)
    }

    /// A hidden region asked for the footer's region: shown — when the
    /// region is free, and the refusal said when it is not — and told so.
    fn show_region(&mut self, id: &'static str) {
        if !self.regions.is_hidden(id) || !self.region_is_free(id) {
            return;
        }
        self.regions.show(id);
        tracing::info!(region = %id, "a hidden region shown");
        self.first_page();
        let _ = self.with_region(id, |r, w| r.shown(w));
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        self.draw_and_paint();
    }

    /// The shown region `id`'s page moves, as it asked — the footer's page,
    /// which a hidden region does not have.
    fn page_region(&mut self, id: &'static str, paging: Paging) {
        if !self.regions.is_open(id) || self.view() != ViewState::Region(id) {
            return;
        }
        match paging {
            Paging::Follow => {
                self.follow_cursor();
            }
            // THE PLACE FIRST ON THE PAGE, and the cursor kept on screen:
            // a place somebody asked for is brought into view whole.
            Paging::Top(row) => {
                self.scroll_offset_to(row.min(self.last_start()));
                let first = self.current_offset();
                let shown = first..first + self.expansion_ceiling();
                if self.list_cursor().is_some_and(|c| !shown.contains(&c)) {
                    self.follow_cursor();
                }
            }
            Paging::Pages(pages) => {
                let page = i32::try_from(self.grid().per_page().max(1)).unwrap_or(1);
                self.scroll_by(pages.saturating_mul(page));
            }
            Paging::Lines(lines) => {
                self.scroll_by(lines);
            }
            Paging::End => {
                let last = self.last_start();
                self.scroll_offset_to(last);
            }
            // ON THE END BEFORE THE LIST GREW, ON THE END AFTER: the last page
            // moved by `grew` rows at most, so the page was on it when it is
            // that close to the new one.
            Paging::Tail(grew) => {
                let last = self.last_start();
                if self.current_offset().saturating_add(grew) >= last {
                    self.scroll_offset_to(last);
                }
            }
        }
    }

    /// What a region that stepped aside asked (`Slots::linger`): what it
    /// says, and nothing that acts on the region shown — closed, it has no
    /// line to ask on, nothing to paste into, no page and no region to give
    /// back.
    fn lingering_owed(&mut self, owed: Vec<Effect>) {
        for effect in owed {
            match effect {
                Effect::Say(text, class) => self.echo_for(text, class),
                Effect::Hold(text) => self.echo_held(text),
                Effect::Repaint | Effect::RowsChanged => self.owes_paint = true,
                Effect::News(news) => self.region_news(&news),
                Effect::Launch(_)
                | Effect::OpenWindow(_)
                | Effect::Queue(_)
                | Effect::Run(..)
                | Effect::Ask(_)
                | Effect::Paste
                | Effect::Close
                | Effect::Show
                | Effect::Page(_)
                | Effect::GiveWay(_)
                | Effect::Fold(_)
                | Effect::Follow
                | Effect::Written(_)
                | Effect::LetGoOf(_)
                | Effect::Want(_)
                | Effect::FirstPage
                | Effect::QueueCopied(..)
                | Effect::Desk(_)
                | Effect::FollowIfSeen(_)
                | Effect::StepAside
                | Effect::Clipboard(..)
                | Effect::Resized
                | Effect::TakeBack => {
                    tracing::debug!(
                        ?effect,
                        "a lingering region asked what only an open one may"
                    );
                }
            }
        }
    }

    /// The region `id` is SHOWN — what `view()` ranks — and open in the host.
    pub(super) fn shown_region(&self) -> Option<&'static str> {
        match self.view() {
            ViewState::Region(id) if self.regions.is_open(id) => Some(id),
            _ => None,
        }
    }

    /// A key of the shown region (`Footer::view_took_the_key`).
    pub(super) fn hosted_key(&mut self, id: &'static str, key: ronoctua_keymap::Key) {
        // A KEY IT DOES NOT TAKE IS SWALLOWED all the same, as every modal
        // view swallows the keyboard it holds: a wrong key loses nothing.
        let _ = self.with_region(id, |r, w| r.key(key, w));
        self.apply_geometry(self.interactivity());
        // THE FRAME IS OWED, not drawn (`Footer::ask_paint`): a held-down
        // arrow is a burst of keys, and presenting per key blocked the loop
        // on every one of them while the rest piled up — the file manager's
        // listing walked behind the hand. The owed frame is drawn once the
        // queue is empty, which for a single key is the same instant.
        self.ask_paint();
    }

    /// The shown region's rows, as the footer's one painter draws them
    /// (`Footer::expansion`): a row that takes a click is a click on the
    /// region's row, what it does is said under the pointer, and a line being
    /// typed carries its caret (`Region::caret`).
    pub(super) fn hosted_expansion(&self, id: &str) -> Vec<ExpansionRow> {
        // A REGION IN SECTIONS is laid onto the lines by the footer's own
        // arithmetic (`surface/sections.rs`), the whole of it: the window of
        // lines asked for no ceiling is the same question the drawing asks.
        if self.regions.kept_shape(id) == Shape::Sections {
            return self.expansion_window(0, self.expansion_len());
        }
        let len = self.hosted_len(id);
        self.hosted_window(id, 0, len)
    }

    /// How many rows the shown region has — counted by the region, never
    /// built (`Region::row_count`): a log of thousands of lines is counted
    /// on every frame and every pointer motion.
    pub(super) fn hosted_len(&self, id: &str) -> usize {
        let Some(region) = self.regions.get(id) else {
            return 0;
        };
        if region.shape() == Shape::Cells {
            return self.expansion_ceiling();
        }
        region.row_count(&self.region_seen())
    }

    /// The shown region's rows from `skip`, at most `take` — only those
    /// built (`Region::rows_from`). A row that takes a click takes its
    /// CONTROL's (`Region::control_of`): the rows a sentence wraps over are
    /// one click, and light together.
    pub(super) fn hosted_window(&self, id: &str, skip: usize, take: usize) -> Vec<ExpansionRow> {
        let Some(region) = self.regions.get(id) else {
            return Vec::new();
        };
        let seen = self.region_seen();
        if region.shape() == Shape::Cells {
            // BLANK ROWS, as many as the region may hold: it paints its own
            // cells over them (`Footer::hosted_paint`).
            return (0..self.expansion_ceiling().saturating_sub(skip).min(take))
                .map(|_| ExpansionRow::empty_one())
                .collect();
        }
        region
            .told(&seen, skip, take)
            .into_iter()
            .enumerate()
            .map(|(k, told)| {
                let n = skip + k;
                let click = told.row.takes.then(|| {
                    Click::Region(told.control.unwrap_or_else(|| region.control_of(n, &seen)))
                });
                let description = told.about.or_else(|| region.about(n, &seen));
                match told.pair {
                    // THE FOOTER'S OWN TWO COLUMNS, laid by its grid.
                    Some(p) => ExpansionRow {
                        key: p.key,
                        text: p.text,
                        key_color: p.key_tone,
                        text_color: p.text_tone,
                        highlights: p.lit,
                        here: p.here,
                        key_flush_right: p.flush_right,
                        separator: p.colon,
                        aside: p.aside,
                        picture: p.picture,
                        beside_picture: p.beside_picture,
                        click,
                        description,
                        region_row: Some(n),
                        ..ExpansionRow::empty_one()
                    },
                    // A ROW'S PICTURE in the cell the region named — its
                    // glyph's, which it left out of the fields once the
                    // picture was here — the caret of a line being typed
                    // (said with the row, or `Region::caret`), and what the
                    // footer's one painter draws beside the fields: chips,
                    // covers, what the pointer lights, a grid's cursor.
                    None => ExpansionRow {
                        click,
                        description,
                        caret: told.caret.or_else(|| region.caret(n, &seen)),
                        fields: told.row.fields,
                        pixmap_at: told.picture.as_ref().map(|p| p.at),
                        pixmap: told.picture.map(|p| p.key),
                        chips: told.chips,
                        covers: told.covers,
                        continues: told.continues,
                        lit: told.lights,
                        chosen: told.chosen,
                        region_row: Some(n),
                        ..ExpansionRow::empty_one()
                    },
                }
            })
            .collect()
    }

    /// A click on row `n` of the shown region (`Footer::click_at`), with the
    /// cell of the line the pointer pressed it at — a row of several
    /// stretches takes the one under it (`Region::clicked_at`) — and the page
    /// after it ([`Footer::after_a_click`]): left where it is, under the
    /// pointer.
    pub(super) fn hosted_click(&mut self, n: usize) {
        if let Some(id) = self.shown_region() {
            let under = crate::region::Under {
                cell: self.here.cell_under_pointer,
                ..self.hosted_under(self.here.pointer_at.unwrap_or_default())
            };
            let _ = self.with_region(id, |r, w| r.clicked_under(n, &under, w));
            // THE HEIGHT FOLLOWS WHAT THE CLICK LEFT, as after a key
            // (`hosted_key`): a click that moves a cursor changes what the
            // rows say about it — a head that wraps over another number of
            // lines, a question or a menu it opens — and the pointer and the
            // keyboard leave the same height.
            self.after_a_click();
        }
    }

    /// Where a pointer at `at` is over the shown region, as the footer
    /// measures it (`region::Under`).
    fn hosted_under(&self, at: (f64, f64)) -> crate::region::Under {
        let line = self.row_at(at.0, at.1);
        let row = line
            .and_then(|i| self.visible_row(i))
            .and_then(|l| l.region_row);
        let base = line.map_or(0, |l| self.grid().position(l).0);
        let mods = self.pointer_mods();
        crate::region::Under {
            at,
            line,
            row,
            cell: self.cell_at(at.0),
            base,
            ctrl: mods.ctrl,
            shift: mods.shift,
            frame: self.metric(),
        }
    }

    /// CALLS the shown region `id` with the hand this pointer holds over it
    /// (`PerSeat::region_hand`) — a hand another region left is no hand of
    /// this one's.
    fn with_region_hand<T>(
        &mut self,
        id: &'static str,
        f: impl FnOnce(
            &mut dyn crate::region::Region,
            &mut Option<Box<dyn std::any::Any>>,
            &mut dyn crate::region::Doorway,
        ) -> T,
    ) -> Option<T> {
        let mut hand = match self.here.region_hand.take() {
            Some((of, held)) if of == id => Some(held),
            _ => None,
        };
        let out = self.with_region(id, |r, w| f(r, &mut hand, w));
        self.here.region_hand = hand.map(|held| (id, held));
        out
    }

    /// Whether `at` is on a control of the shown region's own that no row is
    /// under — no row lights there.
    pub(super) fn hosted_owns_point(&self, at: (f64, f64)) -> bool {
        self.shown_region()
            .and_then(|id| self.regions.get(id))
            .is_some_and(|r| r.owns_point(at, self.metric()))
    }

    /// The pointer moved over the surface: the shown region told — whether
    /// a frame is owed.
    pub(super) fn hosted_hovered(&mut self, at: (f64, f64)) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let under = self.hosted_under(at);
        self.with_region_hand(id, |r, hand, w| r.hovered(&under, hand, w))
            .unwrap_or(false)
    }

    /// The pointer moved with something of the shown region's held: whether
    /// one is.
    pub(super) fn hosted_dragged(&mut self, at: (f64, f64)) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let under = self.hosted_under(at);
        self.with_region_hand(id, |r, hand, w| r.dragged(&under, hand, w))
            .unwrap_or(false)
    }

    /// The left button went down at `at`: whether it was on one of the shown
    /// region's own controls.
    pub(super) fn hosted_pressed(&mut self, at: (f64, f64)) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let under = self.hosted_under(at);
        self.with_region_hand(id, |r, hand, w| r.pressed(&under, hand, w))
            .unwrap_or(false)
    }

    /// The left button came up: what the shown region held let go.
    pub(super) fn hosted_released(&mut self) {
        if let Some(id) = self.shown_region() {
            let _ = self.with_region_hand(id, |r, hand, w| r.released(hand, w));
        }
    }

    /// The pointer left the surface: what the shown region held or lit goes.
    pub(super) fn hosted_left(&mut self) {
        if let Some(id) = self.shown_region() {
            let _ = self.with_region_hand(id, |r, hand, w| r.left(hand, w));
        }
    }

    /// What the message line says of what is under this pointer in the shown
    /// region — before any row's sentence.
    pub(super) fn hosted_said_under(&self) -> Option<String> {
        let id = self.shown_region()?;
        let hand = self
            .here
            .region_hand
            .as_ref()
            .filter(|(of, _)| *of == id)
            .map(|(_, held)| held.as_ref());
        self.regions.get(id)?.said_under(hand, &self.texts)
    }

    /// Whether the wheel over `at` is the shown region's own.
    pub(super) fn hosted_takes_the_wheel(&self, at: (f64, f64)) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let under = self.hosted_under(at);
        self.regions
            .get(id)
            .is_some_and(|r| r.takes_the_wheel(&under))
    }

    /// The wheel over `at`, `lines` of it: whether the shown region took it
    /// on a control of its own.
    pub(super) fn hosted_wheeled(&mut self, at: (f64, f64), lines: i64) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let under = self.hosted_under(at);
        self.with_region_hand(id, |r, hand, w| r.wheeled(&under, lines, hand, w))
            .unwrap_or(false)
    }

    /// Whether the shown region's lines came or went before this frame.
    pub(super) fn hosted_before_measuring(&mut self) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        self.framing(id, |r, _, seen| r.before_measuring(seen))
            .unwrap_or(false)
    }

    /// TOUCHES region `id` for one of the frame's hooks: what it sees, the
    /// hand this pointer holds over it when it is the shown one — and NO
    /// doorway (`region::Slots::frame`): a frame asks nothing of the footer,
    /// so a paint never runs a command that paints.
    fn framing<T>(
        &mut self,
        id: &'static str,
        f: impl FnOnce(
            &mut dyn crate::region::Region,
            &mut Option<Box<dyn std::any::Any>>,
            &crate::region::Seen<'_>,
        ) -> T,
    ) -> Option<T> {
        let columns = self.screen_columns();
        let lines = u16::try_from(self.expansion_ceiling()).unwrap_or(u16::MAX);
        let photographing = self.photographs();
        let shown = self.shown_region() == Some(id);
        let mut hand = match self.here.region_hand.take() {
            Some((of, held)) if shown && of == id => Some(held),
            other => {
                self.here.region_hand = other;
                None
            }
        };
        let seen = crate::region::Seen {
            texts: &self.texts,
            columns,
            lines,
            zone: &self.zone,
            photographing,
            lent: crate::region::Lent::of(held_by_the_footer!(self)),
        };
        let out = self.regions.frame(id, |r| f(r, &mut hand, &seen));
        // ONLY THE SHOWN REGION'S HAND is this pointer's: one a region under
        // it made is dropped, as a hand of no pointer.
        if let (true, Some(held)) = (shown, hand) {
            self.here.region_hand = Some((id, held));
        }
        out
    }

    /// The frame's layout, told to every open region before it is painted
    /// (`Region::laid_out`) — the one the footer shows, and the ones under
    /// it, which show nothing.
    pub(super) fn hosted_laid_out(&mut self) {
        if self.regions.open_ids().next().is_none() {
            return;
        }
        let shown = self.shown_region();
        let first = self.first_of_expansion();
        let scroll = self.region_scroll();
        let laid = crate::region::Laid {
            first,
            lines: u16::try_from(self.grid().lines).unwrap_or(u16::MAX),
            columns: self.screen_columns(),
            offset: scroll.offset,
            room: scroll.lines,
            shown: self.shape.region_shown,
            screens: self
                .screens
                .iter()
                .enumerate()
                .map(|(i, t)| crate::region::Screen {
                    connector: t.connector.clone(),
                    key: t.connector.clone().unwrap_or_else(|| format!("screen-{i}")),
                    scale: u32::try_from(t.scale.max(1)).unwrap_or(1),
                })
                .collect(),
            frame: self.metric(),
            pointer: self.here.pointer_at,
            ranked: false,
        };
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        for id in open {
            let laid = crate::region::Laid {
                ranked: shown == Some(id),
                ..laid.clone()
            };
            let _ = self.framing(id, |r, hand, seen| r.laid_out(&laid, hand, seen));
        }
    }

    /// Every screen painted the frame — `presented` when one did.
    pub(super) fn hosted_painted(&mut self, presented: bool) {
        if let Some(id) = self.shown_region() {
            let _ = self.framing(id, |r, _, seen| r.painted(presented, seen));
        }
    }

    /// After the frame, told to every open region: whether something under a
    /// resting pointer changed in the shown one, and one more frame is owed.
    pub(super) fn hosted_after_the_frame(&mut self) -> bool {
        if self.regions.open_ids().next().is_none() {
            return false;
        }
        let shown = self.shown_region();
        let under = self.here.pointer_at.map(|at| self.hosted_under(at));
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        let mut owed = false;
        for id in open {
            let under = if shown == Some(id) {
                under.as_ref()
            } else {
                None
            };
            let moved = self
                .framing(id, |r, hand, seen| r.after_the_frame(under, hand, seen))
                .unwrap_or(false);
            owed |= shown == Some(id) && moved;
        }
        owed
    }

    /// The screens the footer knows changed — named, gone, the pointer on
    /// another footer: told to the regions (`region::Screens`).
    pub(super) fn screens_told(&mut self) {
        let (known, here) = self.known_screens();
        self.hosted_heard::<crate::region::Screens>(&crate::region::Screens { known, here });
    }

    /// Where a pointer is across a region's rows on a line whose grid column
    /// starts `base` cells in — the ONE answer the band a grid's cell lights
    /// with, the cursor's cell and the cell a click takes all read: the rows
    /// are drawn `chrome::FIELDS_LEAD` after the column's first cell.
    pub(super) fn region_across(base: u16) -> crate::chrome::Across {
        crate::chrome::Across {
            lead: crate::chrome::FIELDS_LEAD.saturating_add(base),
            cell_px: crate::chrome::cell_width(),
        }
    }

    /// Whether a held button sweeps the shown region's text — a region is
    /// a list unless it says so (`Region::sweeps_text`).
    pub(super) fn hosted_sweeps_text(&self) -> bool {
        self.shown_region()
            .and_then(|id| self.regions.get(id))
            .is_none_or(crate::region::Region::sweeps_text)
    }

    /// Whether the shown region types text where Caps Lock means capitals.
    pub(super) fn hosted_types_capitals(&self) -> bool {
        region_types_capitals(self.view(), &self.regions)
    }

    /// The picture the shown region holds under `key` — a cover of its rows.
    pub(super) fn hosted_picture(
        &self,
        key: &str,
    ) -> Option<&(String, std::sync::Arc<ronoctua_render::image::Rgba>)> {
        let id = self.shown_region()?;
        self.regions.get(id)?.cover_picture(key)
    }

    /// The band the pointer lights over the shown region's row, in the
    /// surface's pixels: `None` for the whole row, as every list's, and
    /// `Some(None)` where a click takes nothing — by the region's own answer
    /// (`Region::lit`), the one its click reads. `click` is the row the
    /// pointer is on, as drawn (`Click::Region`: its row in the region).
    pub(super) fn hosted_band(
        &self,
        click: Option<Click>,
        cell: Option<u16>,
        cell_px: f32,
    ) -> Option<Option<(f32, f32)>> {
        let Some(Click::Region(n)) = click else {
            return None;
        };
        let region = self.shown_region().and_then(|id| self.regions.get(id))?;
        match region.lit(n, cell, &self.region_seen()) {
            Lit::Row => None,
            Lit::Nothing => Some(None),
            Lit::Cells(from, to) => {
                let lead = crate::chrome::FIELDS_LEAD;
                Some(Some((
                    f32::from(lead + from) * cell_px,
                    f32::from(to.saturating_sub(from)) * cell_px,
                )))
            }
        }
    }

    /// Whether a click on `line` where THIS pointer rests takes something —
    /// the row has a click, and the shown region does not say the cell under
    /// the pointer takes none (`Region::lit`'s `Lit::Nothing`). What a field
    /// under the pointer is lit by, as the band is: a title, a note or the
    /// dead stretch of a row lights nothing, since it takes nothing.
    ///
    /// IN SECTIONS, the sections' own answer at the point (`Sectioned::takes_at`),
    /// as the frame already asked it once (`lit`, `Sectioned::lit_at`): a
    /// line there is every section's, and the line's click says only that
    /// one of them has something on it.
    pub(super) fn takes_under_pointer(
        &self,
        line: &ExpansionRow,
        lit: Option<&crate::sections::LitAt>,
    ) -> bool {
        line.click.is_some()
            && self.hosted_band(
                line.click,
                self.here.cell_under_pointer,
                tokens::metrics::cell_width(tokens::text::size()),
            ) != Some(None)
            && self
                .sections()
                .is_none_or(|_| lit.is_some_and(|l| l.taken.is_some()))
    }

    /// What the pointer lights over the shown region's row `row` (`None`: a
    /// line that takes no click) at cell `cell`, on a surface `width` by
    /// `bottom`, for a region that places its own bands (`Region::band`) —
    /// `None` when no region of the slot host is shown.
    pub(super) fn hosted_placed_band(
        &self,
        row: Option<usize>,
        cell: Option<u16>,
        width: f32,
        bottom: f32,
    ) -> Option<crate::region::Band> {
        let region = self.shown_region().and_then(|id| self.regions.get(id))?;
        let seen = self.region_seen();
        Some(region.band(row, cell, &self.region_area(width, bottom), &seen))
    }

    /// What the pointer lights among the shown region's fields — `None` when
    /// no region of the slot host is shown.
    pub(super) fn hosted_lights(&self) -> Option<crate::region::Lights> {
        self.shown_region()
            .and_then(|id| self.regions.get(id))
            .map(crate::region::Region::lights)
    }

    /// The shown region's cursor, as drawn — which the page follows.
    pub(super) fn hosted_cursor(&self, id: &str) -> Option<usize> {
        let seen = self.region_seen();
        self.regions.get(id).and_then(|r| r.cursor_in(&seen))
    }

    /// What the minibuffer's line says while the shown region is up — what
    /// was typed into its search — or `None`, the keys it teaches.
    pub(super) fn hosted_prompt(&self) -> Option<String> {
        self.shown_region()
            .and_then(|id| self.regions.get(id))
            .and_then(crate::region::Region::prompt)
    }

    /// What the pointer rests on in the shown region has to add, whole, on
    /// the explanation line — a region that lays out its own cells.
    pub(super) fn hosted_about_the_pointer(&self) -> Option<String> {
        let region = self.shown_region().and_then(|id| self.regions.get(id))?;
        let (w, h) = self.pointer_screen()?;
        let area = self.region_area(w, h);
        region.about_the_pointer(&area, &self.region_seen())
    }

    /// What the row the keyboard is on in the shown region has to add, said
    /// on the explanation's line (`Region::note`).
    pub(super) fn hosted_note(&self) -> Option<String> {
        let id = self.shown_region()?;
        let seen = self.region_seen();
        self.regions.get(id)?.note(&seen)
    }

    /// What the shown region's row `row` says at cell `cell`, when the row
    /// alone cannot (`Region::about_at`).
    pub(super) fn hosted_about_at(&self, row: usize, cell: Option<u16>) -> Option<String> {
        let id = self.shown_region()?;
        let seen = self.region_seen();
        self.regions.get(id)?.about_at(row, cell, &seen)
    }

    /// The hover band's pixels inside the part of a row the shown region
    /// lights at cell `cell` — one of two halves — on a screen `columns`
    /// wide, a cell `cell_px` wide (`Region::hover_span`).
    pub(super) fn hosted_hover_px(
        &self,
        columns: u16,
        row: usize,
        cell: u16,
        cell_px: f32,
    ) -> Option<(f32, f32)> {
        let id = self.shown_region()?;
        let seen = crate::region::Seen {
            columns,
            ..self.region_seen()
        };
        let (from, to) = self.regions.get(id)?.hover_span(row, cell, &seen)?;
        let lead = crate::chrome::FIELDS_LEAD;
        Some((
            f32::from(lead + from) * cell_px,
            f32::from(to.saturating_sub(from)) * cell_px,
        ))
    }

    /// The text a sweep from cell `cell` of the shown region's row `row`
    /// stays in, when it is narrower than the region — on a screen `columns`
    /// wide whose region starts on line `top` (`Region::text_area`).
    pub(super) fn hosted_text_area(
        &self,
        row: usize,
        cell: u16,
        columns: u16,
        top: u16,
    ) -> Option<crate::text_select::Area> {
        let id = self.shown_region()?;
        let seen = crate::region::Seen {
            columns,
            ..self.region_seen()
        };
        let (width, bottom) = self.pointer_screen()?;
        let area = crate::region::Area {
            first: top,
            ..self.region_area(width, bottom)
        };
        self.regions.get(id)?.text_area(row, cell, &area, &seen)
    }

    /// The wheel over cell `cell` of the shown region's row `row`, `lines`
    /// of it: whether the region scrolled something of its own.
    pub(super) fn hosted_wheel(&mut self, row: usize, cell: u16, lines: i64) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        self.with_region(id, |r, w| r.wheel(row, cell, lines, w))
            .unwrap_or(false)
    }

    /// A press at `at` on a bar the shown region draws itself: whether it was
    /// one — and THIS POINTER holds it, by the grip the region named, until
    /// its button comes up (`PerSeat::dragging_hosted`): two seats may hold
    /// two bars at once, and each drag answers the one it started on.
    pub(super) fn hosted_bar_pressed(&mut self, at: (f64, f64)) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let Some((w, h)) = self.pointer_screen() else {
            return false;
        };
        let area = self.region_area(w, h);
        let Some(grip) = self
            .with_region(id, |r, way| r.grab(at, &area, way))
            .flatten()
        else {
            return false;
        };
        self.here.dragging_hosted = Some((id, grip));
        self.owes_paint = true;
        true
    }

    /// The pointer at `at` while it holds a region's bar: whether it holds
    /// one — and it keeps holding it until the button comes up, as every
    /// bar's drag does, though a region no longer shown moves nothing.
    pub(super) fn hosted_bar_dragged(&mut self, at: (f64, f64)) -> bool {
        let Some((id, grip)) = self.here.dragging_hosted else {
            return false;
        };
        if self.shown_region() != Some(id) {
            return true;
        }
        let Some((w, h)) = self.pointer_screen() else {
            return true;
        };
        let area = self.region_area(w, h);
        if self
            .with_region(id, |r, way| r.drag(grip, at, &area, way))
            .unwrap_or(false)
        {
            self.owes_paint = true;
        }
        true
    }

    /// The drawings the shown region's rows show, rasterised once each into
    /// the map the applications' pixmaps live in — under keys that carry
    /// what they show, their size and the palette, so a theme's switch is a
    /// new key (`Region::pictures`).
    pub(super) fn hosted_pictures(&mut self) {
        let Some(id) = self.shown_region() else {
            return;
        };
        let wanted: Vec<String> = {
            let seen = self.region_seen();
            self.regions
                .get(id)
                .map(|r| r.pictures(&seen))
                .unwrap_or_default()
        };
        for key in wanted {
            if self.icons.contains_key(&key) {
                continue;
            }
            let pixels = self.regions.get(id).and_then(|r| r.picture(&key));
            self.icons.insert(key, pixels);
        }
    }

    /// The keys the shown region's hint teaches — none, when it teaches
    /// nothing now (`Region::teaches`: a filter being typed, a menu with a
    /// line of its own): the line then says what it says without a view's
    /// keys. And none with "hide shortcuts" on: a feature's panel is a
    /// program, and gives its keys up to the line the way a window does —
    /// it keeps its content and answers the arrows and `RET`.
    pub(super) fn hosted_hint(&self, id: &str) -> Option<Vec<crate::chrome::Piece>> {
        if self.modeline.hide_shortcuts() {
            return None;
        }
        let seen = self.region_seen();
        self.regions
            .get(id)
            .filter(|r| r.teaches())
            .map(|r| r.hint(&seen))
    }

    /// A command, for the regions of the host (`Footer::run_with`): the
    /// answer to a question one of them asked, a command of an open one, or
    /// one that a closed one takes — opening it when the region is free.
    pub(super) fn hosted_command(&mut self, id: &str, argument: Option<&str>) -> bool {
        // NO REGION REGISTERED, NOTHING ASKED — and no doorway built, whose
        // gate would open the sudo bar's channel before any gesture needs it.
        if self.regions.ids().next().is_none() {
            return false;
        }
        if self.hosted_answer(id, argument) {
            return true;
        }
        // EVERY LIVE REGION, shown or hidden: a hidden one is still the
        // view's, and its commands are still its own.
        let live: Vec<&'static str> = self.regions.live_ids().collect();
        for region in live {
            let taken = self.with_region(region, |r, w| r.command(id, argument, w));
            if taken.is_some_and(crate::region::Taken::took) {
                self.after_a_region_spoke();
                return true;
            }
        }
        // AN OPENING COMMAND OF A REGION ALREADY OPEN, which the region
        // itself did not take, is refused as every opening is while the region
        // is held: the same sentence, by the same question (`region_is_free`).
        // Its `closed` is ASKED over a doorway whose asks are dropped — it acts
        // only through its doorway (`region::Entry::closed`).
        let scratch = self.region_way("");
        let mut scratch = scratch.lending(crate::region::Lent::of(held_by_the_footer!(self)));
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        let reopened = open.iter().any(|held| {
            self.regions
                .entry(held)
                .is_some_and(|e| (e.closed)(id, argument, &mut scratch) == Closed::Opens)
        });
        drop(scratch);
        if reopened {
            let _ = self.region_is_free(id);
            self.after_a_region_spoke();
            return true;
        }
        // A COMMAND OF A HIDDEN REGION that did not take it: the region is a
        // closed view still saying what its work answers, not a page to bring
        // back, and its entry's `closed` answers for it as for a closed one —
        // a window or a deed (`Closed::Taken`) done here, and one that opens
        // the region or brings it behind finds the slot free: the hidden one
        // steps aside, still heard by its threads.
        let hidden: Vec<&'static str> = self
            .regions
            .live_ids()
            .filter(|r| self.regions.is_hidden(r))
            .collect();
        for held in hidden {
            let Some(entry) = self.regions.entry(held) else {
                continue;
            };
            let way = self.region_way("");
            let (answer, owed) = {
                let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
                let answer = (entry.closed)(id, argument, &mut way);
                (answer, std::mem::take(&mut way.owed))
            };
            match answer {
                Closed::NotMine => {}
                Closed::Taken => {
                    self.region_owed(held, owed);
                    self.after_a_region_spoke();
                    return true;
                }
                Closed::Opens | Closed::Behind => self.set_a_hidden_region_aside(held),
            }
        }
        // A CLOSED REGION'S COMMAND reads what the footer lends too — the
        // camera's keys from anywhere, the studio's state.
        let way = self.region_way("");
        let (closed, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let closed = self.regions.closed_command(id, argument, &mut way);
            (closed, std::mem::take(&mut way.owed))
        };
        let Some((region, answer)) = closed else {
            return false;
        };
        self.region_owed(region, owed);
        match answer {
            Closed::Opens => self.open_region(region, id, argument),
            Closed::Behind => self.bring_a_region_behind(region, id, argument),
            Closed::Taken | Closed::NotMine => {}
        }
        self.after_a_region_spoke();
        true
    }

    /// THE ANSWER TO A REGION'S OWN QUESTION, which comes back under the
    /// question's id: to the region that asked it, shown or hidden — asked
    /// BEFORE any view's commands (`Footer::run_with`), since a question's id
    /// may be one another view asks too, and the answer is the asker's.
    /// `false` when `id` is no region's question.
    pub(super) fn hosted_answer(&mut self, id: &str, argument: Option<&str>) -> bool {
        let (Some((asker, question)), Some(text)) = (self.region_asking, argument) else {
            return false;
        };
        if question != id {
            return false;
        }
        self.region_asking = None;
        if !self.regions.is_live(asker) {
            return false;
        }
        let _ = self.with_region(asker, |r, w| r.answered(question, text, w));
        self.after_a_region_spoke();
        true
    }

    /// Opens region `id` for `command` — when the region is free, or held
    /// by the region that asked for it ([`Footer::region_free_for`]) — gives
    /// it a wake of its own, and hands it the command that opened it.
    fn open_region(&mut self, id: &'static str, command: &str, argument: Option<&str>) {
        if !self.region_free_for(id, command) {
            return;
        }
        let Some(entry) = self.regions.entry(id) else {
            return;
        };
        self.hang_a_region_wake(id);
        // IT OPENS ON WHAT THE FOOTER HOLDS, lent: the studio's state, the
        // queue's jobs, the clock's zone.
        let way = self.region_way(id);
        let (region, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let region = (entry.open)(command, argument, &mut way);
            (region, std::mem::take(&mut way.owed))
        };
        if region.id() != id {
            // NOT KEPT, and nothing it asked is done: a region the
            // precedence cannot rank has nothing to say on its behalf.
            tracing::error!(
                registered = id,
                answers = region.id(),
                "a region opened under another id; it is not kept"
            );
            self.let_go_of_a_region_wake(id);
            return;
        }
        if self.regions.give_back(region).is_err() {
            self.let_go_of_a_region_wake(id);
            return;
        }
        self.region_owed(id, owed);
        tracing::info!(region = %id, command = %command, "a region opened");
        // THE FIRST PAGE BEFORE THE REGION SPEAKS: a page it asks for when
        // it is told what the footer holds, shown or handed its command (its
        // cursor born below the first page; a modeline mark opening a view on
        // the row it is about, its section's title at the top —
        // `Region::take_anchor`, followed through `Doorway::follow`) is the
        // one it keeps.
        self.first_page();
        self.hosted_replay(id);
        let _ = self.with_region(id, |r, w| r.shown(w));
        let _ = self.with_region(id, |r, w| r.command(command, argument, w));
        self.apply_geometry(KeyboardInteractivity::Exclusive);
    }

    /// Whether region `id` may open for `command` now: the region is free —
    /// or it is held by the SHOWN region that asked for this command
    /// (`Footer::region_opener`), which `id` is ranked above: it opens OVER
    /// it, and the one under is shown again as it was when it closes (the
    /// file manager's `C-h` opens a repository's history over the manager).
    /// Who holds the region is the one that asked, so the question of a free
    /// region is not asked of it; every other opening asks it.
    fn region_free_for(&mut self, id: &'static str, command: &str) -> bool {
        if let Some(asker) = self.region_opener
            && self.view() == ViewState::Region(asker)
            && ranks_above(PRECEDENCE, id, asker)
        {
            return true;
        }
        self.region_is_free(command)
    }

    /// Brings region `id` to life HIDDEN for `command` (`Closed::Behind`):
    /// handed the command, and let go at once unless it lingers. The region
    /// that holds the footer's lines is left where it is.
    fn bring_a_region_behind(&mut self, id: &'static str, command: &str, argument: Option<&str>) {
        let Some(entry) = self.regions.entry(id) else {
            return;
        };
        self.hang_a_region_wake(id);
        let way = self.region_way(id);
        let (region, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let region = (entry.open)(command, argument, &mut way);
            (region, std::mem::take(&mut way.owed))
        };
        if region.id() != id || self.regions.keep_hidden(region).is_err() {
            tracing::error!(registered = id, "a region brought behind is not kept");
            self.let_go_of_a_region_wake(id);
            return;
        }
        tracing::info!(region = %id, command = %command, "a region lives hidden");
        self.region_owed(id, owed);
        self.hosted_replay(id);
        let _ = self.with_region_held(id, |r, w| r.command(command, argument, w));
        self.let_a_hidden_region_go(id);
    }

    /// What the footer has heard already, told to a region that comes to
    /// life as if it had just arrived (`Footer::standing`: everything the
    /// system offers, the brain's last mind, the queue's last list, the last
    /// reading of each of the footer's threads and the lines to them) — so
    /// it starts from what the footer knows and not from nothing until the
    /// next reading: a region that opened on nothing would draw "not asked
    /// yet" over a reading the footer already has, and wait for the next one.
    fn hosted_replay(&mut self, id: &'static str) {
        let standing = self.standing();
        let _ = self.with_region_held(id, |r, w| {
            for news in &standing {
                r.heard(news.as_ref(), w);
            }
        });
    }

    /// Region `id` closes: told, and dropped with its wake — or, with work
    /// still in flight (`Region::lingers`), kept HIDDEN, still heard: its
    /// answer is still this door's to say — and the region given back to
    /// whoever is under it. The wake stays while one of it that stepped aside
    /// still lingers (`Slots::linger`).
    pub(super) fn close_region(&mut self, id: &'static str) {
        // A REGION THAT LAYS OUT ITS OWN CELLS named what this hand rested
        // on in its own codes (`Region::rests_on`): they go with it.
        if self.regions.kept_shape(id) == Shape::Cells {
            self.here.row_under_pointer = None;
        }
        let was_shown = self.regions.is_open(id);
        let Some(mut region) = self.regions.take(id) else {
            return;
        };
        // A HAND THIS POINTER HELD OVER IT goes with it.
        if self
            .here
            .region_hand
            .as_ref()
            .is_some_and(|(of, _)| *of == id)
        {
            self.here.region_hand = None;
        }
        let mut way = self.region_way_for_news(id);
        if was_shown {
            region.closing(&mut way);
        }
        let kept = was_shown && region.lingers() && self.regions.keep_hidden(region).is_ok();
        if kept {
            tracing::info!(region = %id, "a region closed with work in flight lives hidden");
        } else {
            self.let_go_of_a_region_wake(id);
            if self.region_asking.is_some_and(|(asker, _)| asker == id) {
                self.region_asking = None;
            }
        }
        self.region_owed(id, way.owed);
        if was_shown {
            self.first_page();
            self.apply_geometry(self.interactivity());
            self.draw_and_paint();
        }
    }

    /// The loop's channel for region `id`'s threads: what they call wakes the
    /// loop, and the region hears [`crate::region::Region::woken`] — or,
    /// through the second wake, [`crate::region::Region::framed`].
    fn hang_a_region_wake(&mut self, id: &'static str) {
        // KEPT WHILE A CLOSED ONE LINGERS: its threads still ring this wake,
        // and the region opened again hears through the same one — and A
        // RESIDENT'S IS HUNG ONCE, for the session.
        if self.region_wakes.iter().any(|w| w.id == id) {
            return;
        }
        let (sender, heard) = channel::channel::<bool>();
        let sender = std::sync::Arc::new(std::sync::Mutex::new(sender));
        let framing = std::sync::Arc::clone(&sender);
        let wake = crate::wake::Wake::new(move || {
            if let Ok(s) = sender.lock() {
                let _ = s.send(false);
            }
        });
        let frame = crate::wake::Wake::new(move || {
            if let Ok(s) = framing.lock() {
                let _ = s.send(true);
            }
        });
        let token = self.repeat.clone().and_then(|handle| {
            handle
                .insert_source(heard, move |event, (), state: &mut Self| {
                    if let channel::Event::Msg(frame) = event {
                        state.region_woken(id, frame);
                    }
                })
                .map_err(|e| tracing::warn!(error = %e, region = id, "the region hears no thread"))
                .ok()
        });
        self.regions.set_wakes(id, Some((wake, frame)));
        self.region_wakes.push(RegionWake { id, token });
    }

    /// The wakes of every feature that keeps a resident, hung once the loop
    /// is: its threads wake the loop with the region closed too.
    pub(super) fn hang_the_residents(&mut self) {
        let ids: Vec<&'static str> = self.regions.resident_ids().collect();
        for id in ids {
            self.hang_a_region_wake(id);
        }
    }

    /// Region `id`'s wake, let go — unless a closed one still lingers, whose
    /// threads ring it until their work answered, or the feature keeps a
    /// resident, whose threads wake it with the region closed.
    fn let_go_of_a_region_wake(&mut self, id: &'static str) {
        if !self.regions.lingers(id) && self.regions.residence(id).is_none() {
            self.unhang_a_region_wake(id);
        }
    }

    /// Region `id`'s wake, gone with it.
    fn unhang_a_region_wake(&mut self, id: &'static str) {
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.region_wakes)
            .into_iter()
            .partition(|w| w.id == id);
        self.region_wakes = kept;
        self.regions.set_wakes(id, None);
        for w in gone {
            if let (Some(token), Some(handle)) = (w.token, self.repeat.as_ref()) {
                handle.remove(token);
            }
        }
    }

    /// A thread of region `id` woke the loop: the ones of it that stepped
    /// aside and still linger (`Slots::linger`) hear it first — what they say
    /// is the answer to a deed told before the live one was — then the live
    /// one, shown or hidden. The wake goes with the last of them.
    ///
    /// The height follows what it holds now, and the frame is OWED, never
    /// painted here: a thread may wake the loop again before it is drawn (a
    /// rebuild's journal, a folder handing back dozens of thumbnails a
    /// second), and a frame per wake would be the burst painted one by one.
    ///
    /// A WAKE THAT BROUGHT NOTHING costs nothing, for a region that says
    /// what a wake brought (`Region::says_what_a_wake_brought`): a thread
    /// posts three readings with a wake each, and the first wake took all
    /// three — the later ones ask nothing of the door, and are neither a
    /// frame nor a geometry. News for the footer's own marks is not the
    /// region speaking: the marks draw themselves when they moved
    /// (`Footer::region_news`). The live one's call is `with_region`'s,
    /// written out, so what it asked is looked at before it is done.
    fn region_woken(&mut self, id: &'static str, frame: bool) {
        // A PICTURE'S WAKE: only a paint is owed, for the open region whose
        // thread brought it (`Region::framed`) — a film's frames are no news
        // for anything else.
        if frame {
            if self.regions.is_open(id) && self.framing(id, |r, _, _| r.framed()) == Some(true) {
                self.ask_paint();
            }
            return;
        }
        // WITH THE REGION CLOSED — neither live nor lingering — the
        // feature's resident hears it.
        if !self.regions.is_live(id) && !self.regions.lingers(id) {
            let Some(kept) = self.regions.residence(id) else {
                return;
            };
            let mut way = self.region_way_for_news(id);
            kept.hooks.borrow_mut().woken(&mut way);
            if !way.owed.is_empty() {
                self.region_owed(id, way.owed);
                self.after_a_region_spoke();
            }
            return;
        }
        let height = self.expansion_rows();
        let mut spoke = false;
        if self.regions.lingers(id) {
            let mut way = self.region_way_for_news(id);
            self.regions.call_lingering(id, &mut way, |r, w| r.woken(w));
            self.lingering_owed(way.owed);
            if !self.regions.is_live(id) {
                self.let_go_of_a_region_wake(id);
            }
            spoke = true;
        }
        let says = self
            .regions
            .get(id)
            .is_some_and(crate::region::Region::says_what_a_wake_brought);
        let way = self.region_way_for_news(id);
        let (called, owed) = {
            let mut way = way.lending(crate::region::Lent::of(held_by_the_footer!(self)));
            let called = self.regions.call(id, &mut way, |r, w| r.woken(w)).is_some();
            (called, std::mem::take(&mut way.owed))
        };
        if called {
            spoke |= !says || owed.iter().any(|e| !matches!(e, Effect::News(_)));
            self.region_owed(id, owed);
            self.let_a_hidden_region_go(id);
        }
        if !spoke {
            return;
        }
        // AND THE PAGE IS CLAMPED to the rows a shown one has now, as after a
        // region spoke (`after_a_region_spoke`): a sweep that came back, a
        // list that got shorter.
        if self.regions.is_open(id) {
            self.settle_region();
        }
        if self.expansion_rows() != height {
            self.apply_geometry(self.interactivity());
        }
        self.ask_paint();
    }

    /// After a region did something: the height and the frame follow what
    /// it now holds, and the page is CLAMPED to the rows it has now — a sweep
    /// that came back, a list that got shorter (`settle_region`). (The page
    /// follows its cursor when the region asks — `Effect::Follow` — and after
    /// a key, `Footer::typed`: the module says who moves the page; data that
    /// arrives does not move it.)
    fn after_a_region_spoke(&mut self) {
        self.settle_region();
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// After a click on a region's row: the height and the page clamped —
    /// and the page moved ONLY to bring back a cursor the click left off it
    /// (a second click's `RET` that put it on another level), as every
    /// other view's click leaves its page. A region that follows its cursor
    /// after the click that acts (`Effect::Follow`) has it on the page
    /// already, and nothing more moves.
    ///
    /// # Why a click does not follow the cursor's margin
    ///
    /// Because the row is under the pointer. The first click selects it and
    /// the second, on the same spot, does what `RET` does (invariant 19); a
    /// page that followed the cursor with `SCROLL_MARGIN` after the first
    /// one slid a row in the last two lines out from under the pointer, the
    /// second click landed on another row — a first click again — and the
    /// page slid again: a row on the bottom edge could not be acted on.
    fn after_a_click(&mut self) {
        self.apply_geometry(self.interactivity());
        self.settle_region();
        let offset = self.current_offset();
        if a_click_moves_the_page(self.list_cursor(), offset, self.expansion_ceiling()) {
            self.follow_cursor();
        }
        self.draw_and_paint();
    }

    /// THE LINE MOVES ON without the answer to a region's question — the
    /// finder (`LEADER :`), the buffer finder, another question, the
    /// keyboard gone: the question is over, and the region lets it go, what
    /// it kept for the answer with it, as after `Esc`
    /// ([`Footer::hosted_unanswered`]). Without this the footer went on
    /// taking a later command of the same id, run with an argument, for the
    /// answer — and the region, for the question it had asked.
    pub(super) fn the_line_moves_on(&mut self) {
        if self.region_asking.is_some() {
            self.hosted_unanswered();
        }
    }

    /// The minibuffer's question was let go: the region that asked it lets
    /// it go too (`Footer::questions_let_go`), shown or hidden.
    pub(super) fn hosted_unanswered(&mut self) {
        if let Some((asker, question)) = self.region_asking.take() {
            if self.regions.is_live(asker) {
                let _ = self.with_region(asker, |r, w| r.unanswered(question, w));
            } else if let Some(kept) = self.regions.residence(asker) {
                // ASKED WITH THE REGION CLOSED: what its resident kept for
                // the answer goes with the question.
                kept.hooks.borrow_mut().let_go(question);
            }
        }
    }

    /// The clipboard's text a region asked for (`Footer::pasted`).
    pub(super) fn hosted_pasted(&mut self, id: &'static str, text: &str) {
        if self.with_region(id, |r, w| r.pasted(text, w)).is_some() {
            self.after_a_region_spoke();
        }
    }

    /// Whether the shown region takes files dropped on it.
    pub(super) fn hosted_takes_a_drop(&self) -> bool {
        self.shown_region()
            .and_then(|id| self.regions.get(id))
            .is_some_and(crate::region::Region::takes_drops)
    }

    /// Files let go over the shown region: whether it took them.
    pub(super) fn hosted_dropped(&mut self, paths: &[std::path::PathBuf], copies: bool) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        let took = self
            .with_region(id, |r, w| r.dropped(paths, copies, w))
            .unwrap_or(false);
        if took {
            self.after_a_region_spoke();
        }
        took
    }

    /// What the footer heard from the machine, told to every LIVE region —
    /// a hidden one waits on it too — and whoever knows its type reads it
    /// (`Region::heard`).
    ///
    /// TOLD BY THE TYPE NAMED HERE (`hosted_heard::<T>`), where the compiler
    /// holds the argument to it, so the type a region downcasts to can be
    /// paired with one the footer tells (`regions-hear-what-the-footer-tells`):
    /// a reading told as one type and read as another is a region that never
    /// refreshes, and nothing else would say so.
    pub(super) fn hosted_heard<T: std::any::Any>(&mut self, news: &T) {
        // NOTHING LIVE AND NOBODY RESIDENT, NOTHING TOLD — and nothing
        // allocated on the feeds' path, which every reading of the machine
        // takes, unless a region is live or a resident hears it.
        if self.regions.live_ids().next().is_none() && self.regions.resident_ids().next().is_none()
        {
            return;
        }
        // A REGION THAT CHANGED SAYS HOW: `Doorway::repaint` when only what a
        // row says changed (a meter's level, ten times a second — a frame
        // when its bar moved a cell, and nothing more), `Doorway::rows_changed`
        // when rows came or went (the page clamps and the height follows,
        // `Effect::RowsChanged`). A reading it ignores is no frame, and
        // nothing here builds its rows to find out.
        //
        // A REGION THAT DOES NOT SAY HOW (`Region::says_when_its_rows_change`)
        // has the height follow its rows after any frame it asks for here:
        // one that changed what it holds (a directory read again when a copy
        // ended) would otherwise be painted into a surface sized for the
        // shorter listing.
        let owed = std::mem::take(&mut self.owes_paint);
        let live: Vec<&'static str> = self.regions.live_ids().collect();
        let mut measure = false;
        for id in live {
            let says = self
                .regions
                .get(id)
                .is_some_and(crate::region::Region::says_when_its_rows_change);
            let before = std::mem::take(&mut self.owes_paint);
            let _ = self.with_region_for_news(id, |r, w| r.heard(news, w));
            measure |= self.owes_paint && !says;
            self.owes_paint |= before;
        }
        if measure {
            self.apply_geometry(self.interactivity());
        }
        self.owes_paint |= owed;
        // AND A CLOSED REGION'S RESIDENT, which keeps what the feature's
        // commands ask with the region closed — only one that hears this
        // kind of news (`Resident::hears`): a doorway is made for it alone,
        // and a reading nobody hears allocates nothing.
        let news: &dyn std::any::Any = news;
        let resting: Vec<(&'static str, crate::region::Residence)> = self
            .regions
            .resident_ids()
            .filter(|id| !self.regions.is_live(id))
            .filter_map(|id| {
                self.regions
                    .residence(id)
                    .filter(|kept| kept.hooks.borrow().hears(news))
                    .map(|kept| (id, kept))
            })
            .collect();
        for (id, kept) in resting {
            let mut way = self.region_way_for_news(id);
            kept.hooks.borrow_mut().heard(news, &mut way);
            if !way.owed.is_empty() {
                self.region_owed(id, way.owed);
            }
        }
    }

    /// The language changed: every open region says again what it keeps in
    /// words (`Region::language_changed`).
    pub(super) fn hosted_language_changed(&mut self) {
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        for id in open {
            let _ = self.with_region(id, |r, w| r.language_changed(w));
        }
    }

    /// The dates' shape changed: every open region hears it.
    pub(super) fn hosted_locale_changed(&mut self) {
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        for id in open {
            let _ = self.with_region(id, |r, w| r.locale_changed(w));
        }
    }

    /// How many lines the region shows, told to the shown region before a
    /// key reaches it (`Footer::pickers_see_the_page`).
    pub(super) fn hosted_page_is(&self, lines: usize) {
        if let Some(region) = self.shown_region().and_then(|id| self.regions.get(id)) {
            region.page_is(lines);
        }
    }

    /// The row the shown region wants at the top of the region, once
    /// (`Region::take_anchor`, `Footer::took_the_anchor`).
    pub(super) fn hosted_take_anchor(&mut self) -> Option<usize> {
        let id = self.shown_region()?;
        self.regions
            .get_mut(id)
            .and_then(crate::region::Region::take_anchor)
    }

    /// The box every band of a region's list is drawn in, on a line of
    /// `columns` cells — out of the same `extent` the rows were built with: a
    /// ground measured from another width than the text inside it cuts its
    /// own rows.
    ///
    /// # Why it takes the columns instead of asking
    ///
    /// Because the drawing runs once per SCREEN and `screen_columns()` is the
    /// first one's. With two outputs of different widths the box measured on
    /// the wide one was drawn on the narrow one, and on that screen its bar
    /// landed in the very cell the page's bar occupies — the collision this
    /// box exists to end, on the monitor nobody was looking at while fixing
    /// it. The pointer sites pass `screen_columns()`, which is the screen the
    /// pointer is on, and that is the same number they resolve rows with.
    pub(super) fn shelf_box_of(columns: u16) -> crate::chrome::ShelfBox {
        let (left, room) = crate::chrome::extent(columns);
        crate::chrome::shelf_box(left, room)
    }

    /// A band on screen line `line`, cut to the box of the shelf it falls in.
    ///
    /// # Why a band has to be cut at all
    ///
    /// Because the hover band says WHICH ROW the pointer is on, and a row
    /// inside a shelf is as wide as the shelf. A band running the width of
    /// the region would leave the box on one side and reach the page's bar on
    /// the other, saying that the row extends where it does not. It is the
    /// rule `fields::band_of` applies to the manager's panes.
    ///
    /// THE BANDS COME IN, and are not asked for again: `hosted_shelf_bands`
    /// asks the region for its whole list, and this door repaints on pointer
    /// motion.
    pub(super) fn cut_to_the_shelf(
        bands: &[(u16, usize, crate::region::Shelf)],
        shelf: crate::chrome::ShelfBox,
        line: u16,
        at: f32,
        wide: f32,
        cell: f32,
    ) -> (f32, f32) {
        let Some((_, seen, band)) = bands
            .iter()
            .find(|(first, seen, _)| line >= *first && usize::from(line - *first) < *seen)
        else {
            return (at, wide);
        };
        let left = f32::from(shelf.from) * cell;
        // AND IT STOPS BEFORE THE SHELF'S OWN BAR — WHILE ONE IS DRAWN: a band
        // under the thumb would tint the one thing on that column that is not
        // a row, and a band that stopped short with no bar there would be a
        // band with a hole in it. The condition is the one the bar is drawn by
        // (`hosted_shelf_bar_at`): the whole band on screen, and more than it
        // holds.
        let barred = *seen >= band.lines && band.total > band.lines;
        let keep = if barred { crate::chrome::BAR_CELLS } else { 0 };
        let right = f32::from(shelf.from + shelf.cells - keep) * cell;
        let from = at.max(left);
        ((from), (right - from).clamp(0.0, wide))
    }

    /// The shown region's shelves ([`crate::region::Shelf`]) with rows on
    /// screen, and which screen line each one starts on and how many of its
    /// lines are on it.
    ///
    /// THE VISIBLE SLICE, and not only the shelves that fit whole: the GROUND
    /// follows whatever is on screen, because a shelf half out of the region
    /// still has to say where it is. Only the BAR asks for the whole band —
    /// see the drawing site.
    pub(super) fn hosted_shelf_bands(&self) -> Vec<(u16, usize, crate::region::Shelf)> {
        let Some(region) = self.shown_region().and_then(|id| self.regions.get(id)) else {
            return Vec::new();
        };
        let shelves = region.shelves(&self.region_seen());
        if shelves.is_empty() {
            return Vec::new();
        }
        let offset = self.current_offset();
        let ceiling = self.expansion_ceiling();
        shelves
            .into_iter()
            .filter_map(|b| {
                let from = b.first.max(offset);
                let to = (b.first + b.lines).min(offset + ceiling);
                if to <= from {
                    return None;
                }
                let line = self.first_of_expansion() + u16::try_from(from - offset).ok()?;
                Some((line, to - from, b))
            })
            .collect()
    }

    /// The shelf whose BAR is under `(x, y)`, if any — asking exactly what
    /// the drawing asked (`hosted_shelf_bands`), so what can be grabbed is
    /// what was drawn: only a band wholly inside the region has a bar.
    fn hosted_shelf_bar_at(&self, x: f64, y: f64) -> Option<usize> {
        let (w, h) = self.pointer_screen()?;
        #[allow(clippy::cast_possible_truncation)]
        let (px, py) = (x as f32, y as f32);
        self.hosted_shelf_bands()
            .into_iter()
            .find_map(|(line, seen, b)| {
                if seen < b.lines || b.total <= b.lines {
                    return None;
                }
                let inner = crate::chrome::list_scroll(b.total, b.lines, b.offset);
                let shelf = Self::shelf_box_of(self.screen_columns());
                crate::chrome::on_the_band(inner, shelf, line, w, h, px, py).then_some(b.id)
            })
    }

    /// Scrolls the shown region's shelf `which` from a drag of its own bar.
    fn hosted_drag_shelf_to(&mut self, which: usize, y: f64) {
        let Some((w, h)) = self.pointer_screen() else {
            return;
        };
        #[allow(clippy::cast_possible_truncation)]
        let py = y as f32;
        let Some((line, _, band)) = self
            .hosted_shelf_bands()
            .into_iter()
            .find(|(_, _, b)| b.id == which)
        else {
            return;
        };
        let inner = crate::chrome::list_scroll(band.total, band.lines, band.offset);
        let at = crate::chrome::shelf_offset_at(inner, line, w, h, py);
        let Some(id) = self.shown_region() else {
            return;
        };
        if self
            .with_region(id, |r, w| r.shelf_scrolled(which, at, w))
            .is_some()
        {
            self.owes_paint = true;
        }
    }

    /// A shelf's bar, held and dragged to `at`: whether one is held.
    pub(super) fn hosted_shelf_dragged(&mut self, at: (f64, f64)) -> bool {
        let Some(which) = self.here.dragging_shelf else {
            return false;
        };
        self.hosted_drag_shelf_to(which, at.1);
        true
    }

    /// A press on a shelf's bar — whether it was one.
    pub(super) fn hosted_shelf_pressed(&mut self, at: (f64, f64)) -> bool {
        // A SHELF'S BAR FIRST. It is drawn at its box's right edge and the
        // region's at the surface's, so the two are different columns — but
        // the region's TRACK spans the whole height, so asking it first would
        // let it answer for a press that landed on a shelf's thumb and drag
        // the page instead of the shelf.
        let Some(which) = self.hosted_shelf_bar_at(at.0, at.1) else {
            return false;
        };
        self.here.dragging_shelf = Some(which);
        self.hosted_drag_shelf_to(which, at.1);
        true
    }

    /// The footer's second, told to every live region: whether a SHOWN one
    /// changed — a hidden one has nothing on the screen to change. A region
    /// whose rows change length on it asks the surface to follow
    /// (`Doorway::follow`) — a question with a clock that ran out gives back
    /// the longer sheet it stood over.
    pub(super) fn hosted_tick(&mut self, now: std::time::Instant) -> bool {
        if self.regions.live_ids().next().is_none() {
            return false;
        }
        let live: Vec<&'static str> = self.regions.live_ids().collect();
        let mut changed = false;
        for id in live {
            let shown = self.regions.is_open(id);
            let moved = self
                .with_region_for_news(id, |r, w| r.tick(now, w))
                .unwrap_or(false);
            changed |= moved && shown;
        }
        changed
    }

    /// EVERY OPEN REGION LETS GO, as the footer ends: each is told it is
    /// closing (`Region::closing`) — a standing change is put back, a thread
    /// let go of — and dropped with what it holds. What a region asks on its
    /// way out is not done: the session it would be done for is ending.
    pub(super) fn let_every_region_go(&mut self) {
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        for id in open {
            if let Some(mut region) = self.regions.take(id) {
                let mut way = self.region_way_for_news(id);
                region.closing(&mut way);
                drop(region);
                self.unhang_a_region_wake(id);
            }
        }
        self.region_asking = None;
    }

    /// Where the shown region is on a surface `width` by `bottom`.
    fn region_area(&self, width: f32, bottom: f32) -> crate::region::Area {
        crate::region::Area {
            width,
            bottom,
            first: self.first_of_expansion(),
            lines: u16::try_from(self.grid().lines).unwrap_or(u16::MAX),
            columns: self.screen_columns(),
        }
    }

    /// What the shown region paints over its rows itself, on a surface
    /// `width` by `bottom` (`Footer::paint`), and the pictures it paints.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn hosted_paint(
        &self,
        width: f32,
        bottom: f32,
        under: &mut Vec<ronoctua_render::quad::Quad>,
        over: &mut Vec<ronoctua_render::quad::Quad>,
        spans: &mut Vec<ronoctua_render::Span>,
        bars: &mut crate::bars::Drawn,
        screen: crate::region::Screen,
        pictures: &mut Vec<crate::region::Painted>,
    ) {
        let Some(id) = self.shown_region() else {
            return;
        };
        let Some(region) = self.regions.get(id) else {
            return;
        };
        let hand = self
            .here
            .region_hand
            .as_ref()
            .filter(|(of, _)| *of == id)
            .map(|(_, held)| held.as_ref());
        // EVERY HAND'S TARGET, in its own colour — the window's rule, from
        // the footer's own hands: the code each rests on is the region's
        // (`Region::rests_on`).
        let lit: Vec<(tokens::Color, usize)> = self
            .pointers
            .lit(&self.here)
            .filter_map(|(c, p)| p.row_under_pointer.map(|n| (c, n)))
            .collect();
        let mut easel = crate::region::RegionEasel {
            area: self.region_area(width, bottom),
            texts: &self.texts,
            offset: self.current_offset(),
            under,
            over,
            spans,
            bars,
            lent: crate::region::Lent::of(held_by_the_footer!(self)),
            lit: &lit,
            pictures,
            placed: &self.region_placed,
            screen,
            hand,
        };
        region.paint(&mut easel);
    }

    /// The pointer over a region that lays out its own cells
    /// ([`Shape::Cells`]): whether it took the frame. A list is the footer's
    /// to point at, row by row, as every other view's is.
    pub(super) fn hosted_took_the_pointer(&mut self, events: &[PointerEvent]) -> bool {
        let Some(id) = self.shown_region() else {
            return false;
        };
        if self.regions.kept_shape(id) != Shape::Cells {
            return false;
        }
        let Some((w, h)) = self.pointer_screen() else {
            return false;
        };
        let area = self.region_area(w, h);
        let pointed: Vec<crate::region::Pointed> = events
            .iter()
            .map(|e| crate::region::Pointed {
                at: e.position,
                pointing: match e.kind {
                    PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                        crate::region::Pointing::Moved
                    }
                    PointerEventKind::Leave { .. } => crate::region::Pointing::Left,
                    PointerEventKind::Press { button, .. } => {
                        crate::region::Pointing::Pressed(button)
                    }
                    PointerEventKind::Release { button, .. } => {
                        crate::region::Pointing::Released(button)
                    }
                    PointerEventKind::Axis {
                        vertical,
                        horizontal,
                        ..
                    } => crate::region::Pointing::Wheel {
                        vertical,
                        horizontal,
                    },
                },
            })
            .collect();
        let moved = events.iter().any(|e| {
            matches!(
                e.kind,
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. }
            )
        });
        let (took, rests) = self
            .with_region(id, |r, w| (r.pointer(&pointed, &area, w), r.rests_on()))
            .unwrap_or((false, None));
        // WHAT THIS HAND RESTS ON, kept by the hand — each lit in its own
        // colour (`Footer::hosted_paint`); a frame that did not move it keeps
        // what it rested on. ONLY WHILE THE REGION IS STILL UP: a frame that
        // moved and then pressed what opens a hit closed it on the way
        // (`close_region` let its codes go), and its code kept now would be
        // read as a row of whatever the region gave the footer back to.
        if let Some(rests) = rests
            && self.regions.kept_shape(id) == Shape::Cells
        {
            let had = self.here.row_under_pointer.is_some();
            self.here.row_under_pointer = rests;
            if !took && had {
                self.ask_paint();
            }
        }
        if took {
            if moved {
                self.here.under_pointer = true;
            }
            self.ask_paint();
        }
        took
    }

    /// WHAT THE SHOWN REGION'S THREADS ARE TO BE ASKED FOR NOW, once a
    /// frame and before it (`Region::before_frame`): the width and the lines
    /// its listing has, the scale its pictures are made for, and the rows of
    /// its list the page shows, in the frame's words — the pictures of the
    /// visible page only, before the frame borrows anything.
    pub(super) fn hosted_before_frame(&mut self) {
        let Some(id) = self.shown_region() else {
            return;
        };
        let grid = self.grid();
        let (columns, scale, first) = (
            self.screen_columns(),
            self.scale_now(),
            self.current_offset(),
        );
        let framing = crate::region::Framing {
            columns,
            room: grid.lines,
            scale,
            first,
            page: grid.per_page(),
            texts: &self.texts,
        };
        if let Some(r) = self.regions.get_mut(id) {
            r.before_frame(&framing);
        }
    }

    /// The frame was painted: what the shown region handed up is up.
    pub(super) fn hosted_after_paint(&mut self) {
        if let Some(r) = self.shown_region().and_then(|id| self.regions.get_mut(id)) {
            r.after_paint();
        }
    }

    /// After the frame, or a turn of the hand: what the frame measured, told
    /// to the shown region's threads (`Region::after_frame`).
    pub(super) fn hosted_after_frame(&mut self) {
        if let Some(r) = self.shown_region().and_then(|id| self.regions.get_mut(id)) {
            r.after_frame();
        }
    }

    /// The shown region's own clock, armed while it wants one
    /// (`Region::clock`) and dropped by its own beat when it does not.
    ///
    /// # Why this one is armed, when nothing else here is
    ///
    /// Because a name walking sideways is the only thing in this footer that
    /// moves faster than the second it already wakes for — five cells a
    /// second — and because it is usually not happening at all. Riding the
    /// second would make the walk a crawl; a permanent timer would wake the
    /// loop five times a second for the whole session so a name could move on
    /// the few frames one rests on it.
    pub(super) fn arm_the_region_clock(&mut self) {
        if self.region_clock_armed {
            return;
        }
        let Some(step) = self
            .shown_region()
            .and_then(|id| self.regions.get(id))
            .and_then(crate::region::Region::clock)
        else {
            return;
        };
        let Some(handle) = self.repeat.clone() else {
            return;
        };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(step),
            move |_, (), state: &mut Self| {
                let now = std::time::Instant::now();
                // NO REGION WANTING IT, NO CLOCK: the region answers `None`
                // the moment nothing walks — the cursor moved, the column got
                // wider, the region closed.
                let Some(id) = state.shown_region() else {
                    state.region_clock_armed = false;
                    return calloop::timer::TimeoutAction::Drop;
                };
                let Some(step) = state.regions.get(id).and_then(crate::region::Region::clock)
                else {
                    state.region_clock_armed = false;
                    return calloop::timer::TimeoutAction::Drop;
                };
                // A BEAT THAT MOVES NOTHING ASKS FOR NO FRAME.
                if state.regions.get_mut(id).is_some_and(|r| r.clocked(now)) {
                    state.draw_and_paint();
                }
                calloop::timer::TimeoutAction::ToDuration(step)
            },
        );
        match armed {
            Ok(_) => self.region_clock_armed = true,
            Err(e) => tracing::warn!(error = %e, "could not hang the region's clock"),
        }
    }

    /// The keyboard went away: every open region that goes with it goes
    /// (`Region::closes_without_the_keyboard`).
    pub(super) fn hosted_lost_the_keyboard(&mut self) {
        let going: Vec<&'static str> = self
            .regions
            .open_ids()
            .filter(|id| {
                self.regions
                    .get(id)
                    .is_some_and(crate::region::Region::closes_without_the_keyboard)
            })
            .collect();
        for id in going {
            self.close_region(id);
        }
    }

    /// The language changed: every open region speaks it.
    pub(super) fn hosted_speak(&mut self) {
        let texts = std::rc::Rc::clone(&self.texts);
        let open: Vec<&'static str> = self.regions.open_ids().collect();
        for id in open {
            if let Some(r) = self.regions.get_mut(id) {
                r.speak(std::rc::Rc::clone(&texts));
            }
        }
    }

    /// The shape the shown region was kept with — a stored fact, safe under
    /// `metric` (the module says why).
    pub(super) fn hosted_shape(&self) -> Option<Shape> {
        match self.view() {
            ViewState::Region(id) if self.regions.is_open(id) => Some(self.regions.kept_shape(id)),
            _ => None,
        }
    }

    /// Whether the shown region is the film's stage — what lets it past the
    /// footer's ceilings (`Footer::expansion_ceiling`).
    pub(super) fn hosted_stage(&self) -> bool {
        self.hosted_shape() == Some(Shape::Stage)
    }
}

/// Whether row `row` is on a page of `rows` lines that starts at `offset`.
const fn on_the_page(row: usize, offset: usize, rows: usize) -> bool {
    row >= offset && row < offset.saturating_add(rows)
}

/// Whether the page moves after a click ([`Footer::after_a_click`]): only
/// when the cursor is off the page of `rows` lines at `offset` — the row
/// clicked, still under the pointer, never moves.
const fn a_click_moves_the_page(cursor: Option<usize>, offset: usize, rows: usize) -> bool {
    match cursor {
        Some(row) => !on_the_page(row, offset, rows),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{a_click_moves_the_page, on_the_page};

    /// FAILS IF: a click on a row of the page moves the page — the last two
    /// lines included, where the cursor's margin would have slid it out from
    /// under the pointer — or a cursor a second click put off the page is
    /// left out of sight, or a view with no cursor moves at all.
    #[test]
    fn a_click_leaves_the_row_under_the_pointer() {
        let (offset, rows) = (10, 8);
        for row in offset..offset + rows {
            assert!(on_the_page(row, offset, rows), "row {row}");
            assert!(
                !a_click_moves_the_page(Some(row), offset, rows),
                "a click on row {row}, on the page, moved it"
            );
        }
        assert!(
            a_click_moves_the_page(Some(offset - 1), offset, rows),
            "above"
        );
        assert!(
            a_click_moves_the_page(Some(offset + rows), offset, rows),
            "below"
        );
        assert!(
            a_click_moves_the_page(Some(0), offset, rows),
            "a new level's top"
        );
        assert!(!a_click_moves_the_page(None, offset, rows));
        assert!(!on_the_page(0, 0, 0), "an empty page holds no row");
        assert!(
            on_the_page(usize::MAX - 1, usize::MAX - 1, 5),
            "no overflow"
        );
    }
}
