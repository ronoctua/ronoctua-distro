//! A VIDEO FROM A WEBSITE — its own page of the player (`LEADER p h`), a row
//! of "Listen and watch".
//!
//! # What it is for
//!
//! A page's address — YouTube's, or one of the many other sites yt-dlp
//! knows — played now or put in the queue. It was a row of the files
//! ("a path or an address"), where nobody looked for it: an address is not
//! a file, and the files now type paths only.
//!
//! # Every door types it
//!
//! "Play now…" and "Add to the queue…" are questions of the door's own line
//! — the footer's minibuffer, a window's and the column's header — where
//! `C-v` pastes, in both (`crate::line`). What is typed comes back to
//! [`View::website_typed`], whichever door typed it.
//!
//! # The addresses played lately
//!
//! Each address played or queued from here is remembered, newest first and
//! at most [`MOST`], in `player-websites.json` — the person's, beside their
//! other preferences, written MERGED with what is on disk
//! ([`keep`]): another door may have kept one meanwhile. THE FILE IS THE
//! BROADCAST: a door with a loop watches it while this page is on screen
//! ([`Websites::look`]), and a photograph's is handed its list
//! ([`Websites::given`]) and reads none of the machine's. `RET` on one
//! plays it now, `a` puts it in the queue, `d` forgets it.

use crate::key;
use crate::player::{Errand, Target, View};
use crate::player_rows::Builder;
use ronoctua_player::queue::Name;

/// The file the addresses are kept in, under the config root.
pub const FILE: &str = "player-websites.json";

/// How many addresses are kept: the oldest go.
pub const MOST: usize = 12;

/// The question that plays an address now — right after what plays, the
/// queue kept.
pub const PLAY: &str = "player-website-play";

/// The question that puts one at the queue's end.
pub const ENQUEUE: &str = "player-website-enqueue";

/// Whether question `id` is this page's.
#[must_use]
pub fn asks(id: &str) -> bool {
    id == PLAY || id == ENQUEUE
}

/// The file's shape: the addresses, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Addresses {
    /// Newest first, each once.
    pub addresses: Vec<String>,
}

/// One change to the list — what a door hands [`keep`], never its copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Keep this address, at the front — once.
    Add(String),
    /// Forget it.
    Forget(String),
}

/// `list` with `change` applied, at most [`MOST`]. Pure.
#[must_use]
pub fn applied(mut list: Vec<String>, change: &Change) -> Vec<String> {
    match change {
        Change::Add(address) => {
            list.retain(|a| a != address);
            list.insert(0, address.clone());
        }
        Change::Forget(address) => list.retain(|a| a != address),
    }
    list.truncate(MOST);
    list
}

/// Why the list was not written.
#[derive(Debug)]
pub enum Unkept {
    /// There is no home to keep it in.
    NoHome,
    /// The file is there and does not parse: it is kept as it is.
    Unparseable,
    /// It could not be written.
    Unwritable(std::io::Error),
}

impl Unkept {
    /// In a sentence.
    #[must_use]
    pub fn sentence(&self, texts: &ronoctua_i18n::Texts) -> String {
        let why = match self {
            Self::NoHome => texts.t(key::PLAYER_FILE_NO_HOME),
            Self::Unparseable => texts.t(key::PLAYER_FILE_UNPARSEABLE),
            Self::Unwritable(e) => e.to_string(),
        };
        texts.t_with(
            key::ECHO_SETTING_NOT_WRITTEN,
            &[("file", FILE), ("why", &why)],
        )
    }
}

/// The store of this instance.
fn store() -> ronoctua_store::Store<Addresses> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// `change` applied to the file AS IT IS NOW — read with `parsed` (a file
/// that does not parse is refused, never written over) — and written whole.
///
/// # Errors
///
/// [`Unkept`].
pub fn keep(change: &Change) -> Result<(), Unkept> {
    keep_in(&store(), change)
}

/// [`keep`], over a given store.
fn keep_in(store: &ronoctua_store::Store<Addresses>, change: &Change) -> Result<(), Unkept> {
    if store.path().is_none() {
        return Err(Unkept::NoHome);
    }
    let now = store.parsed().ok_or(Unkept::Unparseable)?;
    let addresses = applied(now.addresses, change);
    store
        .write(&Addresses { addresses })
        .map_err(Unkept::Unwritable)
}

/// THE ADDRESSES AS THIS DOOR SHOWS THEM: read when the page comes up and
/// again when the file moves — in this door or another — and never read by
/// a door with no loop, which is handed them ([`Websites::given`]).
#[derive(Debug, Default)]
pub struct Websites {
    /// Newest first.
    list: Vec<String>,
    /// Read at least once.
    read: bool,
    /// The file, watched while the page is on screen in a door with a loop.
    watched: crate::player_internet::Watched,
}

impl Websites {
    /// The list HANDED to the door — a photograph's, a test's.
    pub fn given(&mut self, list: Vec<String>) {
        self.list = list;
        self.read = true;
    }

    /// What the page takes on each settle: the file read when it moved —
    /// or this door wrote it ([`Websites::changed`]) — and watched from the
    /// first look on, waking the door. With no `wake`, nothing of this
    /// machine's is read. `true` when the list changed, and with it the
    /// rows the cursor is on.
    pub fn look(&mut self, wake: Option<&crate::wake::Wake>) -> bool {
        let Some(wake) = wake else {
            return false;
        };
        let place = [(ronoctua_store::config_dir(), FILE)];
        if self.watched.look(&place, wake) || !self.read {
            let before = std::mem::replace(&mut self.list, store().read().addresses);
            self.read = true;
            return before != self.list;
        }
        false
    }

    /// This door changed the list: shown so at once, and the file read back
    /// on the next look — a write that was refused is not shown as done.
    pub fn changed(&mut self, change: &Change) {
        self.list = applied(std::mem::take(&mut self.list), change);
        self.watched.stir();
    }

    /// The addresses, newest first.
    #[must_use]
    pub fn list(&self) -> &[String] {
        &self.list
    }
}

/// The spot of an address's row: the address itself, so the cursor stays on
/// it wherever the list moves it.
#[must_use]
pub fn spot_of(address: &str) -> String {
    format!("website:{address}")
}

/// What was typed as an address: one with its scheme as it is, a bare one
/// with a dot and no space as `https://`, and anything else `None` — a path
/// is the files' page's, and words are no address.
#[must_use]
pub fn address_of(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() || typed.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = typed.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return (typed.len() > lower.find("://")? + 3).then(|| typed.to_owned());
    }
    if typed.contains("://") || typed.starts_with('/') || typed.starts_with('~') {
        return None;
    }
    let host = typed.split('/').next().unwrap_or_default();
    (host.contains('.') && !host.starts_with('.') && !host.ends_with('.'))
        .then(|| format!("https://{typed}"))
}

/// What playing, or queueing, `address` asks, said — and the address kept
/// among the ones played lately after it: a list that could not be written
/// says so in the sentence's place.
fn play(address: String, now: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    let name = crate::player::name_of(
        &Name::Location(address.clone()),
        texts,
        crate::player::NAME_IN_A_SENTENCE,
    );
    let said = texts.t_with(
        if now {
            key::PLAYER_PLAYING_NOW
        } else {
            key::PLAYER_ADDED
        },
        &[("name", &name)],
    );
    Errand::Then(
        Box::new(Errand::TellSaying(
            crate::player::put(address.clone(), now),
            said,
        )),
        Box::new(Errand::Websites(Change::Add(address))),
    )
}

impl View {
    /// The page's rows, under its title (`player_page::PageId::key`): what
    /// works, the two questions, and the addresses played lately.
    pub fn website(&self, b: &mut Builder<'_>) {
        b.note(key::PLAYER_ABOUT_WEBSITE);
        b.pair(
            b.t(key::PLAYER_WEBSITE_PLAY),
            String::new(),
            Target::WebsiteAsk(true),
            "website-play",
        );
        b.pair(
            b.t(key::PLAYER_WEBSITE_ENQUEUE),
            String::new(),
            Target::WebsiteAsk(false),
            "website-enqueue",
        );
        b.heading(key::PLAYER_WEBSITE_RECENT);
        let list = self.websites.list();
        if list.is_empty() {
            b.note(key::PLAYER_WEBSITE_NONE);
        }
        // AN ADDRESS IS DATA, whole on its row — wrapped, never cut: its end
        // is what tells two videos of one site apart. ITS SPOT IS THE
        // ADDRESS: the list reorders on every play, and the cursor keeps the
        // address, never the line (`spot_of`).
        for address in list {
            b.pair_whole(
                address.clone(),
                String::new(),
                Target::Website(address.clone()),
                &spot_of(address),
            );
        }
    }

    /// What a press on a row of the page does — `None` for a target that
    /// is none of its.
    pub fn website_turned(
        &mut self,
        target: &Target,
        press: crate::player::Press,
        state: Option<&ronoctua_player::wire::State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        // THE LIST REORDERS UNDER THE CURSOR — the address played goes to the
        // front — and the cursor stays on the address it was on.
        let here = self.spot_here(state, columns, texts);
        let turned = self.website_list_turned(target, press, texts);
        if turned.is_some()
            && let Some(here) = here
        {
            self.follow(&here, state, columns, texts);
        }
        turned
    }

    /// [`View::website_turned`], before the cursor follows its row.
    fn website_list_turned(
        &mut self,
        target: &Target,
        press: crate::player::Press,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        use crate::player::Press;
        match (target, press) {
            (Target::WebsiteAsk(now), Press::Enter) => Some(Errand::Ask(
                crate::player_question::Question::plain(if *now { PLAY } else { ENQUEUE }, false),
            )),
            // A RECENT ONE: `RET` and `P` play it now, `a` puts it in the
            // queue — as a file of the files.
            (Target::Website(address), Press::Enter | Press::PlayNow | Press::Add) => {
                let now = press != Press::Add;
                self.websites.changed(&Change::Add(address.clone()));
                Some(play(address.clone(), now, texts))
            }
            (Target::Website(address), Press::Remove) => {
                let change = Change::Forget(address.clone());
                self.websites.changed(&change);
                Some(Errand::Then(
                    Box::new(Errand::Said(
                        texts.t_with(key::PLAYER_WEBSITE_FORGOTTEN, &[("address", address)]),
                    )),
                    Box::new(Errand::Websites(change)),
                ))
            }
            _ => None,
        }
    }

    /// An address typed on one of the page's questions: played now or put
    /// in the queue, and kept among the ones played lately. Nothing typed
    /// is never mind; what is no address is said, quoted.
    pub fn website_typed(&mut self, id: &str, said: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        if said.trim().is_empty() {
            return Errand::Nothing;
        }
        let Some(address) = address_of(said) else {
            return Errand::Said(texts.t_with(
                key::PLAYER_WEBSITE_NOT_AN_ADDRESS,
                &[("typed", said.trim())],
            ));
        };
        self.websites.changed(&Change::Add(address.clone()));
        play(address, id == PLAY, texts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Press, fixture};
    use ronoctua_player::wire::{Ask, Thing};

    /// FAILS IF: an address with its scheme stops being taken as it is, a
    /// bare host stops getting `https://`, or a path, words, a bare scheme
    /// or another scheme are taken for one.
    #[test]
    fn what_is_an_address() {
        let ok = |t: &str| address_of(t);
        assert_eq!(
            ok(" https://www.youtube.com/watch?v=abc "),
            Some("https://www.youtube.com/watch?v=abc".into())
        );
        assert_eq!(ok("http://x.org/a"), Some("http://x.org/a".into()));
        assert_eq!(ok("youtu.be/abc"), Some("https://youtu.be/abc".into()));
        assert_eq!(ok("vimeo.com/1"), Some("https://vimeo.com/1".into()));
        for no in [
            "",
            "   ",
            "/music/a.flac",
            "~/v.mkv",
            "file:///v.mkv",
            "ftp://x.org/a",
            "two words.com",
            "noplace",
            ".com",
            "https://",
        ] {
            assert_eq!(ok(no), None, "{no:?}");
        }
    }

    /// FAILS IF: the list stops keeping an address once and at the front,
    /// stops forgetting one, or keeps more than its most.
    #[test]
    fn the_list_is_newest_first_and_once() {
        let l = applied(Vec::new(), &Change::Add("a".into()));
        let l = applied(l, &Change::Add("b".into()));
        let l = applied(l, &Change::Add("a".into()));
        assert_eq!(l, ["a", "b"]);
        let l = applied(l, &Change::Forget("a".into()));
        assert_eq!(l, ["b"]);
        let mut many = Vec::new();
        for i in 0..MOST + 5 {
            many = applied(many, &Change::Add(i.to_string()));
        }
        assert_eq!(many.len(), MOST);
        assert_eq!(many[0], (MOST + 4).to_string());
    }

    /// FAILS IF: a change is written over the file instead of merged with
    /// it — another door's address lost — or a file that does not parse is
    /// written over.
    #[test]
    fn a_change_is_merged_with_the_file() {
        let dir = std::env::temp_dir().join(format!("ronoctua-websites-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let s = ronoctua_store::Store::<Addresses>::at(Some(dir.clone()), FILE);
        keep_in(&s, &Change::Add("https://a.org/1".into())).expect("kept");
        // ANOTHER DOOR wrote one meanwhile.
        s.write(&Addresses {
            addresses: vec!["https://b.org/2".into(), "https://a.org/1".into()],
        })
        .expect("written");
        keep_in(&s, &Change::Add("https://c.org/3".into())).expect("kept");
        assert_eq!(
            s.read().addresses,
            ["https://c.org/3", "https://b.org/2", "https://a.org/1"]
        );
        std::fs::write(dir.join(FILE), "{ not json").expect("broken");
        assert!(matches!(
            keep_in(&s, &Change::Add("https://d.org".into())),
            Err(Unkept::Unparseable)
        ));
        assert_eq!(
            std::fs::read_to_string(dir.join(FILE)).expect("still there"),
            "{ not json"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an address typed plays now — or goes to the queue's end,
    /// asking "already there?" — with the sentence its pick says, stops being
    /// kept among the ones played lately, or words typed are sent to the
    /// player instead of said back.
    #[test]
    fn an_address_typed_plays_and_is_kept() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut v = View::blank(None);
        let errand = v.website_typed(ENQUEUE, "youtu.be/abc", &t);
        let Errand::Then(first, then) = errand else {
            panic!("an order and the list");
        };
        match *first {
            Errand::TellSaying(
                Ask::Put {
                    what, play, once, ..
                },
                said,
            ) => {
                assert_eq!(what, [Thing::Location("https://youtu.be/abc".into())]);
                assert!(!play && once, "queued once");
                assert!(said.contains("abc"), "{said}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            *then,
            Errand::Websites(Change::Add("https://youtu.be/abc".into()))
        );
        assert_eq!(v.websites.list(), ["https://youtu.be/abc"]);
        match v.website_typed(PLAY, "https://x.org/v", &t) {
            Errand::Then(first, _) => assert!(matches!(
                *first,
                Errand::TellSaying(
                    Ask::Put {
                        play: true,
                        once: false,
                        ..
                    },
                    _
                )
            )),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            v.website_typed(PLAY, "some words", &t),
            Errand::Said(s) if s.contains("some words")
        ));
        assert_eq!(v.website_typed(PLAY, "  ", &t), Errand::Nothing);
    }

    /// FAILS IF: the page stops drawing its two questions and every address
    /// kept — or a recent one stops playing on `RET`, going to the queue on
    /// `a`, or being forgotten on `d` — or the cursor keeps its LINE when the
    /// list reorders, and a second `a` acts on another address.
    #[test]
    fn the_page_plays_what_it_lists() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut v = View::blank(None);
        v.websites
            .given(vec!["https://a.org/1".into(), "https://b.org/2".into()]);
        let s = fixture::idle();
        v.key(Press::Go(crate::player::PageId::Website), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let spots: Vec<&str> = rows.iter().map(|r| r.spot.as_str()).collect();
        let (a, b) = (spot_of("https://a.org/1"), spot_of("https://b.org/2"));
        for spot in ["website-play", "website-enqueue", &a, &b] {
            assert!(spots.contains(&spot), "{spot} in {spots:?}");
        }
        let at = rows
            .iter()
            .position(|r| r.target == Target::Website("https://b.org/2".into()))
            .expect("the row");
        v.follow(&b, Some(&s), 80, &t);
        assert_eq!(v.cursor(), at);
        let queued = |e: Errand| match e {
            Errand::Then(first, _) => match *first {
                Errand::TellSaying(
                    Ask::Put {
                        what, play: false, ..
                    },
                    _,
                ) => what,
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        let b_thing = vec![Thing::Location("https://b.org/2".into())];
        assert_eq!(queued(v.key(Press::Add, Some(&s), 80, &t)), b_thing);
        // THE ONE PLAYED went to the front — and the cursor with it: `a`
        // again queues the SAME address, not the one now on its old line.
        assert_eq!(v.websites.list()[0], "https://b.org/2");
        assert_eq!(v.spot_here(Some(&s), 80, &t).as_deref(), Some(b.as_str()));
        assert_eq!(queued(v.key(Press::Add, Some(&s), 80, &t)), b_thing);
        v.follow(&b, Some(&s), 80, &t);
        assert!(matches!(
            v.key(Press::Remove, Some(&s), 80, &t),
            Errand::Then(..)
        ));
        assert_eq!(v.websites.list(), ["https://a.org/1"]);
        let rows = v.rows(Some(&s), 80, &t);
        let ask = rows
            .iter()
            .position(|r| r.spot == "website-play")
            .expect("the question's row");
        v.follow("website-play", Some(&s), 80, &t);
        assert_eq!(v.cursor(), ask);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(crate::player_question::Question::plain(PLAY, false))
        );
    }
}
