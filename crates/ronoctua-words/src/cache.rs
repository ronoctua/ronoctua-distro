//! The translations already made, bounded.
//!
//! A song's lyrics are translated when they arrive, and the same song comes
//! round again; a search asks the same phrase as it is refined back. Asking
//! the host again would spend the person's quota on an answer this process
//! already had.
//!
//! In memory, one per process, and BOUNDED by `wordsCacheEntries`
//! (`assets/cache.nix`): the process is the player's daemon or the footer, and
//! both live for months. Past the bound, the entry used longest ago goes.
//!
//! # The key is everything that changes the answer
//!
//! The provider (and a LibreTranslate's address), both tongues, and the text
//! itself — never the key or the pass, which change who may ask and not what
//! the answer is.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use crate::{Provider, Tongue, Translation};

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// How many translations a process keeps (`wordsCacheEntries`).
pub const MOST_ENTRIES: usize = policy::WORDS_CACHE_ENTRIES;

const _: () = assert!(MOST_ENTRIES > 0, "a cache of nothing asks every time");

/// Translations, by everything that changes them; at most `most`.
#[derive(Debug)]
pub struct Cache {
    most: usize,
    kept: Mutex<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    /// A clock of uses: the entry with the smallest goes first.
    tick: u64,
    entries: HashMap<String, (u64, Translation)>,
}

impl Cache {
    /// A cache of at most `most` translations (at least one).
    #[must_use]
    pub fn new(most: usize) -> Self {
        Self {
            most: most.max(1),
            kept: Mutex::new(Kept::default()),
        }
    }

    /// The translation kept for this question, when there is one.
    #[must_use]
    pub fn get(
        &self,
        provider: &Provider,
        from: Option<&Tongue>,
        to: &Tongue,
        text: &str,
    ) -> Option<Translation> {
        let key = key_of(provider, from, to, text);
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        kept.tick += 1;
        let tick = kept.tick;
        kept.entries.get_mut(&key).map(|(used, t)| {
            *used = tick;
            t.clone()
        })
    }

    /// Keeps `translation` for this question, letting the entry used longest
    /// ago go when the cache is full.
    pub fn put(
        &self,
        provider: &Provider,
        from: Option<&Tongue>,
        to: &Tongue,
        text: &str,
        translation: &Translation,
    ) {
        let key = key_of(provider, from, to, text);
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        kept.tick += 1;
        let tick = kept.tick;
        if !kept.entries.contains_key(&key) && kept.entries.len() >= self.most {
            let oldest = kept
                .entries
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest {
                kept.entries.remove(&oldest);
            }
        }
        kept.entries.insert(key, (tick, translation.clone()));
    }

    /// How many translations it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .len()
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The process's cache, bounded by [`MOST_ENTRIES`].
#[must_use]
pub fn shared() -> &'static Cache {
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Cache::new(MOST_ENTRIES))
}

/// The key of a question: every part separated by a unit separator, which a
/// tongue's code and a provider's id cannot hold.
fn key_of(provider: &Provider, from: Option<&Tongue>, to: &Tongue, text: &str) -> String {
    let at = match provider {
        Provider::LibreTranslate { url } => url.as_str(),
        Provider::MyMemory | Provider::DeepL => "",
    };
    format!(
        "{}\u{1f}{at}\u{1f}{}\u{1f}{}\u{1f}{text}",
        provider.id(),
        from.map_or("", Tongue::code),
        to.code()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str) -> Translation {
        Translation {
            text: text.to_owned(),
            from: None,
            to: Tongue::new("pt-BR").unwrap(),
            by: Provider::MyMemory,
        }
    }

    /// FAILS IF: the cache grows past its bound, or lets the entry that was
    /// just used go instead of the one used longest ago.
    #[test]
    fn the_cache_is_bounded_and_the_least_used_goes_first() {
        let cache = Cache::new(2);
        let pt = Tongue::new("pt-BR").unwrap();
        let p = Provider::MyMemory;
        cache.put(&p, None, &pt, "one", &said("um"));
        cache.put(&p, None, &pt, "two", &said("dois"));
        assert!(
            cache.get(&p, None, &pt, "one").is_some(),
            "`one` used again"
        );
        cache.put(&p, None, &pt, "three", &said("três"));
        assert_eq!(cache.len(), 2, "never past the bound");
        assert!(cache.get(&p, None, &pt, "two").is_none(), "`two` went");
        assert_eq!(cache.get(&p, None, &pt, "one").unwrap().text, "um");
    }

    #[test]
    fn the_key_holds_everything_that_changes_the_answer() {
        let cache = Cache::new(8);
        let pt = Tongue::new("pt-BR").unwrap();
        let es = Tongue::new("es").unwrap();
        let en = Tongue::new("en").unwrap();
        cache.put(&Provider::MyMemory, None, &pt, "hello", &said("olá"));
        assert!(cache.get(&Provider::MyMemory, None, &es, "hello").is_none());
        assert!(cache.get(&Provider::DeepL, None, &pt, "hello").is_none());
        assert!(
            cache
                .get(&Provider::MyMemory, Some(&en), &pt, "hello")
                .is_none()
        );
        let a = Provider::LibreTranslate {
            url: "https://a.example".into(),
        };
        let b = Provider::LibreTranslate {
            url: "https://b.example".into(),
        };
        cache.put(&a, None, &pt, "hello", &said("oi"));
        assert!(cache.get(&b, None, &pt, "hello").is_none());
        assert!(cache.get(&a, None, &pt, "hello").is_some());
    }
}
