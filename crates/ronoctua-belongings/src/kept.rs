//! What is kept — the table `assets/belongings.nix` generates, and the one
//! arithmetic that reads it.
//!
//! The table is data (`RONOCTUA_BELONGINGS_RS`, copied in by `build.rs`); what
//! it MEANS is here, once, for the engine that walks the home, the door that
//! shows "What is kept", and the census test that compares every store's
//! declared verdict with what this arithmetic makes of it.
//!
//! # The rules
//!
//! A path is kept by default when it lies under one of the [`TREES`] — and no
//! [`NEVER`] name pattern matches a component below the tree's root — or is
//! one of the [`FILES`]. A [`NEVER`] path takes it (and what is under it)
//! out. On top of that come the person's own choices ([`kept_here`]): what
//! they added (links kept as links), what they took out — the most specific
//! of the two wins ([`excluded_here`]) — and what another machine added,
//! which travels in the index as a ROOT, so a machine keeps what any of the
//! person's machines chose to keep, new files in it included. A `hard` never
//! is never kept, whoever asks.
//!
//! Name patterns apply to the TREES only: a person who adds `home/.bashrc`
//! asked for a dotfile, and `.*` (the stores' drafts) is not about them.

use crate::places::covers;
use crate::prefs::Belongings;

/// A path kept, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// The logical path.
    pub path: &'static str,
    /// The reason's id (`belongings-reason-<id>`).
    pub reason: &'static str,
}

/// What a [`Never`] matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// A component's name, with at most one `*` (`*.pid`, `.*`, `scroll-*.conf`).
    Name(&'static str),
    /// A logical path, and what is under it.
    Path(&'static str),
}

/// What is never kept, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Never {
    /// What it matches.
    pub pattern: Pattern,
    /// The reason's id.
    pub reason: &'static str,
    /// Not kept even when the person adds it.
    pub hard: bool,
}

/// One entry of the census of preference files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    /// The logical path of the file a `Store` writes.
    pub path: &'static str,
    /// Whether the rules keep it.
    pub kept: bool,
    /// Why.
    pub reason: &'static str,
}

mod table {
    use super::{Census, Never, Pattern, Place};
    include!(concat!(env!("OUT_DIR"), "/belongings.rs"));
}

pub use table::{FILES, LARGE_BYTES, NEVER, REASONS, STORES, TREES};
// THE FORMAT-2 NUMBERS (`assets/belongings.nix`'s `chunk`, `budget`,
// `unlockHours`, `plainMostKiB`), read by the packages that use them.
pub use table::{
    CHUNK_AVG_BYTES, CHUNK_MAX_BYTES, CHUNK_MIN_BYTES, HISTORY_OVER_TREE, KEEP_DAYS,
    PLAIN_MOST_BYTES, PUSH_MOST_BYTES, RECOMMEND_BYTES, UNLOCK_HOURS, WARN_BYTES,
};
// ── anchor: L08 ──
/// The sizes the chunker cuts at, in bytes (the table's `chunk`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSizes {
    /// No chunk is smaller, but a file's last.
    pub min_bytes: u32,
    /// What a chunk is on average.
    pub avg_bytes: u32,
    /// No chunk is larger: what keeps every repository file under GitHub's
    /// limit, whatever the file it came from.
    pub max_bytes: u32,
}

/// What the table asks the chunker for.
pub const CHUNK_SIZES: ChunkSizes = ChunkSizes {
    min_bytes: as_u32(CHUNK_MIN_BYTES),
    avg_bytes: as_u32(CHUNK_AVG_BYTES),
    max_bytes: as_u32(CHUNK_MAX_BYTES),
};

/// A chunk size as the chunker takes it — bounded by the asserts below
/// (16 MiB at most), so it always fits.
const fn as_u32(bytes: u64) -> u32 {
    assert!(bytes <= u32::MAX as u64, "a chunk size past u32");
    bytes as u32
}

// FastCDC's own bounds (`fastcdc::v2020`): it panics outside them, so a
// table past them is refused at compile time instead of at the first sync.
const _: () = assert!(CHUNK_MIN_BYTES >= 64, "chunk.minKiB below FastCDC's floor");
const _: () = assert!(CHUNK_MIN_BYTES <= 1_048_576, "chunk.minKiB past 1 MiB");
const _: () = assert!(CHUNK_AVG_BYTES >= 256, "chunk.avgKiB below FastCDC's floor");
const _: () = assert!(CHUNK_AVG_BYTES <= 4_194_304, "chunk.avgKiB past 4 MiB");
const _: () = assert!(
    CHUNK_MAX_BYTES >= 1024,
    "chunk.maxKiB below FastCDC's floor"
);
const _: () = assert!(CHUNK_MAX_BYTES <= 16_777_216, "chunk.maxKiB past 16 MiB");
const _: () = assert!(
    CHUNK_MIN_BYTES < CHUNK_AVG_BYTES,
    "chunk.minKiB not below avgKiB"
);
const _: () = assert!(
    CHUNK_AVG_BYTES < CHUNK_MAX_BYTES,
    "chunk.avgKiB not below maxKiB"
);
// GitHub refuses a push over 2 GiB: a batch must stay under it.
const _: () = assert!(PUSH_MOST_BYTES < 2 << 30, "budget.pushMB past a 2 GiB push");

/// What the repository's weight asks of the person — the view's budget
/// line and the compaction it offers ("no" drawn first, never automatic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// Nothing to say.
    Light,
    /// Past `budget.warnMB`: said on the page.
    Heavy,
    /// Past `budget.recommendMB` — or past `budget.warnMB` and
    /// `historyOverTree` times the tree — with history a compaction would
    /// fold: compaction is recommended.
    Compact,
}

/// What `history_bytes` and `tree_bytes` ask of the person, `foldable` said
/// whether a compaction of `budget.keepDays` would drop any commit
/// (`status::Budget::foldable`). The ratio counts only past the warning line:
/// a small repository synced often passes twice its tree in days, and that
/// is no weight. And nothing is recommended that a compaction could not do.
/// Pure.
#[must_use]
pub const fn weight(tree_bytes: u64, history_bytes: u64, foldable: bool) -> Weight {
    let heavy = history_bytes >= WARN_BYTES;
    let too_much = history_bytes >= RECOMMEND_BYTES
        || (heavy
            && tree_bytes > 0
            && history_bytes >= tree_bytes.saturating_mul(HISTORY_OVER_TREE));
    if too_much && foldable {
        Weight::Compact
    } else if heavy {
        Weight::Heavy
    } else {
        Weight::Light
    }
}
// ── anchor: L09 ──
/// A root of the PLAIN part (`assets/belongings.nix`'s `plain`): kept in
/// clear under `plain/<at>` of the repository, synced with no passphrase
/// (`crate::plain`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlainRoot {
    /// The logical path of the root (`state/person`).
    pub path: &'static str,
    /// Where it is kept, under `plain/` of the repository (`person`).
    pub at: &'static str,
    /// The reason's id (`belongings-reason-<id>`).
    pub reason: &'static str,
}

pub use table::PLAIN;

/// The plain root `path` lies under (or is), if one does. Pure.
#[must_use]
pub fn plain_root(path: &str) -> Option<&'static PlainRoot> {
    PLAIN.iter().find(|r| covers(r.path, path))
}

/// The `never` whose NAME pattern matches a component of `path` below
/// `root` — the drafts, the numbers of running programs. Pure.
fn never_by_name_below(root: &str, path: &str) -> Option<&'static Never> {
    let below = path.get(root.len()..)?.trim_start_matches('/');
    below
        .split('/')
        .filter(|c| !c.is_empty())
        .find_map(|component| {
            NEVER
                .iter()
                .find(|n| matches!(n.pattern, Pattern::Name(p) if name_matches(p, component)))
        })
}

/// How a path both sides changed may be merged (the merge engine,
/// `engine/merging.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeRule {
    /// As its content allows: JSON by field, text by hunk, nothing else.
    Auto,
    /// Never merged: the person chooses a side (a key ring, whose parts mean
    /// nothing apart) — and no base of it is kept.
    Choose,
    /// Text whose lines are each a fact (`known_hosts`): both sides' lines.
    Union,
}

/// The merge rule of `path`: as its content allows, unless a predefined
/// target says `choose` or `union` for it (`targets.<id>.merge`, by file name
/// or `default` — [`Target::rule_of`]). Pure.
#[must_use]
pub fn merge_rule(path: &str) -> MergeRule {
    target_of(path)
        .and_then(|t| t.rule_of(path))
        .unwrap_or(MergeRule::Auto)
}

/// Whether `path` is a file of the plain part: under a plain root, and no
/// `never` takes it out (by its path, or a name pattern below the root — a
/// draft stays out of the plain part too). Pure.
#[must_use]
pub fn is_plain(path: &str) -> bool {
    matches!(by_default(path), Verdict::Plain(_))
}
// ── anchor: L10 ──
/// A PREDEFINED ENCRYPTED TARGET (`assets/belongings.nix`'s `targets`): a few
/// paths of the person's home — their SSH, git's configuration, GnuPG, the
/// shell's and Doom's local files — kept SEALED on every machine by
/// default. "Not on this machine" puts its paths into the person's
/// `excluded` (the repository's copies stay for the others); "not on any
/// machine" is a request (`crate::requests`) the next sync turns into the
/// index's `dropped`: the repository's copies are deleted, and every other
/// machine forgets its own and keeps the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// Its id (`belongings-target-<id>` on screen).
    pub id: &'static str,
    /// Its logical paths — folders walked whole, or files.
    pub paths: &'static [&'static str],
    /// Name patterns below its paths that are never kept: a socket, a lock,
    /// an agent's seed — this machine's moment, never the person's.
    pub skip: &'static [&'static str],
    /// Whether its files come back with their owner's bits alone and its
    /// folders `0700` ([`kept_mode`], [`PRIVATE_DIR_MODE`]).
    pub private: bool,
    /// How a file both sides changed merges, by file name or `default`.
    pub merge: &'static [(&'static str, MergeRule)],
    /// What a sync that brought its files here offers to turn on.
    pub offers: &'static [Offering],
}

/// Something a target's files make possible, offered after the sync that
/// brought them ("no" drawn first; `crate::status::Offer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Offering {
    /// Sign commits with the person's SSH key.
    SshSigning,
    /// Let a security key answer for the password (`pam_u2f`, sufficient).
    PamU2f,
}

impl Offering {
    /// Every one.
    pub const ALL: [Self; 2] = [Self::SshSigning, Self::PamU2f];

    /// Its word, as the table writes it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::SshSigning => "ssh-signing",
            Self::PamU2f => "pam-u2f",
        }
    }
}

pub use table::TARGETS;

/// The folders of a private target come back with their owner's bits alone
/// — `0700` when the index remembers nothing of them.
pub const PRIVATE_DIR_MODE: u32 = 0o700;

/// The reason's id of what a target skips ([`Verdict::Skipped`],
/// `belongings-reason-<id>`): this machine's moment.
pub const SKIPPED_REASON: &str = "transient";

impl Target {
    /// Whether one of its paths is `path`, or a folder above it. Pure.
    #[must_use]
    pub fn covers(&self, path: &str) -> bool {
        self.paths.iter().any(|p| covers(p, path))
    }

    /// Whether a name of [`Target::skip`] matches a component of `path`
    /// below the target's path that covers it. Pure.
    #[must_use]
    pub fn skips(&self, path: &str) -> bool {
        let Some(root) = self.paths.iter().find(|p| covers(p, path)) else {
            return false;
        };
        let below = path.get(root.len()..).unwrap_or_default();
        below
            .split('/')
            .filter(|c| !c.is_empty())
            .any(|c| self.skip.iter().any(|s| name_matches(s, c)))
    }

    /// The rule its `merge` names for `path`: its file name's, else the
    /// target's `default`; `None` when it names neither. Pure.
    #[must_use]
    pub fn rule_of(&self, path: &str) -> Option<MergeRule> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let named = |n: &str| self.merge.iter().find(|(m, _)| *m == n).map(|(_, r)| *r);
        named(name).or_else(|| named("default"))
    }

    /// Whether a sync that brought its files here offers `offering`.
    #[must_use]
    pub fn offers(&self, offering: Offering) -> bool {
        self.offers.contains(&offering)
    }
}

/// The target `path` lies under (or is), if one does. Pure.
#[must_use]
pub fn target_of(path: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|t| t.covers(path))
}

/// The target named `id`. Pure.
#[must_use]
pub fn target_by_id(id: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|t| t.id == id)
}

/// The mode a FILE at `path` is kept with — what the walk reads and a pull
/// writes: a private target's file keeps its owner's bits alone (`0644`
/// travels and comes back `0600`, a `0400` key stays `0400`), so group and
/// others never reach another machine and a mode read here never differs
/// from the one the repository holds. Everything else as it is. Pure.
#[must_use]
pub fn kept_mode(path: &str, mode: u32) -> u32 {
    if target_of(path).is_some_and(|t| t.private) {
        mode & 0o700
    } else {
        mode
    }
}

/// Whether `path` lies under a target the index says no machine keeps any
/// more (`dropped`, by target id). Pure.
#[must_use]
pub fn dropped_everywhere(path: &str, dropped: &std::collections::BTreeSet<String>) -> bool {
    target_of(path).is_some_and(|t| dropped.contains(t.id))
}

#[cfg(test)]
mod target_tests {
    use super::*;

    /// FAILS IF: a target's path is not a target's verdict, a name its
    /// `skip` lists is kept, a path near a target (by prefix, not by
    /// component) is taken for it, or the deploy key's never stops beating
    /// every target.
    #[test]
    fn a_target_is_its_own_verdict_and_skips_what_it_names() {
        for (path, id) in [
            ("home/.ssh/id_ed25519", "ssh"),
            ("home/.ssh", "ssh"),
            ("home/.gitconfig", "git"),
            ("home/.config/git/allowed_signers", "git"),
            ("home/.config/Yubico/u2f_keys", "git"),
            ("home/.gnupg/private-keys-v1.d/A1.key", "gnupg"),
            ("home/.shellrc.local", "shell"),
            ("home/.config/doom/local.el", "doom"),
        ] {
            assert!(
                matches!(by_default(path), Verdict::Target(t) if t.id == id),
                "{path}: {:?}",
                by_default(path)
            );
            assert!(kept_here(path, &Belongings::default(), false), "{path}");
        }
        for skipped in [
            "home/.ssh/agent.sock",
            "home/.ssh/control-git@github.com:22",
            "home/.gnupg/S.gpg-agent",
            "home/.gnupg/random_seed",
            "home/.gnupg/public-keys.d/pubring.db.lock",
            "home/.gnupg/.#lk0x0000.host.123",
        ] {
            assert!(
                matches!(by_default(skipped), Verdict::Skipped(_)),
                "{skipped}: {:?}",
                by_default(skipped)
            );
            // NOT EVEN ADDED BY HAND: a socket or a seed is this machine's.
            let mut prefs = Belongings::default();
            prefs.extra.push("home".to_owned());
            assert!(!kept_here(skipped, &prefs, true), "{skipped}");
        }
        for near in [
            "home/.sshd/x",
            "home/.gitconfig.bak",
            "home/.config/doom/config.el",
        ] {
            assert!(target_of(near).is_none(), "{near}");
        }
        assert!(matches!(
            by_default("config/belongings-keys/deploy"),
            Verdict::Never(_)
        ));
    }

    /// FAILS IF: a key ring's file is merged instead of chosen, a known host
    /// or a trusted signer is not the union of both sides, or a path no
    /// target names stops merging as its content allows.
    #[test]
    fn a_target_merges_by_its_own_rule() {
        assert_eq!(merge_rule("home/.ssh/id_ed25519"), MergeRule::Choose);
        assert_eq!(merge_rule("home/.ssh/config"), MergeRule::Choose);
        assert_eq!(merge_rule("home/.ssh/known_hosts"), MergeRule::Union);
        assert_eq!(
            merge_rule("home/.config/git/allowed_signers"),
            MergeRule::Union
        );
        assert_eq!(merge_rule("home/.gitconfig"), MergeRule::Auto);
        assert_eq!(merge_rule("home/.gnupg/trustdb.gpg"), MergeRule::Choose);
        assert_eq!(merge_rule("config/theme.json"), MergeRule::Auto);
    }

    /// FAILS IF: a private target's file keeps group or other bits (they
    /// would travel and come back), loses its owner's, or a file of
    /// anything else is changed.
    #[test]
    fn a_private_target_keeps_its_owners_bits_alone() {
        assert_eq!(kept_mode("home/.ssh/id_ed25519.pub", 0o644), 0o600);
        assert_eq!(kept_mode("home/.ssh/id_ed25519", 0o400), 0o400);
        assert_eq!(kept_mode("home/.gnupg/pubring.kbx", 0o664), 0o600);
        assert_eq!(kept_mode("home/.gitconfig", 0o644), 0o644);
        assert_eq!(kept_mode("config/theme.json", 0o644), 0o644);
        let dropped = ["ssh".to_owned()].into_iter().collect();
        assert!(dropped_everywhere("home/.ssh/config", &dropped));
        assert!(!dropped_everywhere("home/.gitconfig", &dropped));
        for t in TARGETS {
            assert_eq!(target_by_id(t.id), Some(t));
        }
        assert!(target_by_id("nonsense").is_none());
    }
}
// ── anchor: L21 ──

/// What the table makes of a path, before the person's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Kept, and why.
    Kept(&'static str),
    /// Taken out by a [`NEVER`], and why.
    Never(&'static Never),
    /// Neither under a tree nor one of the files.
    Outside,
    // ── anchor: L07 ──
    // ── anchor: L09 ──
    /// Under a root of the PLAIN part: kept in clear, by the plain sync —
    /// never sealed too, whatever tree it lies under.
    Plain(&'static PlainRoot),
    // ── anchor: L10 ──
    /// Under a predefined target ([`TARGETS`]): kept, sealed, on every
    /// machine that did not take it out.
    Target(&'static Target),
    /// Under a target, a name its `skip` lists (a socket, a lock, an
    /// agent's seed): never kept, whoever asks.
    Skipped(&'static Target),
}

/// Whether a name matches a pattern with at most one `*`. Pure.
#[must_use]
pub fn name_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((before, after)) => {
            name.len() >= before.len() + after.len()
                && name.starts_with(before)
                && name.ends_with(after)
        }
    }
}

/// The never that takes `path` out by its path, if one does.
fn never_by_path(path: &str) -> Option<&'static Never> {
    NEVER
        .iter()
        .find(|n| matches!(n.pattern, Pattern::Path(p) if covers(p, path)))
}

/// What the table makes of `path` (a logical path). Pure.
#[must_use]
pub fn by_default(path: &str) -> Verdict {
    if let Some(never) = never_by_path(path) {
        return Verdict::Never(never);
    }
    // A PLAIN ROOT BEATS A TREE: `config/brain-modules` is under `config`,
    // and is kept in clear, never sealed as well.
    if let Some(root) = plain_root(path) {
        return never_by_name_below(root.path, path).map_or(Verdict::Plain(root), Verdict::Never);
    }
    // A TARGET'S PATH IS THE TARGET'S, and only its own `skip` takes a name
    // out below it (the generator refuses a target over a tree or a file).
    if let Some(target) = target_of(path) {
        return if target.skips(path) {
            Verdict::Skipped(target)
        } else {
            Verdict::Target(target)
        };
    }
    for tree in TREES {
        if !covers(tree.path, path) {
            continue;
        }
        if let Some(never) = never_by_name_below(tree.path, path) {
            return Verdict::Never(never);
        }
        return Verdict::Kept(tree.reason);
    }
    FILES
        .iter()
        .find(|f| f.path == path)
        .map_or(Verdict::Outside, |f| Verdict::Kept(f.reason))
}

/// Whether this machine keeps `path`: the table, the person's choices, and
/// `added_elsewhere` — a folder or file a person added on any of their
/// machines covers it (the index's roots say so). Pure.
#[must_use]
pub fn kept_here(path: &str, prefs: &Belongings, added_elsewhere: bool) -> bool {
    if never_by_path(path).is_some_and(|n| n.hard) {
        return false;
    }
    // THE PLAIN PART IS NOT SEALED, whoever adds a folder above it: it is
    // kept by the plain sync alone (`crate::plain`).
    if plain_root(path).is_some() {
        return false;
    }
    // WHAT A TARGET SKIPS is this machine's moment, whoever added a folder
    // above it.
    if target_of(path).is_some_and(|t| t.skips(path)) {
        return false;
    }
    if excluded_here(path, prefs) {
        return false;
    }
    if added_here(path, prefs) || added_elsewhere {
        return true;
    }
    matches!(by_default(path), Verdict::Kept(_) | Verdict::Target(_))
}

/// Whether the person took `path` out on this machine: of their choices that
/// cover it, the MOST SPECIFIC is a "taken out" — so `home/notes` added under
/// a `home` taken out is kept, and `config/studio` taken out under the
/// configuration kept is not. Pure.
#[must_use]
pub fn excluded_here(path: &str, prefs: &Belongings) -> bool {
    let deepest = |list: &[String]| {
        list.iter()
            .filter(|e| covers(e, path))
            .map(String::len)
            .max()
    };
    match (deepest(&prefs.excluded), deepest(&prefs.extra)) {
        (Some(out), Some(added)) => out >= added,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Whether the person added `path` on this machine (it or a folder above
/// it) — what the index marks as travelling to every machine. Pure.
#[must_use]
pub fn added_here(path: &str, prefs: &Belongings) -> bool {
    prefs.extra.iter().any(|e| covers(e, path))
}

/// Whether a person may add `path` at all: a logical path that no hard never
/// covers. Pure.
#[must_use]
pub fn may_add(path: &str) -> bool {
    crate::places::is_logical(path) && !never_by_path(path).is_some_and(|n| n.hard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_with_one_star() {
        assert!(name_matches("*.pid", "column-player.pid"));
        assert!(name_matches(".*", ".theme.json.123.tmp"));
        assert!(name_matches("scroll-*.conf", "scroll-theme.conf"));
        assert!(!name_matches("scroll-*.conf", "scroll.conf"));
        assert!(!name_matches("*.pid", "pid"));
        assert!(name_matches("exact", "exact"));
    }

    /// FAILS IF: a store's declared verdict in `assets/belongings.nix` is not
    /// what the rules make of its path — the census would say "kept" over a
    /// file the walk leaves out, or the other way round.
    #[test]
    fn every_store_is_where_the_census_says() {
        assert!(!STORES.is_empty());
        for store in STORES {
            // KEPT IS KEPT, sealed or in clear: both travel.
            let kept = matches!(by_default(store.path), Verdict::Kept(_) | Verdict::Plain(_));
            assert_eq!(
                kept,
                store.kept,
                "{} is declared {} and the rules make it {:?}",
                store.path,
                store.kept,
                by_default(store.path)
            );
            assert!(REASONS.contains(&store.reason));
        }
    }

    #[test]
    fn the_defaults_keep_the_preferences_and_never_the_access() {
        assert!(matches!(by_default("config/theme.json"), Verdict::Kept(_)));
        assert!(matches!(
            by_default("config/player-keys/tmdb"),
            Verdict::Kept(_)
        ));
        for never in [
            "config/column-player.pid",
            "config/.theme.json.42.tmp",
            "config/scroll-theme.conf",
            "config/belongings.json",
            "config/belongings-keys/deploy",
            "state/belongings/last.json",
            "home/.local/state/syncthing/cert.pem",
        ] {
            assert!(
                matches!(by_default(never), Verdict::Never(_)),
                "{never} is kept"
            );
        }
        assert_eq!(by_default("home/notes/a.md"), Verdict::Outside);
        assert!(matches!(
            by_default("state/software/history.jsonl"),
            Verdict::Kept("programs")
        ));
    }

    /// FAILS IF: a path of the plain part is sealed too (the walk would push
    /// it twice, once in clear and once sealed), a draft in it travels in
    /// clear, or the person's data and brain modules stop being plain.
    #[test]
    fn a_plain_root_beats_its_tree_and_keeps_its_drafts_out() {
        let modules = by_default("config/brain-modules/my-tea/module.rs");
        assert!(
            matches!(modules, Verdict::Plain(r) if r.at == "brain-modules"),
            "{modules:?}"
        );
        assert!(matches!(
            by_default("state/person/software/declared.json"),
            Verdict::Plain(r) if r.reason == "person-data"
        ));
        assert!(is_plain("state/person"));
        for draft in [
            "config/brain-modules/my-tea/.module.rs.swp",
            "state/person/software/declared.json.12.new",
            "state/person/.options.json.belongings.9.tmp",
        ] {
            assert!(
                matches!(by_default(draft), Verdict::Never(_)),
                "{draft} travels in clear"
            );
        }
        assert!(matches!(
            by_default("config/brain-modulesx/a"),
            Verdict::Kept(_)
        ));
        // NEVER SEALED, even under a folder the person added.
        let mut prefs = Belongings::default();
        prefs.extra.push("state".to_owned());
        assert!(!kept_here("state/person/options.json", &prefs, true));
        assert!(!kept_here(
            "config/brain-modules/my-tea/module.rs",
            &prefs,
            true
        ));
        assert!(kept_here("state/other/x", &prefs, false));
    }

    #[test]
    fn the_persons_choices_go_over_the_table_but_never_over_a_hard_never() {
        let mut prefs = Belongings::default();
        assert!(!kept_here("home/notes/a.md", &prefs, false));
        assert!(
            kept_here("home/notes/a.md", &prefs, true),
            "added elsewhere"
        );
        prefs.extra.push("home/notes".to_owned());
        assert!(kept_here("home/notes/a.md", &prefs, false));
        assert!(added_here("home/notes/a.md", &prefs));
        // A dotfile the person added is kept: name patterns are the trees'.
        prefs.extra.push("home/.bashrc".to_owned());
        assert!(kept_here("home/.bashrc", &prefs, false));
        // Syncthing's identity is out by default, and in when asked for.
        assert!(!kept_here(
            "home/.local/state/syncthing/cert.pem",
            &prefs,
            false
        ));
        prefs.extra.push("home/.local/state/syncthing".to_owned());
        assert!(kept_here(
            "home/.local/state/syncthing/cert.pem",
            &prefs,
            false
        ));
        // The deploy key never, whoever asks.
        prefs.extra.push("config/belongings-keys".to_owned());
        assert!(!kept_here("config/belongings-keys/deploy", &prefs, true));
        assert!(!may_add("config/belongings-keys"));
        // Taken out beats everything the person did not mark hard.
        prefs.excluded.push("config/theme.json".to_owned());
        assert!(!kept_here("config/theme.json", &prefs, true));
        // THE MOST SPECIFIC CHOICE WINS: a folder kept under one taken out is
        // kept, and one taken out under one kept is not.
        let mut prefs = Belongings::default();
        prefs.excluded.push("home".to_owned());
        prefs.extra.push("home/notes".to_owned());
        assert!(kept_here("home/notes/a.md", &prefs, false));
        assert!(!kept_here("home/music/b.ogg", &prefs, true));
        prefs.excluded.push("home/notes/private".to_owned());
        assert!(!kept_here("home/notes/private/c.md", &prefs, true));
        assert!(excluded_here("home/notes/private", &prefs));
        assert!(!excluded_here("home/notes", &prefs));
    }
}
