//! Approving a person brain module's root part — the request the person
//! stages, and the decision root takes on it.
//!
//! # The gesture
//!
//! A person module's `module.nix` reaches root: it is a NixOS module, built
//! into the machine. So it enters a build only when its hash is in
//! `local/approved.json`, and only the sudo door writes that file:
//!
//! 1. The machine reports the module as awaiting approval, with the hash
//!    `nix/person-hash.nix` computed of the ROOT copy
//!    (`person-applied.json`, `brain.awaiting-approval`).
//! 2. The person stages `{format, id, hash}` — the hash they were shown — as
//!    [`REQUEST`] in their approval folder ([`crate::root::APPROVE`]) and
//!    passes the sudo door with `person-module-approve <id> <hash>`: the same
//!    two, as the door's own arguments.
//! 3. After the password, root's helper writes those two into root's own
//!    folder ([`crate::root::APPROVE_PIN`], [`pin_text`]) and starts ONE
//!    fixed unit ([`crate::root::APPROVE_UNIT`]). It consumes the pin and the
//!    request — refusing when they disagree: the person's folder is any of
//!    their processes', and what is approved is what the door was given —
//!    asks `nix/person-hash.nix` (the single implementation, through `nix
//!    eval`) for the hash of the root copy NOW, and records the approval only
//!    when it is the pinned one ([`decide`]).
//! 4. Whatever happened, it says so in [`Verdict`]
//!    ([`crate::root::APPROVE_VERDICT`]): the door's own line only says that
//!    the unit failed, and "failed" is not a reason — "the module changed
//!    since you saw it" ([`Denied::Changed`]) is.
//!
//! The hash the person staged is never trusted: it is only compared. A
//! module changed between being shown and being approved is refused
//! ([`Denied::Changed`]), and one edited after its approval stops matching
//! and is no longer imported.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::local::{Approval, Approved};
use crate::{Document, Refusal, Stamp, Why, check};

/// The request's file name, inside the approval folder.
pub const REQUEST: &str = "request.json";

/// The pin root's helper writes ([`crate::root::APPROVE_PIN`]): the module's
/// id and the hash, one line each — what the sudo door was given. Pure.
#[must_use]
pub fn pin_text(id: &str, hash: &str) -> String {
    format!("{id}\n{hash}\n")
}

/// The request a pin's text names, validated; `None` for anything else (a
/// line more or less, a word that is no id or no hash). Pure.
#[must_use]
pub fn of_pin(text: &str) -> Option<Request> {
    let mut lines = text.strip_suffix('\n')?.split('\n');
    let (id, hash) = (lines.next()?, lines.next()?);
    if lines.next().is_some() {
        return None;
    }
    let request = Request::new(id, hash);
    request.validate().ok().map(|()| request)
}

/// The most a request may weigh: an id and a hash.
pub const REQUEST_MOST_BYTES: u64 = 4 << 10;

/// Stages `request` in `folder` — the person's approval folder
/// ([`crate::root::APPROVE`]) — whole: written under another name, `0600`
/// and never through a link (`create_new` makes the file or refuses), then
/// renamed over the one before, so the unit reads a whole request or none.
///
/// THE ONE WRITER OF A REQUEST: the person's `ronoctua-person
/// request-approval` and the update's page (`ronoctua-shell-update`) both
/// stage through it. The library's one write, and with `std` alone, so a
/// reader that links the data and nothing else still links nothing more.
///
/// # Errors
///
/// A request that is not one, a folder that is a link or not a folder, one
/// others may write, one that is not the caller's (the owner of the file it
/// just made), or a write that failed.
pub fn stage(folder: &std::path::Path, request: &Request) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind, Write as _};
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let refused = |why: String| Error::new(ErrorKind::PermissionDenied, why);
    request
        .validate()
        .map_err(|r| Error::new(ErrorKind::InvalidInput, format!("the request: {r}")))?;
    let at = std::fs::symlink_metadata(folder)?;
    if !at.is_dir() {
        return Err(refused(format!("{} is not a folder", folder.display())));
    }
    if at.mode() & 0o022 != 0 {
        return Err(refused(format!("others may write {}", folder.display())));
    }
    let draft = folder.join(format!(".{REQUEST}.{}", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&draft)
        .and_then(|mut f| {
            if f.metadata()?.uid() != at.uid() {
                return Err(refused(format!("{} is not yours", folder.display())));
            }
            f.write_all(crate::to_json(request).as_bytes())
        })
        .and_then(|()| std::fs::rename(&draft, folder.join(REQUEST)));
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// What the person asks root to approve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Request {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The module (`my-…`).
    pub id: String,
    /// The hash the person was shown.
    pub hash: String,
}

impl Request {
    /// A request for `id` at `hash`.
    #[must_use]
    pub fn new(id: impl Into<String>, hash: impl Into<String>) -> Self {
        Self {
            format: crate::FORMAT,
            id: id.into(),
            hash: hash.into(),
        }
    }
}

impl Document for Request {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if !check::person_module(&self.id) {
            return Err(Refusal::new(Why::Invalid, "/id", self.id.as_str()));
        }
        if !check::token(&self.hash) {
            return Err(Refusal::new(Why::Invalid, "/hash", self.hash.as_str()));
        }
        Ok(())
    }
}

/// Why an approval was not recorded — every way the approval's unit can
/// end without one, so the verdict says which (the sudo door's own line only
/// says that the unit failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum Denied {
    /// No request was staged.
    NoRequest,
    /// The request was not one: a link, too large, not valid JSON or not
    /// valid values.
    InvalidRequest,
    /// The request, or its folder, is not the person's.
    NotOwned,
    /// Another of root's programs held the lock longer than the wait.
    Busy,
    /// The machine's copy has no such module.
    NoSuchModule,
    /// `nix/person-hash.nix` could not hash the machine's copy.
    Unhashable,
    /// The module is not what the person was shown: its hash now differs.
    Changed,
    /// `local/approved.json` could not be written.
    Unwritable,
}

impl Denied {
    /// Every one, in the order the names are written.
    pub const ALL: [Self; 8] = [
        Self::NoRequest,
        Self::InvalidRequest,
        Self::NotOwned,
        Self::Busy,
        Self::NoSuchModule,
        Self::Unhashable,
        Self::Changed,
        Self::Unwritable,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoRequest => "no-request",
            Self::InvalidRequest => "invalid-request",
            Self::NotOwned => "not-owned",
            Self::Busy => "busy",
            Self::NoSuchModule => "no-such-module",
            Self::Unhashable => "unhashable",
            Self::Changed => "changed",
            Self::Unwritable => "unwritable",
        }
    }

    /// The reason of this name; `None` for any other.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.id() == id)
    }
}

impl From<Denied> for String {
    fn from(denied: Denied) -> Self {
        denied.id().to_owned()
    }
}

impl TryFrom<String> for Denied {
    type Error = String;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::of_id(&id).ok_or_else(|| format!("not a reason: {id:?}"))
    }
}

/// What an approval came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Answer {
    /// Recorded in `local/approved.json`.
    Approved,
    /// Not recorded, for [`Verdict::denied`].
    Refused,
}

/// What the approval's unit says it did —
/// `/run/ronoctua-person/approve-verdict.json` ([`crate::root::APPROVE_VERDICT`]),
/// root's and readable by the person, written on every way out. The request
/// is spent whatever the answer: a refusal is said here, and the person
/// stages again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Verdict {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// When.
    pub at: Stamp,
    /// What it came to.
    pub outcome: Answer,
    /// The module the request named, when a request was read.
    pub id: Option<String>,
    /// The hash the request named (the one the person was shown).
    pub hash: Option<String>,
    /// The hash root computed of the machine's copy, when it did.
    pub computed: Option<String>,
    /// Why it was refused, when it was.
    pub denied: Option<Denied>,
}

impl Verdict {
    /// The verdict at `at` of an approval of `request` (when one was read)
    /// that computed `computed` and came to `denied` (`None`: approved).
    #[must_use]
    pub fn of(
        at: Stamp,
        request: Option<&Request>,
        computed: Option<String>,
        denied: Option<Denied>,
    ) -> Self {
        Self {
            format: crate::FORMAT,
            at,
            outcome: if denied.is_some() {
                Answer::Refused
            } else {
                Answer::Approved
            },
            id: request.map(|r| r.id.clone()),
            hash: request.map(|r| r.hash.clone()),
            computed,
            denied,
        }
    }
}

impl Document for Verdict {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if (self.outcome == Answer::Refused) != self.denied.is_some() {
            return Err(Refusal::new(
                Why::Invalid,
                "/denied",
                "a reason goes with the outcome refused, and only with it",
            ));
        }
        if self.outcome == Answer::Approved
            && (self.id.is_none() || self.hash.is_none() || self.computed != self.hash)
        {
            return Err(Refusal::new(
                Why::Invalid,
                "/computed",
                "an approval names its module, at the hash root computed",
            ));
        }
        if let Some(id) = self.id.as_deref().filter(|id| !check::person_module(id)) {
            return Err(Refusal::new(Why::Invalid, "/id", id));
        }
        for (at, hash) in [("/hash", &self.hash), ("/computed", &self.computed)] {
            if let Some(hash) = hash.as_deref().filter(|h| !check::token(h)) {
                return Err(Refusal::new(Why::Invalid, at, hash));
            }
        }
        Ok(())
    }
}

/// The approvals after `request`, given the modules the machine's copy holds
/// (`present`) and the hash root computed of it now (`computed`).
///
/// Approvals of modules the copy no longer holds are dropped on the way: a
/// module that comes back is approved again.
///
/// # Errors
///
/// [`Denied::NoSuchModule`] when the copy lacks the module;
/// [`Denied::Changed`] when `computed` is not the hash the person saw.
pub fn decide(
    approved: &Approved,
    present: &BTreeSet<String>,
    request: &Request,
    computed: &str,
    now: Stamp,
) -> Result<Approved, Denied> {
    if !present.contains(&request.id) {
        return Err(Denied::NoSuchModule);
    }
    if computed != request.hash {
        return Err(Denied::Changed);
    }
    let mut after = approved.clone();
    after.modules.retain(|id, _| present.contains(id));
    after.modules.insert(
        request.id.clone(),
        Approval {
            hash: request.hash.clone(),
            approved_at: now,
        },
    );
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Stamp {
        Stamp::parse("2026-10-05T12:00:00Z").unwrap()
    }

    fn present(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|&s| s.to_owned()).collect()
    }

    /// FAILS IF: a module is approved at a hash root did not compute, a
    /// module the machine does not hold is approved, or an approval of a
    /// module that left survives it.
    #[test]
    fn only_what_root_computed_is_approved() {
        let mut before = Approved::default();
        before.modules.insert(
            "my-gone".to_owned(),
            Approval {
                hash: "sha256-old".to_owned(),
                approved_at: now(),
            },
        );
        let request = Request::new("my-mod", "sha256-seen");
        let here = present(&["my-mod"]);
        assert_eq!(
            decide(&before, &here, &request, "sha256-other", now()),
            Err(Denied::Changed)
        );
        assert_eq!(
            decide(&before, &present(&["my-x"]), &request, "sha256-seen", now()),
            Err(Denied::NoSuchModule)
        );
        let after = decide(&before, &here, &request, "sha256-seen", now()).unwrap();
        assert_eq!(after.modules.len(), 1, "the gone module's approval left");
        assert_eq!(after.modules["my-mod"].hash, "sha256-seen");
        assert!(after.validate().is_ok());
    }

    #[test]
    fn a_request_names_a_person_module_and_a_hash() {
        let good = crate::to_json(&Request::new("my-mod", "sha256-abc="));
        assert!(crate::parse::<Request>(good.as_bytes()).is_ok());
        for bad in [
            r#"{"format":1,"id":"made-up","hash":"sha256-x"}"#,
            r#"{"format":1,"id":"my-mod","hash":"a b"}"#,
            r#"{"format":1,"id":"my-mod","hash":"x","extra":1}"#,
        ] {
            assert!(crate::parse::<Request>(bad.as_bytes()).is_err(), "{bad}");
        }
        assert_eq!(Denied::Changed.id(), "changed");
        assert_eq!(Denied::NoSuchModule.id(), "no-such-module");
    }

    /// FAILS IF: a reason has two names, or a verdict says "approved" of a
    /// hash root did not compute, or "refused" without saying why.
    #[test]
    fn a_verdict_says_why_and_reads_back() {
        for d in Denied::ALL {
            assert_eq!(Denied::of_id(d.id()), Some(d));
            assert_eq!(
                serde_json::to_string(&d).unwrap(),
                format!("\"{}\"", d.id())
            );
        }
        let request = Request::new("my-mod", "sha256-seen");
        for v in [
            Verdict::of(now(), Some(&request), Some("sha256-seen".to_owned()), None),
            Verdict::of(
                now(),
                Some(&request),
                Some("sha256-now".to_owned()),
                Some(Denied::Changed),
            ),
            Verdict::of(now(), None, None, Some(Denied::NoRequest)),
        ] {
            let text = crate::to_json(&v);
            assert_eq!(crate::parse::<Verdict>(text.as_bytes()).unwrap(), v);
        }
        for lie in [
            Verdict::of(now(), Some(&request), Some("sha256-now".to_owned()), None),
            Verdict::of(now(), None, None, None),
        ] {
            let text = crate::to_json(&lie);
            assert_eq!(
                crate::parse::<Verdict>(text.as_bytes()).unwrap_err().why,
                Why::Invalid,
                "{text}"
            );
        }
    }

    /// FAILS IF: the pin root's helper writes does not read back as the
    /// request the door was given, or a pin with a line more, a line less,
    /// an id that is no person module's or a hash with a quote in it is
    /// taken for one.
    #[test]
    fn a_pin_is_the_two_words_the_door_was_given() {
        let hash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        assert_eq!(
            of_pin(&pin_text("my-tea", hash)),
            Some(Request::new("my-tea", hash))
        );
        for wrong in [
            String::new(),
            "my-tea\n".to_owned(),
            format!("my-tea\n{hash}"),
            format!("my-tea\n{hash}\nmore\n"),
            format!("tea\n{hash}\n"),
            "my-tea\nsha\"256\n".to_owned(),
        ] {
            assert_eq!(of_pin(&wrong), None, "{wrong:?}");
        }
    }

    /// FAILS IF: a request is not staged whole as root's unit reads it —
    /// the module and the hash, `0600`, no draft left — or a link where it
    /// goes is written through instead of replaced, or a folder that is a
    /// link, not a folder or writable by others is staged into, or a request
    /// that is not one is written.
    #[test]
    fn a_request_is_staged_whole_and_never_through_a_link() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = std::env::temp_dir().join(format!("ronoctua-stage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = root.join("approve");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
        let hash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        let request = Request::new("my-tea", hash);
        stage(&folder, &request).unwrap();
        let at = folder.join(REQUEST);
        let staged: Request = crate::parse(&std::fs::read(&at).unwrap()).unwrap();
        assert_eq!(staged, request);
        let mode = std::fs::metadata(&at).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        // A LINK where the request goes is replaced, never followed.
        std::fs::remove_file(&at).unwrap();
        std::fs::write(root.join("elsewhere"), b"untouched").unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), &at).unwrap();
        stage(&folder, &request).unwrap();
        assert_eq!(std::fs::read(root.join("elsewhere")).unwrap(), b"untouched");
        assert!(!std::fs::symlink_metadata(&at).unwrap().is_symlink());
        let left: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            left,
            [std::ffi::OsString::from(REQUEST)],
            "a draft was left"
        );
        // NOWHERE ELSE: no folder, a link to one, one others may write.
        assert!(stage(&root.join("nowhere"), &request).is_err());
        std::os::unix::fs::symlink(&folder, root.join("linked")).unwrap();
        assert!(stage(&root.join("linked"), &request).is_err());
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(
            stage(&folder, &request).is_err(),
            "a folder others may write"
        );
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
        // AND ONLY A REQUEST: an id that is no person module's.
        assert!(stage(&folder, &Request::new("tea", hash)).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
