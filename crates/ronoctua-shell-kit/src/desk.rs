//! THE NOTIFICATION DESK'S WORDS — what a view asks of the queue and the
//! filter book the footer keeps, and what a notification's button comes to.
//!
//! # Why here, and not in the manager's crate
//!
//! The footer keeps the desk: the queue its band draws, the person's filter
//! book that decides what the band shows, and the bus every sender waits on.
//! The manager of the notifications is a VIEW of it, in a crate of its own,
//! and the band is the footer's — and both press the same buttons. So what
//! a button comes to ([`deed_of`]) and what a view asks of the desk ([`Ask`])
//! are the words of both, and the words two of them say come down to the
//! foundation: the footer names no feature, and the feature does not reach
//! up into the footer.

use ronoctua_notify::presentation::Verdict;
use ronoctua_notify::{Record, RecordKey};

/// What a view asks the notification desk to do — the queue's records and
/// the person's filter book are the footer's, and only it changes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Mark this record read, BY HAND: off the band when it is the one there,
    /// and closed on the bus.
    Read(RecordKey),
    /// Mark it unread again, back on the bell's count.
    Unread(RecordKey),
    /// Mark every record read that was shown — never a suppressed one.
    ReadAll,
    /// Forget this record: out of the history for good.
    Forget(RecordKey),
    /// Run action `which` of this record ([`deed_of`]).
    Act(RecordKey, usize),
    /// The person adjusted entry `id` of the catalogue — `None` puts it back
    /// to the distro's.
    Adjusted(String, Option<Verdict>),
    /// Save this filter of the person's own — `was` is the id it replaces.
    Filter {
        /// The id being replaced, when this was an edit.
        was: Option<String>,
        /// What to save.
        rule: ronoctua_notify::Rule,
    },
    /// Drop this filter of the person's own, and the adjustment keyed to it.
    Unfilter(String),
    /// Put this filter of the person's in the repository, for everybody.
    Share(String),
}

/// What the footer tells the view of the desk when what it shows moved — the
/// records a view asked about, or the history another door wrote: the
/// cursor goes back on a row that exists ([`Settle::follow`]: and the page
/// follows it, after a person's own gesture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settle {
    /// Whether the page follows the cursor — after a person's gesture, and
    /// never after a file another process wrote.
    pub follow: bool,
}

/// The file in the repository that holds what somebody shared.
///
/// Under `assets/shared/`, which is where [`ronoctua_store::Shared`] writes and
/// where `nix/notifications.nix` reads at build time.
pub const SHARED_NOTIFICATIONS: &str = "notifications.json";

/// A record by its whole name, as one word pair — the id and when it
/// arrived: the id alone names every record that was ever given that number
/// (`RecordKey`). What the band hands the manager to open on, and the head of
/// a window's action order.
#[must_use]
pub fn record_order(key: RecordKey) -> String {
    let nanos = key
        .at
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{} {nanos}", key.id)
}

/// [`record_order`], read back from the start of `words` — `None` for
/// anything else.
pub fn record_of<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<RecordKey> {
    let id = words.next()?.parse().ok()?;
    let nanos: u128 = words.next()?.parse().ok()?;
    let whole = std::time::Duration::new(
        u64::try_from(nanos / 1_000_000_000).ok()?,
        u32::try_from(nanos % 1_000_000_000).ok()?,
    );
    let at = std::time::UNIX_EPOCH.checked_add(whole)?;
    Some(RecordKey { id, at })
}

/// A record's whole name, alone — [`record_order`] read back, and nothing
/// after it.
#[must_use]
pub fn record_named(argument: &str) -> Option<RecordKey> {
    let mut words = argument.split_whitespace();
    let key = record_of(&mut words)?;
    words.next().is_none().then_some(key)
}

/// The footer's command that runs an action — with [`act_order`] as its
/// argument, the record's; bare, the one under the manager's cursor.
pub const ACT: &str = "notif-act";

/// Action `which` of the record `key`, as the argument of `notif-act` — what
/// a window sends the footer for a deed only the footer can do, and what the
/// band's own button runs.
#[must_use]
pub fn act_order(key: RecordKey, which: usize) -> String {
    format!("{} {which}", record_order(key))
}

/// [`act_order`], read back — `None` for anything else, which the footer
/// refuses rather than taking for the row its own cursor is on.
#[must_use]
pub fn act_of(argument: &str) -> Option<(RecordKey, usize)> {
    let mut words = argument.split_whitespace();
    let key = record_of(&mut words)?;
    let which = words.next()?.parse().ok()?;
    if words.next().is_some() {
        return None;
    }
    Some((key, which))
}

/// What running action `which` of a record COMES TO — decided here, once,
/// for every door, so none can disagree about what a button does.
///
/// Two of the five are the FOOTER'S to do, and a window hands them over
/// (`notif-act`): a foreign app's button is `ActionInvoked` said by the
/// process that owns the bus name the sender is waiting on, and a command of
/// the registry is run where commands run. The other three a window does
/// itself — the clipboard is one call from any process, and a program starts
/// in a scope of its own from any door (`foreign`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    /// A foreign app's button: `ActionInvoked` with this action's id.
    Invoke(String),
    /// A command of the registry, by id.
    Command(String),
    /// This path, on the clipboard as text.
    CopyPath(String),
    /// This image file, on the clipboard as an image.
    CopyImage(std::path::PathBuf),
    /// This program, with these arguments.
    Run(ronoctua_notify::Program, Vec<String>),
}

impl Deed {
    /// Whether only the footer can do it — the bus name, the registry.
    #[must_use]
    pub const fn the_footers(&self) -> bool {
        matches!(self, Self::Invoke(_) | Self::Command(_))
    }
}

/// What action `which` of `record` does — `None` when it has no such action,
/// or wants the file the notification did not carry (an action like that is
/// not run, and the record is not marked read for it).
#[must_use]
pub fn deed_of(record: &Record, which: usize) -> Option<Deed> {
    use ronoctua_notify::{Doing, action::Arg};
    let action = record.notification.actions.get(which)?;
    let file = record.notification.file();
    Some(match &action.doing {
        Doing::Invoke => Deed::Invoke(action.id.clone()),
        Doing::Command(id) => Deed::Command(id.clone()),
        Doing::CopyPath => Deed::CopyPath(file?.to_string_lossy().into_owned()),
        Doing::CopyImage => Deed::CopyImage(file?),
        Doing::Run { program, args } => {
            let mut argv = Vec::with_capacity(args.len());
            for a in args {
                argv.push(match a {
                    Arg::Literal(s) => s.clone(),
                    Arg::File => file.as_ref()?.to_string_lossy().into_owned(),
                    Arg::Folder => file.as_ref()?.parent()?.to_string_lossy().into_owned(),
                });
            }
            Deed::Run(*program, argv)
        }
    })
}

/// Puts what a copying deed carries on the clipboard, and says how it went:
/// the text key every door puts on its line. `None` for a deed that copies
/// nothing.
#[must_use]
pub fn copy_it(deed: &Deed) -> Option<&'static str> {
    use crate::key;
    let done = match deed {
        Deed::CopyPath(text) => {
            crate::clipboard_copy::copy_to_clipboard(text).map(|()| key::ECHO_COPIED)
        }
        Deed::CopyImage(file) => {
            crate::clipboard_copy::copy_image_to_clipboard(file).map(|()| key::ECHO_IMAGE_COPIED)
        }
        Deed::Invoke(_) | Deed::Command(_) | Deed::Run(..) => return None,
    };
    Some(done.unwrap_or_else(|e| {
        tracing::error!(error = %e, "copying for a notification FAILED");
        key::ECHO_COPY_FAILED
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a record's whole name does not read back as it was written,
    /// or something that is not one is taken for one — the band would open
    /// the manager on another record.
    #[test]
    fn a_record_order_names_the_record_and_reads_back() {
        let key = RecordKey {
            id: 42,
            at: std::time::UNIX_EPOCH + std::time::Duration::new(1_790_000_000, 123_456_789),
        };
        let order = record_order(key);
        assert_eq!(order, "42 1790000000123456789");
        assert_eq!(record_named(&order), Some(key));
        for wrong in ["", "42", "x 1", "42 1 2", "42 -1"] {
            assert_eq!(record_named(wrong), None, "{wrong}");
        }
    }
}
