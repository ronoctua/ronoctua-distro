//! THE FOOTER'S CLIPBOARD HISTORY — the service, not the view: the history
//! its own watcher and ClipCascade fill, which both of the clipboard's doors
//! follow and act on (`crate::clipboard_wire`), the sync's thread, and the
//! modeline's mark.
//!
//! # Why this stays in the footer when the view left
//!
//! One watcher of the clipboard is one history. The footer holds the
//! `zwlr_data_control` watcher for the whole session — the history is kept
//! while no view of it is open, and the transfer queue's "add what was
//! copied" reads it — so the history is the footer's, and the VIEW of it is
//! a region of its own crate (`ronoctua-shell-clipboard`), which follows it as
//! the window does: told the whole history whenever it moves
//! ([`Footer::clipboard_told`]), and asking for a change by an act aimed at
//! the history it was shown ([`Footer::clipboard_acted`]). An act is carried
//! out here, by index, whichever door it came from.

use super::*;

impl Footer {
    /// The history, to change: the footer's own — copied first when a region
    /// still holds the one it was told (`clipboard_wire::Held`), so what that
    /// region shows stays what it was told until it is told again.
    pub(super) fn clipboard_mut(&mut self) -> &mut ronoctua_clipboard::Queue {
        if std::rc::Rc::get_mut(&mut self.clipboard).is_none() {
            let copy =
                ronoctua_clipboard::Queue::of_entries(self.clipboard.inputs().iter().cloned());
            self.clipboard = std::rc::Rc::new(copy);
        }
        std::rc::Rc::get_mut(&mut self.clipboard)
            .unwrap_or_else(|| unreachable!("a history copied a line above is held by nobody else"))
    }

    /// The history as the regions the footer hosts are told it: shared.
    pub(super) fn clipboard_held(&self) -> crate::clipboard_wire::Held {
        crate::clipboard_wire::Held {
            queue: std::rc::Rc::clone(&self.clipboard),
            sync: self.clipcascade.clone(),
            generation: self.clip_generation,
        }
    }

    /// The history as it is now, for whoever follows it — with what the
    /// footer said in answer to a window's act, if anything.
    fn clipboard_shot(&self, said: Option<String>) -> crate::clipboard_wire::Shot {
        crate::clipboard_wire::Shot {
            entries: self.clipboard.inputs().iter().cloned().collect(),
            sync: self.clipcascade.clone(),
            generation: self.clip_generation,
            said,
        }
    }

    /// The clipboard's ENTRIES moved: a new generation, and every door
    /// following them told (`Footer::clipboard_told`).
    pub(super) fn clipboard_moved(&mut self) {
        self.clip_generation = self.clip_generation.wrapping_add(1);
        self.clipboard_told(None);
    }

    /// Every door following the history told what it is now: every window
    /// following it — the history CLONED ONCE and shared, and turned into a
    /// line on each follower's own thread — with what the footer said to
    /// them, if anything; and every region the footer hosts, which is told
    /// nothing a window was answered. With no door open this is nothing.
    pub(super) fn clipboard_told(&mut self, said: Option<String>) {
        if !self.clip_followers.is_empty() {
            let shot = std::sync::Arc::new(self.clipboard_shot(said));
            // A WINDOW THAT CLOSED ended its thread; its sender says so here.
            self.clip_followers
                .retain(|tx| tx.send(std::sync::Arc::clone(&shot)).is_ok());
        }
        if self.regions.open_ids().next().is_some() {
            let held = self.clipboard_held();
            self.hosted_heard::<crate::clipboard_wire::Held>(&held);
        }
    }

    /// A clipboard window asked to follow the history: told it now.
    pub(super) fn clipboard_followed(&mut self, stream: crate::leader::Follower) {
        let Some(tx) = crate::clipboard_wire::follower(stream.0) else {
            return;
        };
        let shot = std::sync::Arc::new(self.clipboard_shot(None));
        if tx.send(shot).is_ok() {
            tracing::info!("a clipboard window follows the history");
            self.clip_followers.push(tx);
        }
    }

    /// What a clipboard WINDOW's key asked of the history, carried out — and
    /// what the footer said in answer goes back to that window, to be said
    /// on its own line. An act aimed at a history that has moved since is
    /// refused: its index would be another entry's.
    pub(super) fn clipboard_asked(&mut self, generation: u64, act: crate::clipboard_wire::Act) {
        if generation != self.clip_generation {
            tracing::info!(
                ?act,
                generation,
                now = self.clip_generation,
                "a clipboard act for a history that moved"
            );
            // REFUSED, AND SAID: the window shows the history it has now and
            // a sentence on its line, never a key that silently did nothing.
            let said = self.texts.t(crate::key::CLIPBOARD_ACT_STALE);
            self.clipboard_told(Some(said));
            return;
        }
        self.clip_answering = Some(Vec::new());
        let _ = self.clipboard_carry_out(act);
        let said = self.clip_answering.take().and_then(|mut s| s.pop());
        self.clipboard_told(said);
        self.draw_and_paint();
    }

    /// What the footer's own REGION of the history asked
    /// (`Doorway::clipboard`), carried out once it is back in its slot — and
    /// said on this door's line, which is the region's.
    pub(super) fn clipboard_acted(&mut self, generation: u64, act: crate::clipboard_wire::Act) {
        if generation != self.clip_generation {
            tracing::info!(
                ?act,
                generation,
                now = self.clip_generation,
                "a clipboard act for a history that moved"
            );
            let said = self.texts.t(crate::key::CLIPBOARD_ACT_STALE);
            self.echo_message(said);
            self.clipboard_told(None);
            return;
        }
        // A READING CHANGED BY THE ACT ITSELF — the switch, the account, a
        // setting — is told at once: the region's row answers the key, as
        // the window's does when the footer answers it.
        if self.clipboard_carry_out(act) {
            self.clipboard_told(None);
        }
    }

    /// One act on the history, whichever door asked it. Whether it changed
    /// ClipCascade's reading without moving the history — a move tells every
    /// door by itself (`clipboard_moved`).
    fn clipboard_carry_out(&mut self, act: crate::clipboard_wire::Act) -> bool {
        use crate::clipboard_wire::Act;
        match act {
            Act::Copy { n } => {
                self.clip_copy(n);
                false
            }
            Act::Remove { n } => {
                if self.clipboard_mut().remove(n) {
                    self.clipboard_moved();
                }
                false
            }
            Act::Pin { n } => {
                if self.clipboard_mut().toggle_pinned(n).is_some() {
                    self.clipboard_moved();
                }
                false
            }
            Act::Clear => {
                self.clipboard_mut().clear();
                self.clipboard_moved();
                false
            }
            Act::Sync { on } => {
                // THE SNAPSHOT MOVES AT ONCE, and that is the display view's
                // `text_pending`: the row answers the key at once, and the
                // thread's reading confirms it a moment later.
                self.clipcascade.enabled = on;
                self.tell_clipcascade(ronoctua_clipcascade::Order::Enable(on));
                let said = self.texts.t(if on {
                    crate::key::ECHO_CLIPBOARD_SYNCED
                } else {
                    crate::key::ECHO_CLIPBOARD_SYNC_OFF
                });
                self.echo_message(said);
                true
            }
            Act::LogOut => {
                self.tell_clipcascade(ronoctua_clipcascade::Order::LogOut);
                self.clipcascade = ronoctua_clipcascade::Snapshot::default();
                let said = self.texts.t(crate::key::ECHO_CLIPBOARD_SYNC_OFF);
                self.echo_message(said);
                true
            }
            Act::Keep {
                salt,
                rounds,
                images,
                notify,
                cap,
                mark,
            } => {
                self.clip_keep(ronoctua_clipcascade::prefs::Settings {
                    salt,
                    rounds,
                    images,
                    notify,
                    cap,
                    mark,
                });
                true
            }
            // THE PASSWORD GOES TO CLIPCASCADE AND NOWHERE ELSE: it goes
            // into the order, which derives the key and drops it.
            Act::LogIn {
                server,
                username,
                password,
            } => {
                self.tell_clipcascade(ronoctua_clipcascade::Order::LogIn {
                    server,
                    username,
                    password,
                });
                false
            }
            Act::Typed { id, text } => self.clip_typed(&id, &text),
        }
    }

    /// Entry `n` back on the clipboard, by THE SAME path the buffer uses to
    /// copy: two ways of putting something on the clipboard would diverge
    /// the day one of them changed.
    ///
    /// AN IMAGE IS NOT RE-COPIED YET: `wl-copy` takes text on stdin, and
    /// doing it by bytes would require telling it the type. It stays in
    /// docs/PENDING.md with the rest of the image.
    fn clip_copy(&mut self, n: usize) {
        let Some(e) = self.clipboard.at(n) else {
            return;
        };
        let Some(text) = e.text().map(ToOwned::to_owned) else {
            self.echo_message(self.texts.t(crate::key::ECHO_COPY_ONLY_TEXT));
            return;
        };
        match crate::clipboard_copy::copy_to_clipboard(&text) {
            Ok(()) => {
                tracing::info!(bytes = text.len(), "recopied from the history");
                self.echo_message(self.texts.t(crate::key::ECHO_COPIED));
            }
            Err(err) => {
                tracing::error!(error = %err, "copying from the history FAILED");
                self.echo_message(self.texts.t(crate::key::ECHO_COPY_FAILED));
            }
        }
    }

    /// The settings that are not the account's, kept: THE WHOLE RECORD, so
    /// the file never holds half of two changes. The snapshot takes them at
    /// once for the same reason as the switch, and the boundary is what
    /// writes.
    fn clip_keep(&mut self, settings: ronoctua_clipcascade::prefs::Settings) {
        self.clipcascade.salt.clone_from(&settings.salt);
        self.clipcascade.rounds = settings.rounds;
        self.clipcascade.images = settings.images;
        self.clipcascade.notify = settings.notify;
        self.clipcascade.cap = settings.cap;
        self.clipcascade.mark = settings.mark;
        self.tell_clipcascade(ronoctua_clipcascade::Order::Keep(Box::new(settings)));
    }

    /// THE THREE TYPED SETTINGS. Each keeps the WHOLE record, taken from the
    /// reading on screen with one field replaced — the same merge the ring
    /// does, for the same reason. Whether one was kept.
    fn clip_typed(&mut self, id: &str, said: &str) -> bool {
        let said = said.trim();
        let mut next = ronoctua_clipcascade::prefs::Settings {
            salt: self.clipcascade.salt.clone(),
            rounds: self.clipcascade.rounds,
            images: self.clipcascade.images,
            notify: self.clipcascade.notify,
            cap: self.clipcascade.cap,
            mark: self.clipcascade.mark,
        };
        match id {
            // AN EMPTY SALT IS A VALUE, and the upstream's default: this is
            // the one of the three where clearing the line means something,
            // so it is not refused.
            "clipboard-salt" => next.salt = said.to_owned(),
            // A NUMBER THAT WILL NOT PARSE IS NOT A ZERO. Zero rounds derives
            // a key against nothing and zero bytes would mean "send nothing";
            // refusing leaves what was there.
            "clipboard-rounds" => match said.parse::<u32>() {
                Ok(n) if n > 0 => next.rounds = n,
                _ => return false,
            },
            "clipboard-cap" => match said.parse::<u64>() {
                Ok(n) => next.cap = n,
                Err(_) => return false,
            },
            _ => return false,
        }
        self.clip_keep(next);
        true
    }

    /// Puts whatever was last copied into the queue.
    ///
    /// # Why the clipboard and not a field to type into
    ///
    /// Because an address is never typed. It is copied — from the browser's own
    /// bar, from a chat window, from a page — and the gesture that follows is
    /// already `Ctrl+C`. A field would ask the person to paste into it, which
    /// is the same paste with one more step and a syntax to get wrong.
    ///
    /// The [`Wish`] says what to do with a page and whose session to use. A
    /// magnet ignores both: it is not a page, and a swarm has no account.
    pub(super) fn queue_the_clipboard(&mut self, wish: Wish) {
        let latest = self
            .clipboard
            .at(0)
            .and_then(|e| e.text())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(ToOwned::to_owned);
        let Some(address) = latest else {
            self.echo_message(self.texts.t(crate::key::TRANSFERS_NOTHING_COPIED));
            return;
        };
        // A SANITY CHECK AND NOT A PARSER: what decides the engine is
        // `where_to::work_for`, in the queue's crate, where every other door
        // asks the same question. This only refuses what is plainly not an
        // address, so that copying a paragraph and pressing the key gives a
        // sentence rather than a failed job.
        let plausible = address.contains("://") || address.starts_with("magnet:");
        if !plausible || address.contains(char::is_whitespace) {
            self.echo_message(self.texts.t(crate::key::TRANSFERS_NOT_AN_ADDRESS));
            return;
        }
        let work = ronoctua_transfers::where_to::work_for(
            &address,
            ronoctua_transfers::where_to::landing(),
            wish,
        );
        match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
            Ok(()) => {
                // WHICH OF THE TWO HAPPENED, said at the moment it happened.
                // The row carries it afterwards, but the line that matters is
                // this one: it is the only chance to notice that this download
                // was not anonymous before it has already gone out.
                let message = self.texts.t(if wish.identity == Identity::Mine {
                    crate::key::TRANSFERS_QUEUED_AS_ME
                } else {
                    crate::key::TRANSFERS_QUEUED
                });
                self.echo_message(message);
            }
            Err(e) => {
                tracing::error!(error = %e, "the queue refused the address");
                let message = self.texts.t(crate::key::TRANSFERS_OFFLINE);
                self.echo_message(message);
            }
        }
    }

    /// Raises ClipCascade's thread, if this machine has been set up.
    ///
    /// NOT ON EVERY START. A machine that never told it where a server is
    /// opens no socket — and this is what a laptop that only ever copies
    /// locally should cost, which is nothing.
    pub(super) fn raise_clipcascade_if_configured(&mut self) {
        if self.clipcascade_hand.is_some() {
            return;
        }
        let prefs = ronoctua_clipcascade::prefs::read();
        if prefs.server.is_empty() || prefs.username.is_empty() || !prefs.enabled {
            return;
        }
        self.raise_clipcascade();
    }

    /// Raises it, whatever the preferences say.
    ///
    /// Called by the login, which has just written a server into them — and
    /// which cannot wait for the next start to find out whether it worked.
    fn raise_clipcascade(&mut self) {
        if self.clipcascade_hand.is_some() {
            return;
        }
        let Some(to_me) = self.clipcascade_sender.clone() else {
            return;
        };
        self.clipcascade_hand = ronoctua_clipcascade::watch(move |e| {
            let _ = to_me.send(e);
        })
        .ok();
    }

    /// Sends one order, raising the thread if it is not up.
    pub(super) fn tell_clipcascade(&mut self, order: ronoctua_clipcascade::Order) {
        // A `Send` WITH NOTHING CONFIGURED IS DROPPED IN SILENCE. Every copy
        // would otherwise raise a thread on a machine that never asked to
        // sync, and then echo a refusal about it.
        if self.clipcascade_hand.is_none() && matches!(order, ronoctua_clipcascade::Order::Send(_))
        {
            return;
        }
        self.raise_clipcascade();
        match self.clipcascade_hand.as_ref() {
            Some(h) => h.send(order),
            None => {
                let said = self.texts.t(crate::key::CLIPBOARD_SYNC_DOWN);
                self.echo_message(said);
            }
        }
    }

    /// A fresh reading, or something that arrived, from ClipCascade's thread.
    pub(super) fn clipcascade_event(&mut self, event: ronoctua_clipcascade::Event) {
        match event {
            // A READING MOVES NO ENTRY, so it moves no generation: a window's
            // act aimed at the same history is still aimed right.
            ronoctua_clipcascade::Event::Snapshot(shot) => {
                self.clipcascade = *shot;
                self.clipboard_told(None);
                // NOT `follow_cursor`: a reading ARRIVING is not a key press.
                self.settle_region();
            }
            // WHAT CAME FROM ANOTHER MACHINE GOES INTO THE SAME HISTORY, by
            // the same door a local copy uses. One history, which is the whole
            // argument for speaking this protocol rather than running the
            // upstream client beside our own watcher.
            ronoctua_clipcascade::Event::Arrived(text) => {
                let entry = ronoctua_clipboard::Entry {
                    content: ronoctua_clipboard::Content::Text(text),
                    kinds: vec!["text/plain".to_owned()],
                    pinned: false,
                };
                if self.clipboard_mut().register(entry) {
                    self.clipboard_moved();
                }
            }
            ronoctua_clipcascade::Event::Done { refused, .. } => {
                if let Some(reason) = refused {
                    let said = self.texts.t_with(
                        crate::key::ECHO_CLIPBOARD_SYNC_FAILED,
                        &[("reason", &reason)],
                    );
                    self.echo_message(said);
                }
            }
        }
        self.draw_and_paint();
    }

    /// Whether the clipboard's sync says anything, and which way.
    ///
    /// A MACHINE THAT NEVER SET IT UP SAYS NOTHING, whatever the ring holds —
    /// the rule lives in `prefs::Mark::shows`, with the Syncthing mark's own
    /// words for why.
    ///
    /// THE RING COMES OUT OF THE SNAPSHOT and not out of a field of this
    /// struct. It was `self.clip_prefs`, read ONCE at start-up and never
    /// again: turning the ring in the view wrote the file, and the modeline
    /// went on drawing the old answer until the footer was restarted — a
    /// setting that appears to do nothing until the next boot. The file is the
    /// broadcast and the boundary is its reader; one reading, one answer.
    pub(super) fn clip_mark(&self) -> Option<bool> {
        let syncing = self.clipcascade.syncing();
        self.clipcascade
            .mark
            .shows(self.clipcascade.configured, syncing)
            .then_some(syncing)
    }
}
