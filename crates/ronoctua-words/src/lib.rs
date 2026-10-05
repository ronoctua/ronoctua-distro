//! Translation, definitions and synonyms — the one crate that talks to them.
//!
//! # The one boundary
//!
//! MyMemory (`api.mymemory.translated.net`, keyless, the default), DeepL
//! (the person's key, in the `DeepL-Auth-Key` header), a LibreTranslate the
//! person names (its URL; a key in the JSON body), the offline English
//! WordNet (`open-english-wordnet`, converted at build time into a sorted TSV
//! read by path from `RONOCTUA_WORDS`), and Wiktionary's REST definitions
//! (`en.wiktionary.org`). Nothing else in the workspace names one of these
//! hosts. It serves the player's lyrics translation (the daemon asks, the
//! door shows) and the fast-access `/d`, `/s` and `/t` cards.
//!
//! # Consent, keys, manners
//!
//! Every request carries a [`ronoctua_consent::Pass`] for the host it goes
//! to: without one the function cannot be called. Keys are
//! [`ronoctua_consent::keys::Secret`]s kept `0600` in `words-keys` ([`keys`]),
//! never in argv and never in a query string. One pace per host, and a
//! translation cache bounded by `wordsCacheEntries` (`assets/cache.nix`).
//!
//! # Typed answers, typed refusals
//!
//! Each call answers its value or a [`Refusal`] — a [`Why`] from a closed
//! set, with a sentence each in the door (`words-why-<id>`). Nothing here is
//! a sentence.
//!
//! # Pure, over a [`Net`]
//!
//! Each provider module builds an [`ask::Request`] and reads an answer; the
//! [`Net`] between them is [`internet::Internet`] on the machine (feature
//! `net`) and a list of answers in a test, which records every request and
//! the pass it went under. So every parser here is tested against the
//! provider's own JSON, and every request against its address, headers and
//! body, without a socket. [`Reach`] is the three of them put together — a
//! net, a cache, where the keys and the offline dictionary are — and the free
//! functions ([`translate`], [`define`], [`synonyms`]) are the machine's.
//!
//! # Every function here blocks
//!
//! On the pace, the network or the disk. Call it on a THREAD, never on a
//! drawing one — and a door that may be done with the answer first (the
//! search, on the next key) hands the net an abort
//! (`internet::Internet::abortable`).

#![forbid(unsafe_code)]

use std::path::PathBuf;

use ronoctua_consent::Pass;
use ronoctua_consent::keys::{Dir, Secret};
use serde::{Deserialize, Serialize};

pub mod ask;
pub mod cache;
pub mod deepl;
pub mod html;
#[cfg(feature = "net")]
pub mod internet;
pub mod keys;
pub mod libretranslate;
pub mod mymemory;
pub mod pace;
pub mod wiktionary;
pub mod wordnet;

pub use ask::Net;
pub use cache::Cache;

/// Who is asking, in every request. Wikimedia asks a client to name itself
/// and a way to reach who runs it; the address is the public repository
/// (invariant 2 holds: nothing personal).
pub const USER_AGENT: &str = concat!(
    "ronoctua-words/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/ronoctua/ronoctua-distro )"
);

/// MyMemory's host — the keyless default.
pub const MYMEMORY_HOST: &str = "api.mymemory.translated.net";
/// DeepL's host for a free key (one that ends in `:fx`).
pub const DEEPL_FREE_HOST: &str = "api-free.deepl.com";
/// DeepL's host for a paid key.
pub const DEEPL_HOST: &str = "api.deepl.com";
/// Wiktionary's REST host for definitions (every language's entries of a word
/// are on the English edition's endpoint).
pub const WIKTIONARY_HOST: &str = "en.wiktionary.org";
/// Where the offline English dictionary is: a directory the build writes,
/// set by the unit (full profile only). Read at run time, never baked in.
pub const OFFLINE_VARIABLE: &str = "RONOCTUA_WORDS";

/// A language, by its code: `en`, `pt`, `pt-BR`.
///
/// Normalised as BCP 47 writes it — the primary subtag lowercase, a region
/// uppercase — so `pt-br` and `pt_BR` are one tongue. The names a person
/// reads come from `assets/tongues.nix` through the door, never from here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Tongue(String);

impl Tongue {
    /// The tongue of a code, or `None` for text that is not one (a primary
    /// subtag of two or three letters, optionally a region of two letters or
    /// three digits).
    #[must_use]
    pub fn new(code: &str) -> Option<Self> {
        let code = code.trim().replace('_', "-");
        let mut parts = code.split('-');
        let primary = parts.next()?;
        let region = parts.next();
        if parts.next().is_some()
            || !(2..=3).contains(&primary.len())
            || !primary.chars().all(|c| c.is_ascii_alphabetic())
        {
            return None;
        }
        let mut out = primary.to_ascii_lowercase();
        if let Some(region) = region {
            let letters = region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic());
            let digits = region.len() == 3 && region.chars().all(|c| c.is_ascii_digit());
            if !(letters || digits) {
                return None;
            }
            out.push('-');
            out.push_str(&region.to_ascii_uppercase());
        }
        Some(Self(out))
    }

    /// The code, normalised.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.0
    }

    /// The primary subtag: `pt` of `pt-BR`.
    #[must_use]
    pub fn primary(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }

    /// Whether this is English, the one tongue with an offline dictionary.
    #[must_use]
    pub fn is_english(&self) -> bool {
        self.primary() == "en"
    }
}

impl TryFrom<String> for Tongue {
    type Error = String;

    fn try_from(code: String) -> Result<Self, Self::Error> {
        Self::new(&code).ok_or(code)
    }
}

impl From<Tongue> for String {
    fn from(tongue: Tongue) -> Self {
        tongue.0
    }
}

/// Who translates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Provider {
    /// MyMemory: keyless, the default. An optional e-mail raises its daily
    /// quota; none is sent.
    #[serde(rename = "mymemory")]
    MyMemory,
    /// DeepL, with the person's key (`DeepL-Auth-Key` header). A key ending
    /// in `:fx` goes to [`DEEPL_FREE_HOST`].
    #[serde(rename = "deepl")]
    DeepL,
    /// A LibreTranslate server the person names; its key, when it wants one,
    /// goes in the JSON body.
    #[serde(rename = "libretranslate")]
    LibreTranslate {
        /// The server's base address, `https://…`.
        url: String,
    },
}

impl Provider {
    /// Its id: the tail of its text key (`words-provider-<id>`).
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::MyMemory => "mymemory",
            Self::DeepL => "deepl",
            Self::LibreTranslate { .. } => "libretranslate",
        }
    }

    /// Every host its first request may go to — what a consent row names.
    /// DeepL has two (free and paid key); a LibreTranslate address that names
    /// no host has none, and cannot be asked.
    #[must_use]
    pub fn hosts(&self) -> Vec<String> {
        match self {
            Self::MyMemory => vec![MYMEMORY_HOST.to_owned()],
            Self::DeepL => vec![DEEPL_FREE_HOST.to_owned(), DEEPL_HOST.to_owned()],
            Self::LibreTranslate { url } => host_of(url).into_iter().collect(),
        }
    }

    /// The key it needs, when it needs one.
    #[must_use]
    pub const fn key(&self) -> Option<keys::Key> {
        match self {
            Self::MyMemory => None,
            Self::DeepL => Some(keys::Key::DeepL),
            Self::LibreTranslate { .. } => Some(keys::Key::LibreTranslate),
        }
    }
}

/// The host of an `https://` address: what comes after the scheme and before
/// the first `/`, `:` or `?`. `None` for anything else.
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("https://")?;
    let end = rest.find(['/', ':', '?', '#']).unwrap_or(rest.len());
    ronoctua_consent::normal(&rest[..end])
}

/// A translated text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Translation {
    /// The translation. Line breaks of the source are kept, one for one, when
    /// the provider keeps them; a caller that needs lines aligned (the lyrics)
    /// compares the counts.
    pub text: String,
    /// The source tongue: the one asked for, or the one the provider detected
    /// (`None` when it said nothing).
    pub from: Option<Tongue>,
    /// The tongue it is in.
    pub to: Tongue,
    /// Who translated it — what "translated by" names.
    pub by: Provider,
}

/// A part of speech, as a closed set (the words are the door's:
/// `words-part-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Part {
    /// A noun.
    Noun,
    /// A proper noun.
    ProperNoun,
    /// A verb.
    Verb,
    /// An adjective (WordNet's satellites too).
    Adjective,
    /// An adverb.
    Adverb,
    /// A pronoun.
    Pronoun,
    /// A preposition.
    Preposition,
    /// A conjunction.
    Conjunction,
    /// An interjection.
    Interjection,
    /// A numeral.
    Numeral,
    /// A determiner or an article.
    Determiner,
    /// A phrase or an idiom.
    Phrase,
    /// A prefix or a suffix.
    Affix,
    /// Something else the source named.
    Other,
}

impl Part {
    /// Every part, for a test that walks the words.
    pub const ALL: [Self; 14] = [
        Self::Noun,
        Self::ProperNoun,
        Self::Verb,
        Self::Adjective,
        Self::Adverb,
        Self::Pronoun,
        Self::Preposition,
        Self::Conjunction,
        Self::Interjection,
        Self::Numeral,
        Self::Determiner,
        Self::Phrase,
        Self::Affix,
        Self::Other,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Noun => "noun",
            Self::ProperNoun => "proper-noun",
            Self::Verb => "verb",
            Self::Adjective => "adjective",
            Self::Adverb => "adverb",
            Self::Pronoun => "pronoun",
            Self::Preposition => "preposition",
            Self::Conjunction => "conjunction",
            Self::Interjection => "interjection",
            Self::Numeral => "numeral",
            Self::Determiner => "determiner",
            Self::Phrase => "phrase",
            Self::Affix => "affix",
            Self::Other => "other",
        }
    }
}

/// Where a sense came from — what the card credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The offline English WordNet.
    #[serde(rename = "wordnet")]
    WordNet,
    /// Wiktionary, online.
    Wiktionary,
}

/// One meaning of a word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sense {
    /// Its part of speech, when the source says.
    pub part: Option<Part>,
    /// The definition — data, drawn as it came (like a log line).
    pub gloss: String,
    /// Example sentences, when the source has them.
    pub examples: Vec<String>,
    /// Words of the same sense (WordNet's synset members).
    pub synonyms: Vec<String>,
    /// Where it came from.
    pub source: Source,
}

/// Why a call answered nothing, as a closed set (`words-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// There was nothing to translate or look up.
    Empty,
    /// The text is longer than the provider takes in one request.
    TooLong,
    /// The provider does not know this tongue (or the pair).
    UnknownTongue,
    /// The pass is for another host than the one this would ask.
    WrongPass,
    /// The provider needs a key and the person kept none.
    NoKey,
    /// The provider refused the key.
    KeyRefused,
    /// The provider's quota for today is spent.
    Quota,
    /// Asked too often; the pace would wait longer than a person does.
    Paced,
    /// The word is not in the dictionary.
    NotFound,
    /// No offline dictionary for this tongue, and no pass to ask online.
    NoDictionary,
    /// The host could not be reached.
    Unreachable,
    /// The host answered with a failure.
    Failed,
    /// The host answered something that is not what was asked for.
    Garbled,
}

impl Why {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 13] = [
        Self::Empty,
        Self::TooLong,
        Self::UnknownTongue,
        Self::WrongPass,
        Self::NoKey,
        Self::KeyRefused,
        Self::Quota,
        Self::Paced,
        Self::NotFound,
        Self::NoDictionary,
        Self::Unreachable,
        Self::Failed,
        Self::Garbled,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::TooLong => "too-long",
            Self::UnknownTongue => "unknown-tongue",
            Self::WrongPass => "wrong-pass",
            Self::NoKey => "no-key",
            Self::KeyRefused => "key-refused",
            Self::Quota => "quota",
            Self::Paced => "paced",
            Self::NotFound => "not-found",
            Self::NoDictionary => "no-dictionary",
            Self::Unreachable => "unreachable",
            Self::Failed => "failed",
            Self::Garbled => "garbled",
        }
    }
}

/// A refusal: why, and the host it concerns (when one was asked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
    /// The host that refused or would have been asked.
    pub host: Option<String>,
}

impl Refusal {
    /// A refusal that concerns no host.
    #[must_use]
    pub const fn new(why: Why) -> Self {
        Self { why, host: None }
    }
}

/// The directory of the offline English dictionary, read from
/// [`OFFLINE_VARIABLE`] now — `None` when it is unset or holds no dictionary
/// this reader knows.
#[must_use]
pub fn offline_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os(OFFLINE_VARIABLE)?);
    wordnet::is_there(&dir).then_some(dir)
}

/// Everything a question needs besides its words: the net it goes through,
/// the cache it is kept in, and where the keys and the offline dictionary
/// are. The free functions build the machine's; a door that aborts hands its
/// own net, and a test hands answers, a cache and folders of its own.
pub struct Reach<'a> {
    net: &'a dyn Net,
    cache: &'a Cache,
    keys: Option<Dir>,
    offline: Option<PathBuf>,
}

impl<'a> Reach<'a> {
    /// The keys in `words-keys` and the offline dictionary of
    /// [`OFFLINE_VARIABLE`], asked through `net` and kept in `cache`.
    #[must_use]
    pub fn new(net: &'a dyn Net, cache: &'a Cache) -> Self {
        Self {
            net,
            cache,
            keys: keys::dir(),
            offline: offline_dir(),
        }
    }

    /// The same, with the keys in `keys` (`None`: no key is kept).
    #[must_use]
    pub fn keys_in(mut self, keys: Option<Dir>) -> Self {
        self.keys = keys;
        self
    }

    /// The same, with the offline dictionary in `dir` (`None`: there is none).
    #[must_use]
    pub fn offline_in(mut self, dir: Option<PathBuf>) -> Self {
        self.offline = dir;
        self
    }

    /// Whether `tongue` has an offline dictionary here — English, when the
    /// data is there. A door says "offline" or asks for consent by it.
    #[must_use]
    pub fn has_offline(&self, tongue: &Tongue) -> bool {
        tongue.is_english() && self.offline.is_some()
    }

    fn key(&self, key: keys::Key) -> Result<Option<Secret>, Why> {
        let Some(dir) = &self.keys else {
            return Ok(None);
        };
        dir.read(key.file()).map_err(|_| Why::KeyRefused)
    }

    /// The host `provider`'s request goes to — what the door mints a
    /// [`Pass`] for. DeepL's depends on the key (a free one's host is not a
    /// paid one's), so it needs one.
    ///
    /// # Errors
    ///
    /// [`Why::NoKey`] for DeepL without a key, [`Why::KeyRefused`] for a key
    /// file that cannot be read, [`Why::Failed`] for a LibreTranslate address
    /// that is not `https` with a host.
    pub fn host_for(&self, provider: &Provider) -> Result<String, Refusal> {
        self.host_and_key(provider).map(|(host, _)| host)
    }

    fn host_and_key(&self, provider: &Provider) -> Result<(String, Option<Secret>), Refusal> {
        match provider {
            Provider::MyMemory => Ok((MYMEMORY_HOST.to_owned(), None)),
            Provider::DeepL => {
                let key = self
                    .key(keys::Key::DeepL)
                    .map_err(Refusal::new)?
                    .ok_or_else(|| Refusal::new(Why::NoKey))?;
                Ok((deepl::host_for(&key).to_owned(), Some(key)))
            }
            Provider::LibreTranslate { url } => {
                let (host, _) =
                    libretranslate::endpoint(url).ok_or_else(|| Refusal::new(Why::Failed))?;
                let key = self.key(keys::Key::LibreTranslate).map_err(|why| Refusal {
                    why,
                    host: Some(host.clone()),
                })?;
                Ok((host, key))
            }
        }
    }

    /// `text` translated into `to` by `provider`, asking the host `pass`
    /// lets; see [`translate`].
    ///
    /// # Errors
    ///
    /// As [`translate`].
    pub fn translate(
        &self,
        text: &str,
        from: Option<&Tongue>,
        to: &Tongue,
        provider: &Provider,
        pass: &Pass,
    ) -> Result<Translation, Refusal> {
        if text.trim().is_empty() {
            return Err(Refusal::new(Why::Empty));
        }
        let (host, key) = self.host_and_key(provider)?;
        let refused = |why| Refusal {
            why,
            host: Some(host.clone()),
        };
        // THE PASS IS LOOKED AT FIRST, before the cache: a pass for another
        // host is a caller's mistake, and it is said the same way whether the
        // answer happened to be kept or not.
        if !pass.covers(&host) {
            return Err(refused(Why::WrongPass));
        }
        let most = match provider {
            Provider::MyMemory => mymemory::MOST_TEXT_BYTES,
            Provider::DeepL => deepl::MOST_TEXT_BYTES,
            Provider::LibreTranslate { .. } => libretranslate::MOST_TEXT_BYTES,
        };
        if text.len() > most {
            return Err(refused(Why::TooLong));
        }
        if let Some(kept) = self.cache.get(provider, from, to, text) {
            return Ok(kept);
        }
        let (said, detected) = match provider {
            Provider::MyMemory => self.by_mymemory(text, from, to, pass),
            Provider::DeepL => {
                // `host_and_key` answered a key for DeepL or refused.
                let key = key.ok_or(Why::NoKey).map_err(refused)?;
                let reply = self
                    .net
                    .send(pass, &deepl::request(text, from, to, &key))
                    .map_err(refused)?;
                match ask::why_of_status(reply.status) {
                    Some(why) => Err(why),
                    None => deepl::read(&reply.body),
                }
            }
            Provider::LibreTranslate { url } => {
                let request = libretranslate::request(url, text, from, to, key.as_ref())
                    .ok_or(Why::Failed)
                    .map_err(refused)?;
                let reply = self.net.send(pass, &request).map_err(refused)?;
                libretranslate::read(reply.status, &reply.body, key.is_some())
            }
        }
        .map_err(refused)?;
        let translation = Translation {
            text: said,
            from: from.cloned().or(detected),
            to: to.clone(),
            by: provider.clone(),
        };
        self.cache.put(provider, from, to, text, &translation);
        Ok(translation)
    }

    /// MyMemory, piece by piece ([`mymemory::pieces`]), joined back with the
    /// breaks the source had.
    fn by_mymemory(
        &self,
        text: &str,
        from: Option<&Tongue>,
        to: &Tongue,
        pass: &Pass,
    ) -> Result<(String, Option<Tongue>), Why> {
        let mut out = String::with_capacity(text.len());
        let mut detected = None;
        for piece in mymemory::pieces(text, mymemory::PIECE_BYTES) {
            if piece.text.trim().is_empty() {
                // A blank line between verses: nothing to ask about.
                out.push_str(&piece.text);
            } else {
                let reply = self
                    .net
                    .send(pass, &mymemory::request(&piece.text, from, to))?;
                if let Some(why) = ask::why_of_status(reply.status) {
                    // MyMemory's own refusals are inside a 200; an HTTP 429
                    // from it is the day's quota, not a pace.
                    return Err(if why == Why::Paced { Why::Quota } else { why });
                }
                let (said, from) = mymemory::read(&reply.body)?;
                out.push_str(&said);
                detected = detected.or(from);
            }
            out.push_str(piece.then);
        }
        Ok((out, detected))
    }

    /// The senses of `word` in `tongue`; see [`define`].
    ///
    /// # Errors
    ///
    /// As [`define`].
    pub fn define(
        &self,
        word: &str,
        tongue: &Tongue,
        online: Option<&Pass>,
    ) -> Result<Vec<Sense>, Refusal> {
        let word = word.trim();
        if word.is_empty() {
            return Err(Refusal::new(Why::Empty));
        }
        if let Some(offline) = self.offline_first(word, tongue, online, wordnet::senses)? {
            return Ok(offline);
        }
        let pass = self.online(online)?;
        let refused = |why| Refusal {
            why,
            host: Some(WIKTIONARY_HOST.to_owned()),
        };
        let reply = self
            .net
            .send(pass, &wiktionary::definition_request(word))
            .map_err(refused)?;
        if let Some(why) = ask::why_of_status(reply.status) {
            return Err(refused(why));
        }
        wiktionary::read_definitions(&reply.body, tongue).map_err(refused)
    }

    /// The synonyms of `word` in `tongue`; see [`synonyms`].
    ///
    /// # Errors
    ///
    /// As [`synonyms`].
    pub fn synonyms(
        &self,
        word: &str,
        tongue: &Tongue,
        online: Option<&Pass>,
    ) -> Result<Vec<String>, Refusal> {
        let word = word.trim();
        if word.is_empty() {
            return Err(Refusal::new(Why::Empty));
        }
        if let Some(offline) = self.offline_first(word, tongue, online, wordnet::synonyms)? {
            return Ok(offline);
        }
        let pass = self.online(online)?;
        let refused = |why| Refusal {
            why,
            host: Some(WIKTIONARY_HOST.to_owned()),
        };
        let reply = self
            .net
            .send(pass, &wiktionary::wikitext_request(word))
            .map_err(refused)?;
        if let Some(why) = ask::why_of_status(reply.status) {
            return Err(refused(why));
        }
        let wikitext = wiktionary::read_wikitext(&reply.body).map_err(refused)?;
        let found = wiktionary::synonyms_in(&wikitext, word, tongue);
        if found.is_empty() {
            return Err(refused(Why::NotFound));
        }
        Ok(found)
    }

    /// The offline answer, when the tongue has one: `Some` for a word it
    /// holds; `None` to go online (no data, or a word it lacks and a pass to
    /// ask with).
    fn offline_first<T>(
        &self,
        word: &str,
        tongue: &Tongue,
        online: Option<&Pass>,
        read: fn(&std::path::Path, &str) -> Result<T, wordnet::Missing>,
    ) -> Result<Option<T>, Refusal> {
        let Some(dir) = self.offline.as_deref().filter(|_| tongue.is_english()) else {
            return Ok(None);
        };
        match read(dir, word) {
            Ok(found) => Ok(Some(found)),
            Err(wordnet::Missing::NoWord) if online.is_none() => Err(Refusal::new(Why::NotFound)),
            Err(_) => Ok(None),
        }
    }

    /// The pass to ask Wiktionary with, or why there is none.
    fn online<'p>(&self, online: Option<&'p Pass>) -> Result<&'p Pass, Refusal> {
        let pass = online.ok_or_else(|| Refusal::new(Why::NoDictionary))?;
        if pass.covers(WIKTIONARY_HOST) {
            Ok(pass)
        } else {
            Err(Refusal {
                why: Why::WrongPass,
                host: Some(WIKTIONARY_HOST.to_owned()),
            })
        }
    }
}

/// The machine's [`Reach`]: the internet (or, built without `net`, a network
/// that is not there), the process's cache, the person's keys and the
/// offline dictionary the environment names.
fn machine<T>(with: impl FnOnce(&Reach<'_>) -> T) -> T {
    #[cfg(feature = "net")]
    let net = internet::Internet::new();
    #[cfg(not(feature = "net"))]
    let net = ask::Unwired;
    with(&Reach::new(&net, cache::shared()))
}

/// The host `provider`'s request goes to, with the machine's keys — what a
/// door mints the [`Pass`] for. See [`Reach::host_for`].
///
/// # Errors
///
/// As [`Reach::host_for`].
pub fn host_for(provider: &Provider) -> Result<String, Refusal> {
    machine(|reach| reach.host_for(provider))
}

/// `text` translated into `to` by `provider`, asking the host `pass` lets.
///
/// `from` is the source tongue, or `None` to let the provider detect it.
/// Line breaks are kept (MyMemory's pieces are cut at them); the lyrics ask
/// one batch per song. An answer already given is not asked for again
/// ([`cache`]).
///
/// # Errors
///
/// A [`Refusal`] naming the host: [`Why::Empty`], [`Why::TooLong`],
/// [`Why::WrongPass`] (the pass is for another host; nothing is sent),
/// [`Why::NoKey`], [`Why::KeyRefused`], [`Why::Quota`],
/// [`Why::UnknownTongue`], [`Why::Paced`], [`Why::Unreachable`],
/// [`Why::Failed`], [`Why::Garbled`].
pub fn translate(
    text: &str,
    from: Option<&Tongue>,
    to: &Tongue,
    provider: &Provider,
    pass: &Pass,
) -> Result<Translation, Refusal> {
    machine(|reach| reach.translate(text, from, to, provider, pass))
}

/// The senses of `word` in `tongue`: the offline WordNet first for English,
/// Wiktionary otherwise — online only with a pass for [`WIKTIONARY_HOST`]
/// (`online`; `None` asks nothing).
///
/// # Errors
///
/// A [`Refusal`]: [`Why::Empty`], [`Why::NotFound`], [`Why::NoDictionary`]
/// (no offline data for the tongue and no pass), [`Why::WrongPass`], and the
/// network's.
pub fn define(word: &str, tongue: &Tongue, online: Option<&Pass>) -> Result<Vec<Sense>, Refusal> {
    machine(|reach| reach.define(word, tongue, online))
}

/// The synonyms of `word` in `tongue`, the same way as [`define`]: WordNet's
/// synsets offline, the page's `{{syn}}` and `Synonyms` online.
///
/// # Errors
///
/// As [`define`].
pub fn synonyms(
    word: &str,
    tongue: &Tongue,
    online: Option<&Pass>,
) -> Result<Vec<String>, Refusal> {
    machine(|reach| reach.synonyms(word, tongue, online))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::canned::{Canned, pass_for};

    fn tongue(code: &str) -> Tongue {
        Tongue::new(code).unwrap()
    }

    fn keys_with(tag: &str, kept: &[(keys::Key, &str)]) -> Dir {
        let dir = Dir::at(
            std::env::temp_dir().join(format!("ronoctua-words-keys-{}-{tag}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(dir.path());
        for (key, secret) in kept {
            dir.keep(key.file(), &Secret::new(*secret)).unwrap();
        }
        dir
    }

    #[test]
    fn tongues_are_normalised_and_every_refusal_has_its_own_key() {
        assert_eq!(Tongue::new("pt_br").unwrap().code(), "pt-BR");
        assert_eq!(Tongue::new("EN").unwrap().primary(), "en");
        assert!(Tongue::new("pt-BR-x").is_none());
        assert!(Tongue::new("portuguese").is_none());
        assert_eq!(
            host_of("https://libre.example.org:5000/translate").as_deref(),
            Some("libre.example.org")
        );
        assert_eq!(host_of("http://libre.example.org"), None, "https only");
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len(), "every refusal has its own key");
        let parts: std::collections::BTreeSet<_> = Part::ALL.iter().map(|p| p.id()).collect();
        assert_eq!(parts.len(), Part::ALL.len());
    }

    /// FAILS IF: anything reaches the network before the person let the
    /// host — a definition asked with no pass, or a translation under a pass
    /// for another host.
    #[test]
    fn nothing_reaches_the_network_until_somebody_says_so() {
        let net = Canned::new(&[]);
        let cache = Cache::new(4);
        let reach = Reach::new(&net, &cache).keys_in(None).offline_in(None);
        for code in ["pt-BR", "en"] {
            assert_eq!(
                reach.define("casa", &tongue(code), None).unwrap_err().why,
                Why::NoDictionary
            );
            assert_eq!(
                reach.synonyms("casa", &tongue(code), None).unwrap_err().why,
                Why::NoDictionary
            );
        }
        let wiktionary = pass_for(WIKTIONARY_HOST);
        let refused = reach
            .translate(
                "hello",
                None,
                &tongue("pt-BR"),
                &Provider::MyMemory,
                &wiktionary,
            )
            .unwrap_err();
        assert_eq!(
            (refused.why, refused.host.as_deref()),
            (Why::WrongPass, Some(MYMEMORY_HOST))
        );
        let mymemory = pass_for(MYMEMORY_HOST);
        assert_eq!(
            reach
                .define("casa", &tongue("pt"), Some(&mymemory))
                .unwrap_err()
                .why,
            Why::WrongPass
        );
        assert!(
            net.asked().is_empty(),
            "nothing was sent: {:?}",
            net.asked()
        );
    }

    #[test]
    fn a_song_goes_to_mymemory_in_pieces_and_comes_back_whole_and_kept() {
        let pt = tongue("pt-BR");
        let first = "a".repeat(300);
        let second = "b".repeat(300);
        let song = format!("{first}\n{second}");
        let ask = |piece: &str| mymemory::request(piece, None, &pt).url;
        let answer = |t: &str| {
            format!(
                r#"{{"responseData":{{"translatedText":"{t}","detectedLanguage":"en"}},"responseStatus":200}}"#
            )
        };
        let (one, two) = (answer("um"), answer("dois"));
        let urls = [ask(&first), ask(&second)];
        let net = Canned::new(&[(&urls[0], 200, &one), (&urls[1], 200, &two)]);
        let cache = Cache::new(4);
        let reach = Reach::new(&net, &cache).keys_in(None).offline_in(None);
        let pass = pass_for(MYMEMORY_HOST);
        let t = reach
            .translate(&song, None, &pt, &Provider::MyMemory, &pass)
            .unwrap();
        assert_eq!(t.text, "um\ndois", "the break is kept between the pieces");
        assert_eq!(t.from.as_ref().unwrap().code(), "en");
        assert_eq!(t.by, Provider::MyMemory);
        assert_eq!(net.asked().len(), 2);
        assert!(net.asked().iter().all(|(host, _)| host == MYMEMORY_HOST));
        // Asked again: the cache answers, and nothing is sent.
        let again = reach
            .translate(&song, None, &pt, &Provider::MyMemory, &pass)
            .unwrap();
        assert_eq!(again, t);
        assert_eq!(
            net.asked().len(),
            2,
            "the second answer came from the cache"
        );
        let long = "x ".repeat(mymemory::MOST_TEXT_BYTES);
        assert_eq!(
            reach
                .translate(&long, None, &pt, &Provider::MyMemory, &pass)
                .unwrap_err()
                .why,
            Why::TooLong
        );
    }

    #[test]
    fn deepl_needs_a_key_and_its_pass_is_for_the_keys_host() {
        let pt = tongue("pt-BR");
        let cache = Cache::new(4);
        let url = format!("https://{DEEPL_FREE_HOST}/v2/translate");
        let net = Canned::new(&[(
            &url,
            200,
            r#"{"translations":[{"text":"olá","detected_source_language":"EN"}]}"#,
        )]);
        let none = Reach::new(&net, &cache)
            .keys_in(Some(keys_with("deepl-none", &[])))
            .offline_in(None);
        let paid = pass_for(DEEPL_HOST);
        assert_eq!(
            none.translate("hello", None, &pt, &Provider::DeepL, &paid)
                .unwrap_err()
                .why,
            Why::NoKey
        );
        let dir = keys_with("deepl", &[(keys::Key::DeepL, "k-1:fx")]);
        let reach = Reach::new(&net, &cache)
            .keys_in(Some(dir.clone()))
            .offline_in(None);
        assert_eq!(reach.host_for(&Provider::DeepL).unwrap(), DEEPL_FREE_HOST);
        assert_eq!(
            reach
                .translate("hello", None, &pt, &Provider::DeepL, &paid)
                .unwrap_err()
                .why,
            Why::WrongPass,
            "a free key's host is not the paid one's"
        );
        let free = pass_for(DEEPL_FREE_HOST);
        let t = reach
            .translate("hello", None, &pt, &Provider::DeepL, &free)
            .unwrap();
        assert_eq!((t.text.as_str(), t.from.unwrap().code()), ("olá", "en"));
        let (_, sent) = &net.asked()[0];
        assert_eq!(sent.headers[0].1.expose(), "DeepL-Auth-Key k-1:fx");
        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[test]
    fn a_status_that_is_a_refusal_is_said_with_its_host() {
        let pt = tongue("pt-BR");
        let cache = Cache::new(4);
        let url = format!("https://{DEEPL_HOST}/v2/translate");
        let net = Canned::new(&[(&url, 456, "")]);
        let dir = keys_with("quota", &[(keys::Key::DeepL, "paid")]);
        let reach = Reach::new(&net, &cache)
            .keys_in(Some(dir.clone()))
            .offline_in(None);
        let refused = reach
            .translate("hello", None, &pt, &Provider::DeepL, &pass_for(DEEPL_HOST))
            .unwrap_err();
        assert_eq!(
            refused,
            Refusal {
                why: Why::Quota,
                host: Some(DEEPL_HOST.into())
            }
        );
        assert!(cache.is_empty(), "a refusal is never kept");
        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[test]
    fn a_libretranslate_server_is_asked_at_its_address_with_or_without_a_key() {
        let pt = tongue("pt-BR");
        let cache = Cache::new(4);
        let provider = Provider::LibreTranslate {
            url: "https://lt.example.org/".into(),
        };
        let net = Canned::new(&[(
            "https://lt.example.org/translate",
            200,
            r#"{"translatedText":"olá"}"#,
        )]);
        let dir = keys_with("libre", &[(keys::Key::LibreTranslate, "lt-key")]);
        let reach = Reach::new(&net, &cache)
            .keys_in(Some(dir.clone()))
            .offline_in(None);
        let t = reach
            .translate(
                "hello",
                Some(&tongue("en")),
                &pt,
                &provider,
                &pass_for("lt.example.org"),
            )
            .unwrap();
        assert_eq!(t.text, "olá");
        assert_eq!(
            t.from.unwrap().code(),
            "en",
            "the tongue asked, when one was"
        );
        let (_, sent) = &net.asked()[0];
        assert!(
            sent.body
                .as_ref()
                .unwrap()
                .expose()
                .contains("\"api_key\":\"lt-key\"")
        );
        let bad = Provider::LibreTranslate {
            url: "http://lt.example.org".into(),
        };
        assert_eq!(reach.host_for(&bad).unwrap_err().why, Why::Failed);
        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[test]
    fn english_is_answered_offline_and_asks_online_only_for_what_it_lacks() {
        let dir = wordnet::fixture::dictionary("reach", 0);
        let body = r#"{"en":[{"partOfSpeech":"Noun","definitions":[{"definition":"a <a>made-up</a> word"}]}]}"#;
        let url = wiktionary::definition_request("frobnicate").url;
        let net = Canned::new(&[(&url, 200, body)]);
        let cache = Cache::new(4);
        let reach = Reach::new(&net, &cache)
            .keys_in(None)
            .offline_in(Some(dir.clone()));
        let en = tongue("en-GB");
        assert!(reach.has_offline(&en) && !reach.has_offline(&tongue("pt")));
        let house = reach.define("house", &en, None).unwrap();
        assert_eq!(house[0].source, Source::WordNet);
        assert_eq!(
            reach.synonyms("abode", &en, None).unwrap(),
            ["home"],
            "offline, with no pass at all"
        );
        assert_eq!(
            reach.define("frobnicate", &en, None).unwrap_err().why,
            Why::NotFound,
            "not in WordNet, and no pass to ask with"
        );
        assert!(net.asked().is_empty());
        let pass = pass_for(WIKTIONARY_HOST);
        let online = reach.define("frobnicate", &en, Some(&pass)).unwrap();
        assert_eq!(online[0].gloss, "a made-up word");
        assert_eq!(online[0].source, Source::Wiktionary);
        assert_eq!(net.asked().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn another_tongue_is_asked_online_with_a_pass() {
        let pt = tongue("pt-BR");
        let defs =
            r#"{"pt":[{"partOfSpeech":"Noun","definitions":[{"definition":"<a>house</a>"}]}]}"#;
        let text =
            r#"{"parse":{"wikitext":"==Portuguese==\n# house\n#: {{syn|pt|lar|moradia}}\n"}}"#;
        let missing = r#"{"error":{"code":"missingtitle"}}"#;
        let urls = [
            wiktionary::definition_request("casa").url,
            wiktionary::wikitext_request("casa").url,
            wiktionary::wikitext_request("xyzzy").url,
            wiktionary::definition_request("xyzzy").url,
        ];
        let net = Canned::new(&[
            (&urls[0], 200, defs),
            (&urls[1], 200, text),
            (&urls[2], 200, missing),
            (&urls[3], 404, ""),
        ]);
        let cache = Cache::new(4);
        let reach = Reach::new(&net, &cache).keys_in(None).offline_in(None);
        let pass = pass_for(WIKTIONARY_HOST);
        assert_eq!(
            reach.define("casa", &pt, Some(&pass)).unwrap()[0].gloss,
            "house"
        );
        assert_eq!(
            reach.synonyms("casa", &pt, Some(&pass)).unwrap(),
            ["lar", "moradia"]
        );
        assert_eq!(
            reach.synonyms("xyzzy", &pt, Some(&pass)).unwrap_err(),
            Refusal {
                why: Why::NotFound,
                host: Some(WIKTIONARY_HOST.into())
            }
        );
        assert_eq!(
            reach.define("xyzzy", &pt, Some(&pass)).unwrap_err().why,
            Why::NotFound
        );
        assert_eq!(
            reach.define("   ", &pt, Some(&pass)).unwrap_err().why,
            Why::Empty
        );
    }
}
