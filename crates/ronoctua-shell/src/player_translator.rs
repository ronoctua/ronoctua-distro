//! WHO TRANSLATES THE LYRICS, chosen on the player's Internet page: MyMemory
//! (keyless, the default), DeepL with the person's key, or a LibreTranslate
//! server the person names (`ronoctua_words::Provider`).
//!
//! # The rows
//!
//! A RING, "Translator", turns the three; the consent row under it names the
//! hosts of the one chosen — DeepL's two (a free key and a paid one go to
//! different hosts), a LibreTranslate server's own — and lets or stops
//! letting them, host by host (`ronoctua_consent`). DeepL shows the row of
//! its key; LibreTranslate the row of its address and the one of its key,
//! which some servers want and others do not. A key is asked on a masked
//! line and kept private from its first byte (`ronoctua_words::keys`); the
//! row says whether one is kept, never what it is, and `d` forgets it after
//! a question, "no" first. Typing a key lets the host it is sent to: the row
//! named it.
//!
//! # Where it is kept
//!
//! In `player.json`, MERGED with what is on disk (`Errand::Keep`), like every
//! switch of the page: the rows are there with no player running, and a
//! player that runs reads the file again (`Ask::Reload`) and asks the lyrics
//! of the one chosen. A LibreTranslate address is kept beside it, so the ring
//! comes back to the same server; turning the ring to LibreTranslate with
//! none kept asks for one first.

use crate::player::{Errand, Switch, Target, Typing, View};
use crate::player_question::Question;
use crate::player_rows::Builder;
use ronoctua_i18n::key;
use ronoctua_player::prefs::Preferences;
use ronoctua_words::Provider;
use ronoctua_words::keys::Key;

/// The address of a LibreTranslate server, typed.
pub const ADDRESS: &str = "player-translator-address";
/// DeepL's key — masked.
pub const KEY_DEEPL: &str = "player-key-deepl";
/// A LibreTranslate server's key — masked.
pub const KEY_LIBRETRANSLATE: &str = "player-key-libretranslate";

/// Whether question `id` is one of these rows'.
#[must_use]
pub fn asks(id: &str) -> bool {
    matches!(id, ADDRESS | KEY_DEEPL | KEY_LIBRETRANSLATE)
}

/// The question a key is asked on.
const fn question_of(k: Key) -> &'static str {
    match k {
        Key::DeepL => KEY_DEEPL,
        Key::LibreTranslate => KEY_LIBRETRANSLATE,
    }
}

/// A translator's name, by its id (`Provider::id`) — what the ring says and
/// what "Translated by" says; `None` for an id this build does not know.
#[must_use]
pub fn name_key(id: &str) -> Option<&'static str> {
    Some(match id {
        "mymemory" => key::PLAYER_TRANSLATOR_MYMEMORY,
        "deepl" => key::PLAYER_TRANSLATOR_DEEPL,
        "libretranslate" => key::PLAYER_TRANSLATOR_LIBRETRANSLATE,
        _ => return None,
    })
}

/// A translator's name, by its id — the id itself for one this build does
/// not know, as a fact the daemon said.
#[must_use]
pub fn name_of(id: &str, texts: &ronoctua_i18n::Texts) -> String {
    name_key(id).map_or_else(|| id.to_owned(), |k| texts.t(k))
}

/// The hosts the chosen translator is asked at — what its consent row names
/// and lets.
#[must_use]
pub fn hosts(prefs: &Preferences) -> Vec<String> {
    prefs.lyrics_translation.by.hosts()
}

/// The hosts a press that lets `host` lets: every host of the chosen
/// translator when `host` is one of them — DeepL's two, both named on its
/// consent row, which says "allowed" only when all are — else `host` alone.
#[must_use]
pub fn to_let(chosen: &[String], host: &str) -> Vec<String> {
    if chosen.iter().any(|h| h == host) {
        chosen.to_vec()
    } else {
        vec![host.to_owned()]
    }
}

/// Whether every host of the chosen translator is let — none, for an
/// address that names no host, is not.
#[must_use]
pub fn let_now(prefs: &Preferences) -> bool {
    let hosts = hosts(prefs);
    !hosts.is_empty() && hosts.iter().all(|h| prefs.allows_host(h))
}

/// The translation keys kept on this machine, by whether their files are
/// there — never what they hold.
#[must_use]
pub fn kept_keys() -> Vec<Key> {
    let Some(dir) = ronoctua_words::keys::dir() else {
        return Vec::new();
    };
    Key::ALL
        .into_iter()
        .filter(|k| dir.path().join(k.file()).is_file())
        .collect()
}

/// The files of the translation keys, by folder and name — what a door
/// watches while its Internet page could show them.
#[must_use]
pub fn key_files() -> Vec<(Option<std::path::PathBuf>, &'static str)> {
    let dir = ronoctua_words::keys::dir().map(|d| d.path().to_path_buf());
    Key::ALL.iter().map(|k| (dir.clone(), k.file())).collect()
}

/// A key's row: its words.
const fn key_label(k: Key) -> &'static str {
    match k {
        Key::DeepL => key::PLAYER_KEY_DEEPL,
        Key::LibreTranslate => key::PLAYER_KEY_LIBRETRANSLATE,
    }
}

/// What a key is, in a sentence about it.
const fn key_words(k: Key) -> &'static str {
    match k {
        Key::DeepL => key::PLAYER_KEY_WORDS_DEEPL,
        Key::LibreTranslate => key::PLAYER_KEY_WORDS_LIBRETRANSLATE,
    }
}

/// A translation key's trouble as the player's key rows say theirs — the
/// same closed set, one sentence each (`player_internet::key_trouble`).
#[must_use]
pub const fn trouble_of(t: ronoctua_words::keys::KeyTrouble) -> ronoctua_lookup::keys::KeyTrouble {
    use ronoctua_lookup::keys::KeyTrouble as Lookup;
    use ronoctua_words::keys::KeyTrouble as Words;
    match t {
        Words::NoHome => Lookup::NoHome,
        Words::Empty => Lookup::Empty,
        Words::Malformed => Lookup::Malformed,
        Words::Unreadable => Lookup::Unreadable,
        Words::Unwritable => Lookup::Unwritable,
    }
}

impl View {
    /// The translator's rows on the Internet page: the ring, the consent of
    /// the hosts of the one chosen, and its key and address.
    pub(crate) fn translator_rows(&self, b: &mut Builder<'_>) {
        let by = &self.prefs.lyrics_translation.by;
        b.pair(
            b.t(key::PLAYER_TRANSLATOR),
            name_of(by.id(), b.texts),
            Target::TranslatorWho,
            "translator",
        );
        let hosts = hosts(&self.prefs);
        b.pair_under(
            b.t(key::PLAYER_CONSENT_TRANSLATION),
            b.t(if self.translator_let {
                key::PLAYER_CONSENT_ALLOWED
            } else {
                key::PLAYER_CONSENT_NOT_ALLOWED
            }),
            &b.texts.t_with(
                key::PLAYER_CONSENT_HOSTS,
                &[("hosts", &hosts.join(&b.t(key::PLAYER_SET_JOINER)))],
            ),
            Target::Translator,
            "consent-translation",
        );
        b.nested(|b| {
            if let Provider::LibreTranslate { url } = by {
                b.pair(
                    b.t(key::PLAYER_TRANSLATOR_ADDRESS),
                    url.clone(),
                    Target::TranslatorAddress,
                    "translator-address",
                );
            }
            if let Some(k) = by.key() {
                b.pair(
                    b.t(key_label(k)),
                    b.t(if self.words_kept.contains(&k) {
                        key::PLAYER_KEY_KEPT
                    } else {
                        key::PLAYER_KEY_NONE
                    }),
                    Target::WordsKey(k),
                    &format!("key-{}", k.file()),
                );
            }
        });
    }

    /// What a press on one of the translator's rows does — `None` for a
    /// target that is none of its.
    pub(crate) fn translator_turned(
        &mut self,
        target: &Target,
        press: crate::player::Press,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        use crate::player::Press;
        match (target, press) {
            // THE RING: the next translator — LibreTranslate with no server
            // kept asks for one first.
            (Target::TranslatorWho, Press::Enter | Press::Less | Press::More) => {
                let t = &self.prefs.lyrics_translation;
                let ring = t.ring();
                let ids = ["mymemory", "deepl", "libretranslate"];
                let now = ids.iter().position(|id| *id == t.by.id()).unwrap_or(0);
                let next = if press == Press::Less {
                    (now + ids.len() - 1) % ids.len()
                } else {
                    (now + 1) % ids.len()
                };
                Some(match ring[next].clone() {
                    Some(provider) => self.translator_chosen(provider),
                    None => {
                        self.typing = Some(Typing::TranslatorAddress);
                        Errand::Ask(Question::plain(ADDRESS, false))
                    }
                })
            }
            (Target::TranslatorAddress, Press::Enter) => {
                self.typing = Some(Typing::TranslatorAddress);
                let now = self.prefs.lyrics_translation.address.clone();
                Some(Errand::Ask(now.map_or_else(
                    || Question::plain(ADDRESS, false),
                    |url| Question::holding(ADDRESS, url),
                )))
            }
            (Target::WordsKey(k), Press::Enter) => {
                self.typing = Some(Typing::WordsKey(*k));
                Some(Errand::Ask(Question::plain(question_of(*k), true)))
            }
            (Target::WordsKey(k), Press::Remove) => {
                let what = texts.t(key_words(*k));
                if !self.words_kept.contains(k) {
                    return Some(Errand::Said(
                        texts.t_with(key::PLAYER_KEY_NOTHING_TO_FORGET, &[("what", &what)]),
                    ));
                }
                let sentence = texts.t_with(key::PLAYER_ASK_FORGET_KEY, &[("what", &what)]);
                Some(self.ask_about(sentence, Errand::ForgetWordsKey(*k)))
            }
            _ => None,
        }
    }

    /// `provider` chosen: shown at once, kept in `player.json` merged, and
    /// its hosts' consent read for the row under it.
    fn translator_chosen(&mut self, provider: Provider) -> Errand {
        Switch::Translator(provider.clone()).apply(&mut self.prefs);
        self.translator_let = let_now(&self.prefs);
        self.prefs_written();
        Errand::Keep(Switch::Translator(provider))
    }

    /// What was typed on one of the translator's questions: a server's
    /// address — refused in a sentence when it names no `https://` host — or
    /// a key, kept and its host let.
    pub(crate) fn translator_answered(
        &mut self,
        said: &str,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let said = said.trim();
        match self.typing.take() {
            Some(Typing::TranslatorAddress) => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                if ronoctua_words::host_of(said).is_none() {
                    return Errand::Said(
                        texts.t_with(key::PLAYER_TRANSLATOR_NOT_AN_ADDRESS, &[("typed", said)]),
                    );
                }
                self.translator_chosen(Provider::LibreTranslate {
                    url: said.to_owned(),
                })
            }
            Some(Typing::WordsKey(k)) => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                let secret = ronoctua_consent::keys::Secret::new(said);
                // THE HOSTS THE ROW NAMED, let: DeepL's two — the key goes to
                // one of them, and the consent row says "allowed" only when
                // every host it names is (`let_now`).
                let hosts = match k {
                    Key::DeepL => Provider::DeepL.hosts(),
                    Key::LibreTranslate => hosts(&self.prefs),
                };
                if !self.words_kept.contains(&k) {
                    self.words_kept.push(k);
                }
                self.translator_let = true;
                Errand::LetHosts {
                    hosts,
                    on: true,
                    then: Box::new(Errand::KeepWordsKey { key: k, secret }),
                }
            }
            other => {
                self.typing = other;
                Errand::Nothing
            }
        }
    }
}

/// The sentence said when a translation key was handed to its thread to be
/// kept — one that could not be is said when the thread says so.
#[must_use]
pub fn kept_said(k: Key, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        key::PLAYER_KEY_KEPT_SAID,
        &[("what", &texts.t(key_words(k)))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{PageId, Press};
    use ronoctua_player::wire::State;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    fn on_the_internet_page(t: &ronoctua_i18n::Texts) -> View {
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::Internet), None::<&State>, 80, t);
        v
    }

    fn stand(v: &mut View, target: &Target, t: &ronoctua_i18n::Texts) {
        let rows = v.rows(None, 80, t);
        let at = rows
            .iter()
            .position(|r| r.target == *target && r.stops())
            .unwrap_or_else(|| panic!("no row {target:?}"));
        v.clicked(at, &rows);
    }

    /// FAILS IF: the lyrics' translator is not choosable on the Internet
    /// page — the ring does not turn MyMemory, DeepL and LibreTranslate, the
    /// consent row does not name the chosen one's hosts, LibreTranslate with
    /// no server kept is chosen without asking one, an address that names no
    /// host is taken, or what is chosen is not written as a switch.
    #[test]
    fn the_translator_is_chosen_on_the_internet_page() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut v = on_the_internet_page(&t);
            let said = |v: &View, target: &Target| {
                let rows = v.rows(None, 80, &t);
                let row = rows.iter().find(|r| r.target == *target).expect("a row");
                row.fields
                    .iter()
                    .map(|f| f.text.clone())
                    .collect::<Vec<_>>()
            };
            assert!(
                said(&v, &Target::TranslatorWho).contains(&t.t(key::PLAYER_TRANSLATOR_MYMEMORY))
            );
            // MyMemory → DeepL.
            stand(&mut v, &Target::TranslatorWho, &t);
            let errand = v.key(Press::Enter, None, 80, &t);
            assert_eq!(errand, Errand::Keep(Switch::Translator(Provider::DeepL)));
            assert!(said(&v, &Target::TranslatorWho).contains(&t.t(key::PLAYER_TRANSLATOR_DEEPL)));
            let rows = v.rows(None, 80, &t);
            let words: String = rows
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(words.contains(ronoctua_words::DEEPL_FREE_HOST), "{words}");
            assert!(words.contains(ronoctua_words::DEEPL_HOST));
            assert!(
                rows.iter()
                    .any(|r| r.target == Target::WordsKey(Key::DeepL))
            );
            // DeepL → LibreTranslate, with no server kept: asked first.
            stand(&mut v, &Target::TranslatorWho, &t);
            let errand = v.key(Press::Enter, None, 80, &t);
            assert_eq!(errand, Errand::Ask(Question::plain(ADDRESS, false)));
            assert_eq!(
                v.translator_answered("http://plain.example", &t),
                Errand::Said(t.t_with(
                    key::PLAYER_TRANSLATOR_NOT_AN_ADDRESS,
                    &[("typed", "http://plain.example")]
                )),
                "plain http names no host to ask"
            );
            v.typing = Some(Typing::TranslatorAddress);
            let url = "https://translate.example".to_owned();
            assert_eq!(
                v.translator_answered(&url, &t),
                Errand::Keep(Switch::Translator(Provider::LibreTranslate {
                    url: url.clone()
                }))
            );
            assert!(said(&v, &Target::TranslatorAddress).contains(&url));
            assert!(
                v.rows(None, 80, &t)
                    .iter()
                    .any(|r| r.target == Target::WordsKey(Key::LibreTranslate))
            );
            // ROUND THE RING to MyMemory, and back to the server kept — not
            // asked again.
            stand(&mut v, &Target::TranslatorWho, &t);
            assert_eq!(
                v.key(Press::Enter, None, 80, &t),
                Errand::Keep(Switch::Translator(Provider::MyMemory))
            );
            stand(&mut v, &Target::TranslatorWho, &t);
            assert_eq!(
                v.key(Press::Less, None, 80, &t),
                Errand::Keep(Switch::Translator(Provider::LibreTranslate { url }))
            );
        }
    }

    /// FAILS IF: a translation key typed is not kept, or not kept masked, or
    /// the host it is sent to is not let — or one never kept is "forgotten"
    /// without a word, or one kept is forgotten without a question.
    #[test]
    fn a_translation_key_is_asked_masked_kept_and_forgotten_after_a_question() {
        let t = texts("en-GB");
        let mut v = on_the_internet_page(&t);
        Switch::Translator(Provider::DeepL).apply(&mut v.prefs);
        stand(&mut v, &Target::WordsKey(Key::DeepL), &t);
        assert_eq!(
            v.key(Press::Remove, None, 80, &t),
            Errand::Said(t.t_with(
                key::PLAYER_KEY_NOTHING_TO_FORGET,
                &[("what", &t.t(key::PLAYER_KEY_WORDS_DEEPL))]
            ))
        );
        assert_eq!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Ask(Question::plain(KEY_DEEPL, true)),
            "asked masked"
        );
        let Errand::LetHosts { hosts, on, then } = v.translator_answered(" abc:fx ", &t) else {
            panic!("the key's host was not let");
        };
        let both = vec![
            ronoctua_words::DEEPL_FREE_HOST.to_owned(),
            ronoctua_words::DEEPL_HOST.to_owned(),
        ];
        assert_eq!((hosts.clone(), on), (both, true));
        // READ AGAIN, as the consent file's watch reads it: the row says
        // "allowed" — every host it names is let.
        let refs: Vec<&str> = hosts.iter().map(String::as_str).collect();
        v.prefs.hosts = std::mem::take(&mut v.prefs.hosts).with(&refs, true);
        assert!(let_now(&v.prefs), "the consent row says what is asked");
        assert!(matches!(
            *then,
            Errand::KeepWordsKey {
                key: Key::DeepL,
                ..
            }
        ));
        assert!(v.words_kept.contains(&Key::DeepL));
        stand(&mut v, &Target::WordsKey(Key::DeepL), &t);
        v.key(Press::Remove, None, 80, &t);
        assert!(
            v.waits_for_an_answer(),
            "forgetting one kept is asked first"
        );
    }

    /// FAILS IF: the lyrics' row that lets the host DeepL was refused at
    /// lets that one alone — the consent row names both of DeepL's hosts
    /// and would go on saying "not allowed" while DeepL is asked — or a host
    /// that is not the chosen translator's lets the translator's.
    #[test]
    fn letting_a_translator_s_host_lets_what_its_row_names() {
        let both = Provider::DeepL.hosts();
        assert_eq!(to_let(&both, ronoctua_words::DEEPL_HOST), both);
        assert_eq!(
            to_let(&both, "elsewhere.example"),
            vec!["elsewhere.example".to_owned()]
        );
        let t = texts("en-GB");
        let mut v = on_the_internet_page(&t);
        Switch::Translator(Provider::DeepL).apply(&mut v.prefs);
        let errand = v.translation_turned(
            &Target::LetTranslator(ronoctua_words::DEEPL_FREE_HOST.to_owned()),
            Press::Enter,
            None,
            &t,
        );
        let Some(Errand::LetHosts {
            hosts, on: true, ..
        }) = errand
        else {
            panic!("not let: {errand:?}");
        };
        assert_eq!(hosts, both);
    }

    /// FAILS IF: answering the server's address leaves the cursor on another
    /// row — the address takes room in the value column and a label above
    /// may wrap into two rows, moving every row under it by one.
    #[test]
    fn the_cursor_stays_on_the_ring_after_the_address() {
        for (language, columns) in [("en-GB", 80), ("pt-BR", 80), ("en-GB", 60)] {
            let t = texts(language);
            let mut v = on_the_internet_page(&t);
            Switch::Translator(Provider::DeepL).apply(&mut v.prefs);
            let rows = v.rows(None, columns, &t);
            let at = rows
                .iter()
                .position(|r| r.target == Target::TranslatorWho && r.stops())
                .expect("the ring");
            v.clicked(at, &rows);
            assert_eq!(
                v.key(Press::Enter, None, columns, &t),
                Errand::Ask(Question::plain(ADDRESS, false))
            );
            v.answered(
                ADDRESS,
                "https://a-rather-long-translation-server.example/translate",
                &mut crate::player_machines::Pairing::default(),
                None,
                columns,
                &t,
            );
            let rows = v.rows(None, columns, &t);
            assert_eq!(
                rows[v.cursor()].target,
                Target::TranslatorWho,
                "{language} at {columns}"
            );
        }
    }

    /// FAILS IF: "Translated by" says a translator's raw id instead of its
    /// name, in either language.
    #[test]
    fn a_translator_is_named_not_coded() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for id in ["mymemory", "deepl", "libretranslate"] {
                let name = name_of(id, &t);
                assert_ne!(name, id, "{language}");
                assert!(!name.is_empty());
            }
            assert_eq!(name_of("someone-new", &t), "someone-new");
        }
    }
}
