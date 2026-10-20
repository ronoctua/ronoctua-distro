//! The update's check of every declared entry, one at a time — what it asks
//! `nix`, and what it concludes. The binary runs the commands; this decides.
//!
//! # Why one evaluation per entry
//!
//! The machine's Nix consumer skips an entry it can see is wrong (a missing
//! attribute, an alias that throws, `meta` refusing it) and reports why. What
//! it cannot catch is an error deep inside a dependency: `tryEval` does not
//! reach it, and the whole system stops evaluating. So before the rebuild,
//! root asks about each entry in a process of its own:
//!
//! 1. `nix eval --json <machine>#personFingerprints` — the baseline. If the
//!    machine's flake does not evaluate at all, no entry is to blame and
//!    nothing is held: verify refuses and `held.json` stays as it was.
//! 2. per entry, `nix eval --raw …#personEntries."<id>".drvPath` — a failure
//!    is `eval-failed`;
//! 3. then `nix build --no-link …#personEntries."<id>"` — a failure of an
//!    entry with a pinned download asks `nix store prefetch-file --json` for
//!    the pin's address: it cannot be fetched → `fetch-failed`; it is there
//!    with another hash → `upstream-changed`; the same → `build-failed`.
//!    An entry with no pin that does not build is `build-failed`. No log is
//!    ever parsed: each answer is an exit status or a JSON field.
//!
//! What fails is written to `local/held.json` with the fingerprint Nix gave
//! it, and the consumer skips it while the fingerprint stays the same. The
//! update goes on: a held entry is said, never fatal.
//!
//! # And the distro's AppImages the person follows
//!
//! A followed pin (`crate::distro_pins`) is a release the distro's gate never
//! built, and the rebuild that would meet it first stops the update. So the
//! machine flake hands each one the registry would build — the option on,
//! newer than the distro's — to this same round, as `distro-appimage:<id>`
//! ([`crate::distro_pins::ENTRY`]), with its pin from the machine's copy of
//! `distro-pins.json`: one that does not build is held like a declared
//! entry, and the registry builds the distro's pin in its place.
//!
//! AND FOR THESE, BUILT IS WHAT LETS THEM IN: one that built is recorded in
//! `local/verified.json` at its fingerprint ([`verified_after`]), and the
//! registry builds a followed pin only while that record names it — so a
//! build of the machine that ran no verify (the software centre's apply, the
//! rescue's rebuild) never meets an untried release: it builds the last
//! verified one, or the distro's.
//!
//! # The contract with the machine flake
//!
//! `personEntries` and `personFingerprints` have the same names — every
//! declared entry this machine would build, held or not (a held entry is
//! tried again each update) — and a fingerprint changes whenever the entry or
//! what builds it does. Only Nix computes it.

use std::collections::BTreeMap;

use crate::declared::{Entry, Source};
use crate::local::{Held, HeldEntry, HeldWhy, Verified, VerifiedEntry};
use crate::{Stamp, check};

/// The machine flake's output of the declared entries' builds.
pub const ENTRIES: &str = "personEntries";
/// The machine flake's output of the entries' fingerprints.
pub const FINGERPRINTS: &str = "personFingerprints";

/// The machine directory as `nix` names it: a `path:` flake (it has no
/// `.git`).
#[must_use]
pub fn flake(machine: &str) -> String {
    format!("path:{machine}")
}

/// What every flake command is given: flakes on, and the lock never moved —
/// moving the inputs is the update's `flake-update`, never this.
fn flake_flags() -> [String; 3] {
    [
        "--extra-experimental-features".to_owned(),
        "nix-command flakes".to_owned(),
        "--no-update-lock-file".to_owned(),
    ]
}

/// Whether `id` may be put between quotes in an attribute path:
/// `<source>:<rest>`, a known source, and nothing that could close the quote
/// or start a new word.
#[must_use]
pub fn plain_id(id: &str) -> bool {
    id.len() <= 256
        && id.split_once(':').is_some_and(|(source, rest)| {
            (Source::ALL.iter().any(|s| s.word() == source) || source == crate::distro_pins::ENTRY)
                && !rest.is_empty()
        })
        && id
            .chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\\' | '$' | '`'))
}

/// `nix eval --json <flake>#personFingerprints`.
#[must_use]
pub fn fingerprints_args(flake: &str) -> Vec<String> {
    let mut args = vec![
        "eval".to_owned(),
        "--json".to_owned(),
        format!("{flake}#{FINGERPRINTS}"),
    ];
    args.extend(flake_flags());
    args
}

/// `nix eval --raw <flake>#personEntries."<id>".drvPath`, for a [`plain_id`].
#[must_use]
pub fn eval_args(flake: &str, id: &str) -> Vec<String> {
    let mut args = vec![
        "eval".to_owned(),
        "--raw".to_owned(),
        format!("{flake}#{ENTRIES}.\"{id}\".drvPath"),
    ];
    args.extend(flake_flags());
    args
}

/// `nix build --no-link <flake>#personEntries."<id>"`, for a [`plain_id`].
#[must_use]
pub fn build_args(flake: &str, id: &str) -> Vec<String> {
    let mut args = vec![
        "build".to_owned(),
        "--no-link".to_owned(),
        format!("{flake}#{ENTRIES}.\"{id}\""),
    ];
    args.extend(flake_flags());
    args
}

/// `nix store prefetch-file --json --hash-type <algo> <url>` for a pin, the
/// algorithm read off its SRI hash; `None` for a hash that is not SRI.
#[must_use]
pub fn prefetch_args(url: &str, hash: &str) -> Option<Vec<String>> {
    let algo = hash.split_once('-').map(|(a, _)| a)?;
    if !matches!(algo, "sha256" | "sha512") {
        return None;
    }
    Some(
        [
            "store",
            "prefetch-file",
            "--json",
            "--hash-type",
            algo,
            url,
            "--extra-experimental-features",
            "nix-command",
        ]
        .map(str::to_owned)
        .to_vec(),
    )
}

/// The fingerprints `nix eval --json` printed, every name a [`plain_id`] and
/// every fingerprint a short token.
///
/// # Errors
///
/// A sentence, for the journal, when the JSON is not an object of strings or
/// a name or a fingerprint is not plain — the baseline is then refused.
pub fn fingerprints_of(json: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let all: BTreeMap<String, String> = serde_json::from_slice(json)
        .map_err(|e| format!("{FINGERPRINTS} is not an object of strings: {e}"))?;
    for (id, fingerprint) in &all {
        if !plain_id(id) {
            return Err(format!("{FINGERPRINTS} names {id:?}, not a plain entry id"));
        }
        if !check::token(fingerprint) {
            return Err(format!("the fingerprint of {id} is not a plain token"));
        }
    }
    Ok(all)
}

/// The hash `nix store prefetch-file --json` printed.
#[must_use]
pub fn prefetched(json: &[u8]) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Prefetched {
        hash: String,
    }
    serde_json::from_slice::<Prefetched>(json)
        .ok()
        .map(|p| p.hash)
}

/// The pinned download of an entry — address and SRI hash — if it has one.
#[must_use]
pub fn pin_of(entry: &Entry) -> Option<(&str, &str)> {
    match entry {
        Entry::AppImage(a) => Some((&a.pin.url, &a.pin.hash)),
        Entry::Android(a) => Some((&a.pin.url, &a.pin.hash)),
        Entry::Nix(_) | Entry::Flatpak(_) | Entry::Web(_) => None,
    }
}

/// What fetching an entry's pin said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetch {
    /// It could not be fetched.
    Failed,
    /// It was fetched, and its hash is the pin's.
    Matches,
    /// It was fetched, and its hash is another.
    Differs,
}

impl Fetch {
    /// What a prefetch that printed `got` (`None`: it failed) says of the
    /// pin's `hash`.
    #[must_use]
    pub fn of(got: Option<&str>, hash: &str) -> Self {
        match got {
            None => Self::Failed,
            Some(h) if h == hash => Self::Matches,
            Some(_) => Self::Differs,
        }
    }
}

/// What trying an entry came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tried {
    /// It evaluated and built.
    Built,
    /// Its evaluation failed.
    EvalFailed,
    /// It evaluated and did not build; what fetching its pin said, if it has
    /// one.
    BuildFailed {
        /// The pin's fetch, when there is a pin.
        fetch: Option<Fetch>,
    },
}

/// Why an entry is held, if it is.
#[must_use]
pub const fn reason(tried: Tried) -> Option<HeldWhy> {
    match tried {
        Tried::Built => None,
        Tried::EvalFailed => Some(HeldWhy::EvalFailed),
        Tried::BuildFailed {
            fetch: Some(Fetch::Failed),
        } => Some(HeldWhy::FetchFailed),
        Tried::BuildFailed {
            fetch: Some(Fetch::Differs),
        } => Some(HeldWhy::UpstreamChanged),
        Tried::BuildFailed {
            fetch: None | Some(Fetch::Matches),
        } => Some(HeldWhy::BuildFailed),
    }
}

/// The held entries after a round: `failed` is `(id, fingerprint, why)` of
/// every entry that failed. An entry held before for the same reason keeps
/// its `since`; every other starts now. An entry that is not in `failed` is
/// not held any more.
#[must_use]
pub fn held_after(before: &Held, failed: &[(String, String, HeldWhy)], now: &Stamp) -> Held {
    let mut held: Vec<HeldEntry> = failed
        .iter()
        .map(|(id, fingerprint, reason)| HeldEntry {
            id: id.clone(),
            fingerprint: fingerprint.clone(),
            reason: *reason,
            since: before
                .held
                .iter()
                .find(|h| &h.id == id && h.reason == *reason)
                .map_or_else(|| now.clone(), |h| h.since.clone()),
        })
        .collect();
    held.sort_by(|a, b| a.id.cmp(&b.id));
    held.dedup_by(|a, b| a.id == b.id);
    Held {
        format: crate::FORMAT,
        held,
    }
}

/// The verified record after a round: every followed distro AppImage that
/// BUILT now (`built`, each stamped now), its `since` kept when it built at
/// the same fingerprint before — and every other entry of `before` as it
/// was: a followed AppImage held or not tried keeps the last pin that built.
#[must_use]
pub fn verified_after(before: &Verified, built: &[VerifiedEntry]) -> Verified {
    let mut verified: Vec<VerifiedEntry> = before
        .verified
        .iter()
        .filter(|v| !built.iter().any(|b| b.id == v.id))
        .cloned()
        .collect();
    for b in built {
        let since = before
            .verified
            .iter()
            .find(|v| v.id == b.id && v.fingerprint == b.fingerprint)
            .map_or_else(|| b.since.clone(), |v| v.since.clone());
        verified.push(VerifiedEntry { since, ..b.clone() });
    }
    verified.sort_by(|a, b| a.id.cmp(&b.id));
    verified.dedup_by(|a, b| a.id == b.id);
    Verified {
        format: crate::FORMAT,
        verified,
    }
}

/// What a REHEARSAL can say of the entries whose evaluation passed: it
/// builds nothing, so it cannot say they build. An entry `before` held at the
/// fingerprint it still has, for a reason only a build finds (`build-failed`,
/// `fetch-failed`, `upstream-changed`), is STILL held — the rebuild goes on
/// skipping it while `held.json` says so — and is returned as
/// `(id, fingerprint, why)`, to be carried with its `since`; every other one
/// that evaluated is untried. A held `eval-failed` was tried again, so it is
/// not carried: the rehearsal's own evaluation answers for it.
#[must_use]
pub fn still_held(
    before: &Held,
    fingerprints: &BTreeMap<String, String>,
    evaluated: &[String],
) -> Vec<(String, String, HeldWhy)> {
    before
        .held
        .iter()
        .filter(|h| h.reason != HeldWhy::EvalFailed)
        .filter(|h| evaluated.contains(&h.id))
        .filter(|h| fingerprints.get(&h.id) == Some(&h.fingerprint))
        .map(|h| (h.id.clone(), h.fingerprint.clone(), h.reason))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn the_commands_name_one_entry_between_quotes() {
        let f = flake("/etc/nixos");
        assert_eq!(f, "path:/etc/nixos");
        assert_eq!(
            eval_args(&f, "nix:ripgrep")[..3],
            [
                "eval",
                "--raw",
                "path:/etc/nixos#personEntries.\"nix:ripgrep\".drvPath"
            ]
        );
        assert_eq!(
            build_args(&f, "appimage:obsidian")[..3],
            [
                "build",
                "--no-link",
                "path:/etc/nixos#personEntries.\"appimage:obsidian\""
            ]
        );
        for args in [
            fingerprints_args(&f),
            eval_args(&f, "nix:x"),
            build_args(&f, "nix:x"),
        ] {
            assert!(
                args.iter().any(|a| a == "--no-update-lock-file"),
                "{args:?}"
            );
        }
        assert_eq!(
            prefetch_args("https://e.org/a", "sha512-abc").unwrap()[..6],
            [
                "store",
                "prefetch-file",
                "--json",
                "--hash-type",
                "sha512",
                "https://e.org/a"
            ]
        );
        assert_eq!(prefetch_args("https://e.org/a", "md5-abc"), None);
    }

    /// FAILS IF: a name that could close the quote — and so name another
    /// attribute, or a second argument — is put in a command.
    #[test]
    fn only_plain_ids_reach_a_command() {
        for good in [
            "nix:ripgrep",
            "nix:python3Packages.requests",
            "android:org.fdroid.fdroid",
            "nix:foo'",
            "distro-appimage:cadcraft",
        ] {
            assert!(plain_id(good), "{good}");
        }
        for bad in [
            "ripgrep",
            "nix:",
            "snap:x",
            "distro-appimage:",
            "nix:a\".drvPath",
            "nix:a b",
            "nix:a\\b",
            "nix:$(id)",
            "nix:\n",
        ] {
            assert!(!plain_id(bad), "{bad:?}");
        }
        assert!(fingerprints_of(br#"{"nix:a": "abc"}"#).is_ok());
        assert!(fingerprints_of(br#"{"nix:a\"": "abc"}"#).is_err());
        assert!(fingerprints_of(br#"{"nix:a": "a b"}"#).is_err());
        assert!(fingerprints_of(br#"["nix:a"]"#).is_err());
    }

    /// FAILS IF: the four reasons are told apart by anything but the exit
    /// statuses and the prefetch's hash.
    #[test]
    fn a_failure_is_classified_by_what_answered() {
        let pin = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        assert_eq!(reason(Tried::Built), None);
        assert_eq!(reason(Tried::EvalFailed), Some(HeldWhy::EvalFailed));
        assert_eq!(
            reason(Tried::BuildFailed { fetch: None }),
            Some(HeldWhy::BuildFailed)
        );
        let fetched = |got| {
            reason(Tried::BuildFailed {
                fetch: Some(Fetch::of(got, pin)),
            })
        };
        assert_eq!(fetched(None), Some(HeldWhy::FetchFailed));
        assert_eq!(fetched(Some(pin)), Some(HeldWhy::BuildFailed));
        assert_eq!(
            fetched(Some("sha256-mOpuTyFvL7S2n/+bOkSELDhobKaF8/VdxIxdP7EQe+Q=")),
            Some(HeldWhy::UpstreamChanged)
        );
        assert_eq!(
            prefetched(br#"{"hash":"sha256-x=","storePath":"/nix/store/a-b"}"#).as_deref(),
            Some("sha256-x=")
        );
        assert_eq!(prefetched(b"not json"), None);
    }

    /// FAILS IF: a rehearsal, which builds nothing, lets go of an entry the
    /// rebuild will still skip — or keeps one whose fingerprint moved, which
    /// the rebuild will try again.
    #[test]
    fn a_rehearsal_carries_what_only_a_build_could_let_go() {
        let old = Stamp::parse("2026-10-01T00:00:00Z").unwrap();
        let held = |id: &str, fingerprint: &str, reason| HeldEntry {
            id: id.to_owned(),
            fingerprint: fingerprint.to_owned(),
            reason,
            since: old.clone(),
        };
        let before = Held {
            format: crate::FORMAT,
            held: vec![
                held("appimage:gone", "f1", HeldWhy::FetchFailed),
                held("nix:moved", "f2", HeldWhy::BuildFailed),
                held("nix:was-eval", "f3", HeldWhy::EvalFailed),
                held("nix:now-eval", "f4", HeldWhy::BuildFailed),
            ],
        };
        let fingerprints: BTreeMap<String, String> = [
            ("appimage:gone", "f1"),
            ("nix:moved", "f9"),
            ("nix:was-eval", "f3"),
            ("nix:now-eval", "f4"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect();
        let evaluated = ["appimage:gone", "nix:moved", "nix:was-eval"].map(str::to_owned);
        let carried = still_held(&before, &fingerprints, &evaluated);
        assert_eq!(
            carried,
            [(
                "appimage:gone".to_owned(),
                "f1".to_owned(),
                HeldWhy::FetchFailed
            )]
        );
        let now = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        let after = held_after(&before, &carried, &now);
        assert_eq!(after.held[0].since, old, "carried with its since");
    }

    /// FAILS IF: a followed AppImage that built is not recorded at the
    /// fingerprint it built at, one held or not tried loses the last pin
    /// that built, or `since` moves while the fingerprint stays.
    #[test]
    fn what_built_is_recorded_and_the_last_verified_pin_stays() {
        let old = Stamp::parse("2026-10-01T00:00:00Z").unwrap();
        let now = Stamp::parse("2026-10-20T12:00:00Z").unwrap();
        let entry = |id: &str, fp: &str, version: &str, since: &Stamp| VerifiedEntry {
            id: crate::distro_pins::entry_id(id),
            fingerprint: fp.to_owned(),
            url: format!(
                "https://github.com/storytold/{id}/releases/download/v{version}/{id}-{version}-linux-x86_64.AppImage"
            ),
            hash: format!("sha256-{}=", "A".repeat(43)),
            version: version.to_owned(),
            since: since.clone(),
        };
        let before = Verified {
            format: crate::FORMAT,
            verified: vec![
                entry("cadcraft", "f1", "0.9.0", &old),
                entry("figcraft", "f2", "0.7.2", &old),
                entry("pdfcraft", "f3", "0.6.2", &old),
            ],
        };
        let after = verified_after(
            &before,
            &[
                entry("cadcraft", "f1", "0.9.0", &now),
                entry("figcraft", "f9", "0.7.3", &now),
                entry("photocraft", "f4", "0.12.1", &now),
            ],
        );
        assert!(after.validate().is_ok(), "{after:?}");
        let by = |id: &str| {
            after
                .verified
                .iter()
                .find(|v| v.id == crate::distro_pins::entry_id(id))
                .unwrap()
                .clone()
        };
        assert_eq!(
            by("cadcraft").since,
            old,
            "the same fingerprint keeps its since"
        );
        assert_eq!(
            (by("figcraft").version.as_str(), &by("figcraft").since),
            ("0.7.3", &now)
        );
        assert_eq!(
            by("pdfcraft").version,
            "0.6.2",
            "a held one keeps the last that built"
        );
        assert_eq!(by("photocraft").fingerprint, "f4");
        assert_eq!(after.verified.len(), 4);
    }

    #[test]
    fn a_held_entry_keeps_its_since_while_it_fails_the_same_way() {
        let old = Stamp::parse("2026-10-01T00:00:00Z").unwrap();
        let now = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        let before = Held {
            format: crate::FORMAT,
            held: vec![
                HeldEntry {
                    id: "nix:a".to_owned(),
                    fingerprint: "f1".to_owned(),
                    reason: HeldWhy::EvalFailed,
                    since: old.clone(),
                },
                HeldEntry {
                    id: "nix:fixed".to_owned(),
                    fingerprint: "f2".to_owned(),
                    reason: HeldWhy::BuildFailed,
                    since: old.clone(),
                },
            ],
        };
        let after = held_after(
            &before,
            &[
                ("nix:b".to_owned(), "f3".to_owned(), HeldWhy::BuildFailed),
                ("nix:a".to_owned(), "f9".to_owned(), HeldWhy::EvalFailed),
            ],
            &now,
        );
        assert!(after.validate().is_ok());
        let ids: Vec<_> = after.held.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["nix:a", "nix:b"], "sorted, and the fixed one is gone");
        assert_eq!(after.held[0].since, old);
        assert_eq!(
            after.held[0].fingerprint, "f9",
            "the fingerprint Nix gave now"
        );
        assert_eq!(after.held[1].since, now);
    }
}
