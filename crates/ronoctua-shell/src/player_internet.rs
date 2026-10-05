//! The player's INTERNET page: everything of the player's that reaches a
//! stranger, each row naming whom it asks, and the person's keys.
//!
//! # Asked on their own
//!
//! The two switches that ask by themselves — lyrics from `lrclib.net` when a
//! song starts, sponsored parts skipped through SponsorBlock — each naming its
//! host in its label, and what is skipped, one row a category, set in under
//! the switch it belongs to. Both are born off (`ronoctua_player::prefs`).
//!
//! # Asked when a person presses
//!
//! Every other service asks only when a person presses a row that names it,
//! and that press is the consent (`player_find::consented`): here each one is
//! a switch — its name and its hosts in its label — that says whether it is
//! let, and turning one off takes the consent back. The directories of radio,
//! TV, films and lives are let the same way, each apart. What is written is
//! MERGED into `player.json` as it is on disk (`Errand::Let`): another door
//! may have let something meanwhile.
//!
//! # The person's keys
//!
//! OpenSubtitles' key and account, TMDB's token, SubDL's key: each a row that
//! says whether one is kept — never what it is. `RET` asks for it on a masked
//! line; what is typed is kept in a file of the person's, private from its
//! first byte (`ronoctua_lookup::keys`), and the player tries it at once —
//! the door says whether the service took it. The row names the host it
//! reaches, so typing a key there lets the player ask it. `d` forgets one,
//! after a question, "no" first.
//!
//! The page says why before its first row: a subpage carries its sentence,
//! the home carries none.

use crate::player::{Errand, Target, View};
use crate::player_rows::{Builder, on_off};
use ronoctua_lookup::Service;
use ronoctua_lookup::keys::Key;
use ronoctua_player::find::Directory;

/// The services a person lets by pressing a row, each row its own — the
/// Cover Art Archive with MusicBrainz, whose searches name both.
pub const CONSENTS: [&[Service]; 6] = [
    &[Service::Lrclib],
    &[Service::MusicBrainz, Service::CoverArtArchive],
    &[Service::Wikidata],
    &[Service::Tmdb],
    &[Service::OpenSubtitles],
    &[Service::SubDl],
];

/// FILES A DOOR WATCHES while a page shows what they hold — the person's
/// preferences, the keys kept: THE FILE IS THE BROADCAST, and another door
/// that writes one is heard here without a word between the two. Each file
/// is watched once its folder is there — a folder not there yet (a machine
/// on its first morning) is looked for again on the next look — and a change
/// wakes the door, whose next look says the files moved.
#[derive(Debug, Default)]
pub struct Watched {
    /// The files watched, by folder and name.
    watching: Vec<(std::path::PathBuf, String)>,
    /// Their watches.
    watches: Vec<ronoctua_files::notice::Notice>,
    /// A file moved — or this door wrote one — since the last look.
    stirred: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Looked at least once.
    looked: bool,
}

impl Watched {
    /// Read again on the next look: this door wrote one of the files.
    pub fn stir(&self) {
        self.stirred
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether the files are to be read, for a view with no loop to watch
    /// them with: only after [`Watched::stir`].
    pub fn take_stir(&self) -> bool {
        self.stirred
            .swap(false, std::sync::atomic::Ordering::SeqCst)
    }

    /// Watches each of `files` — a folder and a name — whose folder is there
    /// and is not watched yet, and says whether they are to be READ: the
    /// first look, and every one after a file moved or [`Watched::stir`].
    pub fn look(
        &mut self,
        files: &[(Option<std::path::PathBuf>, &str)],
        wake: &crate::player_subtitles::Wake,
    ) -> bool {
        use std::sync::atomic::Ordering;
        // A FILE WATCHED ONLY NOW may have moved while it was not: read.
        let mut new = false;
        for (dir, name) in files {
            let Some(dir) = dir.as_ref().filter(|d| d.is_dir()) else {
                continue;
            };
            if self.watching.iter().any(|(d, n)| d == dir && n == name) {
                continue;
            }
            let (stirred, wake) = (self.stirred.clone(), wake.clone());
            match ronoctua_files::notice::Notice::on_file(
                dir,
                name,
                ronoctua_files::notice::Pace::Coalesced,
                move || {
                    stirred.store(true, Ordering::SeqCst);
                    wake.call();
                    true
                },
            ) {
                Ok(n) => {
                    self.watches.push(n);
                    self.watching.push((dir.clone(), (*name).to_owned()));
                    new = true;
                }
                Err(e) => {
                    tracing::info!(error = %e, file = name, "a file of the player is not watched")
                }
            }
        }
        let first = !std::mem::replace(&mut self.looked, true);
        self.stirred.swap(false, Ordering::SeqCst) || first || new
    }
}

/// WHICH OF THE PERSON'S KEYS ARE KEPT — whether, never what. Read from their
/// files when the view opens, and again when one of them moves — kept or
/// forgotten in this door or in another ([`Watched`]); a photograph's and a
/// test's view reads none and is told ([`Kept::given`]).
#[derive(Debug, Default)]
pub struct Kept {
    /// The files are read — a view on a screen; `false` for one that shows
    /// what it is given.
    reads: bool,
    /// The keys kept.
    has: Vec<Key>,
    /// Read again at the next settle: this door kept or forgot one.
    stale: bool,
    /// The keys' files, watched while a page shows them.
    watched: Watched,
}

impl Kept {
    /// The keys as their files say — a view on a screen.
    #[must_use]
    pub fn read() -> Self {
        let mut k = Self {
            reads: true,
            has: Vec::new(),
            stale: true,
            watched: Watched::default(),
        };
        k.look(None);
        k
    }

    /// Keys as given — a photograph's, a test's: nothing is read.
    #[must_use]
    pub fn given(has: Vec<Key>) -> Self {
        Self {
            reads: false,
            has,
            stale: false,
            watched: Watched::default(),
        }
    }

    /// Whether `key` is kept.
    #[must_use]
    pub fn has(&self, key: Key) -> bool {
        self.has.contains(&key)
    }

    /// Read again when a key's file moved — in this door or another — or
    /// this door kept or forgot one since. With a `wake` (a door with a
    /// loop) the files are watched from now on.
    pub fn look(&mut self, wake: Option<&crate::player_subtitles::Wake>) {
        if !self.reads {
            self.stale = false;
            return;
        }
        if let Some(wake) = wake {
            let dir = ronoctua_lookup::keys::dir();
            let files: Vec<(Option<std::path::PathBuf>, &str)> =
                Key::ALL.iter().map(|k| (dir.clone(), k.file())).collect();
            if self.watched.look(&files, wake) {
                self.stale = true;
            }
        }
        if !std::mem::take(&mut self.stale) {
            return;
        }
        self.has = Key::ALL
            .into_iter()
            .filter(|k| ronoctua_lookup::keys::has(*k))
            .collect();
    }

    /// This door keeps (or forgets) `key`: shown so at once — the file is
    /// written off the loop, and its watch reads it back when it lands. One
    /// that could not be written is read again on the word of the thread
    /// that tried ([`Kept::stir`]).
    pub fn changed(&mut self, key: Key, kept: bool) {
        self.has.retain(|k| *k != key);
        if kept {
            self.has.push(key);
        }
    }

    /// Read the keys' files again at the next look — a key this door tried to
    /// keep or forget could not be.
    pub fn stir(&mut self) {
        self.stale = self.reads;
    }
}

/// A key's row: its words.
fn key_label(key: Key, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    texts.t(match key {
        Key::OpenSubtitles => key::PLAYER_KEY_OPENSUBTITLES,
        Key::OpenSubtitlesAccount => key::PLAYER_KEY_OPENSUBTITLES_ACCOUNT,
        Key::Tmdb => key::PLAYER_KEY_TMDB,
        Key::SubDl => key::PLAYER_KEY_SUBDL,
        // The security centre's; the page lists only its own (`Key::scope`).
        Key::VirusTotal => key::SECURITY_ONLINE_VT_KEY,
    })
}

/// What a key is, in a question about it — its words without the host.
fn key_words(key: Key, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    texts.t(match key {
        Key::OpenSubtitles => key::PLAYER_KEY_WORDS_OPENSUBTITLES,
        Key::OpenSubtitlesAccount => key::PLAYER_KEY_WORDS_OPENSUBTITLES_ACCOUNT,
        Key::Tmdb => key::PLAYER_KEY_WORDS_TMDB,
        Key::SubDl => key::PLAYER_KEY_WORDS_SUBDL,
        Key::VirusTotal => key::SECURITY_ONLINE_VT_KEY,
    })
}

/// A directory's name on its switch — for the radio and the TV, the name of
/// the page it lists on (`PageId::key`): one place, one key, and the switch
/// read the same words as the row that opens the page.
const fn directory_key(d: Directory) -> &'static str {
    use ronoctua_i18n::key;
    match d {
        Directory::Radio => crate::player::PageId::Radio.key(),
        Directory::Iptv => crate::player::PageId::Tv.key(),
        Directory::Archive => key::PLAYER_DIRECTORY_ARCHIVE,
        Directory::Lives => key::PLAYER_DIRECTORY_LIVES,
    }
}

/// Whether the person let every one of `services` by a press.
fn consented(prefs: &ronoctua_player::prefs::Preferences, services: &[Service]) -> bool {
    services.iter().all(|s| prefs.pressed(*s))
}

/// A let or not let, in words.
fn let_said(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        ronoctua_i18n::key::PLAYER_CONSENT_ALLOWED
    } else {
        ronoctua_i18n::key::PLAYER_CONSENT_NOT_ALLOWED
    })
}

impl View {
    /// The page's rows, under its title (`player_page::PageId::key`).
    pub(crate) fn internet(&self, b: &mut Builder<'_>) {
        use ronoctua_i18n::key;
        use ronoctua_player::prefs;
        b.note(key::PLAYER_ABOUT_INTERNET);
        b.heading(key::PLAYER_HEADING_ON_THEIR_OWN);
        // THE HOST IN THE LABEL, and the answer alone in the value: the row a
        // person taps to consent NAMES who is asked, and a label is never cut
        // on these pages — it wraps (`chrome::aligned_pair`) — while a value
        // that carried the host was cut to `desligado (lrc…` in a narrow door.
        b.pair(
            b.texts
                .t_with(key::PLAYER_LYRICS_ONLINE, &[("host", prefs::LRCLIB_HOST)]),
            on_off(self.prefs.lyrics_online, b.texts),
            Target::LyricsOnline,
            "lyrics-online",
        );
        b.pair(
            b.texts.t_with(
                key::PLAYER_SPONSORBLOCK,
                &[("host", prefs::SPONSORBLOCK_HOST)],
            ),
            on_off(self.prefs.sponsorblock, b.texts),
            Target::Sponsors,
            "sponsorblock",
        );
        // WHAT IS SKIPPED, one row a category, SET IN under the switch they
        // belong to — hierarchy is air and colour here, never a rule.
        b.nested(|b| {
            for category in prefs::CATEGORIES {
                b.pair(
                    b.t(skip_key(category)),
                    on_off(self.prefs.skipped.iter().any(|c| c == category), b.texts),
                    Target::Skip(category),
                    &format!("skip-{category}"),
                );
            }
        });
        // WHAT ASKS ONLY WHEN A PERSON PRESSES a row naming it: a switch
        // each, its name, and UNDER IT the hosts it asks — a line of the row
        // of its own, whole: a host cut mid-word in a narrow door named
        // nobody.
        // THE LYRICS' TRANSLATOR, among those that ask on their own: once
        // their translation is on (`T`), every song's lyrics go to it with
        // no press — its host let, or no longer, by the distro's consent
        // file, host by host (`ronoctua_consent`).
        // WHO TRANSLATES is the person's to choose — MyMemory, DeepL or a
        // LibreTranslate server — and the consent row names the chosen one's
        // hosts (`crate::player_translator`).
        self.translator_rows(b);
        b.heading(key::PLAYER_HEADING_WHEN_ASKED);
        let joiner = b.t(key::PLAYER_SET_JOINER);
        for services in CONSENTS {
            let names: Vec<String> = services
                .iter()
                .map(|s| b.t(crate::player::service_key(*s)))
                .collect();
            let hosts = b.texts.t_with(
                key::PLAYER_CONSENT_HOSTS,
                &[("hosts", &crate::player_find::hosts_of(services, b.texts))],
            );
            // WHAT THE PLAYER DOES, and why: LRCLIB is asked by the switch
            // above as well as by a press — the row says it is allowed, and
            // the line under it says by what; turning it off turns that off
            // too.
            let allowed = crate::player_find::allowed(&self.prefs, services);
            let by_the_switch = allowed && !consented(&self.prefs, services);
            let under = if by_the_switch {
                [hosts, b.t(ronoctua_i18n::key::PLAYER_CONSENT_BY_THE_SWITCH)]
                    .join(&b.t(ronoctua_i18n::key::PLAYER_META_JOINER))
            } else {
                hosts
            };
            b.pair_under(
                names.join(&joiner),
                let_said(allowed, b.texts),
                &under,
                Target::Consent(services),
                &format!("consent-{}", services[0].id()),
            );
        }
        for d in Directory::ALL {
            let hosts = b
                .texts
                .t_with(key::PLAYER_CONSENT_HOSTS, &[("hosts", d.host())]);
            b.pair_under(
                b.t(directory_key(d)),
                let_said(self.prefs.allows_directory(d), b.texts),
                &hosts,
                Target::LetDirectory(d),
                &format!("directory-{}", d.id()),
            );
        }
        // THE PERSON'S KEYS: whether each is kept, never what it is.
        b.heading(key::PLAYER_HEADING_KEYS);
        // THE PLAYER'S KEYS ONLY: the security centre's is on its own page.
        for k in Key::ALL
            .into_iter()
            .filter(|k| k.scope() == ronoctua_lookup::Scope::Player)
        {
            let sent = b.texts.t_with(
                key::PLAYER_KEY_SENT_TO,
                &[("host", ronoctua_player::prefs::host_of(k.service()))],
            );
            b.pair_under(
                key_label(k, b.texts),
                b.t(if self.kept.has(k) {
                    key::PLAYER_KEY_KEPT
                } else {
                    key::PLAYER_KEY_NONE
                }),
                &sent,
                Target::Key(k),
                &format!("key-{}", k.file()),
            );
            // SUBDL TAKES ITS KEY ONLY IN THE ADDRESS, and SubDL's own logs
            // keep addresses: said under its row, before it is typed.
            if k == Key::SubDl {
                b.note(key::PLAYER_KEY_SUBDL_ADDRESS);
            }
        }
    }

    /// What a press on a switch or a key of the Internet page does — `None`
    /// for a target that is none of its.
    pub(crate) fn internet_turned(
        &mut self,
        target: &Target,
        press: crate::player::Press,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        use crate::player::Press;
        match (target, press) {
            (Target::Consent(services), Press::Enter | Press::Less | Press::More) => {
                // WHAT THE ROW SHOWS is what turns: a service the switch above
                // asks is off only once that switch is off too.
                let on = !crate::player_find::allowed(&self.prefs, services);
                for s in *services {
                    if on {
                        self.prefs.consent(*s);
                    } else {
                        self.prefs.withdraw(*s);
                    }
                }
                self.prefs_written();
                let then = if !on && self.prefs.lyrics_online && services.contains(&Service::Lrclib)
                {
                    self.prefs.lyrics_online = false;
                    Errand::Keep(crate::player::Switch::LyricsOnline(false))
                } else {
                    Errand::Nothing
                };
                Some(Errand::Let {
                    services: services.to_vec(),
                    directories: Vec::new(),
                    on,
                    then: Box::new(then),
                })
            }
            (Target::LetDirectory(d), Press::Enter | Press::Less | Press::More) => {
                let on = !self.prefs.allows_directory(*d);
                if on {
                    self.prefs.let_directory(*d);
                } else {
                    self.prefs.withdraw_directory(*d);
                }
                self.prefs_written();
                Some(Errand::Let {
                    services: Vec::new(),
                    directories: vec![*d],
                    on,
                    then: Box::new(Errand::Nothing),
                })
            }
            // A KEY, asked on a masked line — the account's user name first,
            // which is no secret, then its password, which is.
            (Target::Key(k), Press::Enter) => {
                let id = crate::player_find::key_question(*k);
                let typing = match k {
                    Key::OpenSubtitlesAccount => crate::player::Typing::AccountUser,
                    other => crate::player::Typing::Key(*other),
                };
                self.typing = Some(typing);
                Some(Errand::Ask(crate::player_question::Question::plain(
                    id,
                    *k != Key::OpenSubtitlesAccount,
                )))
            }
            (Target::Key(k), Press::Remove) => {
                if !self.kept.has(*k) {
                    return Some(Errand::Said(texts.t_with(
                        ronoctua_i18n::key::PLAYER_KEY_NOTHING_TO_FORGET,
                        &[("what", &key_words(*k, texts))],
                    )));
                }
                let sentence = texts.t_with(
                    ronoctua_i18n::key::PLAYER_ASK_FORGET_KEY,
                    &[("what", &key_words(*k, texts))],
                );
                Some(self.ask_about(sentence, Errand::ForgetKey(*k)))
            }
            _ => None,
        }
    }

    /// A key typed on its masked line: kept, the service let — its row named
    /// the host — and tried by the player; nothing typed is never mind.
    pub(crate) fn key_typed(&mut self, key: Key, said: &str) -> Errand {
        let said = said.trim();
        if said.is_empty() {
            return Errand::Nothing;
        }
        self.kept.changed(key, true);
        crate::player_find::consented(
            &mut self.prefs,
            &[key.service()],
            Errand::KeepKey {
                key,
                secret: ronoctua_lookup::Secret::new(said),
            },
        )
    }

    /// The account's user name typed: its password is asked next, masked.
    pub(crate) fn account_user_typed(&mut self, said: &str) -> Errand {
        let user = said.trim();
        if user.is_empty() {
            return Errand::Nothing;
        }
        self.typing = Some(crate::player::Typing::AccountPassword(user.to_owned()));
        Errand::Ask(crate::player_question::Question::plain(
            crate::player_question::ACCOUNT_PASSWORD,
            true,
        ))
    }

    /// The account's password typed: the account kept, OpenSubtitles let,
    /// and the account tried.
    pub(crate) fn account_password_typed(&mut self, user: String, said: &str) -> Errand {
        if said.is_empty() {
            return Errand::Nothing;
        }
        self.kept.changed(Key::OpenSubtitlesAccount, true);
        crate::player_find::consented(
            &mut self.prefs,
            &[Service::OpenSubtitles],
            Errand::KeepAccount {
                user,
                password: ronoctua_lookup::Secret::new(said),
            },
        )
    }
}

/// A key that could not be kept or read, in a sentence.
#[must_use]
pub fn key_trouble(t: ronoctua_lookup::keys::KeyTrouble, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    use ronoctua_lookup::keys::KeyTrouble;
    texts.t(match t {
        KeyTrouble::NoHome => key::PLAYER_KEY_TROUBLE_NO_HOME,
        KeyTrouble::Empty => key::PLAYER_KEY_TROUBLE_EMPTY,
        KeyTrouble::Malformed => key::PLAYER_KEY_TROUBLE_MALFORMED,
        KeyTrouble::TooLong => key::PLAYER_KEY_TROUBLE_TOO_LONG,
        KeyTrouble::Unwritable => key::PLAYER_KEY_TROUBLE_UNWRITABLE,
        KeyTrouble::Unreadable => key::PLAYER_KEY_TROUBLE_UNREADABLE,
    })
}

/// Each category's name on screen.
fn skip_key(category: &str) -> &'static str {
    use ronoctua_i18n::key;
    match category {
        "sponsor" => key::PLAYER_SKIP_SPONSOR,
        "selfpromo" => key::PLAYER_SKIP_SELFPROMO,
        "interaction" => key::PLAYER_SKIP_INTERACTION,
        "intro" => key::PLAYER_SKIP_INTRO,
        "outro" => key::PLAYER_SKIP_OUTRO,
        "preview" => key::PLAYER_SKIP_PREVIEW,
        "hook" => key::PLAYER_SKIP_HOOK,
        "music_offtopic" => key::PLAYER_SKIP_MUSIC_OFFTOPIC,
        _ => key::PLAYER_SKIP_FILLER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{PageId, Press};
    use ronoctua_player::wire::State;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// The view on its Internet page, walked to as a person does: the row on
    /// the home, clicked twice.
    fn on_the_internet_page(s: &State) -> View {
        let t = texts();
        let mut v = View::blank(None);
        v.settle_in(Some(s), 80, &t);
        let rows = v.rows(Some(s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Go(PageId::Internet))
            .expect("the home has a row for the internet");
        v.clicked(at, &rows);
        assert_eq!(v.key(Press::Enter, Some(s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Internet);
        v
    }

    /// FAILS IF: a category the daemon can skip has no row of its own —
    /// the table's last arm answers for anything, so a forgotten one would be
    /// drawn under another's name — or a row stops flipping its category.
    #[test]
    fn every_category_is_a_row_of_its_own_and_turns() {
        use ronoctua_player::prefs::CATEGORIES;
        let names: std::collections::HashSet<&str> =
            CATEGORIES.iter().map(|c| super::skip_key(c)).collect();
        assert_eq!(names.len(), CATEGORIES.len(), "two categories share a name");
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        for c in CATEGORIES {
            assert!(
                rows.iter().any(|r| r.target == Target::Skip(c)),
                "{c} has no row"
            );
        }
        let at = rows
            .iter()
            .position(|r| r.target == Target::Skip("intro"))
            .expect("intro is a row");
        v.clicked(at, &rows);
        let Errand::Keep(switch) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("a category writes the preferences");
        };
        // ONE SWITCH, merged over the file — never the view's whole copy.
        assert_eq!(
            switch,
            crate::player::Switch::Skip {
                category: "intro".into(),
                on: true
            }
        );
        let mut on_disk = ronoctua_player::prefs::Preferences {
            consented: vec![Service::MusicBrainz],
            ..ronoctua_player::prefs::Preferences::default()
        };
        switch.apply(&mut on_disk);
        assert!(
            on_disk.skipped.iter().any(|c| c == "intro"),
            "intro was not added"
        );
        assert!(
            on_disk.skipped.iter().any(|c| c == "sponsor"),
            "the others moved"
        );
        assert_eq!(
            on_disk.consented,
            [Service::MusicBrainz],
            "a consent another door wrote is kept"
        );
        assert!(
            v.prefs.skipped.iter().any(|c| c == "intro"),
            "shown at once"
        );
    }

    #[test]
    fn a_network_switch_turns_and_says_so() {
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Sponsors)
            .expect("the switch is a row");
        v.clicked(at, &rows);
        let Errand::Keep(switch) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("a switch writes the preferences");
        };
        assert_eq!(switch, crate::player::Switch::Sponsors(true));
        assert!(v.prefs.sponsorblock && !v.prefs.lyrics_online);
        assert!(
            rows[at]
                .fields
                .iter()
                .any(|f| f.text.contains(ronoctua_player::prefs::SPONSORBLOCK_HOST)),
            "the row names the host it reaches"
        );
    }

    /// FAILS IF: LRCLIB's row says "not allowed" while the switch above asks
    /// it on its own — or turning the row off leaves the switch asking it.
    #[test]
    fn lrclib_s_row_says_what_asks_it_and_turns_it_all_off() {
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        v.prefs.lyrics_online = true;
        let rows = v.rows(Some(&s), 80, &t);
        let lrclib = Target::Consent(CONSENTS[0]);
        let at = rows
            .iter()
            .position(|r| r.target == lrclib)
            .expect("LRCLIB's row");
        let said: String = rows[at..]
            .iter()
            .take_while(|r| r.target == lrclib)
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            said.contains(&t.t(ronoctua_i18n::key::PLAYER_CONSENT_ALLOWED))
                && said.contains(&t.t(ronoctua_i18n::key::PLAYER_CONSENT_BY_THE_SWITCH)),
            "{said}"
        );
        v.clicked(at, &rows);
        let Errand::Let { on, then, .. } = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("the press let nothing");
        };
        assert!(!on);
        assert_eq!(
            *then,
            Errand::Keep(crate::player::Switch::LyricsOnline(false)),
            "the switch still asks LRCLIB"
        );
        assert!(!crate::player_find::allowed(&v.prefs, CONSENTS[0]));
    }

    /// FAILS IF: `d` on a key that is not kept does nothing and says nothing.
    #[test]
    fn forgetting_a_key_not_kept_says_so() {
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Key(Key::Tmdb))
            .expect("TMDB's row");
        v.clicked(at, &rows);
        assert!(matches!(
            v.key(Press::Remove, Some(&s), 80, &t),
            Errand::Said(said) if said.contains(&t.t(ronoctua_i18n::key::PLAYER_KEY_WORDS_TMDB))
        ));
    }

    /// FAILS IF: the switches stop being there with no player running — they
    /// are the machine's, read by a player when it starts.
    #[test]
    fn the_switches_are_there_with_no_player() {
        let t = texts();
        let mut v = View::blank(None);
        v.settle_in(None, 80, &t);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Go(PageId::Internet))
            .expect("the home has a row for the internet with no player");
        v.clicked(at, &rows);
        v.key(Press::Enter, None, 80, &t);
        let rows = v.rows(None, 80, &t);
        assert!(rows.iter().any(|r| r.target == Target::LyricsOnline));
    }

    /// FAILS IF: a service asked only when a person presses — or a
    /// directory — stops being a switch that names its hosts, or turning one
    /// stops writing just that one, merged, or the view's copy stops showing
    /// it let.
    #[test]
    fn every_service_and_directory_is_a_switch_that_names_its_hosts() {
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        // A LABEL WRAPS onto the lines under its row: its words are all of
        // them.
        let label = |target: &Target| -> String {
            rows.iter()
                .filter(|r| &r.target == target)
                .flat_map(|r| r.fields.first().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ")
        };
        for services in CONSENTS {
            let words = label(&Target::Consent(services));
            assert!(!words.is_empty(), "{services:?} is no switch");
            for host in services.iter().flat_map(|s| s.hosts()) {
                assert!(
                    words.contains(host.name()),
                    "{words} does not name {}",
                    host.name()
                );
            }
        }
        for d in Directory::ALL {
            let words = label(&Target::LetDirectory(d));
            assert!(words.contains(d.host()), "{d:?}: {words}");
        }
        let at = rows
            .iter()
            .position(|r| r.target == Target::Consent(CONSENTS[1]))
            .expect("musicbrainz");
        v.clicked(at, &rows);
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Let {
                services: vec![Service::MusicBrainz, Service::CoverArtArchive],
                directories: Vec::new(),
                on: true,
                then: Box::new(Errand::Nothing),
            }
        );
        assert!(v.prefs.allows(Service::CoverArtArchive));
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[at].fields.last().map(|f| f.text.clone()),
            Some(t.t(ronoctua_i18n::key::PLAYER_CONSENT_ALLOWED))
        );
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Let { on: false, .. }
        ));
        let at = rows
            .iter()
            .position(|r| r.target == Target::LetDirectory(Directory::Radio))
            .expect("the radio");
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Let {
                services: Vec::new(),
                directories: vec![Directory::Radio],
                on: true,
                then: Box::new(Errand::Nothing),
            }
        );
    }

    /// FAILS IF: a key is asked on a line that is not masked, or kept
    /// without letting the service its row names, or the row says what it
    /// is instead of whether one is kept, or the account's password is asked
    /// unmasked, or a key is forgotten without a question born on its "no".
    #[test]
    fn a_key_is_asked_masked_kept_and_forgotten_after_a_question() {
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        let mut pairing = crate::player_machines::Pairing::default();
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Key(Key::Tmdb))
            .expect("the token's row");
        // THE HOST IT IS SENT TO, on a line of its row.
        assert!(
            rows.iter()
                .filter(|r| r.target == Target::Key(Key::Tmdb))
                .any(|r| r
                    .fields
                    .iter()
                    .any(|f| f.text.contains("api.themoviedb.org"))),
            "the token's row names no host"
        );
        v.grabbed(at);
        let Errand::Ask(q) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("the token asked nothing");
        };
        assert!(q.secret, "a key asked unmasked");
        assert_eq!(q.initial, None);
        match v.answered(q.id, " eyJh.token ", &mut pairing, Some(&s), 80, &t) {
            Errand::Let {
                services,
                on: true,
                then,
                ..
            } => {
                assert_eq!(services, [Service::Tmdb]);
                assert_eq!(
                    *then,
                    Errand::KeepKey {
                        key: Key::Tmdb,
                        secret: ronoctua_lookup::Secret::new("eyJh.token"),
                    }
                );
                assert!(!format!("{then:?}").contains("eyJh"), "a key printed");
            }
            other => panic!("a key typed asked {other:?}"),
        }
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[at].fields.last().map(|f| f.text.clone()),
            Some(t.t(ronoctua_i18n::key::PLAYER_KEY_KEPT))
        );
        assert!(
            !rows
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .any(|text| text.contains("eyJh")),
            "a key is said on the page"
        );
        // THE ACCOUNT: its user, then its password — masked.
        let account = rows
            .iter()
            .position(|r| r.target == Target::Key(Key::OpenSubtitlesAccount))
            .expect("the account's row");
        v.grabbed(account);
        let Errand::Ask(user) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("the account asked nothing");
        };
        assert!(!user.secret);
        let Errand::Ask(password) = v.answered(user.id, "marta", &mut pairing, Some(&s), 80, &t)
        else {
            panic!("no password asked");
        };
        assert!(password.secret, "a password asked unmasked");
        assert!(matches!(
            v.answered(password.id, "hunter2", &mut pairing, Some(&s), 80, &t),
            Errand::Let { then, .. } if matches!(*then, Errand::KeepAccount { ref user, .. } if user == "marta")
        ));
        // FORGOTTEN after a question, "no" first.
        v.grabbed(at);
        assert_eq!(v.key(Press::Remove, Some(&s), 80, &t), Errand::Nothing);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false));
        v.key(Press::Down, Some(&s), 80, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::ForgetKey(Key::Tmdb)
        );
    }

    /// FAILS IF: the lyrics' translator stops having its switch on the
    /// Internet page — naming its host, saying whether it is let — or leaves
    /// the group that asks on its own (with `T` on, every song's lyrics go to
    /// it unpressed); or a press stops letting the host, and a second one
    /// taking it back, merged into the consent file (`Errand::LetHosts`).
    #[test]
    fn the_translator_is_a_switch_of_the_internet_page() {
        let t = texts();
        let s = State::default();
        let mut v = on_the_internet_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Translator)
            .expect("the translator's switch");
        let title = |k: &str| {
            rows.iter()
                .position(|r| r.heading && r.fields.first().is_some_and(|f| f.text == t.t(k)))
                .expect(k)
        };
        let (own, pressed) = (
            title(ronoctua_i18n::key::PLAYER_HEADING_ON_THEIR_OWN),
            title(ronoctua_i18n::key::PLAYER_HEADING_WHEN_ASKED),
        );
        assert!(own < at && at < pressed, "{own} < {at} < {pressed}");
        assert_eq!(
            rows[at].fields.last().map(|f| f.text.clone()),
            Some(t.t(ronoctua_i18n::key::PLAYER_CONSENT_NOT_ALLOWED))
        );
        let host_line: String = rows[at..]
            .iter()
            .take(3)
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            host_line.contains(ronoctua_words::MYMEMORY_HOST),
            "the host is not named: {host_line}"
        );
        v.grabbed(at);
        for on in [true, false] {
            match v.key(Press::Enter, Some(&s), 80, &t) {
                Errand::LetHosts { hosts, on: now, .. } => {
                    assert_eq!(hosts, [ronoctua_words::MYMEMORY_HOST.to_owned()]);
                    assert_eq!(now, on);
                }
                other => panic!("{other:?}"),
            }
        }
    }
}
