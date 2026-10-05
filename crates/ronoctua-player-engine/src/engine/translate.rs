//! The lyrics, translated: the whole song in one question, line by line, to
//! the person's translator — asked on a thread of its own, under the pass the
//! person gave its host, and kept in the player's cache so a song is asked
//! once per language.
//!
//! # When
//!
//! While the person has it on ([`Preferences::lyrics_translation`]), every
//! time the lyrics shown are new — a song with its own, a `.lrc` beside it,
//! LRCLIB's, a candidate taken — and never on a clock: the lyrics are looked
//! at when the state is said ([`Engine::mind_translation`], from
//! `broadcast`), and asked only when they are not the ones asked last.
//!
//! # Line by line, or apart
//!
//! The song goes as ONE text, its lines joined, so a verse is translated with
//! the verses around it; the answer is split back at its line breaks. When it
//! comes back with as many lines as went, each line carries its own
//! ([`LyricLine::translated`]); when it does not, a line under a verse would
//! be another verse's, so none does and the whole of it is said apart
//! ([`Translation::unsynced`]).
//!
//! # Whose
//!
//! The person's choice ([`LyricsTranslation::by`], a door's ring on the
//! Internet page, written into `player.json` and read again on `reload`):
//! MyMemory (`ronoctua_words`, keyless, the default), DeepL with the person's
//! key, or a LibreTranslate server they named — each asked only with the pass
//! `ronoctua-consent` mints for its host ([`host_for`]): not let, the refusal
//! names the host ([`Trouble::Off`]) and a door draws the row that lets it.
//!
//! [`Preferences::lyrics_translation`]: ronoctua_player::prefs::Preferences::lyrics_translation
//! [`LyricLine::translated`]: ronoctua_player::wire::LyricLine::translated

use std::hash::{Hash as _, Hasher as _};
use std::time::{Duration, SystemTime};

use ronoctua_broadcast::cache::Cache;
use ronoctua_player::{
    prefs::{self, LyricsTranslation},
    wire::{LyricLine, Translation, TranslationStatus, Trouble},
};
use ronoctua_words::{Provider, Tongue};

/// The host `provider` is asked at: MyMemory's; DeepL's free or paid one, as
/// the person's key says (a key kept that ends in `:fx` is a free one; with
/// none kept, the free one, and the words crate says no key is kept); a
/// LibreTranslate server's own. `None` for an address that names no host.
#[must_use]
pub(super) fn host_for(provider: &Provider) -> Option<String> {
    let deepl_key = || {
        ronoctua_words::keys::read(ronoctua_words::keys::Key::DeepL)
            .ok()
            .flatten()
    };
    host_with(provider, deepl_key)
}

/// [`host_for`], with the person's DeepL key as `deepl_key` reads it. Pure
/// but for that.
fn host_with(
    provider: &Provider,
    deepl_key: impl FnOnce() -> Option<ronoctua_consent::keys::Secret>,
) -> Option<String> {
    match provider {
        Provider::DeepL => Some(
            deepl_key()
                .map_or(ronoctua_words::DEEPL_FREE_HOST, |key| {
                    ronoctua_words::deepl::host_for(&key)
                })
                .to_owned(),
        ),
        other => other.hosts().into_iter().next(),
    }
}

use super::{Engine, Refused, Shared};

mod policy {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// One mebibyte.
const MIB: u64 = 1024 * 1024;

/// How long a translation kept in the cache is good for: a song's words do
/// not change, and the cache's ceiling is what lets old ones go.
const KEPT_FOR: Duration = Duration::from_secs(365 * 24 * 3600);

/// The language a door reads, when the person chose none for the lyrics:
/// the one they chose for the doors (`language.json`, the shell's file, in
/// `config`), else the machine's (`LC_ALL`, `LC_MESSAGES`, `LANG`), else
/// English. The folder is the caller's to find.
fn doors_language(config: Option<std::path::PathBuf>) -> String {
    let chosen = config
        .and_then(|d| std::fs::read_to_string(d.join("language.json")).ok())
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("language")?.as_str().map(str::to_owned));
    let machine = || {
        ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|var| {
            let value = std::env::var(var).ok()?;
            let tag = value
                .split('.')
                .next()
                .unwrap_or_default()
                .replace('_', "-");
            Tongue::new(&tag).map(|t| t.code().to_owned())
        })
    };
    chosen
        .or_else(machine)
        .unwrap_or_else(|| "en-GB".to_owned())
}

/// What the lyrics are, as one number: a new song, new words or new lines
/// is a new number. `None` with no lyrics. Pure.
#[must_use]
pub(super) fn lyrics_key(lines: &[LyricLine]) -> Option<u64> {
    if lines.iter().all(|l| l.text.trim().is_empty()) {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for line in lines {
        line.text.hash(&mut h);
        line.at.map(f64::to_bits).hash(&mut h);
    }
    Some(h.finish())
}

/// The song as one text, a line for every line.
fn text_of(lines: &[LyricLine]) -> String {
    lines
        .iter()
        .map(|l| l.text.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The translation laid over the lines: each its own when as many came back
/// as went — a blank verse left blank — else none, and the whole as one
/// block (`Some` of it). Pure.
#[must_use]
pub(super) fn laid_over(lines: &mut [LyricLine], translated: &str) -> Option<String> {
    let back: Vec<&str> = translated.split('\n').collect();
    if back.len() != lines.len() {
        for line in lines.iter_mut() {
            line.translated = None;
        }
        return Some(translated.trim().to_owned());
    }
    for (line, said) in lines.iter_mut().zip(back) {
        let said = said.trim();
        line.translated =
            (!line.text.trim().is_empty() && !said.is_empty()).then(|| said.to_owned());
    }
    None
}

/// The question under way, or answered: which lyrics, and into what the
/// person's setting said — the setting itself (`None`: the doors' language),
/// so a look at the lyrics as the state is said never reads a file.
#[derive(Debug, Default)]
pub(super) struct Translating {
    asked: Option<(u64, Option<String>)>,
    /// Who it was asked of: another translator chosen since is a new
    /// question, and what the one before says is let go.
    by: Option<Provider>,
    /// What came back for [`Translating::asked`], kept: the same lyrics set
    /// again — a candidate taken whose words are the ones shown — arrive as
    /// fresh lines without it, and get it laid over again ([`relaid`]).
    answer: Option<String>,
}

/// The translation `answer` laid over `lines` AGAIN when they lost it: the
/// same lyrics set again come as fresh lines, none translated, while the
/// status still says they are. `true` when it laid it; nothing is done when
/// a line still carries its own, or the answer was said apart (`unsynced`,
/// which the lines never carried). Pure.
#[must_use]
pub(super) fn relaid(lines: &mut [LyricLine], answer: Option<&str>, unsynced: bool) -> bool {
    let Some(answer) = answer else { return false };
    if unsynced || lines.iter().any(|l| l.translated.is_some()) {
        return false;
    }
    laid_over(lines, answer).is_none() && lines.iter().any(|l| l.translated.is_some())
}

impl Engine {
    /// `lyrics-translation`: on — into `to`, else the doors' language — or
    /// off; kept in `player.json` (merged with what is on disk), and asked
    /// again now, even for the same lyrics: a person who just let the host
    /// asks by this.
    pub(super) fn translate_lyrics(&mut self, on: bool, to: Option<String>) -> Result<(), Refused> {
        let to = to.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        if let Some(code) = &to
            && Tongue::new(code).is_none()
        {
            return Err(Trouble::Translation(ronoctua_words::Refusal::new(
                ronoctua_words::Why::UnknownTongue,
            ))
            .into());
        }
        // ON AND INTO WHAT, merged: who translates is the file's as it is.
        self.keep_translation(|t| {
            t.on = on;
            t.to = to;
        })
    }

    /// The lyrics' translation setting changed by `change`, MERGED into
    /// `player.json` as it is on disk and kept here; the lyrics asked again.
    fn keep_translation(
        &mut self,
        change: impl FnOnce(&mut LyricsTranslation),
    ) -> Result<(), Refused> {
        let store = prefs::store();
        let mut on_disk = store.parsed().ok_or_else(|| Trouble::Unwritable {
            path: prefs::FILE.to_owned(),
            why: "it will not parse".to_owned(),
        })?;
        change(&mut on_disk.lyrics_translation);
        store.write(&on_disk).map_err(|e| Trouble::Unwritable {
            path: prefs::FILE.to_owned(),
            why: e.to_string(),
        })?;
        self.prefs.lyrics_translation = on_disk.lyrics_translation;
        self.translating.asked = None;
        self.translating.answer = None;
        self.dirty = true;
        Ok(())
    }

    /// The preferences read again — a key kept, a host let, a server named:
    /// a translation that was REFUSED is asked again at the next look
    /// ([`Self::mind_translation`]), where the same lyrics would otherwise
    /// keep the refusal until the song changes.
    pub(super) fn retry_a_refused_translation(&mut self) {
        if refused(self.state.lyrics_translation.as_ref()) {
            self.translating.asked = None;
            self.translating.answer = None;
            self.dirty = true;
        }
    }

    /// The lyrics shown, looked at as the state is said: new lyrics are
    /// asked for — once — while the translation is on; off, or with no
    /// lyrics, nothing of one is said.
    pub(super) fn mind_translation(&mut self) {
        let key = lyrics_key(&self.state.lyrics).filter(|_| self.prefs.lyrics_translation.on);
        let Some(key) = key else {
            self.translating.asked = None;
            self.translating.answer = None;
            self.state.lyrics_translation = None;
            for line in &mut self.state.lyrics {
                line.translated = None;
            }
            return;
        };
        let setting = self.prefs.lyrics_translation.to.clone();
        let by = self.prefs.lyrics_translation.by.clone();
        if self.translating.asked.as_ref() == Some(&(key, setting.clone()))
            && self.translating.by.as_ref() == Some(&by)
        {
            // THE SAME LYRICS SET AGAIN, as fresh lines: what came back for
            // them is laid over them again.
            let unsynced = self
                .state
                .lyrics_translation
                .as_ref()
                .is_some_and(|t| t.unsynced.is_some());
            if relaid(
                &mut self.state.lyrics,
                self.translating.answer.as_deref(),
                unsynced,
            ) {
                self.dirty = true;
            }
            return;
        }
        let Ok(shared) = self.shared() else { return };
        // THE DOORS' LANGUAGE read once, here, when a question is asked.
        let to = setting
            .clone()
            .unwrap_or_else(|| doors_language(ronoctua_store::config_dir()));
        self.translating.asked = Some((key, setting.clone()));
        self.translating.by = Some(by.clone());
        self.translating.answer = None;
        for line in &mut self.state.lyrics {
            line.translated = None;
        }
        self.state.lyrics_translation = Some(Translation {
            to: to.clone(),
            from: None,
            status: TranslationStatus::Translating,
            unsynced: None,
        });
        spawn(key, setting, text_of(&self.state.lyrics), (to, by), shared);
    }

    /// What came back for the lyrics `key` in `to` — laid over the lines
    /// when they are still the ones asked about, else let go.
    fn translated(
        &mut self,
        (key, setting): (u64, Option<String>),
        (to, by): (&str, &Provider),
        got: Result<ronoctua_words::Translation, Trouble>,
    ) {
        if self.translating.asked.as_ref() != Some(&(key, setting))
            || self.translating.by.as_ref() != Some(by)
            || lyrics_key(&self.state.lyrics) != Some(key)
        {
            return;
        }
        self.state.lyrics_translation = Some(match got {
            Ok(t) => {
                let unsynced = laid_over(&mut self.state.lyrics, &t.text);
                self.translating.answer = Some(t.text.clone());
                Translation {
                    to: to.to_owned(),
                    from: t.from.map(|f| f.code().to_owned()),
                    status: TranslationStatus::Translated {
                        by: t.by.id().to_owned(),
                    },
                    unsynced,
                }
            }
            Err(trouble) => Translation {
                to: to.to_owned(),
                from: None,
                status: TranslationStatus::Refused(trouble),
                unsynced: None,
            },
        });
        self.dirty = true;
    }
}

/// The name a translation is kept under in the player's cache: by its text,
/// its language and who translated it — another translator's words are
/// another translation.
fn cache_name(text: &str, to: &str, by: &Provider) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let hex: String = digest.as_ref()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("translation-{hex}-{to}-{}", by.id())
}

/// Asks on a thread of its own, and hands the answer to the engine.
fn spawn(
    key: u64,
    setting: Option<String>,
    text: String,
    (to, by): (String, Provider),
    shared: Shared,
) {
    let _ = std::thread::Builder::new()
        .name("player-translate".into())
        .spawn(move || {
            let got = ask(&text, &to, &by);
            if let Err(t) = &got {
                tracing::info!(trouble = ?t, "the lyrics were not translated");
            }
            let (lock, bell) = &*shared;
            if let Ok(mut e) = lock.lock() {
                e.translated((key, setting), (&to, &by), got);
                if e.dirty {
                    e.broadcast();
                }
            }
            bell.notify_all();
        });
}

/// Whether the translation said is a refusal — what a reread of the
/// preferences asks again.
fn refused(said: Option<&Translation>) -> bool {
    matches!(said.map(|t| &t.status), Some(TranslationStatus::Refused(_)))
}

/// The translation: kept, or asked of the host under the person's pass and
/// kept. Blocks: call it on a thread.
fn ask(text: &str, to: &str, by: &Provider) -> Result<ronoctua_words::Translation, Trouble> {
    let tongue = Tongue::new(to).ok_or(Trouble::Translation(ronoctua_words::Refusal::new(
        ronoctua_words::Why::UnknownTongue,
    )))?;
    let cache = Cache::player(u64::from(policy::PLAYER_MIB) * MIB).ok();
    let name = cache_name(text, tongue.code(), by);
    if let Some(kept) = cache
        .as_ref()
        .and_then(|c| c.read(&name, KEPT_FOR, SystemTime::now()).ok().flatten())
        .and_then(|k| serde_json::from_slice(&k.bytes).ok())
    {
        return Ok(kept);
    }
    let host = host_for(by).ok_or_else(|| Trouble::NotAnAddress {
        address: match by {
            Provider::LibreTranslate { url } => url.clone(),
            other => other.id().to_owned(),
        },
    })?;
    let pass = ronoctua_consent::pass(&host).map_err(|n| Trouble::Off { host: n.host })?;
    let translation =
        ronoctua_words::translate(text, None, &tongue, by, &pass).map_err(Trouble::Translation)?;
    if let (Some(cache), Ok(bytes)) = (cache, serde_json::to_vec(&translation))
        && let Err(why) = cache.write(&name, &bytes, None)
    {
        tracing::info!(?why, "a translation was not kept");
    }
    Ok(translation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(at: Option<f64>, text: &str) -> LyricLine {
        LyricLine {
            at,
            text: text.to_owned(),
            translated: None,
        }
    }

    /// FAILS IF: a translation that came back a line for every line is not
    /// laid under its verses, a blank verse gets words, or one that came
    /// back with another count is laid under the wrong verses instead of
    /// being said apart.
    #[test]
    fn a_translation_is_laid_line_by_line_or_said_apart() {
        let mut lines = vec![
            line(Some(1.0), "Hello"),
            line(Some(2.0), ""),
            line(Some(3.0), "World"),
        ];
        assert_eq!(text_of(&lines), "Hello\n\nWorld");
        assert_eq!(laid_over(&mut lines, "Hallo\n\nWelt"), None);
        let under: Vec<Option<&str>> = lines.iter().map(|l| l.translated.as_deref()).collect();
        assert_eq!(under, vec![Some("Hallo"), None, Some("Welt")]);
        // ANOTHER COUNT: apart, and nothing under a verse.
        assert_eq!(
            laid_over(&mut lines, "Hallo Welt\n"),
            Some("Hallo Welt".to_owned())
        );
        assert!(lines.iter().all(|l| l.translated.is_none()));
    }

    /// FAILS IF: the same lyrics set again lose their translation while the
    /// status says they are translated, a translation said apart is laid
    /// under the verses, or lines that carry theirs are laid over again.
    #[test]
    fn the_same_lyrics_set_again_get_their_translation_back() {
        let answer = Some("Hallo\nWelt");
        let mut fresh = vec![line(Some(1.0), "Hello"), line(Some(2.0), "World")];
        assert!(relaid(&mut fresh, answer, false));
        let under: Vec<Option<&str>> = fresh.iter().map(|l| l.translated.as_deref()).collect();
        assert_eq!(under, vec![Some("Hallo"), Some("Welt")]);
        // STILL CARRIED: nothing to do.
        assert!(!relaid(&mut fresh, answer, false));
        // SAID APART, or nothing came back: the lines stay without.
        let mut fresh = vec![line(Some(1.0), "Hello"), line(Some(2.0), "World")];
        assert!(!relaid(&mut fresh, answer, true));
        assert!(!relaid(&mut fresh, None, false));
        assert!(fresh.iter().all(|l| l.translated.is_none()));
    }

    /// FAILS IF: new lyrics are not new to the translation — another song,
    /// another line, another time — or no lyrics are asked about.
    #[test]
    fn new_lyrics_are_a_new_question() {
        let a = vec![line(Some(1.0), "Hello"), line(Some(2.0), "World")];
        let mut b = a.clone();
        b[1].text = "Word".to_owned();
        let mut c = a.clone();
        c[0].at = Some(1.5);
        assert!(lyrics_key(&a).is_some());
        assert_eq!(lyrics_key(&a), lyrics_key(&a.clone()));
        assert_ne!(lyrics_key(&a), lyrics_key(&b));
        assert_ne!(lyrics_key(&a), lyrics_key(&c));
        assert_eq!(lyrics_key(&[]), None);
        assert_eq!(lyrics_key(&[line(None, "  ")]), None);
        // THE TRANSLATION ITSELF is not what the lyrics are.
        let mut d = a.clone();
        d[0].translated = Some("Hallo".to_owned());
        assert_eq!(lyrics_key(&a), lyrics_key(&d));
    }

    /// FAILS IF: two songs, or one song into two languages, share a name in
    /// the cache, or a name is one the cache refuses.
    #[test]
    fn a_translation_is_kept_by_its_text_and_language() {
        let my = Provider::MyMemory;
        let a = cache_name("Hello", "pt-BR", &my);
        assert!(ronoctua_broadcast::cache::is_name(&a), "{a}");
        assert_ne!(a, cache_name("Hello", "es", &my));
        assert_ne!(a, cache_name("Hullo", "pt-BR", &my));
        assert_eq!(a, cache_name("Hello", "pt-BR", &my));
        // ANOTHER TRANSLATOR'S WORDS are another translation.
        let libre = Provider::LibreTranslate {
            url: "https://translate.example".into(),
        };
        assert_ne!(a, cache_name("Hello", "pt-BR", &Provider::DeepL));
        assert!(ronoctua_broadcast::cache::is_name(&cache_name(
            "Hello", "pt-BR", &libre
        )));
    }

    /// FAILS IF: a reread of the preferences does not ask again a
    /// translation that was refused (a key typed, a host let after it), or
    /// asks again one that came back or is on its way.
    #[test]
    fn a_refused_translation_is_asked_again_and_no_other() {
        let said = |status| Translation {
            to: "pt-BR".to_owned(),
            from: None,
            status,
            unsynced: None,
        };
        assert!(refused(Some(&said(TranslationStatus::Refused(
            Trouble::Off {
                host: ronoctua_words::DEEPL_HOST.to_owned()
            }
        )))));
        assert!(!refused(Some(&said(TranslationStatus::Translating))));
        assert!(!refused(Some(&said(TranslationStatus::Translated {
            by: "deepl".to_owned()
        }))));
        assert!(!refused(None));
    }

    /// FAILS IF: the translator chosen is not the host asked — DeepL's host
    /// not the one its key goes to, a LibreTranslate server not its own.
    #[test]
    fn each_translator_is_asked_at_its_own_host() {
        use ronoctua_consent::keys::Secret;
        let none = || None;
        assert_eq!(
            host_with(&Provider::MyMemory, none).as_deref(),
            Some(ronoctua_words::MYMEMORY_HOST)
        );
        assert_eq!(
            host_with(
                &Provider::LibreTranslate {
                    url: "https://translate.example/x".into()
                },
                none
            )
            .as_deref(),
            Some("translate.example")
        );
        assert_eq!(
            host_with(
                &Provider::LibreTranslate {
                    url: "http://translate.example".into()
                },
                none
            ),
            None,
            "plain http names no host to ask"
        );
        assert_eq!(
            host_with(&Provider::DeepL, || Some(Secret::new("abc:fx"))).as_deref(),
            Some(ronoctua_words::DEEPL_FREE_HOST)
        );
        assert_eq!(
            host_with(&Provider::DeepL, || Some(Secret::new("abc"))).as_deref(),
            Some(ronoctua_words::DEEPL_HOST)
        );
        assert_eq!(
            host_with(&Provider::DeepL, none).as_deref(),
            Some(ronoctua_words::DEEPL_FREE_HOST)
        );
    }
}
