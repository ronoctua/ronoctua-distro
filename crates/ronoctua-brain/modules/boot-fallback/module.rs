//! The boot trial, told: when the system an update brought did not start,
//! and the machine came back to the last good one by itself, one line on the
//! minibuffer says so — once per failed trial.
//!
//! Why each piece is so is in `why.md`, beside this file.
//!
//! # It runs nothing
//!
//! A watch (`schedule = null`): inotify on the folder of the machine's record
//! (`ronoctua_machine::STATE_DIR`), which `ronoctua-boot-bless.service`
//! rewrites whole, by a rename, a minute after the boot came up (and the
//! rescue's rollback, when it disarms a trial). No clock.
//!
//! # Once
//!
//! The record keeps saying `failed` until the next apply arms another trial,
//! and the brain starts again at every login. So what was told is kept under
//! the module's state directory (the failed trial's toplevel): a failure is
//! told the first time it is seen — at the bless, or at the next login when
//! nobody was logged in — and never again.

use std::path::PathBuf;
use std::sync::Mutex;

use ronoctua_machine::{Places, TrialOutcome};

use crate::module::{Birth, Ctx, Event, Heard, Manifest, Module, Source, Watcher};
use crate::moment::Moment;

/// The file, under the module's state directory, naming the failed trial
/// already told of.
const TOLD_FILE: &str = "told";

/// The entry.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    BootTrial::reading(PathBuf::from(ronoctua_machine::STATE_DIR))
}

/// The watch.
struct BootTrial {
    /// The folder of the machine's record — handed in, so a test reads a
    /// folder of its own.
    record: PathBuf,
    /// The failed trial told of in this process, for a brain with nowhere
    /// to keep it.
    told: Mutex<Option<String>>,
}

impl BootTrial {
    fn reading(record: PathBuf) -> Self {
        Self {
            record,
            told: Mutex::new(None),
        }
    }

    /// The toplevel of the trial the record says failed, if one did.
    fn failed(&self) -> Option<String> {
        let places = Places {
            dir: PathBuf::new(),
            state: self.record.clone(),
            run: PathBuf::new(),
        };
        let state = ronoctua_machine::state(&places).ok()?;
        state
            .trial
            .filter(|t| t.outcome == TrialOutcome::Failed)
            .map(|t| t.toplevel)
    }

    /// Reads the record, and tells a failure nobody was told of.
    fn look(&self, ctx: &Ctx) {
        // WHERE what was told is kept is the brain's to say (`Ctx`), and the
        // part that reads and writes it is handed the answer.
        self.tell_once(ctx, ctx.state_dir().map(|d| d.join(TOLD_FILE)));
    }

    /// [`Self::look`], with the file that remembers what was told.
    fn tell_once(&self, ctx: &Ctx, file: Option<PathBuf>) {
        let Some(failed) = self.failed() else {
            return;
        };
        let mut told = self
            .told
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if told.is_none() {
            *told = file
                .as_ref()
                .and_then(|f| std::fs::read_to_string(f).ok())
                .map(|t| t.trim().to_owned());
        }
        if told.as_deref() == Some(failed.as_str()) {
            return;
        }
        ctx.tell(Event::of(&super::event::FAILED));
        if let Some(file) = &file {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(file, &failed);
        }
        *told = Some(failed);
    }
}

impl Module for BootTrial {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }
}

impl Watcher for BootTrial {
    fn sources(&self) -> Vec<Source> {
        vec![Source::Path(self.record.clone())]
    }

    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.look(ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        if matches!(heard, Heard::Path { .. }) {
            self.look(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::module::Kept;
    use crate::prefs::Where;

    const PENDING: &str = r#"{"format": 1, "last-good": null,
        "trial": {"toplevel": "/nix/store/00000000000000000000000000000001-nixos-system-b", "outcome": "pending"},
        "hold": null, "last-apply": null}"#;

    fn scratch(what: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-boot-fallback-{what}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("record")).unwrap();
        dir
    }

    fn ctx(dir: &std::path::Path) -> (Ctx, Arc<Kept>) {
        let kept = Arc::new(Kept::default());
        let roots = Where {
            config: None,
            state: Some(dir.join("state")),
        };
        (
            Ctx::new("boot-fallback", 0, roots, false, kept.clone()),
            kept,
        )
    }

    /// FAILS IF: a failed trial is not told, is told twice — in one process
    /// or across a login — or a trial that is only pending is told at all.
    #[test]
    fn a_failed_trial_is_told_once() {
        let dir = scratch("once");
        let watch = BootTrial::reading(dir.join("record"));
        let record = dir.join("record/state.json");
        std::fs::write(&record, PENDING).unwrap();
        let (c, kept) = ctx(&dir);
        watch.started(0, &c);
        assert!(kept.events().is_empty(), "a pending trial is no news");

        std::fs::write(&record, PENDING.replace("pending", "failed")).unwrap();
        watch.heard(
            Heard::Path {
                source: 0,
                path: &record,
            },
            &c,
        );
        let told = kept.events();
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].1.tag, super::super::event::FAILED.tag);
        watch.heard(
            Heard::Path {
                source: 0,
                path: &record,
            },
            &c,
        );
        assert_eq!(kept.events().len(), 1, "told once in this process");

        // The next login: a new brain, the same record.
        let again = BootTrial::reading(dir.join("record"));
        let (c2, kept2) = ctx(&dir);
        again.started(0, &c2);
        assert!(kept2.events().is_empty(), "told once across logins");

        // Another trial failing later is news again.
        std::fs::write(
            &record,
            PENDING
                .replace("pending", "failed")
                .replace("0001-nixos-system-b", "0002-nixos-system-c"),
        )
        .unwrap();
        again.heard(
            Heard::Path {
                source: 0,
                path: &record,
            },
            &c2,
        );
        assert_eq!(kept2.events().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the watch the brain builds (`new`) listens anywhere but the
    /// folder the machine's record is written into.
    #[test]
    fn it_watches_the_records_folder() {
        let module = new(&crate::module::Birth::detached());
        let sources = module.watcher().map(Watcher::sources).unwrap_or_default();
        assert!(
            matches!(sources.as_slice(), [Source::Path(p)] if p == std::path::Path::new(ronoctua_machine::STATE_DIR)),
            "{sources:?}"
        );
    }
}
