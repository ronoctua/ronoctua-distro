//! THE PERSON'S BRAIN MODULES THAT WAIT FOR AN APPROVAL — on the update's
//! records, in both doors (owner, 2026-11-01).
//!
//! # What waits, and why here
//!
//! A brain module of the person's (`my-…`) with a NixOS half reaches root,
//! so a machine compiles it only once `local/approved.json` names it at the
//! hash `nix/person-hash.nix` computes of the machine's copy now
//! (`nix/machine.nix`). The machine's last build says which ones it left out,
//! each with that hash (`person-applied.json`, `brain.awaiting-approval`),
//! and an approval enters the machine at the next update: so the update's
//! page is where the person sees what waits — a sentence with the hash, and
//! the row under it that asks — and where an approval already recorded is
//! said to enter with the next update.
//!
//! # Asked twice, the no first
//!
//! As the machine's own gestures (`crate::machine_gesture`): a click on the
//! row asks there, the no first and the cursor born on it; a yes stages the
//! request in the person's approval folder (`ronoctua_person::root::APPROVE`)
//! through the one writer of it, the person's own tool's
//! (`ronoctua_person::approve::stage`),
//! and passes the sudo door with `person-module-approve <id> <hash>` —
//! through the door's gate, on a thread, never the loop. Root's helper pins
//! what the door was given and starts `ronoctua-person-approve.service`,
//! which approves only at the hash it computes then; why it refused is its
//! verdict (`ronoctua_person::root::APPROVE_VERDICT`), read back here and
//! said in the door that asked.
//!
//! # The hash shown is never trusted
//!
//! It is what the machine reported; root holds the request to the door's
//! words and both to what it computes of its own copy, and a module changed
//! since is refused as `changed`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ronoctua_person::approve::{Answer, Denied, Request, Verdict};
use ronoctua_person::local::Approved;
use ronoctua_person::reports::PersonApplied;
use ronoctua_security::verb::RootVerb;
use ronoctua_shell_kit::aim::Aim;

use crate::key;

/// The command that asks to approve a module that waits — the one named, or
/// the first — by name (`LEADER :`).
pub const APPROVE: &str = "machine-approve-module";

/// One of the person's brain modules the machine left out, waiting: its id
/// and the hash an approval must name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// The module (`my-…`).
    pub id: String,
    /// What `nix/person-hash.nix` computed of the machine's copy.
    pub hash: String,
}

/// What the machine says of the person's modules that wait.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Readings {
    /// The ones the last build left out, in the report's order.
    pub waiting: Vec<Waiting>,
    /// The approvals root recorded, by id: the hash each names.
    pub approved: BTreeMap<String, String>,
}

impl Readings {
    /// What `places` say: a report or an approvals file that does not read
    /// is no module waiting and no approval — the machine's own builds fail
    /// closed the same way.
    #[must_use]
    pub fn read(places: &Places) -> Self {
        let waiting = std::fs::read(&places.applied)
            .ok()
            .and_then(|b| ronoctua_person::parse::<PersonApplied>(&b).ok())
            .map(|a| {
                a.brain
                    .awaiting_approval
                    .into_iter()
                    .map(|w| Waiting {
                        id: w.id,
                        hash: w.hash,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let approved = std::fs::read(&places.approved)
            .ok()
            .and_then(|b| ronoctua_person::parse::<Approved>(&b).ok())
            .map(|a| {
                a.modules
                    .into_iter()
                    .map(|(id, approval)| (id, approval.hash))
                    .collect()
            })
            .unwrap_or_default();
        Self { waiting, approved }
    }

    /// Whether `w` is approved at the hash the machine reported: it enters
    /// with the next update, and nothing is left to ask.
    #[must_use]
    pub fn approved_as_shown(&self, w: &Waiting) -> bool {
        self.approved.get(&w.id) == Some(&w.hash)
    }

    /// The modules still to approve: waiting, and not approved as shown.
    pub fn to_approve(&self) -> impl Iterator<Item = &Waiting> {
        self.waiting.iter().filter(|w| !self.approved_as_shown(w))
    }
}

/// Where the records are read and the request staged — this machine's, or
/// the footer camera's room while it photographs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The machine's report (`/etc/ronoctua/person-applied.json`).
    pub applied: PathBuf,
    /// Root's approvals (`<machine dir>/local/approved.json`).
    pub approved: PathBuf,
    /// What the approval's unit said it did.
    pub verdict: PathBuf,
    /// The person's approval folder, where the request is staged.
    pub requests: PathBuf,
}

impl Places {
    /// This machine's.
    #[must_use]
    pub fn here() -> Self {
        let machine = |p: &str| match crate::photo::room() {
            Some(room) => room.join(p.trim_start_matches('/')),
            None => PathBuf::from(p),
        };
        let at = crate::update_window::record_places();
        Self {
            applied: machine(&format!(
                "/etc/ronoctua/{}",
                ronoctua_person::names::PERSON_APPLIED
            )),
            approved: at.machine_dir.join(ronoctua_person::names::APPROVED),
            verdict: machine(ronoctua_person::root::APPROVE_VERDICT),
            requests: machine(ronoctua_person::root::APPROVE),
        }
    }

    /// When each file read last moved — what a door compares to read again
    /// only when one did.
    #[must_use]
    pub fn stamp(&self) -> Vec<Option<std::time::SystemTime>> {
        [&self.applied, &self.approved, &self.verdict]
            .into_iter()
            .map(|p| mtime(p))
            .collect()
    }
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// The question an approval asks on the page, before the sudo bar does: its
/// `no` first, and the cursor born there — the screen's, so the first click
/// on it only picks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// The module asked about.
    pub of: Waiting,
    /// Which answer the cursor is on (`true` is yes), and whether a person
    /// put it there.
    aim: Aim<bool>,
}

impl Question {
    /// The question about `of`, its cursor born on `no`.
    #[must_use]
    pub const fn of(of: Waiting) -> Self {
        Self {
            of,
            aim: Aim::born(false),
        }
    }

    /// The answer the cursor is on — `true` is yes.
    #[must_use]
    pub fn yes(&self) -> bool {
        self.aim.on().copied().unwrap_or(false)
    }

    /// A key put the cursor on an answer.
    pub fn moved(&mut self, yes: bool) {
        self.aim.moved(yes);
    }

    /// A click on an answer: `true` when it answers — the person's own pick,
    /// clicked again.
    pub fn clicked(&mut self, yes: bool) -> bool {
        self.aim.clicked(yes)
    }
}

/// What came of an approval asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Came {
    /// Root recorded it: the module enters with the next update.
    Approved(String),
    /// Root refused it, for its own reason.
    Refused(String, Denied),
    /// The request could not be staged where root's unit reads it.
    Unstaged(String),
    /// The door refused it: cancelled, a wrong password, no footer — or a
    /// unit that failed with no verdict of its own.
    Door(ronoctua_security::Why),
}

/// Runs the approval of `w` through the door's `gesture`, blocking — a
/// door's thread, never its loop: the request staged, the sudo door passed,
/// and a refusal read back from the unit's verdict when it is this
/// request's and new.
pub fn run(w: &Waiting, places: &Places, mut gesture: crate::sudo::Gesture) -> Came {
    // STAGED BY THE ONE WRITER OF A REQUEST, the person's own tool's
    // (`ronoctua_person::approve::stage`).
    if let Err(e) = ronoctua_person::approve::stage(&places.requests, &Request::new(&w.id, &w.hash))
    {
        tracing::warn!(error = %e, module = %w.id, "the approval's request was not staged");
        return Came::Unstaged(w.id.clone());
    }
    let before = mtime(&places.verdict);
    let verb = RootVerb::PersonModuleApprove {
        id: w.id.clone(),
        hash: w.hash.clone(),
    };
    match gesture.run(verb.into(), &mut |_| {}) {
        Ok(()) => Came::Approved(w.id.clone()),
        Err(why) => {
            let fresh = mtime(&places.verdict) != before;
            match denied_in(&places.verdict, w).filter(|_| fresh) {
                Some(denied) => Came::Refused(w.id.clone(), denied),
                None => Came::Door(why),
            }
        }
    }
}

/// Why root refused `w`, as its unit's verdict at `at` says — `None` when
/// the verdict is not about this request or not a refusal.
fn denied_in(at: &Path, w: &Waiting) -> Option<Denied> {
    let verdict: Verdict = ronoctua_person::parse(&std::fs::read(at).ok()?).ok()?;
    (verdict.outcome == Answer::Refused
        && verdict.id.as_deref() == Some(w.id.as_str())
        && verdict.hash.as_deref() == Some(w.hash.as_str()))
    .then_some(verdict.denied)
    .flatten()
}

/// Root's reason, as a sentence about the module.
#[must_use]
pub const fn refused_key(denied: Denied) -> &'static str {
    match denied {
        Denied::NoRequest | Denied::InvalidRequest | Denied::NotOwned => {
            key::MACHINE_APPROVE_SAID_UNASKED
        }
        Denied::Busy => key::MACHINE_APPROVE_SAID_BUSY,
        Denied::NoSuchModule => key::MACHINE_APPROVE_SAID_GONE,
        Denied::Unhashable => key::MACHINE_APPROVE_SAID_UNHASHABLE,
        Denied::Changed => key::MACHINE_APPROVE_SAID_CHANGED,
        Denied::Unwritable => key::MACHINE_APPROVE_SAID_UNWRITABLE,
    }
}

/// The sentence of what came of an approval.
#[must_use]
pub fn said(came: &Came, texts: &ronoctua_i18n::Texts) -> String {
    match came {
        Came::Approved(id) => texts.t_with(key::MACHINE_APPROVE_SAID_APPROVED, &[("id", id)]),
        Came::Refused(id, denied) => texts.t_with(refused_key(*denied), &[("id", id)]),
        Came::Unstaged(id) => texts.t_with(key::MACHINE_APPROVE_SAID_UNSTAGED, &[("id", id)]),
        Came::Door(why) => texts.t(crate::security_words::refusal(*why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    fn tea() -> Waiting {
        Waiting {
            id: "my-tea".to_owned(),
            hash: "sha256-bsxxoKoBi/zTX82E9vCaao7mCkH4nLxO3mU4EoeSeO8=".to_owned(),
        }
    }

    /// A scratch folder, gone with the test.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let at = std::env::temp_dir()
                .join(format!("ronoctua-approval-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&at);
            std::fs::create_dir_all(at.join("approve")).expect("a scratch folder");
            Self(at)
        }

        fn places(&self) -> Places {
            Places {
                applied: self.0.join("person-applied.json"),
                approved: self.0.join("approved.json"),
                verdict: self.0.join("approve-verdict.json"),
                requests: self.0.join("approve"),
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// FAILS IF: what waits is not read from the machine's report with its
    /// hash, an approval at that hash does not take it off what is left to
    /// approve (or one at another hash does), or a file that does not read
    /// is anything but nothing waiting.
    #[test]
    fn what_waits_is_the_reports_and_an_approval_as_shown_ends_it() {
        let s = Scratch::new("read");
        let places = s.places();
        assert_eq!(Readings::read(&places), Readings::default());
        std::fs::write(
            &places.applied,
            r#"{"format":1,"machine":"noctua","distro-rev":"abc","applied":[],"parts":{},
                "skipped":[],"declined":{"honoured":[],"refused":[],"unknown":[],"still-present":[]},
                "brain":{"included":["my-plain"],"awaiting-approval":[
                  {"id":"my-tea","hash":"sha256-bsxxoKoBi/zTX82E9vCaao7mCkH4nLxO3mU4EoeSeO8="},
                  {"id":"my-cake","hash":"sha256-cake"}]},
                "signature":{"pinned":null,"counter":0,"refused":null,"kept-back":[]}}"#,
        )
        .expect("a report");
        std::fs::write(
            &places.approved,
            r#"{"format":1,"modules":{
                "my-tea":{"hash":"sha256-bsxxoKoBi/zTX82E9vCaao7mCkH4nLxO3mU4EoeSeO8=","approved-at":"2026-11-01T12:00:00Z"},
                "my-cake":{"hash":"sha256-older","approved-at":"2026-11-01T12:00:00Z"}}}"#,
        )
        .expect("approvals");
        let r = Readings::read(&places);
        assert_eq!(r.waiting.len(), 2, "{r:?}");
        assert_eq!(r.waiting[0], tea());
        assert!(r.approved_as_shown(&r.waiting[0]));
        assert!(
            !r.approved_as_shown(&r.waiting[1]),
            "an approval of another hash"
        );
        let left: Vec<&str> = r.to_approve().map(|w| w.id.as_str()).collect();
        assert_eq!(left, ["my-cake"]);
        std::fs::write(&places.applied, b"{").expect("a broken report");
        assert!(Readings::read(&places).waiting.is_empty());
    }

    /// FAILS IF: an approval said yes to does not pass the door as the
    /// helper's `person-module-approve` with the module and the hash shown,
    /// or a refusal is said as anything but root's own reason when its
    /// verdict is this request's and new — the door's otherwise.
    #[test]
    fn the_approval_passes_the_door_and_says_roots_reason() {
        let s = Scratch::new("run");
        let places = s.places();
        let log = Arc::new(Mutex::new(Vec::new()));
        let gate = crate::sudo::Gate::fake(Arc::clone(&log), Ok(()));
        let came = run(&tea(), &places, gate.gesture(crate::sudo::Asker::Update));
        assert_eq!(came, Came::Approved("my-tea".to_owned()));
        assert_eq!(
            *log.lock().expect("log"),
            vec![(
                crate::sudo::Asker::Update,
                RootVerb::PersonModuleApprove {
                    id: "my-tea".to_owned(),
                    hash: tea().hash,
                }
            )]
        );
        assert!(
            places
                .requests
                .join(ronoctua_person::approve::REQUEST)
                .exists(),
            "nothing staged"
        );
        // THE DOOR REFUSED, and no verdict moved: the door's reason.
        let refusing = crate::sudo::Gate::fake(
            Arc::new(Mutex::new(Vec::new())),
            Err(ronoctua_security::Why::Cancelled),
        );
        let came = run(
            &tea(),
            &places,
            refusing.gesture(crate::sudo::Asker::Update),
        );
        assert_eq!(came, Came::Door(ronoctua_security::Why::Cancelled));
        // A VERDICT ABOUT THIS REQUEST is root's reason; one about another
        // is not.
        let verdict = |id: &str| {
            let request = Request::new(id, tea().hash);
            ronoctua_person::to_json(&Verdict::of(
                ronoctua_person::Stamp::parse("2026-11-01T12:00:00Z").expect("a stamp"),
                Some(&request),
                Some("sha256-now".to_owned()),
                Some(Denied::Changed),
            ))
        };
        std::fs::write(&places.verdict, verdict("my-tea")).expect("a verdict");
        assert_eq!(
            denied_in(&places.verdict, &tea()),
            Some(Denied::Changed),
            "root's reason was not read"
        );
        std::fs::write(&places.verdict, verdict("my-other")).expect("a verdict");
        assert_eq!(denied_in(&places.verdict, &tea()), None);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for denied in Denied::ALL {
                let said = said(&Came::Refused("my-tea".to_owned(), denied), &t);
                assert!(said.contains("my-tea") || denied == Denied::Busy, "{said}");
                assert!(!said.starts_with('['), "{language}: {denied:?}");
            }
            for came in [
                Came::Approved("my-tea".to_owned()),
                Came::Unstaged("my-tea".to_owned()),
            ] {
                assert!(said(&came, &t).contains("my-tea"), "{language}: {came:?}");
            }
            assert_eq!(
                said(&Came::Door(ronoctua_security::Why::Cancelled), &t),
                t.t(crate::security_words::refusal(
                    ronoctua_security::Why::Cancelled
                ))
            );
        }
    }

    /// FAILS IF: the question's cursor is not born on `no`, a first click
    /// on it answers, or a key that moved it is not the person's pick.
    #[test]
    fn the_question_is_born_on_no_and_a_click_picks_first() {
        let mut q = Question::of(tea());
        assert!(!q.yes());
        assert!(!q.clicked(false), "the born cursor answered one click");
        assert!(q.clicked(false));
        let mut q = Question::of(tea());
        assert!(!q.clicked(true), "the first click on yes answered");
        assert!(q.yes());
        assert!(q.clicked(true));
        let mut q = Question::of(tea());
        q.moved(true);
        assert!(q.clicked(true), "a key's pick is the person's");
    }
}
