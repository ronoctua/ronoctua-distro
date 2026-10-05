//! THE CATALOGUE'S SOURCES, as a door sees them: the defaults, each with its
//! consent and its posters; the person's own, each with its address MASKED,
//! its kind, its switches and a way to forget it; and the row that adds one
//! by pasting its link.
//!
//! # An address is never shown whole
//!
//! A pasted IPTV address carries the person's account. The daemon keeps it
//! in `sources.json` (`0600`), and every door is told the MASKED one only —
//! `host/…` (`ronoctua_player::wire::SourceStatus::shown`). An `http` one is
//! taken — the owner's decision — and said, beside it, to cross the network
//! in the clear.
//!
//! # Every switch is the daemon's to write
//!
//! Posters and "adult" are the daemon's (`Ask::SetSource`): it writes the
//! book, and an adult switch moves the source's rows between the two files
//! with a new reading. Turning a default source's posters on lets the hosts
//! its pictures come from, which its row names. Forgetting a source asks
//! first, "no" drawn first.
//!
//! # A source added straight to the adult catalogue
//!
//! The adult catalogue has a section of its own here, behind its lock: shut,
//! how many sources it keeps and the row that asks the password; open, its
//! sources, and a row that adds one straight into it (`Ask::AddSource` with
//! `adult`) — a pasted adult list never passes through the main catalogue,
//! where its titles, letters and posters would be outside the lock until
//! someone found its switch.

use ronoctua_broadcast::table::DefaultSource;
use ronoctua_i18n::key;
use ronoctua_player::wire::{Ask, SourceId, SourceStatus, SourceUrl, State};

use crate::player::{Errand, Press, Target, Typing, View};
use crate::player_catalog::{ASK_ADULT_SOURCE, ASK_SOURCE, SEARCHED, source_name};
use crate::player_question::Question;
use crate::player_rows::Builder;

/// A kind of source's name.
const fn kind_key(kind: ronoctua_broadcast::sources::Kind) -> &'static str {
    use ronoctua_broadcast::sources::Kind as K;
    match kind {
        K::IptvOrg => key::PLAYER_SOURCE_KIND_IPTV_ORG,
        K::Archive => key::PLAYER_SOURCE_KIND_ARCHIVE,
        K::Lives => key::PLAYER_SOURCE_KIND_YOUTUBE_LIVES,
        K::Nasa => key::PLAYER_SOURCE_KIND_NASA,
        K::Commons => key::PLAYER_SOURCE_KIND_COMMONS,
        K::PeerTube => key::PLAYER_SOURCE_KIND_PEERTUBE,
        K::Playlist => key::PLAYER_SOURCE_KIND_PLAYLIST,
        K::Feed => key::PLAYER_SOURCE_KIND_FEED,
        K::YouTube => key::PLAYER_SOURCE_KIND_YOUTUBE,
        K::Collection => key::PLAYER_SOURCE_KIND_COLLECTION,
    }
}

/// Where a source's posters come from, in words.
fn poster_hosts(id: &str, b: &Builder<'_>) -> String {
    match DefaultSource::of(id) {
        Some(d) if !d.poster_hosts.is_empty() => b.texts.t_with(
            key::PLAYER_SOURCE_HOSTS,
            &[("hosts", &d.poster_hosts.join(&b.t(key::PLAYER_LIST_JOINER)))],
        ),
        _ => b.t(key::PLAYER_SOURCE_HOSTS_ANY),
    }
}

/// THE SOURCES' PAGE: what it is, the row that adds one, and every source —
/// the defaults, then the person's.
pub(crate) fn page(v: &View, state: Option<&State>, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_SOURCES);
    b.pair(
        b.t(key::PLAYER_SOURCE_ADD),
        String::new(),
        Target::SourceAdd,
        "source-add",
    );
    let Some(s) = state else {
        return;
    };
    if let Some(t) = &s.catalog.trouble {
        b.warning(&crate::player::trouble_sentence(t, b.texts));
    }
    let listed = || s.catalog.sources.iter().filter(|x| x.id.0 != SEARCHED);
    for status in listed().filter(|x| !x.adult) {
        source_rows(v, status, state, b);
    }
    // THE ADULT CATALOGUE'S SOURCES, behind its lock here too: shut, their
    // names and addresses are not on screen — only how many there are, and
    // the row that asks the password; open, each of them, and the row that
    // adds one straight into it.
    b.heading(key::PLAYER_SOURCE_ADULT_HIDDEN);
    b.note(key::PLAYER_ADULT_HONEST);
    if !v.catalog.lock.open() {
        let kept_apart = listed().filter(|x| x.adult).count();
        v.lock_rows(u64::try_from(kept_apart).unwrap_or(u64::MAX), b);
        return;
    }
    b.pair(
        b.t(key::PLAYER_SOURCE_ADD_ADULT),
        String::new(),
        Target::SourceAddAdult,
        "source-add-adult",
    );
    b.pair(
        b.t(key::PLAYER_ADULT_LOCK),
        String::new(),
        Target::CatalogLock,
        "adult-lock",
    );
    for status in listed().filter(|x| x.adult) {
        source_rows(v, status, state, b);
    }
}

/// One source's rows, under its name.
fn source_rows(v: &View, status: &SourceStatus, state: Option<&State>, b: &mut Builder<'_>) {
    let id = status.id.0.as_str();
    let default = DefaultSource::of(id).is_some();
    b.heading_said(source_name(id, state, b.texts));
    if !default {
        b.pair(
            b.t(key::PLAYER_SOURCE_ADDRESS),
            status.shown.clone(),
            Target::Nothing,
            &format!("address-{id}"),
        );
        if status.plain {
            b.warning(&b.t(key::PLAYER_SOURCE_PLAIN));
        }
    }
    b.pair(
        b.t(key::PLAYER_SOURCE_KIND),
        b.t(kind_key(status.kind)),
        Target::Nothing,
        &format!("kind-{id}"),
    );
    v.source_standing(id, state, b);
    b.pair_whole(
        b.t(key::PLAYER_SOURCE_POSTERS),
        format!(
            "{} — {}",
            crate::player_rows::on_off(status.posters, b.texts),
            poster_hosts(id, b)
        ),
        Target::SourcePosters(id.to_owned()),
        &format!("posters-{id}"),
    );
    if !default {
        b.pair(
            b.t(key::PLAYER_SOURCE_ADULT),
            crate::player_rows::on_off(status.adult, b.texts),
            Target::SourceAdult(id.to_owned()),
            &format!("adult-{id}"),
        );
        b.pair(
            b.t(key::PLAYER_SOURCE_REMOVE),
            String::new(),
            Target::SourceRemove(id.to_owned()),
            &format!("remove-{id}"),
        );
    }
}

/// What was pasted to add a source — to the adult catalogue when `adult`:
/// told to the daemon, which recognises it — or refuses it, in a sentence —
/// and reads it into that file.
pub(crate) fn pasted(said: &str, adult: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    let url = said.trim();
    if url.is_empty() {
        return Errand::Nothing;
    }
    let shown = SourceUrl(url.to_owned()).masked();
    Errand::TellSaying(
        Ask::AddSource {
            url: SourceUrl(url.to_owned()),
            adult,
        },
        texts.t_with(key::PLAYER_SOURCE_ADDING, &[("name", &shown)]),
    )
}

impl View {
    /// What a press does on a row of the sources' page — `None` for a target
    /// that is none of its.
    pub(crate) fn sources_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        let status = |id: &str| state.and_then(|s| s.catalog.sources.iter().find(|x| x.id.0 == id));
        let turns = matches!(press, Press::Enter | Press::Less | Press::More);
        match target {
            Target::SourceAdd if press == Press::Enter => {
                self.typing = Some(Typing::SourceUrl);
                Some(Errand::Ask(Question::plain(ASK_SOURCE, false)))
            }
            // STRAIGHT TO THE ADULT CATALOGUE — a row there only while it is
            // open, and the answer taken only while it still is.
            Target::SourceAddAdult if press == Press::Enter => {
                if !self.catalog.lock.open() {
                    return Some(Errand::Said(texts.t(key::PLAYER_ADULT_LOCKED_AGAIN)));
                }
                self.typing = Some(Typing::AdultSourceUrl);
                Some(Errand::Ask(Question::plain(ASK_ADULT_SOURCE, false)))
            }
            Target::SourcePosters(id) if turns => {
                let on = !status(id).is_some_and(|s| s.posters);
                // A DEFAULT SOURCE'S HOSTS are let by the daemon as it turns
                // the switch on: the consent file is read again after.
                self.catalog.stir_the_consent();
                Some(Errand::TellSaying(
                    Ask::SetSource {
                        id: SourceId(id.clone()),
                        posters: Some(on),
                        adult: None,
                    },
                    texts.t(if on {
                        key::PLAYER_SOURCE_POSTERS_ON
                    } else {
                        key::PLAYER_SOURCE_POSTERS_OFF
                    }),
                ))
            }
            Target::SourceAdult(id) if turns => {
                let on = !status(id).is_some_and(|s| s.adult);
                Some(Errand::TellSaying(
                    Ask::SetSource {
                        id: SourceId(id.clone()),
                        posters: None,
                        adult: Some(on),
                    },
                    texts.t(if on {
                        key::PLAYER_SOURCE_ADULT_ON
                    } else {
                        key::PLAYER_SOURCE_ADULT_OFF
                    }),
                ))
            }
            // FORGOTTEN AFTER A QUESTION, "no" first: its rows go with it.
            Target::SourceRemove(id) if press == Press::Enter => {
                let name = source_name(id, state, texts);
                let yes = Errand::TellSaying(
                    Ask::RemoveSource {
                        id: SourceId(id.clone()),
                    },
                    texts.t_with(key::PLAYER_SOURCE_REMOVED, &[("name", &name)]),
                );
                Some(self.ask_about(
                    texts.t_with(key::ASK_PLAYER_SOURCE_REMOVE, &[("name", &name)]),
                    yes,
                ))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PageId;
    use ronoctua_broadcast::sources::Kind;

    fn person(id: &str, title: &str, adult: bool, plain: bool) -> SourceStatus {
        SourceStatus {
            id: SourceId(id.into()),
            kind: Kind::Playlist,
            title: Some(title.into()),
            shown: "tv.example/…".into(),
            adult,
            posters: false,
            plain,
            items: 3,
            busy: false,
            trouble: None,
        }
    }

    fn said(v: &View, s: &State, t: &ronoctua_i18n::Texts) -> String {
        v.rows(Some(s), 100, t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|")
    }

    /// FAILS IF: a source of the adult catalogue is named on the sources'
    /// page while the catalogue is shut — or stays hidden once it is open —
    /// or a plain `http` source stops saying it is not encrypted, or its
    /// address is shown whole.
    #[test]
    fn an_adult_source_is_behind_the_lock_and_a_plain_one_says_so() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut s = State::default();
        s.catalog.sources = vec![
            person("p1", "Minha lista", false, true),
            person("p2", "Noite", true, false),
        ];
        let mut v = View::blank(None);
        v.push(PageId::Sources, None);
        let shut = said(&v, &s, &t);
        assert!(shut.contains("Minha lista"), "{shut}");
        assert!(
            !shut.contains("Noite"),
            "an adult source named while shut: {shut}"
        );
        assert!(
            shut.contains(&t.t(key::PLAYER_SOURCE_ADULT_HIDDEN)),
            "{shut}"
        );
        // THE SENTENCE WRAPS, as a note does: its first words on one line.
        let plain = t.t(key::PLAYER_SOURCE_PLAIN);
        let head: Vec<&str> = plain.split_whitespace().take(2).collect();
        assert!(shut.contains(&head.join(" ")), "{shut}");
        assert!(shut.contains("tv.example/…"), "{shut}");
        v.unlocked(Ok(()));
        let open = said(&v, &s, &t);
        assert!(open.contains("Noite"), "{open}");
        assert!(open.contains(&t.t(key::PLAYER_SOURCE_ADD_ADULT)), "{open}");
    }

    /// FAILS IF: a source cannot be added straight to the adult catalogue —
    /// a pasted adult list went into the main one first, its titles, letters
    /// and posters outside the lock until its switch was found — or one can
    /// be while the catalogue is shut, or the link is taken after it shut.
    #[test]
    fn a_source_is_added_straight_to_the_adult_catalogue_only_while_it_is_open() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let s = State::default();
        let st = Some(&s);
        let mut v = View::blank(None);
        v.push(PageId::Sources, None);
        let shut = said(&v, &s, &t);
        assert!(!shut.contains(&t.t(key::PLAYER_SOURCE_ADD_ADULT)), "{shut}");
        assert!(shut.contains(&t.t(key::PLAYER_ADULT_LOCKED)), "{shut}");
        let add = Target::SourceAddAdult;
        assert!(matches!(
            v.sources_turned(&add, Press::Enter, st, &t),
            Some(Errand::Said(_))
        ));
        v.unlocked(Ok(()));
        let open = said(&v, &s, &t);
        assert!(open.contains(&t.t(key::PLAYER_SOURCE_ADD_ADULT)), "{open}");
        let Some(Errand::Ask(q)) = v.sources_turned(&add, Press::Enter, st, &t) else {
            panic!("the link asked");
        };
        assert_eq!(q.id, ASK_ADULT_SOURCE);
        let Errand::TellSaying(Ask::AddSource { url, adult }, _) =
            v.catalog_answered(ASK_ADULT_SOURCE, " http://tv.example/l.m3u ", st, 100, &t)
        else {
            panic!("the link is told to the player");
        };
        assert!(adult, "straight into the adult catalogue");
        assert_eq!(url.0, "http://tv.example/l.m3u");
        // SHUT WHILE IT WAS TYPED: taken by nothing.
        v.sources_turned(&add, Press::Enter, st, &t);
        v.catalog.lock.shut();
        assert!(matches!(
            v.catalog_answered(ASK_ADULT_SOURCE, "http://tv.example/l.m3u", st, 100, &t),
            Errand::Said(_)
        ));
    }

    /// FAILS IF: removing a source stops asking first — the cursor born on
    /// "no", which forgets nothing — or a pasted link is told anywhere but
    /// to the player.
    #[test]
    fn a_source_is_removed_after_a_question_and_a_link_goes_to_the_player() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut s = State::default();
        s.catalog.sources = vec![person("p1", "Minha lista", false, false)];
        let s = Some(&s);
        let remove = Target::SourceRemove("p1".into());
        let mut v = View::blank(None);
        v.push(PageId::Sources, None);
        assert_eq!(
            v.sources_turned(&remove, Press::Enter, s, &t),
            Some(Errand::Nothing),
            "asked, not done"
        );
        assert_eq!(
            v.key(Press::Enter, s, 100, &t),
            Errand::Nothing,
            "\"no\" first"
        );
        v.sources_turned(&remove, Press::Enter, s, &t);
        v.key(Press::Down, s, 100, &t);
        assert!(matches!(
            v.key(Press::Enter, s, 100, &t),
            Errand::TellSaying(Ask::RemoveSource { id }, _) if id.0 == "p1"
        ));
        let Errand::TellSaying(Ask::AddSource { url, adult }, _) =
            pasted(" http://iptv.example/list.m3u?u=a&p=b ", false, &t)
        else {
            panic!("the link is told to the player");
        };
        assert_eq!(url.0, "http://iptv.example/list.m3u?u=a&p=b");
        assert!(!adult);
    }
}
