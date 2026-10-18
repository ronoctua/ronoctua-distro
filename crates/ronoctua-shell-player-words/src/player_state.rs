//! WHAT A STATE OF THE PLAYER SAYS — whether something of the queue is
//! loaded, whether the ambient sounds are all that plays, what PLAY/PAUSE
//! asks, what the queue calls an entry, what is playing — and what a door
//! makes of what the player's watch heard (`hearing`).
//!
//! The footer keeps a copy of the player's state for the whole session —
//! the modeline's pair (`player_mark`) draws from it whether or not the
//! player's region is open — and names no feature, so these words stand
//! under the footer and the player's view (`ronoctua-shell-player`), which
//! says them as its own (`player::loaded`, `player::hearing`…).

use ronoctua_player::queue::{EntryId, Name, Node, Outline};
use ronoctua_player::wire::{Ask, State};

/// Whether what `s` says plays is there: a leaf of the queue is current, or
/// this machine plays in step with another — whose leaf is not one of this
/// queue's, so `current` is `None` then.
#[must_use]
pub fn loaded(s: &State) -> bool {
    s.current.is_some() || s.joined.is_some()
}

/// Whether the AMBIENT SOUNDS are all that plays: nothing of the queue is
/// loaded — no leaf current, no other machine followed — and a sound is on
/// (`State::only_ambience`, and the machine followed on top). Then the
/// modeline's pair and every door's toggle are the ambience's: with a leaf
/// current, even stopped, they are the queue's, and a click never holds the
/// rain in place of playing the song.
#[must_use]
pub fn only_ambience(s: &State) -> bool {
    s.joined.is_none() && s.only_ambience()
}

/// What PLAY/PAUSE asks — `SPC`, the transport's button, `LEADER p t` and the
/// modeline's glyph, the one answer each reads: the ambient sounds held or
/// let go on while they are all that plays ([`only_ambience`]), and the
/// queue's toggle otherwise. The media keys never come here: they speak to
/// the player through MPRIS, which toggles the queue only.
#[must_use]
pub fn toggle_ask(s: Option<&State>) -> Ask {
    if s.is_some_and(only_ambience) {
        Ask::AmbientToggle
    } else {
        Ask::Toggle
    }
}

/// What the queue calls an entry, in words — the ONE place a door words a
/// [`Name`]: a title as it is, a location by its last piece
/// (`queue::file_name`), and a set nobody named as every source it plays,
/// joined by `player-set-joiner` and cut to `width_cells` by grapheme
/// (`queue::together`) — never one file's name for several.
#[must_use]
pub fn name_of(name: &Name, texts: &ronoctua_i18n::Texts, width_cells: u16) -> String {
    match name {
        Name::Titled(title) => title.clone(),
        Name::Location(location) => {
            ronoctua_player::queue::file_name(location.trim_end_matches('/')).to_owned()
        }
        Name::Together(parts) => ronoctua_player::queue::together(
            parts,
            &texts.t(crate::key::PLAYER_SET_JOINER),
            width_cells,
        ),
    }
}

/// How wide a name said in a sentence may run, in cells: WHOLE. A hint under
/// the pointer and a title card are cut or wrapped by the surface that draws
/// them, to its own width — a width chosen here would be a size written by
/// hand (invariant 9).
pub const NAME_IN_A_SENTENCE: u16 = u16::MAX;

/// The entry `id` of the outline, wherever it is in the tree.
#[must_use]
pub fn node_of(outline: &Outline, id: EntryId) -> Option<&Node> {
    fn find(nodes: &[Node], id: EntryId) -> Option<&Node> {
        nodes.iter().find_map(|n| {
            if n.id == id {
                Some(n)
            } else {
                find(&n.children, id)
            }
        })
    }
    find(&outline.entries, id)
}

/// What is playing, in the words the modeline's mark says under the pointer
/// and the home's first row: the name the queue gives the current entry —
/// worded here, from the outline, so a set is its sources joined in this
/// language — else the title the player read, else where it plays.
#[must_use]
pub fn now_playing(
    state: &State,
    outline: Option<&Outline>,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    if !loaded(state) {
        return None;
    }
    let node = state
        .current
        .zip(outline)
        .and_then(|(id, o)| node_of(o, id));
    // A SET IS EVERY SOURCE IT PLAYS: the artist the player read is one
    // file's tag, and in front of the set's name it named the set after that
    // one file after all.
    let a_set = node.is_some_and(|n| n.kind == ronoctua_player::queue::NodeKind::Set);
    let named = node.map(|n| name_of(&n.name, texts, NAME_IN_A_SENTENCE));
    let title = named
        .or_else(|| state.title.clone())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            state
                .location
                .as_deref()
                .map(|l| name_of(&Name::Location(l.to_owned()), texts, NAME_IN_A_SENTENCE))
        })?;
    Some(
        match state
            .artist
            .as_deref()
            .filter(|a| !a_set && !a.trim().is_empty())
        {
            Some(a) => format!("{a} — {title}"),
            None => title,
        },
    )
}

/// What a door does with what the player's watch heard — the ONE answer for
/// the footer, the window and the column.
#[derive(Debug, Clone, PartialEq)]
pub enum Hearing {
    /// The player's state to show — `None`, nobody answers: the door stays
    /// and says the player is not running. A crash is not a farewell.
    Shows(Option<Box<State>>),
    /// The queue's outline, whole: the door KEEPS it beside the state and
    /// draws the queue from it ([`View::outline_arrived`]) — the state says
    /// only which entry is current.
    Outline(Box<Outline>),
    /// The player closed everywhere: the door closes.
    Closes,
}

/// What `heard` means to a door ([`Hearing`]).
#[must_use]
pub fn hearing(heard: ronoctua_player::client::Heard) -> Hearing {
    use ronoctua_player::client::Heard;
    match heard {
        Heard::State(s) => {
            remember_sinks(&s.sinks);
            Hearing::Shows(Some(s))
        }
        Heard::Queue(o) => Hearing::Outline(o),
        Heard::Gone => Hearing::Shows(None),
        Heard::Farewell => Hearing::Closes,
    }
}

thread_local! {
    /// THE SOUND OUTPUTS THIS DOOR HAS SEEN, by the name the sound server
    /// gives them, and what it calls them for people — fed by every state
    /// heard ([`hearing`]), so an output that went away is still said by the
    /// name a person knows it by (`bluez_output.…` is the server's).
    /// One door, one loop: the thread's own.
    static SINKS_SEEN: std::cell::RefCell<std::collections::BTreeMap<String, String>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Every output `sinks` says, remembered by its name for people.
fn remember_sinks(sinks: &[ronoctua_player::wire::Sink]) {
    SINKS_SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        for sink in sinks.iter().filter(|k| !k.description.trim().is_empty()) {
            seen.insert(sink.device.clone(), sink.description.clone());
        }
    });
}

/// What the sound server called the output `device` for people, as this
/// door last saw it in a state it heard ([`hearing`]) — an output that went
/// away is still said by the name a person knows it by.
#[must_use]
pub fn sink_seen(device: &str) -> Option<String> {
    SINKS_SEEN.with(|seen| seen.borrow().get(device).cloned())
}
