//! `/private`, the person's encrypted folder, as the doors see it — what the
//! security centre's section (`ronoctua_shell_security::security`, `LEADER s e o` / `s e l`)
//! and the file manager's sidebar row hand to the vault's root unit, and
//! the words they say about it.
//!
//! # What a door does, and what root does
//!
//! A door asks (the passphrase, twice for a new one; the cap), stages the
//! request and its secrets as `0600` files in the person's `0700` folder
//! (`ronoctua_private::stage`), and knocks on the sudo door
//! (`RootVerb::PrivateRoot`, asked in the footer's sudo bar whichever door
//! pressed — `ronoctua_shell_footer::sudo_bar`), which starts the vault's root unit. Whatever
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

use crate::key;
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

/// THE VAULT'S FLOW, AS A DOOR HOLDS IT — the security centre's, which asks
/// the questions and keeps the work, run in a door that is not the centre's:
/// the file manager's window runs it for `m` and `l` on its sidebar's vault
/// row, so the passphrase is asked on ITS line and what came back is said on
/// it — a door never sends the person to another one.
///
/// # Why a trait, here
///
/// The flow is the security centre's and the door is the file manager's, and
/// neither stands on the other: the binary hands the manager's window the
/// centre's flow as this (one line where it opens the window), and the
/// manager names no part of the centre. It lives with the vault's words and
/// work, which both of them already stand on.
pub trait Flow {
    /// `m` (`open`) or `l` (lock), over the vault as it is now — or as a
    /// camera planted it (`planted`: then no work is ever started).
    fn gesture(
        &mut self,
        open: bool,
        planted: Option<Option<Status>>,
        way: &mut dyn crate::region::Doorway,
    ) -> Walk;
    /// One of the vault's questions ([`QUESTIONS`]) was answered on the
    /// door's line.
    fn answered(
        &mut self,
        id: &'static str,
        text: &str,
        way: &mut dyn crate::region::Doorway,
    ) -> Walk;
    /// One of them was let go.
    fn unanswered(&mut self, id: &str);
    /// The door woke for the work's answers (`Doorway::wake`): what came
    /// back is said, and the next question asked.
    fn woken(&mut self, way: &mut dyn crate::region::Doorway) -> Walk;
}

/// What the vault's flow asks of the door that holds it, beyond its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Walk {
    /// Nothing more.
    Stay,
    /// Walk into this folder — the vault, open now.
    Into(std::path::PathBuf),
}

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
    root: &mut dyn crate::sudo::Root,
    step: &mut dyn FnMut(&str),
) -> Result<(), ronoctua_security::Why> {
    run_at(Path::new(ronoctua_private::STAGING), work, root, step)
}

fn run_at(
    staging: &Path,
    work: &mut Work,
    root: &mut dyn crate::sudo::Root,
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
        crate::sudo::Ask {
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
        let gate = crate::sudo::Gate::fake(
            std::sync::Arc::clone(&log),
            Err(ronoctua_security::Why::Cancelled),
        );
        let mut gesture = gate.gesture(crate::sudo::Asker::Security);
        let said = run_at(&dir, &mut work, gesture.as_mut(), &mut |_| {});
        assert_eq!(said, Err(ronoctua_security::Why::Cancelled));
        assert_eq!(
            log.lock().unwrap().as_slice(),
            [(
                crate::sudo::Asker::Security,
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
