//! The stick engine: the installer image downloaded, verified and assembled,
//! then — when the person armed a write — handed to the root writer.
//!
//! # One job, the atlas's shape
//!
//! `Work::Atlas` downloads and then bakes, in one job, because the middle
//! state — a file on disk that is nobody's — is what a queue exists never to
//! leave lying about. This is the same with one more half: download every
//! asset through aria2 (which resumes and retries what a 4 GiB file on bad
//! wifi needs), check each against `SHA256SUMS`, join the parts into the ISO
//! checking the whole, and only then, if armed, stage the request for the
//! unit that the person's password already started.
//!
//! # Nothing before consent
//!
//! The door let the three hosts (`ronoctua_stick::HOSTS`) with one row; this
//! asks `ronoctua_consent` for a pass for every asset's host and for the
//! host GitHub redirects downloads to — read at the moment of asking, so a
//! consent taken back after the job was queued stops it. A pass cannot cross
//! the socket, so it is minted here, from the same file.
//!
//! # One stick job at a time
//!
//! Every stick job shares one cache folder, one staging folder and one
//! `progress.json`. Two at once would delete each other's `SHA256SUMS`, hand
//! aria2 the same path twice, and stage two requests for one waiting unit.
//! So a job waits for its [`Turn`] before it touches any of them: a write
//! armed while "download only" runs waits for that download — and then finds
//! the image in the cache, verified, and goes straight to the drive. (The
//! view refuses a second WRITE while one is armed or under way; this is the
//! queue's half, which also holds for the command line.)
//!
//! # The write is root's, and this only follows it
//!
//! `ronoctua_stick::stage` writes the request and `ready`; the unit answers
//! in `/run/ronoctua-stick/progress.json`, which this watches on inotify
//! while the row runs. Only an answer naming THIS request's number
//! ([`Request::nonce`]) is read — any other is another write's; no answer of
//! ours within [`NO_WRITER`] is a unit that is not there, said on the row.
//! Giving the row up writes an empty `cancel`, which the unit reads between
//! two blocks — and a job that fails for any other reason writes `cancel`
//! holding the stick's own refusal (a part that does not match, a download
//! that did not finish), so a unit never waits its twelve hours for nothing
//! and the view says why instead of "given up".

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, Result, bail};
use ronoctua_files::watch::{Control, Flow};
use ronoctua_stick::release::{self, keep_only, marker_name_of, scratch_name_of};
use ronoctua_stick::{Phase, Refusal, Release, Request, Sums, Target, Why};

use crate::engine::{POLL, Progress};
use crate::job::{Stage, Work};
use crate::ledger::Shared;

/// How long a staged write waits for the unit to say it began: the unit was
/// started with the password, so it is there within a second or not at all.
pub const NO_WRITER: Duration = Duration::from_secs(60);

/// Whose turn it is among the stick's jobs: one at a time.
pub struct Turn {
    taken: Mutex<bool>,
    freed: Condvar,
}

/// The daemon's one turn.
static TURN: Turn = Turn::new();

/// A turn held: given back when it is dropped.
pub struct Held<'a>(&'a Turn);

impl Drop for Held<'_> {
    fn drop(&mut self) {
        *self.0.taken.lock().unwrap_or_else(PoisonError::into_inner) = false;
        self.0.freed.notify_all();
    }
}

impl Turn {
    /// Nobody's yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            taken: Mutex::new(false),
            freed: Condvar::new(),
        }
    }

    /// Waits for the turn; `None` when the switch said stop while waiting.
    /// The switch has no signal of its own, so the wait wakes at the
    /// engine's pace to hear it — and only while there is someone ahead.
    pub fn take(&self, switch: &Control) -> Option<Held<'_>> {
        let mut taken = self.taken.lock().unwrap_or_else(PoisonError::into_inner);
        while *taken {
            if switch.flow() == Flow::Stop {
                return None;
            }
            taken = self
                .freed
                .wait_timeout(taken, POLL)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *taken = true;
        Some(Held(self))
    }
}

impl Default for Turn {
    fn default() -> Self {
        Self::new()
    }
}

/// An error tagged with the stick's refusal for it — what the unit is told
/// in `cancel`, and what the view then says.
fn refused(why: Why, text: String) -> anyhow::Error {
    anyhow::anyhow!(text).context(Refusal::of(why))
}

/// The refusal an error was tagged with; [`Why::Failed`] for one that was
/// not.
fn refusal_of(e: &anyhow::Error) -> Why {
    e.downcast_ref::<Refusal>().map_or(Why::Failed, |r| r.why)
}

/// Runs one job to the end, or to whatever the switch says.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Stick {
        release,
        into,
        write,
    } = &work
    else {
        tracing::error!("the stick engine got a job that is not a stick's");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    let staging = Path::new(ronoctua_stick::STAGING);
    shared.touch(id, |j| {
        j.stage = Stage::Running;
        j.now.clone_from(&release.label);
    });
    let outcome = {
        // THE TURN IS HELD until the unit was told, so the next job's
        // `ready` never lands before this one's `cancel`.
        let held = TURN.take(switch);
        let outcome = if held.is_some() {
            carry(shared, id, switch, release, into, write.as_deref(), staging)
        } else {
            Err(anyhow::anyhow!("given up"))
        };
        if let Err(e) = &outcome
            && write.is_some()
        {
            // THE UNIT IS WAITING for this write: tell it there will be none,
            // and — unless the person gave it up — why.
            let why = (switch.flow() != Flow::Stop).then(|| refusal_of(e));
            let _ = ronoctua_stick::staging::cancel_for(staging, why);
        }
        drop(held);
        outcome
    };
    match outcome {
        Ok(()) => shared.touch(id, |j| j.stage = Stage::Done),
        Err(e) => {
            let cancelled = switch.flow() == Flow::Stop;
            let text = format!("{e:#}");
            shared.touch(id, |j| {
                j.stage = if cancelled {
                    Stage::Cancelled
                } else {
                    Stage::Failed
                };
                if !cancelled {
                    j.trouble = Some(text);
                }
            });
        }
    }
}

/// Every pass the job needs, minted now from the person's own file.
///
/// # Errors
///
/// A host the person has not let, by name.
pub fn passes(release: &Release) -> Result<()> {
    let mut hosts: Vec<&str> = release
        .image
        .assets()
        .iter()
        .map(|a| a.url.as_str())
        .chain([release.sums.url.as_str()])
        .map(|url| release::host_of(url).context("an asset's address has no host"))
        .collect::<Result<_>>()?;
    hosts.push(ronoctua_stick::ASSET_HOST);
    hosts.dedup();
    for host in hosts {
        if ronoctua_consent::pass(host).is_err() {
            bail!("not let: {host} — the stick view's consent row lets it");
        }
    }
    Ok(())
}

fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    release: &Release,
    into: &Path,
    write: Option<&Target>,
    staging: &Path,
) -> Result<()> {
    shared.touch(id, |j| {
        j.stage = Stage::Running;
        j.now.clone_from(&release.label);
    });
    if let Some(target) = write
        && !target.fits(release.image.size_bytes())
    {
        return Err(refused(
            Why::NotWritable,
            format!(
                "the image is {} bytes and {} holds {}",
                release.image.size_bytes(),
                target.label,
                target.size_bytes
            ),
        ));
    }
    std::fs::create_dir_all(into).with_context(|| format!("could not make {}", into.display()))?;
    let iso = release.iso_name();
    let sha = match release::cached(into, release) {
        // ALREADY HERE AND VERIFIED: the marker is written only after the
        // whole was checked against the release's own line.
        Some(sha) => sha,
        None => {
            passes(release).map_err(|e| e.context(Refusal::of(Why::NotLet)))?;
            fetch(shared, id, switch, release, into, &iso)?
        }
    };
    match write {
        None => Ok(()),
        Some(target) => follow(
            shared,
            id,
            switch,
            staging,
            &Request {
                format: ronoctua_stick::FORMAT,
                nonce: nonce_for(id),
                image: into.join(&iso),
                sha256: sha,
                target: target.clone(),
            },
        ),
    }
}

/// Downloads, verifies and assembles; answers the whole's digest.
fn fetch(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    release: &Release,
    into: &Path,
    iso: &str,
) -> Result<String> {
    let scratch = into.join(scratch_name_of(iso));
    std::fs::create_dir_all(&scratch)
        .with_context(|| format!("could not make {}", scratch.display()))?;
    // A SUMS FILE IS NEVER RESUMED: aria2 would keep an older one whole and
    // rename the new beside it.
    let _ = std::fs::remove_file(scratch.join(&release.sums.name));
    let download = |e: anyhow::Error| e.context(Refusal::of(Why::Download));
    crate::aria2::carry(shared, id, switch, &release.sums.url, &scratch, None).map_err(download)?;
    let no_release = |text: &str| refused(Why::NoRelease, text.to_owned());
    let text = std::fs::read_to_string(scratch.join(&release.sums.name))
        .map_err(|_| no_release("SHA256SUMS did not arrive"))?;
    let sums = Sums::parse(&text).map_err(|_| no_release("SHA256SUMS is not one"))?;
    if !sums.covers(release) {
        return Err(no_release(
            "SHA256SUMS does not name every file of the release",
        ));
    }
    let whole = sums
        .of(iso)
        .ok_or_else(|| no_release("SHA256SUMS has no line for the ISO"))?
        .to_owned();

    for asset in release.image.assets() {
        let path = scratch.join(&asset.name);
        let wanted = sums
            .of(&asset.name)
            .ok_or_else(|| no_release("an asset without its line"))?;
        // A PART ALREADY HERE AND RIGHT is not fetched again: a job given up
        // at the fourth part resumes at the fifth.
        if !path.is_file() || !matches_sum(shared, id, &path, wanted, &asset.name)? {
            crate::aria2::carry(shared, id, switch, &asset.url, &scratch, None)
                .map_err(download)?;
            if !matches_sum(shared, id, &path, wanted, &asset.name)? {
                let _ = std::fs::remove_file(&path);
                return Err(refused(
                    Why::Mismatch,
                    format!("{} does not match SHA256SUMS", asset.name),
                ));
            }
        }
        if switch.flow() == Flow::Stop {
            bail!("given up");
        }
    }

    // A FAILED JOIN is a cache with no room left, almost always: the
    // download's own refusal.
    let landed = assemble(shared, id, switch, release, &scratch, into, iso).map_err(download)?;
    if !ronoctua_stick::digest::same(&landed, &whole) {
        let _ = std::fs::remove_file(into.join(iso));
        return Err(refused(
            Why::Mismatch,
            "the assembled ISO does not match SHA256SUMS".to_owned(),
        ));
    }
    ronoctua_stick::staging::write_atomically(
        into,
        &marker_name_of(iso),
        format!("{whole}\n").as_bytes(),
        0o600,
    )
    .context("could not write the image's marker")?;
    let _ = std::fs::remove_dir_all(&scratch);
    keep_only(into, iso);
    Ok(whole)
}

/// Whether the file at `path` has the digest `wanted`, the row saying it
/// reads it.
fn matches_sum(
    shared: &Arc<Shared>,
    id: u64,
    path: &Path,
    wanted: &str,
    name: &str,
) -> Result<bool> {
    let total = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut file = std::fs::File::open(path).with_context(|| format!("could not open {name}"))?;
    let mut progress = Progress::new(shared, id);
    let (sha, _) = ronoctua_stick::digest::of_reader(&mut file, &mut |done| {
        progress.set(done, total, name);
    })?;
    progress.flush();
    Ok(ronoctua_stick::digest::same(&sha, wanted))
}

/// The ISO in `into`, from the scratch's asset(s), hashed on the way;
/// answers its digest. A whole asset is renamed; parts are joined into a
/// scratch file renamed at the end.
fn assemble(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    release: &Release,
    scratch: &Path,
    into: &Path,
    iso: &str,
) -> Result<String> {
    let total = release.image.size_bytes();
    let mut progress = Progress::new(shared, id);
    let assets = release.image.assets();
    let joined: PathBuf = scratch.join(format!("{iso}.joined"));
    let sha = if let [one] = assets.as_slice() {
        let from = scratch.join(&one.name);
        let mut file = std::fs::File::open(&from).context("the image went away")?;
        let (sha, _) = ronoctua_stick::digest::of_reader(&mut file, &mut |done| {
            progress.set(done, total, iso);
        })?;
        std::fs::rename(&from, &joined).context("could not move the image")?;
        sha
    } else {
        let mut out = std::fs::File::create(&joined).context("could not start the ISO")?;
        let mut sha = ronoctua_stick::digest::Sha256::new();
        let mut done = 0u64;
        let mut buffer = vec![0u8; 1 << 20];
        for asset in assets {
            let mut part = std::fs::File::open(scratch.join(&asset.name))
                .with_context(|| format!("{} went away", asset.name))?;
            loop {
                let n = std::io::Read::read(&mut part, &mut buffer)?;
                if n == 0 {
                    break;
                }
                sha.update(&buffer[..n]);
                std::io::Write::write_all(&mut out, &buffer[..n])?;
                done += n as u64;
                progress.set(done, total, iso);
            }
            if switch.flow() == Flow::Stop {
                bail!("given up");
            }
        }
        out.sync_all()?;
        sha.hex()
    };
    progress.flush();
    std::fs::rename(&joined, into.join(iso)).context("could not place the ISO")?;
    Ok(sha)
}

/// Stages the request and follows the root unit until it is over.
fn follow(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    staging: &Path,
    request: &Request,
) -> Result<()> {
    let progress_file = Path::new(ronoctua_stick::PROGRESS_FILE);
    let watched = progress_file.parent().unwrap_or_else(|| Path::new("/"));
    let inotify = watch(watched).context("could not watch the writer's progress")?;
    ronoctua_stick::stage(staging, request)
        .map_err(|r| anyhow::anyhow!("the write could not be staged: {r}"))?;
    let total = std::fs::metadata(&request.image)
        .map(|m| m.len())
        .unwrap_or(0);
    let mut progress = Progress::new(shared, id);
    let started = std::time::Instant::now();
    let mut gave_up = false;
    loop {
        if switch.flow() == Flow::Stop && !gave_up {
            let _ = ronoctua_stick::staging::cancel(staging);
            gave_up = true;
        }
        match own_progress(progress_file, request.nonce) {
            // THE BAR SPANS BOTH HALVES: writing is the first, reading back
            // the second, so it moves once from empty to full.
            Some(p) if p.phase == Phase::Writing => {
                progress.set(p.done_bytes, total * 2, &request.target.label);
            }
            Some(p) if p.phase == Phase::Verifying => {
                progress.set(total + p.done_bytes, total * 2, &request.target.label);
            }
            Some(p) if p.phase == Phase::Done => {
                progress.set(total * 2, total * 2, &request.target.label);
                progress.flush();
                return Ok(());
            }
            Some(p) if p.phase == Phase::Failed => {
                progress.flush();
                match p.why {
                    Some(why) => {
                        return Err(refused(
                            why,
                            format!("the stick writer refused: {}", why.id()),
                        ));
                    }
                    None => bail!("given up"),
                }
            }
            _ if started.elapsed() > NO_WRITER => {
                bail!(
                    "the stick writer is not running: no word from it in {} s",
                    NO_WRITER.as_secs()
                )
            }
            _ => {}
        }
        quiet(&inotify, POLL);
    }
}

/// `progress.json`, when it is about the request numbered `nonce`.
fn own_progress(path: &Path, nonce: u64) -> Option<ronoctua_stick::Progress> {
    ronoctua_stick::staging::read_progress(path).filter(|p| p.nonce == Some(nonce))
}

/// A number for job `id`'s request, never `0`: the clock's nanoseconds, the
/// daemon's pid and the job's number — unique on this machine, which is all
/// a request's number has to be (it is not a secret: the person writes the
/// request themselves).
fn nonce_for(id: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let clock = now.as_secs().rotate_left(30) ^ u64::from(now.subsec_nanos());
    (clock ^ (u64::from(std::process::id()) << 44) ^ id).max(1)
}

/// An inotify on the progress's folder.
fn watch(dir: &Path) -> std::io::Result<rustix::fd::OwnedFd> {
    use rustix::fs::inotify;
    let fd = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)?;
    inotify::add_watch(
        &fd,
        dir,
        inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::ONLYDIR,
    )?;
    Ok(fd)
}

/// Waits until the folder moves or `most` passes — the second only so the
/// switch is heard — and drains what the kernel said.
fn quiet(fd: &rustix::fd::OwnedFd, most: Duration) {
    let deadline = rustix::event::Timespec {
        tv_sec: i64::try_from(most.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(most.subsec_nanos()),
    };
    let mut fds = [rustix::event::PollFd::new(fd, rustix::event::PollFlags::IN)];
    let _ = rustix::event::poll(&mut fds, Some(&deadline));
    let mut buffer = [0u8; 4096];
    while let Ok(n) = rustix::io::read(fd, &mut buffer) {
        if n == 0 {
            break;
        }
    }
}

// ── anchor: L17 ──

#[cfg(test)]
mod tests {
    use super::*;

    /// ANOTHER WRITE'S ANSWER IS NOT THIS JOB'S: a `Done` about another
    /// request — however fresh — does not end this row; one about this
    /// request does, and one about no request (the unit waiting) is nobody's.
    #[test]
    fn a_progress_of_another_write_is_not_this_ones() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-transfers-stick-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("progress.json");
        let theirs = ronoctua_stick::staging::progress(Some(1), Phase::Done, 1, 1, None);
        ronoctua_stick::staging::write_progress(&path, &theirs).unwrap();
        assert_eq!(own_progress(&path, 2), None, "another write's Done");
        let waiting = ronoctua_stick::staging::progress(None, Phase::Waiting, 0, 0, None);
        ronoctua_stick::staging::write_progress(&path, &waiting).unwrap();
        assert_eq!(own_progress(&path, 2), None, "a unit with no request yet");
        let ours = ronoctua_stick::staging::progress(Some(2), Phase::Writing, 1, 9, None);
        ronoctua_stick::staging::write_progress(&path, &ours).unwrap();
        assert_eq!(own_progress(&path, 2), Some(ours));
        assert_ne!(nonce_for(0), 0);
        assert_ne!(nonce_for(3), nonce_for(4), "two jobs, two numbers");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ONE STICK JOB AT A TIME: a second waits for the first to give the
    /// turn back, and a second given up while it waits leaves at once.
    #[test]
    fn stick_jobs_take_turns_and_a_waiting_one_hears_its_switch() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let turn = Arc::new(Turn::new());
        let first = Control::default();
        let held = turn.take(&first).expect("nobody ahead");
        let took = Arc::new(AtomicBool::new(false));
        let second = {
            let (turn, took) = (Arc::clone(&turn), Arc::clone(&took));
            std::thread::spawn(move || {
                let switch = Control::default();
                let held = turn.take(&switch);
                took.store(held.is_some(), Ordering::SeqCst);
            })
        };
        std::thread::sleep(Duration::from_millis(100));
        assert!(!took.load(Ordering::SeqCst), "the second waited");
        drop(held);
        second.join().unwrap();
        assert!(
            took.load(Ordering::SeqCst),
            "and took the turn once it was free"
        );

        let _held = turn.take(&first).expect("free again");
        let gave_up = Control::default();
        gave_up.set(Flow::Stop);
        assert!(turn.take(&gave_up).is_none(), "given up while waiting");
    }

    /// A JOB'S FAILURE IS SAID AS THE STICK'S REFUSAL: what the queue
    /// tagged reaches the unit's `cancel` by its reason, and anything else is
    /// a plain failure.
    #[test]
    fn a_failure_carries_its_refusal() {
        let e = refused(Why::Mismatch, "x does not match".to_owned());
        assert_eq!(refusal_of(&e), Why::Mismatch);
        assert!(format!("{e:#}").contains("x does not match"));
        let e = anyhow::anyhow!("aria2 failed").context(Refusal::of(Why::Download));
        assert_eq!(refusal_of(&e.context("and then this")), Why::Download);
        assert_eq!(refusal_of(&anyhow::anyhow!("a disk error")), Why::Failed);
    }

    #[test]
    fn parts_are_joined_and_hashed_whole() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-transfers-join-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scratch = dir.join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join("x.iso.part-00"), b"hello ").unwrap();
        std::fs::write(scratch.join("x.iso.part-01"), b"world").unwrap();
        let asset = |name: &str, size| ronoctua_stick::Asset {
            name: name.to_owned(),
            url: String::new(),
            size_bytes: size,
        };
        let release = Release {
            tag: "v".to_owned(),
            label: "1".to_owned(),
            published_at: String::new(),
            image: ronoctua_stick::Image::Parts {
                parts: vec![asset("x.iso.part-00", 6), asset("x.iso.part-01", 5)],
            },
            sums: asset("SHA256SUMS", 1),
        };
        let shared = Arc::new(Shared::fresh_one());
        let switch = Arc::new(Control::default());
        let sha = assemble(&shared, 0, &switch, &release, &scratch, &dir, "x.iso").unwrap();
        assert_eq!(std::fs::read(dir.join("x.iso")).unwrap(), b"hello world");
        let mut expected = ronoctua_stick::digest::Sha256::new();
        expected.update(b"hello world");
        assert_eq!(sha, expected.hex());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
