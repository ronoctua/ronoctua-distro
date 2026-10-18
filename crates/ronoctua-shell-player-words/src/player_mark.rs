//! The player's mark on the modeline: ONE segment and two glyphs — play or
//! pause, and the next — with one cell of air between them
//! (`space.modeline.bondGap`).
//!
//! # One segment, and not two
//!
//! The layout drops a removable segment WHOLE (`layout::resolve`): a line too
//! narrow for everything loses the pair, never half of it. Two segments on one
//! rung would split under the same pressure — the tie goes to the last one
//! declared, so "next" would go and play/pause would stay alone — and the two
//! cells of `segmentGap` between them would read as two unrelated marks.
//!
//! # One geometry, three readers
//!
//! [`zone_at`] says which glyph a cell of the segment is. The painter draws a
//! run exactly where it says a glyph is ([`drawn`]), the click asks it which of
//! the two was pressed, and the pointer resting on the pair asks it which
//! sentence to say ([`sentence`]) — so a glyph is taken where it is drawn,
//! and the air between them belongs to neither.
//!
//! Each glyph is its own run, at its own chrome size: `glyph_and_text` sizes
//! only a LEADING glyph, and the second would have come out at the text's
//! size, a fifth shorter than the first.
//!
//! # With only the ambient sounds on
//!
//! Nothing of the queue loaded and a sound on (`player::only_ambience`): the
//! pair is there, and it is the AMBIENCE's. Play/pause holds the sounds or lets
//! them go on (`player::toggle_ask`: `ambient-toggle`, which the media keys
//! never reach), its glyph saying which; next has nowhere to go, so it is
//! drawn INERT — in the tone of a key no click can press — takes no click, and
//! says why under the pointer ([`inert`]). With a leaf current, even stopped,
//! the pair is the queue's again. Same cells, same zones: the geometry does
//! not change with whose pair it is.

use ronoctua_player::queue::Outline;
use ronoctua_player::wire::State;
use ronoctua_tokens as tokens;

/// A glyph of the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// Play or pause: a click toggles, the right button opens the column or
    /// closes it (`player-column`, `LEADER p c`).
    Toggle,
    /// The next item: a click skips to it, the right button opens the
    /// player in the footer.
    Next,
}

impl Zone {
    /// Both, in reading order.
    pub const ALL: [Self; 2] = [Self::Toggle, Self::Next];

    /// Its cell, counted from the segment's first.
    #[must_use]
    pub const fn cell(self) -> u16 {
        match self {
            Self::Toggle => 0,
            Self::Next => 1 + tokens::space::MODELINE_BONDGAP,
        }
    }
}

/// The cells the pair takes: both glyphs and the air between them.
pub const CELLS: u16 = Zone::Next.cell() + 1;

/// Which glyph is at `cell` of the segment — `None` on the air between them.
/// THE ONE ANSWER the painter, the click and the hover read.
#[must_use]
pub fn zone_at(cell: u16) -> Option<Zone> {
    Zone::ALL.into_iter().find(|z| z.cell() == cell)
}

/// The glyph the play/pause half draws: what a click on it DOES — pause while
/// something plays, play while it is paused — the same word the transport's
/// button says (decision of the owner, 2026-09-28). Absent with nothing
/// loaded — and the pair with it: a modeline at rest says nothing.
#[must_use]
pub fn mark(state: &State) -> Option<char> {
    // ONLY THE AMBIENT SOUNDS: the pair is theirs, and says whether a click
    // holds them or lets them go on.
    if crate::player_state::only_ambience(state) {
        return Some(if state.ambience.paused {
            tokens::icon::PLAYER
        } else {
            tokens::icon::PAUSE
        });
    }
    if !crate::player_state::loaded(state) {
        return None;
    }
    Some(if state.playing {
        tokens::icon::PAUSE
    } else {
        tokens::icon::PLAYER
    })
}

/// Whether glyph `zone` of the pair takes no click in `state`: next, while
/// only the ambient sounds are on — the queue has nothing next, and the
/// ambience no order. THE ONE ANSWER the painter (its inert tone, [`tone`]),
/// the click and the sentence under the pointer read.
#[must_use]
pub fn inert(zone: Zone, state: &State) -> bool {
    zone == Zone::Next && crate::player_state::only_ambience(state)
}

/// The tone glyph `zone` is drawn in: the segment's own, or — inert — the
/// tone of a key no click can press (`chrome::Register::Inert`'s).
#[must_use]
pub fn tone(zone: Zone, state: Option<&State>, segment: tokens::Color) -> tokens::Color {
    if state.is_some_and(|s| inert(zone, s)) {
        tokens::color::TEXT_SUBTLE_INERT
    } else {
        segment
    }
}

/// The segment's text, for the play/pause glyph `toggle`: each cell what
/// [`zone_at`] says is there, and air where it says nothing is.
#[must_use]
pub fn text(toggle: char) -> String {
    (0..CELLS)
        .map(|cell| match zone_at(cell) {
            Some(Zone::Toggle) => toggle,
            Some(Zone::Next) => tokens::icon::PLAYER_NEXT,
            None => ' ',
        })
        .collect()
}

/// Each glyph of the segment as the layout placed it — `(zone, glyph, cell)`
/// — read off the placed text BY [`zone_at`], so the painter draws a run
/// exactly where the pointer finds that glyph.
#[must_use]
pub fn drawn(text: &str) -> Vec<(Zone, char, u16)> {
    text.chars()
        .zip(0u16..)
        .filter_map(|(glyph, cell)| zone_at(cell).map(|z| (z, glyph, cell)))
        .collect()
}

/// What the next glyph skips to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coming {
    /// This entry, by its name.
    Item(String),
    /// This entry, by its name — in the shuffled order: what the player
    /// plays next, which is not the one below it in the queue.
    Shuffled(String),
    /// Nothing: this is the last and nothing repeats, so next stops.
    End,
}

/// What comes after what plays, as the player says it (`State::next`) —
/// shuffle and repeat already in its answer — worded here: a set nobody
/// named is its sources joined in this language.
#[must_use]
pub fn coming(state: &State, texts: &ronoctua_i18n::Texts) -> Option<Coming> {
    state.current?;
    Some(match &state.next {
        Some(next) => {
            let name =
                crate::player_state::name_of(next, texts, crate::player_state::NAME_IN_A_SENTENCE);
            if state.shuffle {
                Coming::Shuffled(name)
            } else {
                Coming::Item(name)
            }
        }
        None => Coming::End,
    })
}

/// What the pair says under the pointer, on `zone`: what is playing, or what
/// comes next — each with what its right button does, which nothing on the
/// glyph can say.
#[must_use]
pub fn sentence(
    zone: Zone,
    state: &State,
    outline: Option<&Outline>,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    use crate::key;
    // ONLY THE AMBIENT SOUNDS: what the toggle does to them, and why next
    // does nothing.
    if crate::player_state::only_ambience(state) {
        return Some(texts.t(match zone {
            Zone::Toggle => key::HINT_PLAYER_AMBIENCE,
            Zone::Next => key::HINT_PLAYER_NEXT_AMBIENCE,
        }));
    }
    match zone {
        Zone::Toggle => {
            let what = crate::player_state::now_playing(state, outline, texts)?;
            Some(texts.t_with(key::HINT_PLAYER, &[("what", &what)]))
        }
        Zone::Next => Some(match coming(state, texts)? {
            Coming::Item(what) => texts.t_with(key::HINT_PLAYER_NEXT, &[("what", &what)]),
            Coming::Shuffled(what) => {
                texts.t_with(key::HINT_PLAYER_NEXT_SHUFFLED, &[("what", &what)])
            }
            Coming::End => texts.t(key::HINT_PLAYER_NEXT_END),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_player::queue::{EntryId, Name};

    /// A queue of `n` songs, `current` playing, and what the player says
    /// comes next.
    fn queue(n: u64, current: u64, next: Option<Name>) -> State {
        State {
            protocol: ronoctua_player::wire::PROTOCOL,
            current: (current < n).then_some(EntryId(current)),
            playing: true,
            title: Some(format!("song {current}")),
            next,
            ..State::default()
        }
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// FAILS IF: the mark stops saying what a click does — pause while
    /// playing, play while paused — or draws with nothing loaded: a modeline
    /// at rest says nothing.
    #[test]
    fn the_mark_says_play_or_pause_and_nothing_without_an_item() {
        let mut s = queue(2, 0, None);
        assert_eq!(mark(&s), Some(tokens::icon::PAUSE));
        s.playing = false;
        assert_eq!(mark(&s), Some(tokens::icon::PLAYER));
        s.current = None;
        assert_eq!(mark(&s), None);
    }

    /// FAILS IF: the text and the zones disagree — a glyph drawn in one cell
    /// and taken in another — or the air between the two answers a click.
    #[test]
    fn each_glyph_is_where_its_zone_is() {
        let t = text(tokens::icon::PAUSE);
        assert_eq!(ronoctua_render::layout::width(&t), CELLS);
        let d = drawn(&t);
        assert_eq!(
            d,
            vec![
                (Zone::Toggle, tokens::icon::PAUSE, 0),
                (Zone::Next, tokens::icon::PLAYER_NEXT, CELLS - 1),
            ]
        );
        for cell in 0..CELLS {
            let glyph = t.chars().nth(usize::from(cell));
            match zone_at(cell) {
                Some(_) => assert_ne!(glyph, Some(' '), "cell {cell}"),
                None => assert_eq!(glyph, Some(' '), "the air, cell {cell}"),
            }
        }
        assert_eq!(zone_at(CELLS), None);
    }

    /// FAILS IF: the next glyph names another entry than the one the player
    /// says it skips to — a title, a file by its name, a set by every source
    /// in this language's words — or stops saying the order is shuffled, or
    /// names one at the end of a queue that does not go round.
    #[test]
    fn next_names_what_the_player_says_it_skips_to() {
        let t = texts();
        let titled = queue(3, 0, Some(Name::Titled("song 1".into())));
        assert_eq!(coming(&titled, &t), Some(Coming::Item("song 1".into())));
        let file = queue(3, 0, Some(Name::Location("/music/b.flac".into())));
        assert_eq!(coming(&file, &t), Some(Coming::Item("b.flac".into())));
        let set = queue(
            3,
            0,
            Some(Name::Together(vec!["Film".into(), "Commentary".into()])),
        );
        let joiner = t.t(crate::key::PLAYER_SET_JOINER);
        assert_eq!(
            coming(&set, &t),
            Some(Coming::Item(format!("Film{joiner}Commentary")))
        );
        assert_eq!(coming(&queue(3, 2, None), &t), Some(Coming::End));
        let mut shuffled = titled;
        shuffled.shuffle = true;
        assert_eq!(
            coming(&shuffled, &t),
            Some(Coming::Shuffled("song 1".into()))
        );
        shuffled.current = None;
        assert_eq!(coming(&shuffled, &t), None);
    }

    /// FAILS IF: a glyph of the pair says nothing under the pointer, or both
    /// say the same — each names what IT does, in both languages.
    #[test]
    fn each_glyph_explains_itself() {
        for language in ["en-GB", "pt-BR"] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let s = queue(3, 0, Some(Name::Titled("song 1".into())));
            let toggle = sentence(Zone::Toggle, &s, None, &texts).expect("says");
            let next = sentence(Zone::Next, &s, None, &texts).expect("says");
            assert!(toggle.contains("song 0"), "{toggle}");
            assert!(next.contains("song 1"), "{next}");
            assert_ne!(toggle, next);
            let end = sentence(Zone::Next, &queue(3, 2, None), None, &texts).expect("says");
            assert!(!end.contains("song"), "{end}");
        }
    }
}
