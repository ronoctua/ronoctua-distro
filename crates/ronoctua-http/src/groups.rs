//! What each host agreed to protect its key with — the TLS key-exchange group
//! a handshake negotiated, kept per host in `$XDG_STATE_HOME/ronoctua/`
//! [`FILE`].
//!
//! # Why it is kept
//!
//! A conversation recorded today can be read later by whoever stores it, the
//! day a quantum computer breaks the key it was agreed with ("harvest now,
//! decrypt later"). [`crate::tls`] OFFERS the hybrid `X25519MLKEM768` first,
//! but offering is half of it: the host chooses. The security view's quantum
//! section says, host by host, which of the hosts this machine talked to
//! answered with a post-quantum group and which still agree a classical one —
//! and the only way to know is to have been there, so every handshake writes
//! down what it got.
//!
//! # What is kept, and how much
//!
//! The group's IANA name, whether it holds against a quantum computer, and
//! when it was last heard. At most [`MOST_HOSTS`] hosts (`tlsGroupsHosts` in
//! `assets/cache.nix`): past it, the host heard longest ago goes first.
//!
//! # The file is shared, and written seldom
//!
//! Every process that speaks TLS writes it — the footer, the queue, the
//! brain's modules, the software centre — and every one of them from many
//! threads. So a write is a MERGE by host, done ONE AT A TIME: under a lock
//! of the process and a lock of the file ([`LOCK`]), the file is read fresh,
//! one entry changes, and the rest is written back as found. Without the
//! first, two threads wrote into one draft (`write_atomically` names it by
//! the process) and published a mix of both; without the second, two
//! processes each wrote back what they read, and the slower one erased the
//! other's host.
//!
//! And a process remembers what is already in the file ([`record`]): the
//! same answer from the same host is written again only after [`REFRESH_S`],
//! so a search that asks one host at every keystroke costs one write a day,
//! not one per request. It remembers a host only once the write that kept it
//! succeeded.
//!
//! # A file that will not parse is REPLACED
//!
//! Not refused, as a person's preferences are: this is what the machine
//! HEARD, made again by its next requests, and refusing to write over it
//! would leave it broken for good — every process re-reading it at every
//! request and keeping nothing. It is said once per process, in the log.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use ronoctua_store::Store;
use serde::{Deserialize, Serialize};

/// The file, under the state root.
pub const FILE: &str = "tls-groups.json";

/// The lock every process takes around a write of [`FILE`], beside it: empty,
/// hidden, and left in place (a lock file that is removed can be locked by
/// two processes at once, each through its own copy).
pub const LOCK: &str = ".tls-groups.json.lock";

/// The shape this writes.
pub const FORMAT: u32 = 1;

/// The cache policy (`assets/cache.nix`), for `tlsGroupsHosts`.
mod policy {
    #![allow(dead_code, missing_docs)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// The most hosts the file keeps (`tlsGroupsHosts` in `assets/cache.nix`).
pub const MOST_HOSTS: usize = policy::TLS_GROUPS_HOSTS;

/// How long the same answer from the same host goes unwritten, in seconds.
///
/// A day: what is kept is a fact about the HOST, which changes when its
/// operators upgrade, not between two requests — and the time beside it only
/// has to be good enough to tell which host was heard longest ago.
pub const REFRESH_S: i64 = 24 * 60 * 60;

/// Every host heard, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Groups {
    /// [`FORMAT`] when written by this; `0` for a file nothing wrote yet.
    pub format: u32,
    /// What each host agreed, by its name as the consent keeps it.
    pub hosts: BTreeMap<String, Heard>,
}

/// What one host agreed, the last time it was heard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Heard {
    /// The group's IANA name (`X25519MLKEM768`, `X25519`, `secp256r1`…), or
    /// `0x<hex>` for a code rustls has no name for.
    pub group: String,
    /// Whether the key agreed holds against a quantum computer.
    pub post_quantum: bool,
    /// When it was last heard, in seconds since the epoch.
    pub at_s: i64,
}

impl Groups {
    /// How many hosts answered with a post-quantum group, and how many with a
    /// classical one.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        let quantum = self.hosts.values().filter(|h| h.post_quantum).count();
        (quantum, self.hosts.len() - quantum)
    }

    /// Writes down that `host` agreed `heard`, and keeps at most `most`
    /// hosts — the one heard longest ago goes first. Pure.
    ///
    /// Answers whether the file is worth writing: a host that is new, a group
    /// that changed, or the same answer older than [`REFRESH_S`].
    pub fn note(&mut self, host: &str, heard: Heard, most: usize) -> bool {
        let worth = self.hosts.get(host).is_none_or(|was| {
            was.group != heard.group
                || was.post_quantum != heard.post_quantum
                || heard.at_s.saturating_sub(was.at_s) >= REFRESH_S
        });
        if !worth {
            return false;
        }
        self.format = FORMAT;
        self.hosts.insert(host.to_owned(), heard);
        while self.hosts.len() > most.max(1) {
            // THE OLDEST GOES, and a tie goes by name so two processes trim
            // the same file to the same thing.
            let Some(oldest) = self
                .hosts
                .iter()
                .min_by(|a, b| a.1.at_s.cmp(&b.1.at_s).then_with(|| a.0.cmp(b.0)))
                .map(|(name, _)| name.clone())
            else {
                break;
            };
            self.hosts.remove(&oldest);
        }
        true
    }
}

/// Whether the key-exchange group with this IANA code holds against a quantum
/// computer: ML-KEM on its own, the three hybrids with it, and the two
/// pre-standard Kyber drafts a host may still answer with. Pure.
#[must_use]
pub const fn post_quantum(code: u16) -> bool {
    matches!(code, 0x0200..=0x0202 | 0x11eb..=0x11ed | 0x6399 | 0x639a)
}

/// What a negotiated group is written as: its name, and whether it is
/// post-quantum — at `at_s`.
#[must_use]
pub fn heard_of(group: rustls::NamedGroup, at_s: i64) -> Heard {
    let code = u16::from(group);
    Heard {
        group: group
            .as_str()
            .map_or_else(|| format!("0x{code:04x}"), str::to_owned),
        post_quantum: post_quantum(code),
        at_s,
    }
}

/// The store of this machine's person: [`FILE`] under the state root, or a
/// store that keeps nothing where there is no home (a system unit).
#[must_use]
pub fn store() -> Store<Groups> {
    Store::at(ronoctua_store::state_dir(), FILE)
}

/// Seconds since the epoch, now.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The name a host is kept under: the consent's own spelling of it, so the
/// quantum section can put a host's answer beside the person's permission.
fn key_of(host: &str) -> String {
    ronoctua_consent::normal(host).unwrap_or_else(|| host.trim().to_ascii_lowercase())
}

/// What this process knows is in the files it writes, and what it has
/// already said about them.
struct Memory {
    /// By file and host: the group's name and when it was heard — put here
    /// only once the file says so.
    kept: BTreeMap<(Option<PathBuf>, String), (String, i64)>,
    /// Whether a file this could not use (it would not parse, or a newer
    /// build wrote it) was said, in the log.
    said_unusable: bool,
    /// Whether a write that failed was said.
    said_unwritten: bool,
}

impl Memory {
    /// That the file at `key.0` says what `key.1` agreed.
    fn remember(&mut self, key: (Option<PathBuf>, String), heard: (String, i64)) {
        // BOUNDED LIKE THE FILE: a process that met more hosts than the file
        // keeps starts its memory again rather than growing it.
        if self.kept.len() >= MOST_HOSTS {
            self.kept.clear();
        }
        self.kept.insert(key, heard);
    }
}

/// The process's [`Memory`] — and, held, the lock that makes its records one
/// at a time.
static MEMORY: Mutex<Memory> = Mutex::new(Memory {
    kept: BTreeMap::new(),
    said_unusable: false,
    said_unwritten: false,
});

/// The lock of `store`'s file, taken: the other PROCESSES wait for it. `None`
/// for a store that keeps nothing, or a folder where a lock cannot be made —
/// the write that follows fails there too, and says so.
fn file_lock(store: &Store<Groups>) -> Option<std::fs::File> {
    let path = store.path()?.with_file_name(LOCK);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok()?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    // BLOCKING, and briefly: whoever holds it reads a small file and renames
    // another. This runs on a request's thread, never on an event loop.
    file.lock().ok()?;
    Some(file)
}

/// Keeps what `host` agreed in `store` — merged into what the file already
/// says, and only when it is worth a write ([`Groups::note`]).
///
/// One record at a time: the process's lock is held from the look at what it
/// remembers to the rename, and the file's lock ([`LOCK`]) from the read to
/// the rename. A file that will not parse is replaced (it is derived), a
/// file of a NEWER format is left alone (a newer build wrote it), and either
/// is said once per process; a write that fails is logged once and costs
/// nothing else — the connection it came from goes on.
pub fn record(store: &Store<Groups>, host: &str, heard: Heard) {
    let key = (store.path(), key_of(host));
    let mut memory = MEMORY.lock().unwrap_or_else(PoisonError::into_inner);
    if memory.kept.get(&key).is_some_and(|(group, at_s)| {
        *group == heard.group && heard.at_s.saturating_sub(*at_s) < REFRESH_S
    }) {
        return;
    }
    let held = file_lock(store);
    let mut groups = if let Some(groups) = store.parsed() {
        groups
    } else {
        if !std::mem::replace(&mut memory.said_unusable, true) {
            tracing::warn!(file = FILE, "will not parse; it is started again");
        }
        Groups::default()
    };
    let remembered = (heard.group.clone(), heard.at_s);
    if groups.format > FORMAT {
        if !std::mem::replace(&mut memory.said_unusable, true) {
            tracing::warn!(
                file = FILE,
                format = groups.format,
                "a newer build keeps it; it is left as it is"
            );
        }
        // Remembered all the same: the file is the newer build's to keep,
        // and reading it again at every request would change nothing.
        memory.remember(key, remembered);
        return;
    }
    if groups.note(&key.1, heard, MOST_HOSTS)
        && let Err(e) = store.write(&groups)
    {
        if !std::mem::replace(&mut memory.said_unwritten, true) {
            tracing::warn!(file = FILE, error = %e, "could not keep the agreed group");
        }
        return;
    }
    drop(held);
    memory.remember(key, remembered);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(group: &str, quantum: bool, at_s: i64) -> Heard {
        Heard {
            group: group.to_owned(),
            post_quantum: quantum,
            at_s,
        }
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-groups-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// FAILS IF: a hybrid or a pure ML-KEM group is called classical, or a
    /// classical one post-quantum — the section would say the wrong thing
    /// about every host.
    #[test]
    fn the_quantum_groups_are_the_ml_kem_ones() {
        for group in [
            rustls::NamedGroup::X25519MLKEM768,
            rustls::NamedGroup::secp256r1MLKEM768,
            rustls::NamedGroup::MLKEM768,
            rustls::NamedGroup::MLKEM1024,
        ] {
            let h = heard_of(group, 1);
            assert!(h.post_quantum, "{group:?}");
        }
        for group in [
            rustls::NamedGroup::X25519,
            rustls::NamedGroup::secp256r1,
            rustls::NamedGroup::secp384r1,
            rustls::NamedGroup::FFDHE2048,
        ] {
            assert!(!heard_of(group, 1).post_quantum, "{group:?}");
        }
        assert_eq!(
            heard_of(rustls::NamedGroup::X25519MLKEM768, 7).group,
            "X25519MLKEM768"
        );
        // A code nobody named is still written, by its number.
        assert_eq!(
            heard_of(rustls::NamedGroup::from(0x11ed), 7),
            heard("0x11ed", true, 7)
        );
    }

    /// FAILS IF: the same answer is written at every request, or a changed
    /// one waits for the refresh.
    #[test]
    fn an_answer_is_written_when_it_is_news() {
        let mut groups = Groups::default();
        assert!(groups.note("a.example", heard("X25519", false, 100), 8));
        assert_eq!(groups.format, FORMAT);
        assert!(
            !groups.note("a.example", heard("X25519", false, 200), 8),
            "the same answer an instant later is not news"
        );
        assert!(
            groups.note("a.example", heard("X25519MLKEM768", true, 300), 8),
            "the host upgraded: that is news at once"
        );
        assert!(groups.note(
            "a.example",
            heard("X25519MLKEM768", true, 300 + REFRESH_S),
            8
        ));
        assert_eq!(groups.hosts["a.example"].at_s, 300 + REFRESH_S);
    }

    /// FAILS IF: the file grows past its bound, or the bound throws out a
    /// host heard recently instead of the one heard longest ago.
    #[test]
    fn the_host_heard_longest_ago_goes_first() {
        let mut groups = Groups::default();
        for (i, host) in ["c", "a", "d", "b"].iter().enumerate() {
            let at = i64::try_from(i).unwrap() * 10;
            groups.note(host, heard("X25519", false, at), 3);
        }
        assert_eq!(groups.hosts.len(), 3);
        assert!(!groups.hosts.contains_key("c"), "c was heard first");
        // Heard again, `a` is the youngest; the next one in pushes out `d`.
        groups.note("a", heard("X25519", false, 40 + REFRESH_S), 3);
        groups.note("e", heard("X25519", false, 50 + REFRESH_S), 3);
        let left: Vec<&str> = groups.hosts.keys().map(String::as_str).collect();
        assert_eq!(left, ["a", "b", "e"]);
        assert_eq!(groups.counts(), (0, 3));
    }

    /// FAILS IF: a write replaces what another process kept instead of
    /// merging into it, a broken file is written over, or a second request
    /// to the same host writes again.
    #[test]
    fn a_record_merges_by_host_and_spares_a_broken_file() {
        let dir = scratch("record");
        let store: Store<Groups> = Store::at(Some(dir.clone()), FILE);
        // ANOTHER PROCESS kept a host already.
        let mut theirs = Groups::default();
        theirs.note("other.example", heard("X25519", false, 5), 8);
        store.write(&theirs).unwrap();

        record(&store, "Mine.Example.", heard("X25519MLKEM768", true, 10));
        let kept = store.read();
        assert_eq!(kept.hosts.len(), 2, "{kept:?}");
        assert!(
            kept.hosts["mine.example"].post_quantum,
            "kept by the consent's spelling"
        );
        assert!(kept.hosts.contains_key("other.example"));

        // The same answer again is remembered by the process, not re-read:
        // a file removed meanwhile stays removed.
        std::fs::remove_file(dir.join(FILE)).unwrap();
        record(&store, "mine.example", heard("X25519MLKEM768", true, 20));
        assert!(!dir.join(FILE).exists(), "a second request wrote again");

        // A FILE THAT WILL NOT PARSE IS STARTED AGAIN: it is what this
        // machine heard, and left broken it would keep nothing for ever.
        std::fs::write(dir.join(FILE), b"{ not json").unwrap();
        record(&store, "third.example", heard("X25519", false, 30));
        let kept = store.parsed().expect("replaced by a file that parses");
        assert_eq!(kept.hosts.keys().collect::<Vec<_>>(), ["third.example"]);

        // A file a NEWER build wrote is left as it is.
        let newer = br#"{"format": 99, "hosts": {}}"#;
        std::fs::write(dir.join(FILE), newer).unwrap();
        record(&store, "fourth.example", heard("X25519", false, 40));
        assert_eq!(std::fs::read(dir.join(FILE)).unwrap(), newer);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: two threads of one process record at once and one host is
    /// lost, or the file is left as a mix of two writes — they shared one
    /// draft (`write_atomically` names it by the process) before the records
    /// were made one at a time.
    #[test]
    fn many_threads_recording_at_once_lose_nothing() {
        let dir = scratch("threads");
        let store: Store<Groups> = Store::at(Some(dir.clone()), FILE);
        // A FILE WORTH TEARING: long enough that a write takes a while.
        let mut big = Groups::default();
        for i in 0..200 {
            big.note(
                &format!("{i:04}.before.example"),
                heard("X25519", false, 1),
                512,
            );
        }
        store.write(&big).unwrap();
        let threads: Vec<_> = (0..16)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for round in 0..4 {
                        record(
                            &store,
                            &format!("{i}-{round}.threads.example"),
                            heard("X25519MLKEM768", true, 100 + i64::from(round)),
                        );
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let kept = store.parsed().expect("the file parses");
        for i in 0..16 {
            for round in 0..4 {
                assert!(
                    kept.hosts
                        .contains_key(&format!("{i}-{round}.threads.example")),
                    "{i}-{round} was lost: {} hosts kept",
                    kept.hosts.len()
                );
            }
        }
        assert_eq!(kept.hosts.len(), 200 + 16 * 4);
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != FILE && n != LOCK)
            .collect();
        assert!(left.is_empty(), "drafts left behind: {left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The bound reaches the code from `assets/cache.nix`, and is a bound.
    const _: () = assert!(MOST_HOSTS >= 16 && MOST_HOSTS <= 1 << 16);
}
