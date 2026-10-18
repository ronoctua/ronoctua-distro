//! The player's questions: a line the door asks — its minibuffer in the
//! footer, its header's line in a window and the column — and the ONE
//! function what is typed on it comes back to, whichever door typed it
//! ([`answered`]).
//!
//! A pairing's code is a secret here: asked masked, never logged, and only
//! ever sent to the player.

use crate::player::Errand;
use crate::player_machines::{Code, Pairing};
use ronoctua_player::wire::{Ask, Thing};

/// The command that plays a path or an address now — right after what plays,
/// the queue kept.
pub const PLAY: &str = "player-play-file";
/// The one that adds a path or an address to the queue.
pub const ENQUEUE: &str = "player-enqueue";
/// The one that plays something else together with what plays: a lane.
pub const TOGETHER: &str = "player-together";
/// The one that pairs with another machine: its name, first.
pub const PAIR: &str = "player-pair";
/// The pairing's second question: the code the other machine shows.
pub const PAIR_CODE: &str = "player-pair-code";
/// Another name for an entry of the queue — the view's to answer
/// (`View::answered`), which knows which entry it asked about.
pub const RENAME: &str = "player-rename";
/// The name a set is saved under, typed from no question (`LEADER :` with
/// an argument): the view opens the save's picker holding it.
pub const SAVE_SET: &str = "player-save-set";
/// The name the queue is saved under, the same way.
pub const SAVE_QUEUE: &str = "player-save-queue";
/// When a lane begins, typed on its page — the view's to answer
/// (`View::answered`), which knows which lane it asked about.
pub const LANE_START: &str = "player-lane-start";
/// Where in its file a lane stops, typed on its page — the view's to answer.
pub const LANE_UNTIL: &str = "player-lane-until";
/// A lane's own name, typed on its page — the view's to answer.
pub const LANE_TITLE: &str = "player-lane-title";
/// A tag's new value, typed on the Every-tag page, the line holding the one
/// it has — the view's to answer, which knows which tag it asked about.
pub const TAG_VALUE: &str = "player-tag-value";
/// A tag's new value when the line cannot hold what it has — several
/// lines, a cut value, a name with several values: what is typed replaces
/// all of it, and the question says so.
pub const TAG_VALUE_ALL: &str = "player-tag-value-all";
/// The name of a tag to add, typed on the Every-tag page; its value is
/// asked next.
pub const TAG_NAME: &str = "player-tag-name";
/// Other words to search the internet with, for a desk of the lookups — the
/// view's to answer, which knows which search it was (`crate::player_find`).
pub const FIND_WORDS: &str = "player-find-words";
/// A language to search subtitles in, by its code.
pub const FIND_LANGUAGE: &str = "player-find-language";
/// OpenSubtitles' API key — masked (`crate::player_internet`).
pub const KEY_OPENSUBTITLES: &str = "player-key-opensubtitles";
/// TMDB's API read access token — masked.
pub const KEY_TMDB: &str = "player-key-tmdb";
/// SubDL's API key — masked.
pub const KEY_SUBDL: &str = "player-key-subdl";
/// The OpenSubtitles account's user name.
pub const ACCOUNT_USER: &str = "player-account-user";
/// Its password — masked.
pub const ACCOUNT_PASSWORD: &str = "player-account-password";
/// Words a radio station is named by (`crate::player_directory`).
pub const SEARCH_RADIO: &str = "player-search-radio";
/// Words a TV channel is named by.
pub const SEARCH_TV: &str = "player-search-tv";
/// A genre of radio, by its name — any, the ones past the list's most too.
pub const SEARCH_GENRE: &str = "player-search-genre";
/// A language of radio, by radio-browser's word for it.
pub const SEARCH_LANGUAGE: &str = "player-search-language";

/// Whether what is typed on question `id` is the person's own and stays out
/// of every log, though it is no secret on screen: an account's user name,
/// which `ronoctua_lookup::keys::Account` hides from its `Debug` too.
#[must_use]
pub fn unlogged(id: &str) -> bool {
    id == ACCOUNT_USER
}

/// A line the door asks, whether what is typed on it is a secret, and what
/// it already holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Which question: the `ask-<id>` it is drawn with, and the id its answer
    /// comes back to [`answered`] under.
    pub id: &'static str,
    /// Masked on screen, left out of every log, and never in an argv.
    pub secret: bool,
    /// What the line holds when it opens: CHANGING a name starts from the
    /// name it has, and a folder from the one it would be saved in
    /// (`line::Line::new`).
    pub initial: Option<String>,
}

impl Question {
    /// A question whose line opens empty.
    #[must_use]
    pub const fn plain(id: &'static str, secret: bool) -> Self {
        Self {
            id,
            secret,
            initial: None,
        }
    }

    /// A question whose line opens holding `initial`.
    #[must_use]
    pub fn holding(id: &'static str, initial: String) -> Self {
        Self {
            id,
            secret: false,
            initial: Some(initial),
        }
    }

    /// The line it is asked on, in either door — masked when it is a secret,
    /// and kept out of every log when what is typed is the person's own
    /// ([`unlogged`]): the question says so itself, so the answer is kept out
    /// whatever became of the view that asked it — closed before the person
    /// pressed `RET` included.
    #[must_use]
    pub fn line(&self) -> crate::line::Line {
        crate::line::Line::new(self.id, self.secret, self.initial.as_deref())
            .kept_out_of_the_log(unlogged(self.id))
    }
}

/// The question a command of the registry asks, when it asks one — what the
/// footer opens for `LEADER p o` and its siblings, typed from no view.
#[must_use]
pub fn question(id: &str) -> Option<Question> {
    Some(match id {
        PLAY => Question::plain(PLAY, false),
        ENQUEUE => Question::plain(ENQUEUE, false),
        TOGETHER => Question::plain(TOGETHER, false),
        PAIR => Question::plain(PAIR, false),
        PAIR_CODE => Question::plain(PAIR_CODE, true),
        _ => return None,
    })
}

/// What was typed for one of the player's questions, from whichever door
/// typed it — the ONE place an answer turns into an order.
///
/// An empty answer is never mind, in both doors. A path or an address is
/// [`typed`]'s; a machine's name asks for its code next; the code pairs with
/// the name asked before it, and nothing else.
pub fn answered(id: &str, said: &str, pairing: &mut Pairing) -> Errand {
    match id {
        PAIR => {
            let name = said.trim();
            if name.is_empty() {
                pairing.let_go();
                return Errand::Nothing;
            }
            pairing.name(name);
            Errand::Ask(Question::plain(PAIR_CODE, true))
        }
        PAIR_CODE => {
            let code = said.trim();
            match pairing.take() {
                Some(to) if !code.is_empty() => Errand::Pair {
                    to,
                    code: Code(code.to_owned()),
                },
                _ => Errand::Nothing,
            }
        }
        _ => typed(id, said).map_or(Errand::Nothing, Errand::Tell),
    }
}

/// What was typed, as [`answered`] turns it into an order — and SAID as a
/// pick among the files says the same order: a video typed on the page that
/// picks one said nothing where the one clicked said "plays in step", and a
/// path typed to play or to queue the same. What the doors call.
pub fn answered_saying(
    id: &str,
    said: &str,
    pairing: &mut Pairing,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    match answered(id, said, pairing) {
        Errand::Tell(ask) => told(ask, texts),
        other => other,
    }
}

/// An order typed, with the sentence its pick says (`View::picked`).
fn told(ask: Ask, texts: &ronoctua_i18n::Texts) -> Errand {
    use crate::key;
    let name = |location: &str| {
        crate::player::name_of(
            &ronoctua_player::queue::Name::Location(location.to_owned()),
            texts,
            crate::player::NAME_IN_A_SENTENCE,
        )
    };
    let sentence = match &ask {
        Ask::AddLane { source, .. } => {
            texts.t_with(key::PLAYER_TOGETHER_ADDED, &[("name", &name(source))])
        }
        Ask::Put { what, play, .. } => match what.as_slice() {
            [Thing::Location(one)] => texts.t_with(
                if *play {
                    key::PLAYER_PLAYING_NOW
                } else {
                    key::PLAYER_ADDED
                },
                &[("name", &name(one))],
            ),
            _ => return Errand::Tell(ask),
        },
        _ => return Errand::Tell(ask),
    };
    Errand::TellSaying(ask, sentence)
}

/// What playing `source` together with what plays asks: a lane of the kind
/// the file is (`player_kinds::lane_kind_of`), from the set's start, over
/// the first picture when it is subtitles — the file that played alone
/// becomes a set, and keeps its place in the queue.
#[must_use]
pub fn together(source: String) -> Ask {
    Ask::AddLane {
        kind: crate::player_kinds::lane_kind_of(&source),
        source,
        track: None,
        start_seconds: 0.0,
        over: None,
    }
}

/// A path or an address typed for one of the player's questions, turned into
/// an order — [`answered`]'s, for the three that type one.
///
/// A path that exists is made absolute against home rather than against the
/// footer's working directory, which is nobody's; anything else is handed to
/// the player as typed — an address, which mpv and yt-dlp understand. A
/// FOLDER goes as it is: the player makes it a playlist named after it.
#[must_use]
pub fn typed(id: &str, said: &str) -> Option<Ask> {
    let said = said.trim();
    if said.is_empty() || !matches!(id, PLAY | ENQUEUE | TOGETHER) {
        return None;
    }
    let item = expand(said);
    if id == TOGETHER {
        return Some(together(item));
    }
    Some(crate::player::put(item, id != ENQUEUE))
}

/// `~` and a relative path, against home; a `file://` URL, as the path it
/// names.
pub fn expand(said: &str) -> String {
    // A `file://` URL IS A PATH spelled long — what a drag and many programs
    // copy — and only as a path does a folder become its files.
    if let Some(rest) = said.strip_prefix("file://") {
        return unescaped(rest);
    }
    if said.contains("://") {
        return said.to_owned();
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let path = if let Some(rest) = said.strip_prefix("~/") {
        home.map_or_else(|| said.into(), |h| h.join(rest))
    } else if std::path::Path::new(said).is_absolute() {
        said.into()
    } else {
        home.map_or_else(|| said.into(), |h| h.join(said))
    };
    path.to_string_lossy().into_owned()
}

/// `%20` and its kin, back to the bytes they stand for.
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(Some(h)), Some(Some(l))) = (
                bytes.get(i + 1).map(|b| hex(*b)),
                bytes.get(i + 2).map(|b| hex(*b)),
            )
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a pasted `file://` URL reaches the player as an address
    /// instead of the path it names.
    #[test]
    fn a_file_url_is_the_path_it_names() {
        assert_eq!(expand("file:///music/a%20b.flac"), "/music/a b.flac");
        assert_eq!(expand("file:///m%C3%BAsica/x.ogg"), "/música/x.ogg");
        assert_eq!(expand("https://x.org/a%20b"), "https://x.org/a%20b");
    }

    /// FAILS IF: something typed to play together, or a path typed to play
    /// or to queue, stops saying what was done — the sentence the same order
    /// says when it is picked among the files.
    #[test]
    fn what_is_typed_says_what_its_pick_says() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut pairing = Pairing::default();
        let Errand::TellSaying(Ask::AddLane { source, kind, .. }, said) =
            answered_saying(TOGETHER, "/v/b.mkv", &mut pairing, &t)
        else {
            panic!("a lane together, said");
        };
        assert_eq!(source, "/v/b.mkv");
        assert_eq!(kind, ronoctua_player::own::LaneKind::Video);
        assert_eq!(
            said,
            t.t_with(crate::key::PLAYER_TOGETHER_ADDED, &[("name", "b.mkv")])
        );
        let Errand::TellSaying(Ask::Put { play: false, .. }, said) =
            answered_saying(ENQUEUE, "https://example.org/a", &mut pairing, &t)
        else {
            panic!("an address queued, said");
        };
        assert!(said.contains('a'), "{said}");
        assert_eq!(
            answered_saying(PLAY, "  ", &mut pairing, &t),
            Errand::Nothing,
            "never mind says nothing"
        );
    }

    /// FAILS IF: what is typed to play stops going right after what plays
    /// with the queue kept — the owner's rule for opening from outside — or
    /// a folder is flattened here instead of handed to the player whole.
    #[test]
    fn what_is_typed_is_an_address_or_a_path_against_home() {
        assert_eq!(
            typed(PLAY, " https://example.org/a "),
            Some(Ask::Put {
                what: vec![Thing::Location("https://example.org/a".into())],
                after: None,
                into: None,
                play: true,
                replace: false,
                once: false,
            })
        );
        assert_eq!(typed(PLAY, "   "), None);
        let folder = std::env::temp_dir();
        let Some(Ask::Put { what, .. }) = typed(ENQUEUE, &folder.to_string_lossy()) else {
            panic!("a folder queued");
        };
        assert_eq!(
            what,
            [Thing::Location(folder.to_string_lossy().into_owned())],
            "the folder went in pieces"
        );
        let Some(Ask::AddLane { source, .. }) = typed(TOGETHER, "/v/b.mkv") else {
            panic!("a lane together");
        };
        assert_eq!(source, "/v/b.mkv");
    }
}

#[cfg(test)]
mod unlogged_tests {
    /// FAILS IF: the account's user name — the person's own — reaches a log
    /// as a command's argument, or a question that is no one's stops being
    /// logged.
    #[test]
    fn the_account_s_user_is_not_logged() {
        assert!(super::unlogged(super::ACCOUNT_USER));
        assert!(!super::unlogged(super::SEARCH_RADIO));
        // ON THE LINE IT IS ASKED ON, in both doors: shown, and kept out.
        let user = super::Question::plain(super::ACCOUNT_USER, false).line();
        assert!(user.unlogged && !user.secret, "{user:?}");
        let password = super::Question::plain(super::ACCOUNT_PASSWORD, true).line();
        assert!(password.secret, "{password:?}");
        assert!(
            !super::Question::plain(super::SEARCH_RADIO, false)
                .line()
                .unlogged
        );
    }
}
