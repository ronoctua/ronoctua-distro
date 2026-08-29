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
//! # Why compressing is cancellable at all
//!
//! `bsdtar` is a child process and the only way to stop one is to kill it. It
//! is `spawn` and a poll rather than `output()` for exactly that: `output()`
//! waits for the end, and waiting for the end is the one thing a cancel button
//! must not do.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use ronoctua_files::watch::{self, Control, Flow, Watch};

use crate::job::{Job, Stage, Work};
use crate::ledger::Shared;

/// How often a running job's numbers reach the list.
///
/// A little over a hundred milliseconds: under what an eye reads as stutter,
/// and eight times slower than a sixty-hertz frame, so a copy at full speed
/// never costs more than one repaint out of eight.
pub const BEAT: Duration = Duration::from_millis(120);

/// How often a spawned child is looked at while it runs.
const POLL: Duration = Duration::from_millis(100);

/// Where the timezone comes from, for the trash's timestamps.
const LOCALTIME: &str = "/etc/localtime";

/// Carries one job through, from weighing to whatever ends it.
///
/// It runs on a thread of its own and reports everything through `shared`. It
/// never returns a failure: a job that failed is a job whose row says so, and
/// an error crossing back to a caller that has already moved on has nowhere to
/// be shown.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    // WEIGHING FIRST, and it is a stage. On a folder of a hundred thousand
    // files it is the longest part of a copy that has not started, and a row
    // sitting at zero with no explanation reads as stuck.
    let total = match weigh(&work, switch) {
        Ok(t) => t,
        Err(e) => return end_it(shared, id, &e),
    };
    shared.touch(id, |j| {
        j.total = total;
        j.stage = Stage::Running;
    });

    let mut beats = Beats::new(shared, id);
    let outcome = {
        let mut sink = |b: watch::Beat<'_>| beats.take(b);
        carry_out(&work, &mut Watch::new(switch, &mut sink))
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
        // NOBODY KNOWS. What decides how long a compressor takes is the
        // compressor, and an archive's entry count is inside the archive. Zero
        // means unknown, and the row shows the stage instead of a bar that
        // would be a guess.
        Work::Compress { .. } | Work::Extract { .. } | Work::Measure { .. } => 0,
        // THE NETWORK ONES NEVER ARRIVE HERE, and the arm is not dead code:
        // `serve` routes by `Work::engine()`, so reaching this means the router
        // and the model disagree. An error puts that on the row where somebody
        // sees it; a panic would take the unit down and every copy with it.
        Work::Fetch { .. } | Work::Media { .. } | Work::Torrent { .. } => {
            anyhow::bail!("a network job handed to the local engine")
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
        Work::Compress {
            directory,
            names,
            archive,
        } => {
            let cmd = ronoctua_files::act::packing(directory, names, archive)?;
            waited(cmd, w).map(|()| None)
        }
        Work::Extract { archive, into } => {
            let cmd = ronoctua_files::act::unpacking(archive, into)?;
            waited(cmd, w).map(|()| None)
        }
        Work::Measure { path } => watch::weigh(path, w).map(Some),
        // THE NETWORK ONES NEVER ARRIVE HERE, and the arm is not dead code:
        // `serve` routes by `Work::engine()`, so reaching this means the router
        // and the model disagree. An error puts that on the row where somebody
        // sees it; a panic would take the unit down and every copy with it.
        Work::Fetch { .. } | Work::Media { .. } | Work::Torrent { .. } => {
            anyhow::bail!("a network job handed to the local engine")
        }
    }
}

/// Runs a child, looking at the switch while it does.
///
/// # Errors
///
/// When the child cannot start, ends badly, or somebody pressed cancel — and
/// in that last case the child is killed rather than waited for.
fn waited(mut cmd: std::process::Command, w: &mut Watch<'_>) -> Result<()> {
    let mut child = cmd.spawn().context("start the file's process")?;
    loop {
        match child.try_wait().context("wait for the file's process")? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => anyhow::bail!("the program exited with {status}"),
            None => {}
        }
        // The switch, and then a nap. Killing on cancel is the only way to stop
        // a child, and it is what makes "extract" a row with a stop on it.
        match w.flow() {
            Flow::Run | Flow::Pause => std::thread::sleep(POLL),
            Flow::Stop => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(watch::Cancelled.into());
            }
        }
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
