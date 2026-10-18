//! THE NOTIFICATION DESK THE FOOTER KEEPS: the queue its band draws, the
//! person's filter book that decides what the band shows, the history it
//! writes, and the bus every sender waits on — and what a view or the band
//! asks of it ([`crate::desk::Ask`]).
//!
//! # Why the footer's, and not the manager's
//!
//! Because the band is the footer's and lives for the whole session, with the
//! manager closed as much as open: a notification arrives, is drawn, is
//! clicked, is read. The manager (`ronoctua-shell-notifications`) is a view
//! of the desk — it is lent the queue and the book to draw, and ASKS what it
//! changes, which is done here once it is back in its slot. The band asks the
//! same, through the same words.
//!
//! It is no view: nothing here is ranked, and no dispatcher calls it.

use super::*;

impl Footer {
    /// Does what a view or the band asked of the desk.
    pub(super) fn desk_asked(&mut self, ask: crate::desk::Ask) {
        use crate::desk::Ask;
        match ask {
            Ask::Read(key) => {
                if self.notifications.mark_read(key) {
                    self.closed_by_hand(key);
                }
                self.after_the_queue_moved();
            }
            // PUTTING ONE BACK SAYS NOTHING ON THE BUS, and takes nothing off
            // the band.
            //
            // There is no "un-closed" in the protocol, and there could not be:
            // `NotificationClosed` is final, and the sender has already acted
            // on it. This is the person's own record changing its mind about
            // something the sender was told about long ago — the same reason
            // forgetting a record says nothing either.
            Ask::Unread(key) => {
                if self.notifications.mark_unread(key) {
                    tracing::info!(id = key.id, "notification put back to unread");
                }
                self.after_the_queue_moved();
            }
            Ask::ReadAll => {
                // THE SAME SENTENCE, ONE AT A TIME.
                //
                // `Ask::Read` tells the application `NotificationClosed(
                // ByHand)`; this used to tell it nothing at all, so "mark this
                // one read" and "mark them all read" were two different
                // contracts for one word. An application that clears its own
                // badge on that signal cleared it for one and not for twenty.
                //
                // AND IT NEVER MARKS A SUPPRESSED ONE, which is the other half
                // of the same disagreement: `Space` refuses that column
                // because a suppressed record is already off the count, and
                // this used to sweep it in anyway — closing, on the bus, a
                // notification the person was never shown.
                let keys: Vec<ronoctua_notify::RecordKey> = self
                    .notifications
                    .history()
                    .iter()
                    .filter(|r| {
                        !r.read && !matches!(r.presentation.show, ronoctua_notify::Show::Hidden)
                    })
                    .map(ronoctua_notify::Record::key)
                    .collect();
                for key in keys {
                    if self.notifications.mark_read(key) {
                        ronoctua_notify::dbus::notification_was_closed(
                            key.id,
                            ronoctua_notify::dbus::Closed::ByHand,
                        );
                    }
                }
                self.after_the_queue_moved();
            }
            Ask::Forget(key) => {
                if self.notifications.forget(key) {
                    tracing::info!(id = key.id, "notification forgotten");
                }
                self.after_the_queue_moved();
            }
            Ask::Act(key, which) => self.run_notification_action(key, which),
            Ask::Adjusted(id, verdict) => {
                match verdict {
                    Some(v) => self.book.adjustments.insert(id.clone(), v),
                    None => self.book.adjustments.remove(&id),
                };
                self.save_the_book();
                tracing::info!(id = %id, "notification preference saved");
            }
            // THE PERSON'S OWN FILTERS, saved where they already live: the
            // preferences file both doors watch. Editing REPLACES rather than
            // adds — `was` is the id it had before it was renamed.
            Ask::Filter { was, rule } => {
                if let Some(old) = was {
                    self.book.mine.retain(|r| r.id != old);
                }
                self.book.mine.retain(|r| r.id != rule.id);
                self.book.mine.push(rule);
                self.save_the_book();
            }
            Ask::Unfilter(id) => {
                self.book.mine.retain(|r| r.id != id);
                // AND THE ADJUSTMENT THAT WAS KEYED TO IT. A verdict left
                // behind for a filter nobody has is a line in the file that
                // nothing will ever read again.
                self.book.adjustments.remove(&id);
                self.save_the_book();
            }
            // INTO THE REPOSITORY, one filter at a time, by name, after a
            // screen that said what it means. This writes SOURCE — what makes
            // it part of the distro is the next build, and what publishes it
            // is the owner's own commit, which needs them in front of the
            // machine anyway.
            Ask::Share(id) => {
                let said = self.share_one(&id);
                self.echo_message(self.texts.t(said));
            }
        }
    }

    /// Runs action `which` of the notification with this id.
    ///
    /// # Why it looks the notification up again
    ///
    /// Because the manager holds no records — it holds a cursor. What it hands
    /// over is an id and an index, and the record is found here, where the
    /// queue is. A manager carrying its own copy is the second copy that goes
    /// stale, and this file has already paid for one.
    ///
    /// A COMMAND OF OURS MAY WANT THE REGION, and the manager — the view that
    /// asked, or the one open over the band whose button was pressed — has
    /// already got out of its way: the manager closes itself before it asks
    /// (`notifications_region`), and the band asks through the manager's own
    /// command (`notif-act`).
    pub(super) fn run_notification_action(
        &mut self,
        key: ronoctua_notify::RecordKey,
        which: usize,
    ) {
        use crate::desk::Deed;
        // WHAT THE BUTTON COMES TO is decided once, for every door
        // (`desk::deed_of`); this is the door that does all five.
        let Some(deed) = self
            .notifications
            .history()
            .iter()
            .find(|r| r.key() == key)
            .and_then(|r| crate::desk::deed_of(r, which))
        else {
            return;
        };
        if let Some(said) = crate::desk::copy_it(&deed) {
            self.echo_message(self.texts.t(said));
        }
        match deed {
            // A FOREIGN APP'S BUTTON goes back to the bus, which is what the
            // sender is waiting for.
            Deed::Invoke(action) => {
                ronoctua_notify::dbus::action_was_invoked(key.id, &action);
            }
            Deed::Command(cmd) => self.run_command(&cmd),
            Deed::CopyPath(_) | Deed::CopyImage(_) => {}
            Deed::Run(program, argv) => {
                let Some(path) = crate::actions::notification_program(program) else {
                    // It should not get here: an action whose program is
                    // missing is not offered — see `usable_actions`. The guard
                    // is for the day the profile changes under a running shell.
                    tracing::warn!(
                        program = program.id(),
                        "the action's program is not installed"
                    );
                    return;
                };
                tracing::info!(program = %path, ?argv, "a notification action");
                self.open_foreign(&path, &argv, None);
            }
        }
        // ACTING ON IT IS HAVING SEEN IT — and seen BY HAND, as `r` says it:
        // off the band, and closed on the bus. The same word the window's
        // own actions write to the file, which this door takes the same way
        // (`history_file_changed`), so the two doors leave one state behind.
        if self.notifications.mark_read(key) {
            self.closed_by_hand(key);
        }
        self.after_the_queue_moved();
    }

    /// A record read BY HAND — `r`, an action, or the window's own `r`
    /// taken from the file: off the band when it is the one there, and
    /// `NotificationClosed(ByHand)` to its sender.
    ///
    /// The one on the band goes too: marking it read and leaving it on
    /// screen would be two states for one thing.
    pub(super) fn closed_by_hand(&mut self, key: ronoctua_notify::RecordKey) {
        if self
            .notifications
            .showing()
            .is_some_and(|s| s.notification.key() == key)
        {
            self.notifications
                .dismiss_current(std::time::Instant::now());
        }
        ronoctua_notify::dbus::notification_was_closed(
            key.id,
            ronoctua_notify::dbus::Closed::ByHand,
        );
    }

    /// Another process wrote the preferences — the window, adjusting a kind.
    ///
    /// The book is a settings file with no live half, so it is simply read
    /// again. What changes on screen is which notifications the band shows
    /// and what the catalogue's rows say.
    pub(super) fn book_file_changed(&mut self) {
        let prefs = self.notes.preferences();
        // THE SAME TWO HALVES the start-up builds it from, in one place
        // rather than two: the distro's catalogue, plus the person's rules
        // and adjustments.
        let mut book = ronoctua_notify::Book::distro();
        book.took(prefs.rules, prefs.shared);
        book.adjustments = prefs.adjustments;
        // WHAT CHANGED is the person's half — the distro's catalogue is
        // compiled in and cannot move under us — so that is what is compared.
        if book.mine == self.book.mine
            && book.shared == self.book.shared
            && book.adjustments == self.book.adjustments
        {
            return;
        }
        tracing::info!(
            rules = book.mine.len(),
            adjustments = book.adjustments.len(),
            "the notification preferences changed under us"
        );
        self.book = book;
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
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
    pub(super) fn share_one(&mut self, id: &str) -> &'static str {
        let Some(rule) = self.book.mine.iter().find(|r| r.id == id).cloned() else {
            return crate::key::NOTIF_SHARE_FAILED;
        };
        let shared = ronoctua_store::Shared::of(ronoctua_store::repo_dir());
        if !shared.is_open() {
            return crate::key::NOTIF_SHARE_NOWHERE;
        }
        let mut held: Vec<serde_json::Value> = shared.read(crate::desk::SHARED_NOTIFICATIONS);
        held.retain(|it| it.get("id").and_then(serde_json::Value::as_str) != Some(id));
        held.push(ronoctua_notify::as_table(&rule));
        if let Err(e) = shared.write(crate::desk::SHARED_NOTIFICATIONS, &held) {
            tracing::warn!(error = %e, id, "the filter could not be shared");
            return crate::key::NOTIF_SHARE_FAILED;
        }
        // AND THE SCREEN REMEMBERS IT IS TRAVELLING. The copy on this machine
        // keeps working until a build brings it back as one of the distro's —
        // see `Book::took`, which is where it retires.
        self.book.shared.insert(id.to_owned());
        self.save_the_book();
        tracing::info!(id, "a filter was written into the repository");
        crate::key::NOTIF_SHARED_WRITTEN
    }

    /// Writes the person's half of the filter book: their filters and their
    /// adjustments.
    ///
    /// ONE place, because there are now three things that change it — a knob
    /// turned, a filter written, a filter dropped — and the file is the
    /// broadcast: the other door is watching it by inotify.
    pub(super) fn save_the_book(&mut self) {
        let prefs = ronoctua_notify::Preferences {
            rules: self.book.mine.clone(),
            shared: self.book.shared.clone(),
            adjustments: self.book.adjustments.clone(),
        };
        // A PREFERENCE NOT WRITTEN, in the sentence every other one says —
        // it said "could not copy", which is another failure.
        if let Err(e) = self.notes.save_preferences(&prefs) {
            self.say_a_preference_not_written(ronoctua_notify::store::PREFERENCES_FILE, &e);
        }
        self.draw_and_paint();
    }
}
