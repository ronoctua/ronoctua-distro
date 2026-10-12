//! The folder between the person and the root unit, and the file the unit
//! answers in.
//!
//! # The shape (the file manager's root mould, `nix/modules/files.nix`)
//!
//! [`crate::STAGING`] is the person's own (`0700`): nobody else on the
//! machine may put a request where a unit that runs as root will read it.
//! Three names live in it:
//!
//! - [`REQUEST_FILE`] — the [`Request`], whole, written atomically;
//! - [`READY_FILE`] — written LAST, after the request: the unit, started
//!   when the person gave the password, waits for it while the image
//!   downloads;
//! - [`CANCEL_FILE`] — the queue gave the job up: the unit stops waiting,
//!   or stops writing between two blocks. Empty, the person gave it up;
//!   holding a [`Why`]'s id, the QUEUE refused (a part that does not match,
//!   a download that did not finish), and the unit carries that reason into
//!   its answer, so the view says what happened instead of "given up".
//!
//! The answer comes back in [`crate::PROGRESS_FILE`], which is ROOT's, in a
//! folder that is root's (`0755`): a file root writes into a folder the
//! person owns is a symlink away from any file on the machine.
//!
//! # What the unit trusts of it: nothing it does not check
//!
//! The folder and every file are opened without following a link, must be
//! the person's (the folder's owner), the request is bounded and refused with
//! any field nobody declared, and `ready` counts only when it is newer than
//! the unit itself — a `ready` left behind by a crash does not write anyone's
//! stick.

use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::time::{Duration, SystemTime};

use rustix::fs::{Mode, OFlags};

use crate::{FORMAT, Phase, Progress, Refusal, Request, Why};

/// The request, in the staging folder.
pub const REQUEST_FILE: &str = "request.json";

/// Written after the request: write now.
pub const READY_FILE: &str = "ready";

/// The person gave the write up.
pub const CANCEL_FILE: &str = "cancel";

/// The most bytes of a request.
pub const REQUEST_MOST_BYTES: u64 = 64 << 10;

/// The most bytes of a `cancel`: one reason's id.
pub const REASON_MOST_BYTES: u64 = 64;

/// Whether a request is one a unit may be handed: this format, a number of
/// its own, a digest, a stable name under `/dev/disk/by-id/`, an absolute
/// image path.
#[must_use]
pub fn well_formed(request: &Request) -> bool {
    request.format == FORMAT
        && request.nonce != 0
        && crate::digest::is_sha256(&request.sha256)
        && request.image.is_absolute()
        && crate::device::is_by_id(&request.target.by_id)
        && request.target.size_bytes > 0
}

/// Stages a write for the root unit: the request, then `ready`.
///
/// # Errors
///
/// [`Why::Failed`] for a request that is not well formed
/// ([`well_formed`]) or a folder that could not be written.
pub fn stage(staging: &Path, request: &Request) -> Result<(), Refusal> {
    if !well_formed(request) {
        return Err(Refusal::of(Why::Failed));
    }
    let text = serde_json::to_vec(request).map_err(|_| Refusal::of(Why::Failed))?;
    let _ = std::fs::remove_file(staging.join(CANCEL_FILE));
    let _ = std::fs::remove_file(staging.join(READY_FILE));
    write_atomically(staging, REQUEST_FILE, &text, 0o600)
        .and_then(|()| write_atomically(staging, READY_FILE, b"", 0o600))
        .map_err(|_| Refusal::of(Why::Failed))
}

/// Says the person gave the write up.
///
/// # Errors
///
/// The folder could not be written.
pub fn cancel(staging: &Path) -> std::io::Result<()> {
    cancel_for(staging, None)
}

/// Says the write will not happen, and — when the queue refused — why: the
/// unit answers [`Phase::Failed`] with that reason.
///
/// # Errors
///
/// The folder could not be written.
pub fn cancel_for(staging: &Path, why: Option<Why>) -> std::io::Result<()> {
    write_atomically(
        staging,
        CANCEL_FILE,
        why.map_or("", Why::id).as_bytes(),
        0o600,
    )
}

/// The queue's reason in a `cancel`, as the ROOT unit reads it: the
/// person's regular file, never a link, one bounded word. Empty, unreadable
/// or a word that is no reason is the person giving up (`None`).
#[must_use]
pub fn cancel_reason(staging: &Path, owner: u32) -> Option<Why> {
    let file = open_owned(&staging.join(CANCEL_FILE), owner).ok()?;
    let mut text = String::new();
    file.take(REASON_MOST_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Why::of_id(text.trim())
}

/// Empties the folder of a request, a `ready` and a `cancel` — before the
/// door starts the unit, so the unit waits for THIS write's `ready`.
pub fn clear(staging: &Path) {
    for name in [READY_FILE, CANCEL_FILE, REQUEST_FILE] {
        let _ = std::fs::remove_file(staging.join(name));
    }
}

/// Writes `bytes` as `dir/name` whole or not at all: a scratch file created
/// `create_new` with its final mode from the first byte, then renamed.
///
/// # Errors
///
/// What the filesystem said.
pub fn write_atomically(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let scratch = dir.join(format!(".{name}.{}.new", std::process::id()));
    let _ = std::fs::remove_file(&scratch);
    let written = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc_nofollow())
            .open(&scratch)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        // The umask may have taken bits the caller wanted (a `0644` answer).
        f.set_permissions(std::fs::Permissions::from_mode(mode))?;
        std::fs::rename(&scratch, dir.join(name))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&scratch);
    }
    written
}

/// `O_NOFOLLOW`, as the standard library's `custom_flags` wants it.
fn libc_nofollow() -> i32 {
    i32::try_from(OFlags::NOFOLLOW.bits()).unwrap_or(0)
}

/// What `progress.json` says, if it says anything this format reads.
#[must_use]
pub fn read_progress(path: &Path) -> Option<Progress> {
    let text = std::fs::read(path).ok()?;
    serde_json::from_slice::<Progress>(&text)
        .ok()
        .filter(|p| p.format == FORMAT)
}

/// Writes `progress.json` (root's, `0644`), whole.
///
/// # Errors
///
/// What the filesystem said.
pub fn write_progress(path: &Path, progress: &Progress) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("/"));
    let name = path.file_name().map_or_else(
        || "progress.json".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let text = serde_json::to_vec(progress).map_err(std::io::Error::other)?;
    write_atomically(dir, &name, &text, 0o644)
}

/// A progress line, as the unit writes it — about the request numbered
/// `nonce`, or about none yet.
#[must_use]
pub const fn progress(
    nonce: Option<u64>,
    phase: Phase,
    done_bytes: u64,
    total_bytes: u64,
    why: Option<Why>,
) -> Progress {
    Progress {
        format: FORMAT,
        nonce,
        phase,
        done_bytes,
        total_bytes,
        why,
    }
}

/// Whom the staging folder belongs to: the person the unit writes for.
///
/// # Errors
///
/// [`Why::Failed`] when the folder is a link, not a folder, root's, or open
/// to anybody but its owner.
pub fn owner_of(staging: &Path) -> Result<u32, Refusal> {
    let meta = std::fs::symlink_metadata(staging).map_err(|_| Refusal::of(Why::Failed))?;
    if !meta.is_dir() || meta.uid() == 0 || meta.mode() & 0o077 != 0 {
        return Err(Refusal::of(Why::Failed));
    }
    Ok(meta.uid())
}

/// The request in the folder, as the ROOT unit reads it: opened without
/// following a link, the person's, a regular file, bounded, well formed.
///
/// # Errors
///
/// [`Why::Failed`] for anything else.
pub fn read_request(staging: &Path, owner: u32) -> Result<Request, Refusal> {
    let refused = || Refusal::of(Why::Failed);
    let file = open_owned(&staging.join(REQUEST_FILE), owner).map_err(|_| refused())?;
    let mut text = Vec::new();
    file.take(REQUEST_MOST_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(|_| refused())?;
    if text.len() as u64 > REQUEST_MOST_BYTES {
        return Err(refused());
    }
    let request: Request = serde_json::from_slice(&text).map_err(|_| refused())?;
    if !well_formed(&request) {
        return Err(refused());
    }
    Ok(request)
}

/// A regular file of `owner`'s, opened read-only WITHOUT following a link and
/// without waiting on a FIFO.
///
/// # Errors
///
/// It is a link, not a regular file, someone else's, or not there.
pub fn open_owned(path: &Path, owner: u32) -> std::io::Result<std::fs::File> {
    let fd = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOCTTY,
        Mode::empty(),
    )?;
    let stat = rustix::fs::fstat(&fd)?;
    let regular =
        rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::RegularFile;
    if !regular || stat.st_uid != owner {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "not a regular file of the person's",
        ));
    }
    Ok(std::fs::File::from(fd))
}

/// How a wait for `ready` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// `ready` is there, newer than the unit.
    Ready,
    /// `cancel` is there.
    Cancelled,
    /// The ceiling passed with neither.
    TimedOut,
}

/// How much older than the unit a `ready` may be and still be this write's:
/// the door starts the unit and queues the job in the same breath, and an
/// image already in the cache is staged at once — before the unit has read
/// its own clock. A crash's leftover is minutes or days old, never seconds.
pub const GRACE: Duration = Duration::from_secs(10);

/// Whether `name` is in the folder, the person's, and written at or after
/// `since` (the unit's own start), less [`GRACE`].
#[must_use]
pub fn fresh(staging: &Path, name: &str, owner: u32, since: SystemTime) -> bool {
    std::fs::symlink_metadata(staging.join(name)).is_ok_and(|m| {
        m.is_file() && m.uid() == owner && m.modified().is_ok_and(|t| t + GRACE >= since)
    })
}

/// Waits for `ready` (or `cancel`) in the folder — on inotify, never on a
/// clock: the kernel wakes this when a name lands, and the only timer is the
/// ceiling.
///
/// # Errors
///
/// inotify could not be opened or the folder watched.
pub fn wait(
    staging: &Path,
    owner: u32,
    since: SystemTime,
    ceiling: Duration,
) -> std::io::Result<Waited> {
    use rustix::fs::inotify;
    let fd = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)?;
    inotify::add_watch(
        &fd,
        staging,
        inotify::WatchFlags::CREATE
            | inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::ATTRIB
            | inotify::WatchFlags::ONLYDIR,
    )?;
    let started = std::time::Instant::now();
    let mut buffer = [0u8; 4096];
    loop {
        // CANCEL WINS: a `ready` and a `cancel` together are a write given up.
        if fresh(staging, CANCEL_FILE, owner, since) {
            return Ok(Waited::Cancelled);
        }
        if fresh(staging, READY_FILE, owner, since) {
            return Ok(Waited::Ready);
        }
        let Some(left) = ceiling.checked_sub(started.elapsed()) else {
            return Ok(Waited::TimedOut);
        };
        let deadline = rustix::event::Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = [rustix::event::PollFd::new(
            &fd,
            rustix::event::PollFlags::IN,
        )];
        match rustix::event::poll(&mut fds, Some(&deadline)) {
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
        // DRAINED, and what was read does not matter: the names are looked
        // at again at the top, which is the one truth.
        while let Ok(n) = rustix::io::read(&fd, &mut buffer) {
            if n == 0 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-stick-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    fn request() -> Request {
        Request {
            format: FORMAT,
            nonce: 42,
            image: "/home/p/.cache/ronoctua/stick/x.iso".into(),
            sha256: "a".repeat(64),
            target: Target {
                drive: "/drive/x".to_owned(),
                by_id: "/dev/disk/by-id/usb-x".into(),
                serial: None,
                size_bytes: 1 << 30,
                label: "x".to_owned(),
            },
        }
    }

    #[test]
    fn a_staged_request_is_read_back_as_the_unit_reads_it() {
        let dir = scratch("staged");
        let me = rustix::process::geteuid().as_raw();
        stage(&dir, &request()).unwrap();
        assert!(dir.join(READY_FILE).exists());
        let mode = std::fs::metadata(dir.join(REQUEST_FILE)).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(read_request(&dir, me).unwrap(), request());
        // SOMEONE ELSE'S file is refused, as root reads it.
        assert!(read_request(&dir, me + 1).is_err());
        // A LINK in its place is refused, wherever it points.
        std::fs::remove_file(dir.join(REQUEST_FILE)).unwrap();
        std::os::unix::fs::symlink("/etc/hostname", dir.join(REQUEST_FILE)).unwrap();
        assert!(read_request(&dir, me).is_err());
        // A REQUEST THAT IS NOT ONE is not staged.
        let mut bad = request();
        bad.target.by_id = "/dev/sda".into();
        assert!(stage(&dir, &bad).is_err());
        let mut bad = request();
        bad.sha256 = "xyz".to_owned();
        assert!(stage(&dir, &bad).is_err());
        let mut bad = request();
        bad.nonce = 0;
        assert!(
            stage(&dir, &bad).is_err(),
            "a request with no number of its own"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE QUEUE'S REFUSAL REACHES THE UNIT: a `cancel` holding a reason is
    /// read back as that reason; an empty one, or a word that is no reason,
    /// is the person giving up — and a link in its place is not read.
    #[test]
    fn a_cancel_carries_the_queues_reason() {
        let dir = scratch("reason");
        let me = rustix::process::geteuid().as_raw();
        cancel_for(&dir, Some(Why::Mismatch)).unwrap();
        assert_eq!(cancel_reason(&dir, me), Some(Why::Mismatch));
        assert_eq!(cancel_reason(&dir, me + 1), None, "someone else's file");
        cancel(&dir).unwrap();
        assert_eq!(cancel_reason(&dir, me), None, "the person gave up");
        std::fs::write(dir.join(CANCEL_FILE), b"rm -rf /").unwrap();
        assert_eq!(cancel_reason(&dir, me), None);
        std::fs::remove_file(dir.join(CANCEL_FILE)).unwrap();
        std::fs::write(dir.join("elsewhere"), b"mismatch").unwrap();
        std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join(CANCEL_FILE)).unwrap();
        assert_eq!(cancel_reason(&dir, me), None, "a link is not followed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_wait_ends_on_ready_on_cancel_and_on_its_ceiling() {
        let dir = scratch("wait");
        let me = rustix::process::geteuid().as_raw();
        let since = SystemTime::now();
        assert_eq!(
            wait(&dir, me, since, Duration::from_millis(50)).unwrap(),
            Waited::TimedOut
        );
        let writer = {
            let dir = dir.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                stage(&dir, &request()).unwrap();
            })
        };
        assert_eq!(
            wait(&dir, me, since, Duration::from_secs(10)).unwrap(),
            Waited::Ready
        );
        writer.join().unwrap();
        cancel(&dir).unwrap();
        assert_eq!(
            wait(&dir, me, since, Duration::from_secs(10)).unwrap(),
            Waited::Cancelled,
            "cancel wins over a ready beside it"
        );
        // A READY OLDER THAN THE UNIT is a crash's leftover, not a request.
        clear(&dir);
        stage(&dir, &request()).unwrap();
        let later = SystemTime::now() + GRACE + Duration::from_secs(5);
        assert!(!fresh(&dir, READY_FILE, me, later));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_is_written_whole_and_read_back() {
        let dir = scratch("progress");
        let path = dir.join("progress.json");
        assert_eq!(read_progress(&path), None);
        let p = progress(Some(9), Phase::Writing, 10, 20, None);
        write_progress(&path, &p).unwrap();
        assert_eq!(read_progress(&path), Some(p));
        let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o644, "the person reads what root writes");
        std::fs::write(&path, b"{\"format\":2}").unwrap();
        assert_eq!(read_progress(&path), None, "another format is not read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_folder_is_the_persons_and_nobody_elses() {
        let dir = scratch("owner");
        let me = rustix::process::geteuid().as_raw();
        if me != 0 {
            assert_eq!(owner_of(&dir).unwrap(), me);
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(owner_of(&dir).is_err(), "open to others");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
