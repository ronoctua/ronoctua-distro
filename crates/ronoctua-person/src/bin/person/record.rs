//! `record` — the machine's record, published into the person's own plain
//! part as `machines/<hostname>.json`: the update's last steps run it, as
//! the person, right before the plain part is pushed.
//!
//! # Why a copy, and why this program
//!
//! The person's other machines learn what this one runs from the plain part
//! of the belongings: its last good system, its boot trial, a hold, how its
//! last apply ended. Root writes that record (`ronoctua-machine`, readable
//! by everybody); the person's half copies it into the mirror the plain part
//! is made of. This crate owns the record's schema (`state::MachineState`)
//! and the mirror's names, so the copy is VALIDATED and written in its one
//! canonical form — a record root wrote that does not read is refused, never
//! pushed.
//!
//! # And left alone when it says the same
//!
//! An update that changed nothing must push nothing: the file is compared
//! with what is there and written only when it differs, atomically (a scratch
//! named after this process, then a rename).
//!
//! # And a rehearsal writes nothing
//!
//! `--rehearse` reads and validates the record and compares it with the
//! mirror exactly as a run would, and says what the run would do — the
//! update's rehearsal runs it in this step's place, and it leaves the
//! mirror byte for byte as it was.

use std::path::{Path, PathBuf};

use ronoctua_person::state::MachineState;

/// What `record` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// The machine has no record yet: nothing to publish.
    Nothing,
    /// The mirror already says the same.
    Unchanged,
    /// The mirror's copy was written.
    Written(PathBuf),
    /// A rehearsal: the mirror's copy would be written, and was not.
    Would(PathBuf),
}

/// Copies `<state>/state.json` into `<mirror>/machines/<host>.json` — or,
/// `rehearse`, says whether it would, writing nothing.
///
/// # Errors
///
/// A sentence: the host is not a host name, the record does not read or is
/// not a machine's record, or the copy could not be written.
pub(crate) fn record(
    state: &Path,
    mirror: &Path,
    host: &str,
    rehearse: bool,
) -> Result<Recorded, String> {
    if !ronoctua_person::is_hostname(host) {
        return Err(format!("{host:?} is not a host name"));
    }
    let from = state.join(ronoctua_person::names::STATE);
    let bytes = match std::fs::read(&from) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Recorded::Nothing),
        Err(e) => return Err(format!("{}: {e}", from.display())),
    };
    let record: MachineState = ronoctua_person::parse(&bytes)
        .map_err(|r| format!("{}: not a machine's record ({r})", from.display()))?;
    let text = ronoctua_person::to_json(&record);
    let dir = mirror.join(ronoctua_person::names::MACHINES);
    let leaf = format!("{host}.json");
    let into = dir.join(&leaf);
    if std::fs::read(&into).is_ok_and(|was| was == text.as_bytes()) {
        return Ok(Recorded::Unchanged);
    }
    if rehearse {
        return Ok(Recorded::Would(into));
    }
    write_atomically(&dir, &leaf, text.as_bytes())
        .map_err(|e| format!("{}: {e}", into.display()))?;
    Ok(Recorded::Written(into))
}

/// The person's mirror of their plain part (`$XDG_STATE_HOME/ronoctua/person`),
/// or `None` with no home to find it in.
pub(crate) fn mirror() -> Option<PathBuf> {
    ronoctua_xdg::state_dir().map(|d| d.join(ronoctua_person::names::MIRROR))
}

/// This machine's name, as the kernel has it (`uname`).
pub(crate) fn host() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned()
}

/// A scratch named after this process, `create_new`, then a rename: two
/// writers race only at the rename, and a reader never sees half a file.
fn write_atomically(dir: &Path, leaf: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    std::fs::create_dir_all(dir)?;
    let scratch = dir.join(format!("{leaf}.{}.new", std::process::id()));
    let _ = std::fs::remove_file(&scratch);
    let written = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&scratch)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&scratch, dir.join(leaf))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&scratch);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    const A_RECORD: &str = r#"{"format": 1, "last-good": null,
        "trial": {"toplevel": "/nix/store/00000000000000000000000000000001-nixos-system-b", "outcome": "pending"},
        "hold": null, "last-apply": null}"#;

    /// FAILS IF: the copy is not the canonical record, an update that
    /// changed nothing rewrites it (and so pushes it), or a machine with no
    /// record yet makes a file out of nothing.
    #[test]
    fn the_record_is_copied_canonical_once_and_not_invented() {
        let s = crate::testing::Scratch::new("record-copies");
        s.dir("state");
        s.dir("mirror");
        let (state, mirror) = (s.0.join("state"), s.0.join("mirror"));
        assert_eq!(
            record(&state, &mirror, "laptop", false),
            Ok(Recorded::Nothing)
        );
        assert!(
            !mirror.join("machines").exists(),
            "a record made of nothing"
        );

        s.file("state/state.json", A_RECORD.as_bytes());
        let into = mirror.join("machines/laptop.json");
        assert_eq!(
            record(&state, &mirror, "laptop", false),
            Ok(Recorded::Written(into.clone()))
        );
        let written = std::fs::read_to_string(&into).unwrap();
        let canonical = ronoctua_person::to_json(
            &ronoctua_person::parse::<MachineState>(A_RECORD.as_bytes()).unwrap(),
        );
        assert_eq!(written, canonical, "the copy is not the canonical form");
        assert_eq!(
            record(&state, &mirror, "laptop", false),
            Ok(Recorded::Unchanged)
        );
        let leftovers: Vec<_> = std::fs::read_dir(mirror.join("machines"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers.len(), 1, "a scratch was left: {leftovers:?}");
    }

    /// FAILS IF: a record root wrote that is not a machine's record, or a
    /// name that would put the file elsewhere, reaches the plain part.
    #[test]
    fn a_record_that_does_not_read_and_a_host_that_is_not_one_are_refused() {
        let s = crate::testing::Scratch::new("record-refuses");
        s.dir("state");
        s.dir("mirror");
        let (state, mirror) = (s.0.join("state"), s.0.join("mirror"));
        s.file("state/state.json", br#"{"format": 1, "hold": "#);
        assert!(record(&state, &mirror, "laptop", false).is_err());
        s.file("state/state.json", br#"{"format": 99}"#);
        assert!(
            record(&state, &mirror, "laptop", false).is_err(),
            "a newer record"
        );
        s.file("state/state.json", A_RECORD.as_bytes());
        for bad in ["../escape", "", "Laptop", "a/b"] {
            assert!(record(&state, &mirror, bad, false).is_err(), "{bad:?}");
        }
        assert!(!mirror.join("machines").exists());
    }

    /// FAILS IF: a rehearsal writes the mirror — the file, the folder or a
    /// scratch — or does not say what the run would do, the same refusals
    /// included.
    #[test]
    fn a_rehearsal_says_what_it_would_record_and_writes_nothing() {
        let s = crate::testing::Scratch::new("record-rehearses");
        s.dir("state");
        s.dir("mirror");
        let (state, mirror) = (s.0.join("state"), s.0.join("mirror"));
        assert_eq!(
            record(&state, &mirror, "laptop", true),
            Ok(Recorded::Nothing)
        );
        s.file("state/state.json", A_RECORD.as_bytes());
        let into = mirror.join("machines/laptop.json");
        assert_eq!(
            record(&state, &mirror, "laptop", true),
            Ok(Recorded::Would(into.clone()))
        );
        assert!(
            !mirror.join("machines").exists(),
            "a rehearsal made the folder"
        );
        assert!(record(&state, &mirror, "../escape", true).is_err());

        // ONCE RECORDED, a rehearsal sees it is the same — and a record that
        // moved since, it would write again, leaving the old copy as it was.
        assert!(matches!(
            record(&state, &mirror, "laptop", false),
            Ok(Recorded::Written(_))
        ));
        let was = std::fs::read(&into).unwrap();
        assert_eq!(
            record(&state, &mirror, "laptop", true),
            Ok(Recorded::Unchanged)
        );
        s.file(
            "state/state.json",
            A_RECORD.replace("pending", "good").as_bytes(),
        );
        assert_eq!(
            record(&state, &mirror, "laptop", true),
            Ok(Recorded::Would(into.clone()))
        );
        assert_eq!(
            std::fs::read(&into).unwrap(),
            was,
            "a rehearsal rewrote the copy"
        );
    }
}
