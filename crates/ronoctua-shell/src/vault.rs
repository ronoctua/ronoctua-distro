//! `/private`, the person's encrypted folder, as the doors see it — what the
//! security centre's section (`crate::security`, `LEADER s e o` / `s e l`)
//! and the file manager's sidebar row hand to the vault's root unit, and
//! the words they say about it.
//!
//! # What a door does, and what root does
//!
//! A door asks (the passphrase, twice for a new one; the cap), stages the
//! request and its secrets as `0600` files in the person's `0700` folder
//! (`ronoctua_private::stage`), and knocks on the sudo door
//! (`RootVerb::PrivateRoot`, asked in the footer's sudo bar whichever door
//! pressed — `crate::sudo_bar`), which starts the vault's root unit. Whatever
//! happens, the folder is emptied after ([`run`]); WHY the unit refused is
//! in its verdict file, read back into the work so the door says it.
//!
//! # Never on a loop
//!
//! Nothing here stats `/private`: the doors read the status file (one small
//! read on `/run`) and `stat` the image, which lives on the machine's own
//! disk. A listing that lands in `/private` — by its path or through a link —
//! reads it only while it is open, and then on a thread it waits for a
//! moment at most (`ronoctua_files::listing::Withheld`).
//!
//! # Each door asks on its own line
//!
//! The footer (the section, `LEADER s e o` / `s e l`, its file view's `m`
//! and `l`), the security centre's window and the file manager's window each
//! run this flow themselves — the security centre's view, its questions on
//! that door's line, its refusals said there.

use std::path::Path;

use ronoctua_i18n::key;
use ronoctua_lookup::keys::Secret;
use ronoctua_private::{Op, Slot, State, Status, Why};

/// The question the passphrase in force is typed into (secret).
pub const PASSPHRASE: &str = "private-passphrase";
/// The question a new passphrase is typed into (secret).
pub const NEW_PASSPHRASE: &str = "private-new-passphrase";
/// The question the new passphrase is typed into again (secret).
pub const REPEAT_PASSPHRASE: &str = "private-repeat-passphrase";
/// The question the cap is typed into.
pub const CAP: &str = "private-cap";

/// Every question of the vault's flow — what a door hands back to it.
pub const QUESTIONS: [&str; 4] = [PASSPHRASE, NEW_PASSPHRASE, REPEAT_PASSPHRASE, CAP];

/// What a row of the section does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Act {
    /// Ask the cap and a new passphrase, then make it.
    Create,
    /// Ask the passphrase, then open it.
    OpenPassphrase,
    /// Open it with the enrolled key.
    OpenKey,
    /// Ask the passphrase, then resume it.
    Resume,
    /// Lock it.
    Lock,
    /// Show it in the file manager.
    Show,
    /// Ask a larger cap.
    Grow,
    /// Ask the passphrase, then enrol a key.
    Enrol,
    /// Forget the enrolled key.
    Forget,
    /// Ask the passphrase in force, then a new one twice.
    Passphrase,
}

impl Act {
    /// The command of the registry it is, when it is one.
    #[must_use]
    pub const fn command(self) -> Option<&'static str> {
        match self {
            Self::OpenPassphrase | Self::Resume => Some("private-open"),
            Self::Lock => Some("private-lock"),
            Self::Show => Some("private-show"),
            _ => None,
        }
    }

    /// The sentence under the pointer, for a row that is no command.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Create => key::SECURITY_ABOUT_PRIVATE_CREATE,
            Self::OpenKey => key::SECURITY_ABOUT_PRIVATE_OPEN_KEY,
            Self::Grow => key::SECURITY_ABOUT_PRIVATE_GROW,
            Self::Enrol => key::SECURITY_ABOUT_PRIVATE_ENROL,
            Self::Forget => key::SECURITY_ABOUT_PRIVATE_FORGET,
            Self::Passphrase => key::SECURITY_ABOUT_PRIVATE_PASSPHRASE,
            Self::OpenPassphrase | Self::Resume => key::CMD_PRIVATE_OPEN,
            Self::Lock => key::CMD_PRIVATE_LOCK,
            Self::Show => key::CMD_PRIVATE_SHOW,
        }
    }
}

/// A question flow in progress: what was asked, and what was answered so
/// far. Its secrets wiped when it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// Making it: the cap, then a new passphrase twice.
    Create {
        /// The cap, once answered.
        cap: Option<u64>,
        /// The new passphrase, once typed.
        new: Option<Secret>,
    },
    /// Opening (or resuming) it: the passphrase.
    Open,
    /// Enrolling a key: the passphrase.
    Enrol,
    /// A new passphrase: the old, then the new twice.
    Change {
        /// The passphrase in force, once typed.
        old: Option<Secret>,
        /// The new one, once typed.
        new: Option<Secret>,
    },
    /// A larger cap — and, for an open vault, the passphrase that lets its
    /// mapping grow with it.
    Grow {
        /// The cap, once answered.
        cap: Option<u64>,
    },
}

/// One request for the vault's root unit, with its secrets — and, once the
/// unit refused it, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// What to do.
    pub op: Op,
    /// [`Slot::Passphrase`].
    pub passphrase: Option<Secret>,
    /// [`Slot::NewPassphrase`].
    pub new_passphrase: Option<Secret>,
    /// What the unit's verdict said, when it refused.
    pub refused: Option<Why>,
}

impl Work {
    /// A request with no secret.
    #[must_use]
    pub const fn bare(op: Op) -> Self {
        Self {
            op,
            passphrase: None,
            new_passphrase: None,
            refused: None,
        }
    }

    /// A request with the passphrase in force.
    #[must_use]
    pub const fn with(op: Op, passphrase: Secret) -> Self {
        Self {
            op,
            passphrase: Some(passphrase),
            new_passphrase: None,
            refused: None,
        }
    }

    /// Whether it waits for a touch of the key while it runs.
    #[must_use]
    pub const fn touches(&self) -> bool {
        matches!(self.op, Op::EnrollFido2)
            || (matches!(self.op, Op::Open) && self.passphrase.is_none())
    }
}

/// Runs `work` through the sudo door: the request and its secrets staged
/// (as the person, before anybody types a password), the door knocked on,
/// the folder emptied whatever happened — and, when the unit refused, its
/// verdict read into `work.refused`. BLOCKS: a door's thread.
///
/// # Errors
///
/// The door's refusals, and the unit's as `Failed` (its reason in
/// `work.refused`).
pub fn run(
    work: &mut Work,
    root: &mut dyn crate::sudo_bar::Root,
    step: &mut dyn FnMut(&str),
) -> Result<(), ronoctua_security::Why> {
    run_at(Path::new(ronoctua_private::STAGING), work, root, step)
}

fn run_at(
    staging: &Path,
    work: &mut Work,
    root: &mut dyn crate::sudo_bar::Root,
    step: &mut dyn FnMut(&str),
) -> Result<(), ronoctua_security::Why> {
    use ronoctua_security::Why as Door;
    work.refused = None;
    let since = now_s();
    let staged = ronoctua_private::stage(staging, &ronoctua_private::Request::of(work.op))
        .and_then(|()| match &work.passphrase {
            Some(p) => {
                ronoctua_private::stage_secret(staging, Slot::Passphrase, p.expose().as_bytes())
            }
            None => Ok(()),
        })
        .and_then(|()| match &work.new_passphrase {
            Some(p) => {
                ronoctua_private::stage_secret(staging, Slot::NewPassphrase, p.expose().as_bytes())
            }
            None => Ok(()),
        });
    if let Err(r) = staged {
        ronoctua_private::unstage(staging);
        work.refused = Some(r.why);
        return Err(Door::Failed);
    }
    // THE BAR SAYS WHICH OPERATION: the vault's verb names none.
    let door = root.run(
        crate::sudo_bar::Ask {
            verb: ronoctua_security::verb::RootVerb::PrivateRoot,
            vault: Some(work.op),
        },
        step,
    );
    ronoctua_private::unstage(staging);
    match door {
        Ok(()) => Ok(()),
        Err(why) => {
            if why == Door::Failed {
                work.refused = ronoctua_private::refused_since(
                    Path::new(ronoctua_private::VERDICT_FILE),
                    work.op,
                    since.saturating_sub(1),
                );
            }
            Err(why)
        }
    }
}

/// Seconds since the epoch.
fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// A refusal of the vault, as a sentence.
#[must_use]
pub const fn refusal(why: Why) -> &'static str {
    match why {
        Why::Absent => key::PRIVATE_REFUSED_ABSENT,
        Why::Exists => key::PRIVATE_REFUSED_EXISTS,
        Why::WrongPassphrase => key::PRIVATE_REFUSED_WRONG_PASSPHRASE,
        Why::NoKey => key::PRIVATE_REFUSED_NO_KEY,
        Why::Busy => key::PRIVATE_REFUSED_BUSY,
        Why::NoRoom => key::PRIVATE_REFUSED_NO_ROOM,
        Why::Invalid => key::PRIVATE_REFUSED_INVALID,
        Why::Failed => key::PRIVATE_REFUSED_FAILED,
    }
}

/// What a door says when the vault did it.
#[must_use]
pub const fn done_key(op: Op) -> &'static str {
    match op {
        Op::Create { .. } => key::ECHO_PRIVATE_CREATED,
        Op::Open => key::ECHO_PRIVATE_OPENED,
        Op::Close => key::ECHO_PRIVATE_LOCKED,
        Op::EnrollFido2 => key::ECHO_PRIVATE_KEY_ENROLLED,
        Op::ForgetFido2 => key::ECHO_PRIVATE_KEY_FORGOTTEN,
        Op::Passphrase => key::ECHO_PRIVATE_PASSPHRASE_CHANGED,
        Op::Grow { .. } => key::ECHO_PRIVATE_GROWN,
    }
}

/// The status as a door reads it now, and what the image takes: one read on
/// `/run` and one `stat` of the image — never anything under `/private`.
/// `Err` when the file is there and does not read; `Ok(None)` without the
/// vault's units on this machine.
///
/// # Errors
///
/// The status file does not read.
pub fn status_now() -> Result<Option<Status>, Why> {
    ronoctua_private::status(Path::new(ronoctua_private::STATUS_FILE))
        .map(|s| {
            s.map(|s| {
                ronoctua_private::weighed(
                    s,
                    &ronoctua_private::image_of(Path::new(ronoctua_private::IMAGE_DIR)),
                )
            })
        })
        .map_err(|r| r.why)
}

/// What is free on the disk the image is on — the cap's room. A `statvfs`
/// of the image's folder, never of `/private`.
#[must_use]
pub fn free_bytes() -> Option<u64> {
    rustix::fs::statvfs(ronoctua_private::IMAGE_DIR)
        .ok()
        .map(|v| v.f_bavail.saturating_mul(v.f_frsize))
}

/// A cap as the question's line holds it: whole gigabytes, rounded up — the
/// decimal unit every size on screen is shown in, and the one
/// `ronoctua_private::parse_size` reads by default.
#[must_use]
pub fn cap_text(bytes: u64) -> String {
    format!("{}G", bytes.div_ceil(ronoctua_private::GB))
}

/// The status the sidebar's row is built from: the file's, as it is now
/// (one small read on `/run`); `None` without one, or with one that does not
/// read.
#[must_use]
pub fn status_for_sidebar() -> Option<Status> {
    ronoctua_private::status(Path::new(ronoctua_private::STATUS_FILE))
        .ok()
        .flatten()
}

/// The sidebar's row for the vault, built from what the disk border read
/// (`volumes`: is something mounted at `/private`?) and the status (is there
/// a vault, is it suspended?). `None` when there is no vault.
///
/// Open is what udisks says is mounted — it is what moves the sidebar, and
/// the status file is written only once the unit is done; suspended is the
/// status file's, written before the suspension.
#[must_use]
pub fn sidebar_row(
    volumes: &[ronoctua_disks::Volume],
    status: Option<Status>,
) -> Option<ronoctua_files::sidebar::Row> {
    use ronoctua_files::sidebar::{Row, Seal, Section, Target};
    let point = Path::new(ronoctua_private::MOUNT_POINT);
    let mounted = volumes
        .iter()
        .any(|v| v.mounted_at.iter().any(|at| Path::new(at) == point));
    if !mounted && status.is_none_or(|s| s.state == State::Absent) {
        return None;
    }
    let open = mounted && status.is_none_or(|s| s.state != State::SuspendedInUse);
    Some(Row {
        id: "private".to_owned(),
        target: Target::Private,
        // ITS NAME ON THE DISK, data like a folder's: the leaf of `/private`.
        label: point.file_name().map(|n| n.to_string_lossy().into_owned()),
        section: Section::Devices,
        mounted: Some(open),
        at: open.then(|| point.to_path_buf()),
        quiet: false,
        system: false,
        needed: None,
        removable: false,
        built_in: true,
        seal: Some(if open { Seal::Unlocked } else { Seal::Locked }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a vault's refusal or its "done" goes without a sentence in
    /// either language, or an about falls back to another row's.
    #[test]
    fn every_refusal_and_every_ending_has_words() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a declared language");
            for why in Why::ALL {
                assert!(!t.t(refusal(why)).starts_with('['), "{language}: {why:?}");
            }
            for op in [
                Op::Create { cap_bytes: 0 },
                Op::Open,
                Op::Close,
                Op::EnrollFido2,
                Op::ForgetFido2,
                Op::Passphrase,
                Op::Grow { cap_bytes: 0 },
            ] {
                assert!(!t.t(done_key(op)).starts_with('['), "{language}: {op:?}");
            }
        }
    }

    /// FAILS IF: a cap shown on the question's line stops reading back as
    /// the same cap.
    #[test]
    fn a_cap_reads_back_as_itself() {
        for gb in [1u64, 64, 1500] {
            let bytes = gb * ronoctua_private::GB;
            assert_eq!(ronoctua_private::parse_size(&cap_text(bytes)), Some(bytes));
        }
    }

    /// FAILS IF: the folder is left holding a secret after the door, or a
    /// work that could not be staged goes on to knock.
    #[test]
    fn the_staging_is_emptied_whatever_the_door_said() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("ronoctua-vault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut work = Work::with(Op::Open, Secret::new("correct horse"));
        // THE DOOR REFUSES (a gesture cancelled in the bar), and the secret
        // staged for it is gone all the same — and the bar was told which
        // operation it was asked for.
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let gate = crate::sudo_bar::Gate::fake(
            std::sync::Arc::clone(&log),
            Err(ronoctua_security::Why::Cancelled),
        );
        let mut gesture = gate.gesture(crate::sudo_bar::Asker::Security);
        let said = run_at(&dir, &mut work, gesture.as_mut(), &mut |_| {});
        assert_eq!(said, Err(ronoctua_security::Why::Cancelled));
        assert_eq!(
            log.lock().unwrap().as_slice(),
            [(
                crate::sudo_bar::Asker::Security,
                ronoctua_security::verb::RootVerb::PrivateRoot
            )]
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a secret was left"
        );
        // A FOLDER THAT IS NOT THERE: refused before the door, and said why.
        let mut work = Work::with(Op::Open, Secret::new("x"));
        let said = run_at(&dir.join("gone"), &mut work, gesture.as_mut(), &mut |_| {});
        assert_eq!(said, Err(ronoctua_security::Why::Failed));
        assert_eq!(work.refused, Some(Why::Failed));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(Work::bare(Op::Open).touches());
        assert!(!Work::with(Op::Open, Secret::new("x")).touches());
        assert!(Work::with(Op::EnrollFido2, Secret::new("x")).touches());
    }

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn scene(state: State, fido2: bool) -> crate::security::Scene {
        crate::security::Scene {
            read: true,
            settings: Some(ronoctua_security::firewall::Settings::default()),
            helper: true,
            private: Some(Status {
                state,
                cap_bytes: 64 << 30,
                apparent_bytes: 64 << 30,
                real_bytes: 3 << 30,
                fido2,
                ..Status::absent()
            }),
            private_free: Some(100 << 30),
            ..crate::security::Scene::default()
        }
    }

    /// The rows of the vault's section that act.
    fn acts(view: &crate::security::View, scene: &crate::security::Scene) -> Vec<Act> {
        view.rows(
            scene,
            100,
            &texts("en-GB"),
            &ronoctua_files::when::Zone::utc(),
        )
        .into_iter()
        .filter(|r| r.section == crate::security::Section::Private)
        .filter_map(|r| match r.act {
            Some(crate::security::Act::Private(a)) => Some(a),
            _ => None,
        })
        .collect()
    }

    /// Presses the row that does `act`: a click to choose it, `RET` to run it.
    fn press(
        view: &mut crate::security::View,
        scene: &crate::security::Scene,
        act: Act,
    ) -> crate::security::Errand {
        let t = texts("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let rows = view.rows(scene, 100, &t, &zone);
        let n = rows
            .iter()
            .position(|r| r.act == Some(crate::security::Act::Private(act)))
            .expect("the row is drawn");
        let _ = view.clicked(n, &rows);
        view.press(crate::security::Press::Enter, scene, 100, &t, &zone)
    }

    /// FAILS IF: a state of the vault offers the wrong rows — creating one
    /// that exists, opening one that is open, the key's opening without a
    /// key, locking one that is locked.
    #[test]
    fn each_state_offers_what_it_can_do() {
        let v = crate::security::View::fresh();
        assert_eq!(acts(&v, &scene(State::Absent, false)), [Act::Create]);
        assert_eq!(
            acts(&v, &scene(State::Locked, false)),
            [Act::OpenPassphrase, Act::Grow, Act::Enrol, Act::Passphrase]
        );
        assert_eq!(
            acts(&v, &scene(State::Locked, true)),
            [
                Act::OpenPassphrase,
                Act::OpenKey,
                Act::Grow,
                Act::Forget,
                Act::Passphrase
            ]
        );
        assert_eq!(
            acts(&v, &scene(State::Open, false)),
            [Act::Lock, Act::Show, Act::Grow, Act::Enrol, Act::Passphrase]
        );
        assert_eq!(
            acts(&v, &scene(State::SuspendedInUse, false))[0],
            Act::Resume
        );
        // AND A SUSPENDED VAULT IS IN "NOW", in the danger colour.
        let rows = v.rows(
            &scene(State::SuspendedInUse, false),
            100,
            &texts("en-GB"),
            &ronoctua_files::when::Zone::utc(),
        );
        assert!(
            rows.iter()
                .any(|r| r.section == crate::security::Section::Now
                    && r.act == Some(crate::security::Act::Private(Act::Resume)))
        );
    }

    /// FAILS IF: making the vault stops asking the cap, then a passphrase
    /// twice, before the door — or two passphrases that differ reach it.
    #[test]
    fn creating_asks_the_cap_and_the_passphrase_twice() {
        use crate::security::Errand;
        let t = texts("en-GB");
        let s = scene(State::Absent, false);
        let mut v = crate::security::View::fresh();
        let Errand::Ask {
            id: CAP,
            secret: false,
            initial: Some(initial),
        } = press(&mut v, &s, Act::Create)
        else {
            panic!("the cap is asked first");
        };
        assert_eq!(
            ronoctua_private::parse_size(&initial),
            Some(ronoctua_private::suggested_cap(100 << 30))
        );
        // A SIZE THAT IS NOT ONE is said, and the flow ends.
        assert!(matches!(v.answered(CAP, "lots", &s, &t), Errand::Say(_)));
        assert!(matches!(
            v.answered(NEW_PASSPHRASE, "x", &s, &t),
            Errand::Nothing
        ));
        // THE WHOLE WAY, with a mistyped repetition first.
        let _ = press(&mut v, &s, Act::Create);
        assert!(matches!(
            v.answered(CAP, "64G", &s, &t),
            Errand::Ask {
                id: NEW_PASSPHRASE,
                secret: true,
                ..
            }
        ));
        assert!(matches!(
            v.answered(NEW_PASSPHRASE, "correct horse", &s, &t),
            Errand::Ask {
                id: REPEAT_PASSPHRASE,
                secret: true,
                ..
            }
        ));
        assert_eq!(
            match v.answered(REPEAT_PASSPHRASE, "correct hrose", &s, &t) {
                Errand::Say(said) => said,
                other => panic!("{other:?}"),
            },
            t.t(key::ECHO_PRIVATE_MISMATCH)
        );
        let _ = press(&mut v, &s, Act::Create);
        let _ = v.answered(CAP, "64G", &s, &t);
        let _ = v.answered(NEW_PASSPHRASE, "correct horse", &s, &t);
        match v.answered(REPEAT_PASSPHRASE, "correct horse", &s, &t) {
            Errand::Door(crate::security::Work::Private(w)) => {
                assert_eq!(
                    w.op,
                    Op::Create {
                        cap_bytes: 64 * ronoctua_private::GB
                    }
                );
                assert_eq!(
                    w.passphrase.as_ref().map(Secret::expose),
                    Some("correct horse")
                );
                assert!(w.new_passphrase.is_none());
            }
            other => panic!("{other:?}"),
        }
        // TOO SMALL a cap is said with the floor.
        let _ = press(&mut v, &s, Act::Create);
        assert!(matches!(v.answered(CAP, "100M", &s, &t), Errand::Say(_)));
    }

    /// FAILS IF: changing the passphrase stops asking the old one and the
    /// new one twice, or a question let go keeps its secrets for a later
    /// answer typed by name.
    #[test]
    fn a_new_passphrase_asks_the_old_and_the_new_twice() {
        use crate::security::Errand;
        let t = texts("en-GB");
        let s = scene(State::Locked, false);
        let mut v = crate::security::View::fresh();
        assert!(matches!(
            press(&mut v, &s, Act::Passphrase),
            Errand::Ask {
                id: PASSPHRASE,
                secret: true,
                ..
            }
        ));
        let _ = v.answered(PASSPHRASE, "old one", &s, &t);
        let _ = v.answered(NEW_PASSPHRASE, "new one", &s, &t);
        match v.answered(REPEAT_PASSPHRASE, "new one", &s, &t) {
            Errand::Door(crate::security::Work::Private(w)) => {
                assert_eq!(w.op, Op::Passphrase);
                assert_eq!(w.passphrase.as_ref().map(Secret::expose), Some("old one"));
                assert_eq!(
                    w.new_passphrase.as_ref().map(Secret::expose),
                    Some("new one")
                );
            }
            other => panic!("{other:?}"),
        }
        let _ = press(&mut v, &s, Act::Passphrase);
        v.let_go();
        assert!(matches!(
            v.answered(PASSPHRASE, "old one", &s, &t),
            Errand::Nothing
        ));
    }

    /// FAILS IF: a grow of an open vault stops asking the passphrase its
    /// mapping needs, or a locked one's asks for nothing it needs.
    #[test]
    fn growing_an_open_vault_asks_the_passphrase() {
        use crate::security::Errand;
        let t = texts("en-GB");
        let cap = 128 * ronoctua_private::GB;
        let mut v = crate::security::View::fresh();
        let open = scene(State::Open, false);
        assert!(matches!(
            press(&mut v, &open, Act::Grow),
            Errand::Ask {
                id: CAP,
                secret: false,
                ..
            }
        ));
        assert!(matches!(
            v.answered(CAP, "128G", &open, &t),
            Errand::Ask {
                id: PASSPHRASE,
                secret: true,
                ..
            }
        ));
        match v.answered(PASSPHRASE, "correct horse", &open, &t) {
            Errand::Door(crate::security::Work::Private(w)) => {
                assert_eq!(w.op, Op::Grow { cap_bytes: cap });
                assert!(w.passphrase.is_some());
            }
            other => panic!("{other:?}"),
        }
        let locked = scene(State::Locked, false);
        let _ = press(&mut v, &locked, Act::Grow);
        match v.answered(CAP, "128G", &locked, &t) {
            Errand::Door(crate::security::Work::Private(w)) => {
                assert_eq!(w.op, Op::Grow { cap_bytes: cap });
                assert!(w.passphrase.is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: the leader's two keys stop doing what the state allows —
    /// the key's opening when one is enrolled, the passphrase otherwise, the
    /// section where a vault is made, locking only an open one.
    #[test]
    fn the_leaders_keys_follow_the_state() {
        use crate::security::Errand;
        let t = texts("en-GB");
        let mut v = crate::security::View::fresh();
        let errand = v.private_open(&scene(State::Locked, true), &t);
        assert!(
            matches!(&errand, Errand::Door(crate::security::Work::Private(w)) if w.touches()),
            "{errand:?}"
        );
        assert!(matches!(
            v.private_open(&scene(State::Locked, false), &t),
            Errand::Ask {
                id: PASSPHRASE,
                secret: true,
                ..
            }
        ));
        assert!(matches!(
            v.private_open(&scene(State::SuspendedInUse, true), &t),
            Errand::Ask { id: PASSPHRASE, .. }
        ));
        assert!(matches!(
            v.private_open(&scene(State::Absent, false), &t),
            Errand::Show(crate::security::Section::Private)
        ));
        assert!(matches!(
            v.private_open(&scene(State::Open, false), &t),
            Errand::Command("private-show")
        ));
        match v.private_lock(&scene(State::Open, false), &t) {
            Errand::Door(crate::security::Work::Private(w)) => assert_eq!(w.op, Op::Close),
            other => panic!("{other:?}"),
        }
        for state in [State::Locked, State::Absent, State::SuspendedInUse] {
            assert!(matches!(
                v.private_lock(&scene(state, false), &t),
                Errand::Say(_)
            ));
        }
        let none = crate::security::Scene {
            read: true,
            ..crate::security::Scene::default()
        };
        assert_eq!(
            match v.private_open(&none, &t) {
                Errand::Say(said) => said,
                other => panic!("{other:?}"),
            },
            t.t(key::SECURITY_PRIVATE_MISSING)
        );
    }

    /// FAILS IF: "touch your key" is said before sudo let the person
    /// through (a password question may still come), is not said once the
    /// unit started, or a step of the vault's is taken for the fingerprint's.
    #[test]
    fn the_touch_is_asked_when_the_unit_starts() {
        use crate::security::{Came, Errand, Work as Page};
        let t = texts("en-GB");
        let mut v = crate::security::View::fresh();
        let mut s = scene(State::Locked, true);
        let Errand::Door(work) = v.private_open(&s, &t) else {
            panic!("the key's opening goes to the door");
        };
        v.sent(&work);
        let busy = |v: &crate::security::View, s: &crate::security::Scene| {
            v.rows(s, 100, &t, &ronoctua_files::when::Zone::utc())
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<String>()
        };
        assert!(!busy(&v, &s).contains(&t.t(key::SECURITY_PRIVATE_TOUCH)));
        // SUDO ASKS IN THE BAR (a password, or the key for sudo): nothing of
        // the vault's touch is said while it does — the unit has not started.
        assert!(matches!(&work, Page::Private(w) if w.op == Op::Open && w.touches()));
        // THE UNIT STARTED: now, and only now.
        let out = v.came(Came::Step(ronoctua_private::STARTED.to_owned()), &mut s, &t);
        assert_eq!(out.say, Some(t.t(key::SECURITY_PRIVATE_TOUCH)));
        assert!(busy(&v, &s).contains(&t.t(key::SECURITY_PRIVATE_TOUCH)));
        // A PASSPHRASE OPENING waits for nobody's touch.
        let mut v = crate::security::View::fresh();
        v.sent(&Page::Private(Box::new(Work::with(
            Op::Open,
            Secret::new("x"),
        ))));
        let out = v.came(Came::Step(ronoctua_private::STARTED.to_owned()), &mut s, &t);
        assert_eq!(out.say, None);
    }

    /// FAILS IF: the row pressed to send the vault's work is no longer under
    /// the cursor while the work runs — a busy line drawn above it moves the
    /// index-kept cursor onto the row before, and a second `RET` on "open
    /// with the key" would ask the passphrase instead.
    #[test]
    fn the_cursor_stays_on_the_row_that_sent_the_work() {
        let t = texts("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        for (state, act) in [
            (State::Locked, Act::OpenKey),
            (State::Locked, Act::Forget),
            (State::Open, Act::Lock),
            (State::Open, Act::Forget),
        ] {
            let s = scene(state, true);
            let mut v = crate::security::View::fresh();
            let crate::security::Errand::Door(work) = press(&mut v, &s, act) else {
                panic!("{act:?} goes to the door");
            };
            v.sent(&work);
            let rows = v.rows(&s, 100, &t, &zone);
            assert!(
                rows.iter().any(|r| r
                    .fields
                    .iter()
                    .any(|f| f.text == t.t(key::SECURITY_PRIVATE_BUSY))),
                "{act:?}: no busy line"
            );
            assert_eq!(
                rows.get(v.cursor()).and_then(|r| r.act.clone()),
                Some(crate::security::Act::Private(act)),
                "{state:?}: the cursor left {act:?} while its work runs"
            );
        }
    }

    /// FAILS IF: the unit's reason stops being what the door says — a wrong
    /// passphrase said as "it did not work".
    #[test]
    fn the_units_reason_is_said() {
        let t = texts("pt-BR");
        let mut v = crate::security::View::fresh();
        let mut s = scene(State::Locked, false);
        let mut work = Work::with(Op::Open, Secret::new("x"));
        work.refused = Some(Why::WrongPassphrase);
        let out = v.came(
            crate::security::Came::Door {
                work: crate::security::Work::Private(Box::new(work)),
                result: Err(ronoctua_security::Why::Failed),
            },
            &mut s,
            &t,
        );
        assert_eq!(out.say, Some(t.t(key::PRIVATE_REFUSED_WRONG_PASSPHRASE)));
    }

    /// FAILS IF: a state of the section draws a key that has no words, in
    /// either language.
    #[test]
    fn every_state_is_drawn_in_both_languages() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for state in State::ALL {
                for fido2 in [false, true] {
                    let mut s = scene(state, fido2);
                    s.private_free = Some(1 << 30);
                    let rows = crate::security::View::fresh().rows(
                        &s,
                        100,
                        &t,
                        &ronoctua_files::when::Zone::utc(),
                    );
                    let drawn: String = rows
                        .iter()
                        .filter(|r| r.section == crate::security::Section::Private)
                        .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                        .collect();
                    assert!(!drawn.contains('['), "{language} {state:?}: {drawn}");
                    assert!(!drawn.is_empty(), "{language} {state:?}");
                }
            }
        }
    }

    /// FAILS IF: the sidebar draws a vault that is not there, takes a
    /// suspended one for open, or stops drawing an open one as a place.
    #[test]
    fn the_sidebar_row_follows_the_mount() {
        use ronoctua_files::sidebar::{Seal, Target};
        let mut v = ronoctua_disks::Volume {
            node: "/dev/dm-4".into(),
            id_type: "btrfs".into(),
            mounted_at: vec![ronoctua_private::MOUNT_POINT.to_owned()],
            ..ronoctua_disks::Volume::default()
        };
        let with = |state| {
            Some(Status {
                state,
                ..Status::absent()
            })
        };
        // MOUNTED, whatever the status says (it is written last).
        for status in [None, with(State::Locked), with(State::Open)] {
            let row = sidebar_row(std::slice::from_ref(&v), status).expect("a mounted vault");
            assert_eq!(row.target, Target::Private);
            assert_eq!(row.mounted, Some(true));
            assert_eq!(row.at.as_deref(), Some(Path::new("/private")));
            assert_eq!(row.seal, Some(Seal::Unlocked));
        }
        // SUSPENDED: mounted, and no place to walk into.
        let row = sidebar_row(std::slice::from_ref(&v), with(State::SuspendedInUse)).unwrap();
        assert_eq!(
            (row.mounted, row.at, row.seal),
            (Some(false), None, Some(Seal::Locked))
        );
        v.mounted_at.clear();
        // NOT MOUNTED: a locked vault, or none at all.
        let row = sidebar_row(std::slice::from_ref(&v), with(State::Locked)).unwrap();
        assert_eq!((row.mounted, row.seal), (Some(false), Some(Seal::Locked)));
        assert_eq!(
            sidebar_row(std::slice::from_ref(&v), with(State::Absent)),
            None
        );
        assert_eq!(sidebar_row(&[v], None), None);
    }
}
