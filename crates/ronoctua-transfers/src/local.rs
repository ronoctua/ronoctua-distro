//! The engine that moves this machine's own files.
//!
//! # Why the numbers are throttled on the way in
//!
//! A copy beats once per [`ronoctua_files::watch::CHUNK`] — eight mebibytes —
//! and a fast NVMe does two gibibytes a second. That is two hundred and fifty
//! beats a second, each one taking the queue's lock, bumping the version and
//! waking every reader, which would wake the footer to repaint two hundred and
//! fifty times for a bar that has nine hundred and ninety-nine positions.
//!
//! So the counter is kept in this thread and reaches the list every [`BEAT`].
//! The last one is always sent, whatever the clock says: a job that ends at
//! 97% because its final beat was swallowed is worse than one that never moved.
//!
//! # Why an archive's work can be held and given up
//!
//! 7zz, bsdtar and unrar are child processes of `ronoctua-archive`, and the
//! only way to hold one is `SIGSTOP` and to stop one is to kill it. The job
//! hands the archive crate a [`ronoctua_archive::Switch`] that reads this
//! job's own [`Control`]: a hold on the row holds the program's whole process
//! group, and a cancel kills it — the crate then removes what it had written,
//! and the row ends cancelled, not failed.
//!
//! # And why a password never outlives its job
//!
//! A job that needs one carries the PATH of a `0600` file the door wrote
//! (`crate::secrets`). Whatever ends the job — done, refused, cancelled — the
//! last thing [`run`] does is remove it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ronoctua_files::watch::{self, Control, Flow, Watch};

/// An `anyhow` result, for the archive engine's own functions.
type Result<T> = anyhow::Result<T>;

use crate::job::{Job, Stage, Work};
use crate::ledger::Shared;

/// How often a running job's numbers reach the list.
///
/// A little over a hundred milliseconds: under what an eye reads as stutter,
/// and eight times slower than a sixty-hertz frame, so a copy at full speed
/// never costs more than one repaint out of eight.
pub const BEAT: Duration = Duration::from_millis(120);

/// Where the timezone comes from, for the trash's timestamps.
const LOCALTIME: &str = "/etc/localtime";

/// Carries one job through, from weighing to whatever ends it.
///
/// It runs on a thread of its own and reports everything through `shared`. It
/// never returns a failure: a job that failed is a job whose row says so, and
/// an error crossing back to a caller that has already moved on has nowhere to
/// be shown.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    run_it(shared, id, switch, &work);
    // THE PASSWORD GOES WITH THE JOB, however it ended — and only from the
    // queue's own folder.
    if let Some(dir) = shared.secrets() {
        for secret in work.secrets() {
            crate::secrets::forget_in(secret, dir);
        }
    }
}

/// [`run`], minus the removal of the job's secrets — so that removal cannot be
/// skipped by any of the ways this returns.
fn run_it(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: &Work) {
    // WEIGHING FIRST, and it is a stage. On a folder of a hundred thousand
    // files it is the longest part of a copy that has not started, and a row
    // sitting at zero with no explanation reads as stuck.
    let total = match weigh(work, switch) {
        Ok(t) => t,
        Err(e) => return end_it(shared, id, &e),
    };
    shared.touch(id, |j| {
        j.total = total;
        j.stage = Stage::Running;
    });

    // AN ARCHIVE'S WORK IS THE ARCHIVE CRATE'S, with its own counter: the
    // program says where it is (a per cent, a member), and the job SETS that
    // rather than adding beats up.
    if let Some(outcome) = archived(shared, id, switch, work) {
        match outcome {
            Ok(()) => shared.touch(id, |j| {
                j.stage = Stage::Done;
                if j.total == 0 {
                    j.total = j.done;
                }
            }),
            Err(e) => end_it(shared, id, &e),
        }
        return;
    }

    let mut beats = Beats::new(shared, id);
    let outcome = {
        let mut sink = |b: watch::Beat<'_>| beats.take(b);
        carry_out(work, &mut Watch::new(switch, &mut sink))
    };
    // THE LAST ONE ALWAYS GOES OUT. A job that ends at 97% because its final
    // beat fell inside the throttle window is a bar that lies at the one moment
    // somebody is looking at it.
    beats.flush();

    match outcome {
        Ok(answer) => shared.touch(id, |j| {
            j.stage = Stage::Done;
            j.answer = answer;
            // A total that was never known — a compressor's output — ends at
            // whatever was counted, so the row reads as complete instead of as
            // a bar that stopped somewhere.
            if j.total == 0 {
                j.total = j.done;
            }
        }),
        Err(e) => end_it(shared, id, &e),
    }
}

/// Writes the ending a failure or a cancellation leaves behind.
fn end_it(shared: &Arc<Shared>, id: u64, e: &anyhow::Error) {
    // A CANCELLATION IS NOT A FAILURE, and the difference decides whether a
    // sentence about something going wrong appears on screen. It is the whole
    // reason `watch::Cancelled` is a type of its own.
    if watch::was_cancelled(e) {
        shared.touch(id, |j| j.stage = Stage::Cancelled);
        return;
    }
    // AN ARCHIVE'S REFUSAL IS A WORD THE SCREEN HAS A SENTENCE FOR — and the
    // one that decides whether a door asks for a password and tries again. A
    // stop is not a refusal at all: somebody said so.
    if let Some(Refused(r)) = e.downcast_ref::<Refused>() {
        let why = r.why;
        if why == ronoctua_archive::Why::Stopped {
            shared.touch(id, |j| j.stage = Stage::Cancelled);
            return;
        }
        tracing::warn!(job = id, why = why.id(), tool = ?r.tool, "an archive's work was refused");
        // NO WORDS IN `trouble`: the archive crate hands back the WORD, not
        // the program's prose, and a word in `trouble` is what a screen draws
        // as it is — the queue's window drew `wrong-password`, in English, in
        // every language. The sentence is the door's (`files-archive-why-*`).
        shared.touch(id, |j| {
            j.stage = Stage::Failed;
            j.archive = Some(why);
            j.trouble = None;
        });
        return;
    }
    // `{e:#}` and not `{e}`: anyhow's chain is where the useful half lives —
    // `copiar /x/y` alone does not say `No space left on device`.
    let said = format!("{e:#}");
    tracing::warn!(job = id, trouble = %said, "the job ended badly");
    shared.touch(id, |j| {
        j.stage = Stage::Failed;
        j.trouble = Some(said.clone());
    });
}

/// How much there is, in whatever unit this work counts.
fn weigh(work: &Work, switch: &Arc<Control>) -> Result<u64> {
    let mut nothing = |_: watch::Beat<'_>| {};
    let mut w = Watch::new(switch, &mut nothing);
    Ok(match work {
        Work::Paste { paths, .. } => {
            let mut total = 0u64;
            for p in paths {
                total = total.saturating_add(watch::weigh(p, &mut w)?);
            }
            total
        }
        Work::Shred { paths } => {
            let mut total = 0u64;
            for p in paths {
                total = total.saturating_add(watch::weigh(p, &mut w)?);
            }
            total.saturating_mul(u64::from(ronoctua_files::act::PASSES))
        }
        Work::Delete { paths } => {
            let mut total = 0u64;
            for p in paths {
                total = total.saturating_add(watch::count(p, &mut w)?);
            }
            total
        }
        // The top-level paths ARE the units: the trash moves each one whole,
        // and walking inside to count entries would spend the wait measuring
        // something the operation never touches one by one.
        Work::Trash { paths, .. } => paths.len() as u64,
        Work::Restore { names, .. } => names.len() as u64,
        // NOBODY KNOWS YET. What decides how long a compressor takes is the
        // compressor: the total is written when the program first says how
        // far it is (see `archived`), and until then zero means unknown and the
        // row shows the stage instead of a bar that would be a guess.
        Work::Compress { .. }
        | Work::Extract { .. }
        | Work::ArchiveEdit { .. }
        | Work::ArchiveTest { .. }
        | Work::ArchiveConvert { .. }
        | Work::Measure { .. } => 0,
        // THE OTHER ENGINES' JOBS NEVER ARRIVE HERE, and the arm is not dead code:
        // `serve` routes by `Work::engine()`, so reaching this means the router
        // and the model disagree. An error puts that on the row where somebody
        // sees it; a panic would take the unit down and every copy with it.
        Work::Fetch { .. }
        | Work::Media { .. }
        | Work::Torrent { .. }
        | Work::Atlas { .. }
        | Work::Software { .. }
        | Work::Scan { .. }
        | Work::Container { .. }
        | Work::Belongings { .. }
        | Work::Seal { .. }
        | Work::Incus { .. }
        | Work::Stick { .. } => {
            anyhow::bail!("a job of another engine handed to the local engine")
        }
    })
}

/// Does the thing. The `Option<u64>` is the answer, for the work that has one.
fn carry_out(work: &Work, w: &mut Watch<'_>) -> Result<Option<u64>> {
    match work {
        Work::Paste { paths, into, moves } => {
            for p in paths {
                if *moves {
                    watch::move_into(p, into, w)?;
                } else {
                    watch::copy_into(p, into, w)?;
                }
            }
            Ok(None)
        }
        Work::Delete { paths } => {
            for p in paths {
                watch::remove(p, w)?;
            }
            Ok(None)
        }
        Work::Shred { paths } => {
            for p in paths {
                watch::shred(p, ronoctua_files::act::PASSES, w)?;
            }
            Ok(None)
        }
        Work::Trash { root, paths } => {
            let bin = ronoctua_files::trash::Bin::at(root.clone());
            let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new(LOCALTIME));
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
            for p in paths {
                w.allowed()?;
                bin.put(p, now, &zone)?;
                w.beat_one(p);
            }
            Ok(None)
        }
        Work::Restore { root, names } => {
            let bin = ronoctua_files::trash::Bin::at(root.clone());
            for n in names {
                w.allowed()?;
                let back = bin.restore(n)?;
                w.beat_one(&back);
            }
            Ok(None)
        }
        // THE ARCHIVE CRATE'S, and `archived` runs them before this is
        // reached: here they are a router that disagrees with itself.
        Work::Compress { .. }
        | Work::Extract { .. }
        | Work::ArchiveEdit { .. }
        | Work::ArchiveTest { .. }
        | Work::ArchiveConvert { .. } => {
            anyhow::bail!("an archive's work reached the file engine")
        }
        Work::Measure { path } => watch::weigh(path, w).map(Some),
        // THE OTHER ENGINES' JOBS NEVER ARRIVE HERE, and the arm is not dead code:
        // `serve` routes by `Work::engine()`, so reaching this means the router
        // and the model disagree. An error puts that on the row where somebody
        // sees it; a panic would take the unit down and every copy with it.
        Work::Fetch { .. }
        | Work::Media { .. }
        | Work::Torrent { .. }
        | Work::Atlas { .. }
        | Work::Software { .. }
        | Work::Scan { .. }
        | Work::Container { .. }
        | Work::Belongings { .. }
        | Work::Seal { .. }
        | Work::Incus { .. }
        | Work::Stick { .. } => {
            anyhow::bail!("a job of another engine handed to the local engine")
        }
    }
}

/// Why an archive's work did not happen, carried through `anyhow` to
/// [`end_it`], which writes it on the job as a word the screen can say.
#[derive(Debug)]
pub struct Refused(pub ronoctua_archive::Refusal);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the archive's work was refused: {}", self.0.why.id())
    }
}

impl std::error::Error for Refused {}

/// The archive's work, when `work` is one: `None` for everything else.
///
/// The programs are the archive crate's (`Tools::from_env`: store paths), held
/// by this job's [`Control`] — a hold holds the program, a cancel kills it.
fn archived(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    work: &Work,
) -> Option<Result<()>> {
    let control = Arc::clone(switch);
    let tools =
        ronoctua_archive::Tools::from_env().held_by(ronoctua_archive::Switch::new(move || {
            match control.flow() {
                Flow::Run => ronoctua_archive::Hold::Go,
                Flow::Pause => ronoctua_archive::Hold::Pause,
                Flow::Stop => ronoctua_archive::Hold::Stop,
            }
        }));
    let mut said = crate::engine::Progress::new(shared, id);
    let outcome = match work {
        Work::Compress {
            directory,
            names,
            archive,
            format,
            level,
            volume_bytes,
            hide_names,
            secret,
        } => {
            let sources: Vec<std::path::PathBuf> =
                names.iter().map(|n| directory.join(n)).collect();
            let weight = sources.iter().map(|p| ronoctua_files::act::weigh(p)).sum();
            let options = ronoctua_archive::Options {
                format: format.unwrap_or_else(|| ronoctua_archive::Format::of_path(archive)),
                level: *level,
                volume_bytes: *volume_bytes,
                secret: secret.clone(),
                hide_names: *hide_names,
            };
            tools.create(&sources, archive, &options, &mut |p| {
                heard(&mut said, weight, p);
            })
        }
        Work::Extract {
            archive,
            into,
            members,
            secret,
        } => {
            let weight = size_of(archive);
            tools.extract(archive, into, members, secret.as_deref(), &mut |p| {
                heard(&mut said, weight, p);
            })
        }
        Work::ArchiveEdit {
            archive,
            add,
            remove,
            secret,
        } => {
            let weight = if add.is_empty() {
                size_of(archive)
            } else {
                add.iter().map(|p| ronoctua_files::act::weigh(p)).sum()
            };
            let mut done = Ok(());
            if !add.is_empty() {
                done = tools.add(archive, add, secret.as_deref(), &mut |p| {
                    heard(&mut said, weight, p);
                });
            }
            if done.is_ok() && !remove.is_empty() {
                done = tools.remove(archive, remove, secret.as_deref());
            }
            done
        }
        Work::ArchiveTest { archive, secret } => tools.test(archive, secret.as_deref()),
        Work::ArchiveConvert {
            from,
            from_secret,
            to,
            format,
            level,
            volume_bytes,
            hide_names,
            secret,
        } => {
            let weight = size_of(from);
            let options = ronoctua_archive::Options {
                format: format.unwrap_or_else(|| ronoctua_archive::Format::of_path(to)),
                level: *level,
                volume_bytes: *volume_bytes,
                secret: secret.clone(),
                hide_names: *hide_names,
            };
            tools.convert(from, from_secret.as_deref(), to, &options, &mut |p| {
                heard(&mut said, weight, p);
            })
        }
        _ => return None,
    };
    said.flush();
    Some(outcome.map_err(|r| anyhow::Error::new(Refused(r))))
}

/// How big a file is on disk, or nothing — the weight of an archive's work.
fn size_of(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map_or(0, |m| m.len())
}

/// What the program said about where it is, onto the row: the member it is
/// on, and — once it says a per cent — the share of `weight` behind it. A
/// program that never says one leaves the total unknown, and the row says
/// the stage instead of a bar that would be a guess.
fn heard(said: &mut crate::engine::Progress<'_>, weight: u64, p: &ronoctua_archive::Progress) {
    let now = p
        .member
        .as_deref()
        .map(|m| m.rsplit('/').find(|s| !s.is_empty()).unwrap_or(m))
        .unwrap_or_default();
    match p.percent {
        Some(percent) if weight > 0 => {
            said.set(
                weight / 100 * u64::from(percent) + weight % 100 * u64::from(percent) / 100,
                weight,
                now,
            );
        }
        _ => said.set(0, 0, now),
    }
}

/// The counter that lives on the working thread.
struct Beats<'a> {
    shared: &'a Arc<Shared>,
    id: u64,
    done: u64,
    now: String,
    /// When the list last heard from us.
    said: Instant,
    /// Whether anything has happened since then.
    fresh: bool,
}

impl<'a> Beats<'a> {
    fn new(shared: &'a Arc<Shared>, id: u64) -> Self {
        Self {
            shared,
            id,
            done: 0,
            now: String::new(),
            said: Instant::now(),
            fresh: false,
        }
    }

    /// One beat from the operation.
    fn take(&mut self, beat: watch::Beat<'_>) {
        match beat {
            watch::Beat::On(path) => {
                // The NAME and not the path: the row is one line wide and the
                // directory is already the second half of the sentence.
                self.now = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            watch::Beat::Did(n) => self.done = self.done.saturating_add(n),
        }
        self.fresh = true;
        if self.said.elapsed() >= BEAT {
            self.flush();
        }
    }

    /// Puts what has piled up into the list.
    fn flush(&mut self) {
        if !self.fresh {
            return;
        }
        let (done, now) = (self.done, self.now.clone());
        self.shared.touch(self.id, move |j: &mut Job| {
            j.done = done;
            j.now = now;
        });
        self.said = Instant::now();
        self.fresh = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Engine, Stage};

    /// A folder of this test's own.
    fn place(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-transfers-local-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn job_of(shared: &Shared, id: u64) -> crate::job::Job {
        shared
            .snapshot()
            .0
            .into_iter()
            .find(|j| j.id == id)
            .expect("the job is on the list")
    }

    /// Runs `work` to its end on this thread, the way the socket starts one.
    fn ran(shared: &Arc<Shared>, work: Work) -> crate::job::Job {
        let (id, switch) = shared.take(Engine::Local, work.clone());
        run(shared, id, &switch, work);
        job_of(shared, id)
    }

    /// THE REAL PROGRAMS, or the test fails: one that returned early would
    /// prove nothing and pass.
    fn programs_are_here() {
        let tools = ronoctua_archive::Tools::from_env();
        assert!(
            tools.sevenzz.is_some() && tools.setsid.is_some() && tools.bsdtar.is_some(),
            "RONOCTUA_7ZZ, RONOCTUA_BSDTAR and RONOCTUA_SETSID are needed: run inside `nix develop` or the tests derivation"
        );
    }

    #[test]
    fn an_archive_is_made_and_read_through_the_queue_and_its_password_goes_with_the_job() {
        programs_are_here();
        let root = place("round");
        let secrets = root.join("secrets");
        let shared = Arc::new(Shared::default().with_secrets(secrets.clone()));
        let src = root.join("src");
        std::fs::create_dir_all(src.join("inner")).unwrap();
        std::fs::write(src.join("inner/note.txt"), "inside\n").unwrap();
        std::fs::write(src.join("top.txt"), "top\n").unwrap();

        let pw = crate::secrets::keep_in(&secrets, "queue horse").unwrap();
        let archive = root.join("made.7z");
        let made = ran(
            &shared,
            Work::Compress {
                directory: src,
                names: vec!["inner".into(), "top.txt".into()],
                archive: archive.clone(),
                format: Some(ronoctua_archive::Format::SevenZ),
                level: ronoctua_archive::Level::Maximum,
                volume_bytes: None,
                hide_names: true,
                secret: Some(pw.clone()),
            },
        );
        assert_eq!(
            made.stage,
            Stage::Done,
            "{:?} {:?}",
            made.trouble,
            made.archive
        );
        assert!(archive.exists());
        assert!(!pw.exists(), "the password outlived its job");

        // A WRONG PASSWORD IS A REFUSAL WITH A NAME, and it goes too.
        let wrong = crate::secrets::keep_in(&secrets, "wrong horse").unwrap();
        let into = root.join("into");
        let refused = ran(
            &shared,
            Work::Extract {
                archive: archive.clone(),
                into: into.clone(),
                members: Vec::new(),
                secret: Some(wrong.clone()),
            },
        );
        assert_eq!(refused.stage, Stage::Failed);
        assert_eq!(refused.archive, Some(ronoctua_archive::Why::WrongPassword));
        assert_eq!(
            refused.trouble, None,
            "the refusal's word is in `trouble`, where a screen draws it as it is"
        );
        assert!(!wrong.exists());
        // AND NONE AT ALL IS THE OTHER NAME, the one a door answers with a
        // question.
        let none = ran(
            &shared,
            Work::Extract {
                archive: archive.clone(),
                into: into.clone(),
                members: Vec::new(),
                secret: None,
            },
        );
        assert_eq!(none.archive, Some(ronoctua_archive::Why::NeedsPassword));

        let right = crate::secrets::keep_in(&secrets, "queue horse").unwrap();
        let out = ran(
            &shared,
            Work::Extract {
                archive: archive.clone(),
                into: into.clone(),
                members: vec!["inner/note.txt".into()],
                secret: Some(right.clone()),
            },
        );
        assert_eq!(out.stage, Stage::Done, "{:?}", out.archive);
        assert_eq!(
            std::fs::read_to_string(into.join("inner/note.txt")).unwrap(),
            "inside\n"
        );
        assert!(
            !into.join("top.txt").exists(),
            "only the member asked for came out"
        );
        assert!(!right.exists());

        // TESTED, EDITED AND CONVERTED, each a job.
        let pw = crate::secrets::keep_in(&secrets, "queue horse").unwrap();
        assert_eq!(
            ran(
                &shared,
                Work::ArchiveTest {
                    archive: archive.clone(),
                    secret: Some(pw)
                }
            )
            .stage,
            Stage::Done
        );
        let pw = crate::secrets::keep_in(&secrets, "queue horse").unwrap();
        let gone = ran(
            &shared,
            Work::ArchiveEdit {
                archive: archive.clone(),
                add: Vec::new(),
                remove: vec!["top.txt".into()],
                secret: Some(pw),
            },
        );
        assert_eq!(gone.stage, Stage::Done, "{:?}", gone.archive);
        let from = crate::secrets::keep_in(&secrets, "queue horse").unwrap();
        let tar = root.join("again.tar.zst");
        let converted = ran(
            &shared,
            Work::ArchiveConvert {
                from: archive,
                from_secret: Some(from),
                to: tar.clone(),
                format: None,
                level: ronoctua_archive::Level::Normal,
                volume_bytes: None,
                hide_names: false,
                secret: None,
            },
        );
        assert_eq!(converted.stage, Stage::Done, "{:?}", converted.archive);
        let members = ronoctua_archive::Tools::from_env()
            .list(&tar, None)
            .unwrap();
        let names: Vec<&str> = members
            .iter()
            .map(|m| m.path.trim_end_matches('/'))
            .collect();
        assert!(
            names.contains(&"inner/note.txt") && !names.contains(&"top.txt"),
            "{names:?}"
        );
        assert_eq!(
            std::fs::read_dir(&secrets).unwrap().count(),
            0,
            "a password was left behind in the queue's folder"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_cancelled_archive_job_ends_cancelled_and_not_failed() {
        programs_are_here();
        let root = place("cancel");
        let shared = Arc::new(Shared::default().with_secrets(root.join("secrets")));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        let work = Work::Compress {
            directory: src,
            names: vec!["a.txt".into()],
            archive: root.join("never.zip"),
            format: None,
            level: ronoctua_archive::Level::Normal,
            volume_bytes: None,
            hide_names: false,
            secret: None,
        };
        let (id, switch) = shared.take(Engine::Local, work.clone());
        // STOPPED BEFORE IT STARTED: the program is never run, and the row
        // says cancelled — the difference between a failure and a decision.
        switch.set(Flow::Stop);
        run(&shared, id, &switch, work);
        let job = job_of(&shared, id);
        assert_eq!(job.stage, Stage::Cancelled);
        assert_eq!(job.archive, None);
        assert!(!root.join("never.zip").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
