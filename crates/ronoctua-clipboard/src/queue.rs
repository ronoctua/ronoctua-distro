//! The history of what was copied.
//!
//! # Why it does NOT touch the disk
//!
//! A clipboard history keeps everything that gets copied — passwords,
//! tokens and private keys included. In memory it dies with the session; in a file it
//! survives a reboot, ends up in a backup, and becomes a target nobody
//! remembers exists.
//!
//! The decision is the owner's and it is written here because it is what holds up
//! [`Entry::is_secret`]: without the promise not to persist, skipping the password
//! would be half a protection.
//!
//! # Why time does not come in
//!
//! Unlike the notification queue, nothing here expires on its own. An entry
//! leaves for three reasons, all explicit: the ceiling, removal, or clearing.
//! Expiry by time would be the distro deleting what the person copied without their
//! asking.

use std::collections::VecDeque;

/// The MIME type password managers mark on their offers.
///
/// It is a KDE convention, adopted by 1Password, Bitwarden, KeePassXC and the
/// rest — and it is the only standardised signal there is for "this is a secret,
/// do not store it". Without it, the history would have no way to tell a password from
/// any other text.
pub const PASSWORD_KIND: &str = "x-kde-passwordmanagerhint";

/// How many entries the history keeps.
///
/// Fifty: enough for "that thing I copied a moment ago" without becoming an
/// archive. Since nothing is persisted, the ceiling is about memory and about the
/// list's usefulness — a hundred text entries is not something one scans with the eye.
pub const CEILING: usize = 50;

/// What was copied.
///
/// An `enum`, and not a `text` with an `image: Option<…>` beside it: the two
/// fields would allow the impossible state of having both or neither, and the
/// deduction "is it an image?" would become a query to two places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// Text, as it came.
    Text(String),
    /// The file's bytes — PNG or JPEG — **undecoded**.
    ///
    /// Decoding here would put a whole image in RGBA into memory per
    /// history entry: a 4K screenshot is 33 MB uncompressed against
    /// ~2 MB in the file. What decodes is what is going to draw, and only the focused
    /// item.
    Image(Vec<u8>),
    /// Something was copied, and nothing here can read it.
    ///
    /// # Why it is kept at all
    ///
    /// Because it happened. A selection that claims to be text in an encoding
    /// nobody declared used to be dropped where it was read, and a dropped
    /// copy is indistinguishable on screen from a copy that never occurred —
    /// the owner presses the key, the list does not change, and there is
    /// nothing to look at. Keeping it makes the list able to say what arrived
    /// and why it is not there, which is the same rule the preview panel
    /// follows: a refusal is a VALUE, not an absence.
    ///
    /// It holds no bytes. Whatever they were, they cannot be shown, cannot be
    /// pasted back and cannot be searched — carrying them would be a megabyte
    /// per entry in exchange for nothing.
    Unreadable(crate::Undecodable),
}

impl Content {
    /// The text, when it is text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(t) => Some(t),
            Self::Image(_) | Self::Unreadable(_) => None,
        }
    }

    /// Why this could not be read, when that is what it is.
    #[must_use]
    pub const fn unreadable(&self) -> Option<&crate::Undecodable> {
        match self {
            Self::Unreadable(no) => Some(no),
            Self::Text(_) | Self::Image(_) => None,
        }
    }

    /// The bytes, whatever the shape. It is what measures the size on screen.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Text(t) => t.as_bytes(),
            Self::Image(b) => b,
            // WHAT ARRIVED, not what is held: the size is the one fact about
            // the payload that survived, and the screen has a column for it.
            Self::Unreadable(_) => &[],
        }
    }

    /// How many bytes arrived, whether or not any of them could be kept.
    #[must_use]
    pub fn arrived(&self) -> usize {
        match self {
            Self::Unreadable(no) => no.bytes,
            _ => self.bytes().len(),
        }
    }

    /// Is it empty?
    #[must_use]
    pub fn empty(&self) -> bool {
        self.bytes().is_empty()
    }
}

/// One copied thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The content, as it came.
    pub content: Content,
    /// The MIME types the offer announced.
    ///
    /// Kept because they are what the sheet shows (and, in the image
    /// wave, what decides whether there is a preview). The password filter reads them **before**
    /// the entry exists — see [`Entry::is_secret`].
    pub kinds: Vec<String>,
    /// Pinned: it does not leave through the ceiling.
    pub pinned: bool,
}

impl Entry {
    /// A plain text entry.
    #[must_use]
    pub fn of_text(text: impl Into<String>) -> Self {
        Self {
            content: Content::Text(text.into()),
            kinds: vec!["text/plain".into()],
            pinned: false,
        }
    }

    /// An image entry.
    #[must_use]
    pub fn of_image(bytes: Vec<u8>, kind: &str) -> Self {
        Self {
            content: Content::Image(bytes),
            kinds: vec![kind.to_owned()],
            pinned: false,
        }
    }

    /// The text, when it is text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.content.text()
    }

    /// The image's bytes, when it is an image.
    #[must_use]
    pub fn image(&self) -> Option<&[u8]> {
        match &self.content {
            Content::Image(b) => Some(b),
            Content::Text(_) | Content::Unreadable(_) => None,
        }
    }

    /// Why this entry could not be read, when that is what it is.
    #[must_use]
    pub const fn unreadable(&self) -> Option<&crate::Undecodable> {
        self.content.unreadable()
    }

    /// Is this offer from a password manager?
    ///
    /// A **case-insensitive** comparison: the type appears written in more than one way
    /// across applications, and a history that stores the password because the `H` came
    /// capitalised is a history that protects nothing.
    #[must_use]
    pub fn is_secret(kinds: &[String]) -> bool {
        kinds
            .iter()
            .any(|t| t.to_ascii_lowercase().contains(PASSWORD_KIND))
    }
}

/// The history, from newest to oldest.
#[derive(Debug, Default)]
pub struct Queue {
    inputs: VecDeque<Entry>,
}

impl Queue {
    /// Records what has just been copied.
    ///
    /// Returns `false` when nothing changed — a secret, empty, or a repeat of
    /// what is already on top.
    pub fn register(&mut self, input_of: Entry) -> bool {
        if input_of.content.empty() || Entry::is_secret(&input_of.kinds) {
            return false;
        }
        // REPEATING THE TOP IS NOT AN EVENT. Copying the same thing twice is the
        // gesture of whoever is not sure they copied — and the history turned into a
        // column of the same text. Only the TOP, and not the whole list: re-copying
        // something old should bring it back to the front.
        if self
            .inputs
            .front()
            .is_some_and(|e| e.content == input_of.content)
        {
            return false;
        }
        // If it already exists further back, it MOVES UP instead of duplicating — and it moves up
        // preserving `pinned`, which is the person's choice and not the content's.
        let pinned = self
            .inputs
            .iter()
            .position(|e| e.content == input_of.content)
            .map(|i| self.inputs.remove(i).is_some_and(|e| e.pinned))
            .unwrap_or(false);

        self.inputs.push_front(Entry { pinned, ..input_of });
        self.prune();
        true
    }

    /// Discards the excess, **preserving what is pinned**.
    ///
    /// Pinning exists precisely so the item does not disappear through use; a ceiling that
    /// discarded it would make the button decorative.
    fn prune(&mut self) {
        while self.inputs.len() > CEILING {
            let Some(i) = self.inputs.iter().rposition(|e| !e.pinned) else {
                // All pinned: the ceiling gives way. Preferring to discard something the
                // person asked to keep would be undoing their request.
                break;
            };
            self.inputs.remove(i);
        }
    }

    /// The entries, from newest to oldest.
    #[must_use]
    pub fn inputs(&self) -> &VecDeque<Entry> {
        &self.inputs
    }

    /// How many there are.
    #[must_use]
    pub fn how_many(&self) -> usize {
        self.inputs.len()
    }

    /// The entry at index `i`.
    #[must_use]
    pub fn at(&self, i: usize) -> Option<&Entry> {
        self.inputs.get(i)
    }

    /// Removes the entry at index `i`. Returns whether there was one.
    pub fn remove(&mut self, i: usize) -> bool {
        self.inputs.remove(i).is_some()
    }

    /// Pins or unpins the entry at index `i`. Returns the new state.
    pub fn toggle_pinned(&mut self, i: usize) -> Option<bool> {
        let e = self.inputs.get_mut(i)?;
        e.pinned = !e.pinned;
        Some(e.pinned)
    }

    /// Empties everything, **including what is pinned**.
    ///
    /// Clearing is an explicit request, and sparing the pinned entries from it would leave on screen
    /// a history that refuses to go away — the opposite of what was asked.
    pub fn clear(&mut self) {
        self.inputs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{CEILING, Content, Entry, Queue};

    #[test]
    fn a_password_does_not_go_in() {
        // THE CHECK THAT SEPARATES A HISTORY FROM A PASSWORD FILE.
        //
        // Without it, "we skip whatever comes from a password manager" is a sentence in
        // a comment — and the day the type moves nobody finds out,
        // because the password goes in silently and looks like any other text.
        let mut f = Queue::default();
        let password = Entry {
            content: Content::Text("hunter2".into()),
            kinds: vec!["text/plain".into(), "x-kde-passwordManagerHint".into()],
            pinned: false,
        };
        assert!(
            !f.register(password),
            "an offer of a password must not get in"
        );
        assert_eq!(f.how_many(), 0);

        // And the comparison is CASE-INSENSITIVE: the type appears written in more than one
        // way across applications.
        let otherwise_one = Entry {
            content: Content::Text("hunter2".into()),
            kinds: vec!["X-KDE-PasswordManagerHint".into()],
            pinned: false,
        };
        assert!(!f.register(otherwise_one));
        assert_eq!(f.how_many(), 0);
    }

    #[test]
    fn repeating_the_top_does_not_duplicate_but_recopying_the_old_one_brings_it_back() {
        let mut f = Queue::default();
        assert!(f.register(Entry::of_text("a")));
        assert!(
            !f.register(Entry::of_text("a")),
            "repeating the top is noise"
        );
        assert!(f.register(Entry::of_text("b")));
        // Re-copying something old BRINGS IT BACK to the top, and does not duplicate.
        assert!(f.register(Entry::of_text("a")));
        let seen: Vec<&str> = f.inputs().iter().filter_map(Entry::text).collect();
        assert_eq!(seen, ["a", "b"]);
    }

    #[test]
    fn the_empty_one_does_not_go_in() {
        // Switching focus or closing an app can clear the selection, and an
        // empty entry in the history is a row that says nothing.
        let mut f = Queue::default();
        assert!(!f.register(Entry::of_text("")));
        assert_eq!(f.how_many(), 0);
    }

    #[test]
    fn the_ceiling_discards_the_oldest() {
        let mut f = Queue::default();
        for n in 0..CEILING + 5 {
            assert!(f.register(Entry::of_text(n.to_string())));
        }
        assert_eq!(f.how_many(), CEILING);
        // The top is the last thing copied; the oldest one left.
        assert_eq!(
            f.at(0).unwrap().text(),
            Some((CEILING + 4).to_string().as_str())
        );
        assert!(!f.inputs().iter().any(|e| e.text() == Some("0")));
    }

    #[test]
    fn the_pinned_one_survives_the_ceiling() {
        // PINNING IS ONLY ANY USE IF THE CEILING RESPECTS IT. Without this the button exists,
        // looks like it works, and the item disappears through normal use — a defect that only
        // shows up fifty copies later.
        let mut f = Queue::default();
        assert!(f.register(Entry::of_text("store")));
        assert_eq!(f.toggle_pinned(0), Some(true));
        for n in 0..CEILING + 10 {
            f.register(Entry::of_text(format!("lixo {n}")));
        }
        assert!(
            f.inputs().iter().any(|e| e.text() == Some("store")),
            "the pinned one was dropped by the ceiling"
        );
        assert_eq!(f.how_many(), CEILING);
    }

    #[test]
    fn clearing_takes_the_pinned_one_along() {
        // Clearing is an explicit request. Sparing the pinned entries would leave on screen a
        // history that refuses to go away.
        let mut f = Queue::default();
        f.register(Entry::of_text("x"));
        f.toggle_pinned(0);
        f.clear();
        assert_eq!(f.how_many(), 0);
    }
}
