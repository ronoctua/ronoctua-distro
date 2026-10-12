//! The write itself — what the root unit does once `ready` is there.
//!
//! # Hashing what is written, and then what is read
//!
//! The image is the person's file, and the person could change it between
//! the queue's verification and this read (the time-of-check problem). So
//! the digest that decides is the one taken HERE, of the very bytes that go
//! to the drive, compared with the request's; a stick written from bytes
//! that are not the image's has its first mebibyte zeroed — it must not boot
//! half of something — and the write is refused ([`Why::Mismatch`]).
//!
//! Then the drive is read back, through the page cache's back door
//! (`O_DIRECT`), and hashed again: a stick that lies about what it stored —
//! a fake capacity, a dying cell — is [`Why::ReadBackDiffers`] and not a
//! stick that fails to boot an hour later in somebody's other machine.
//!
//! # `O_DIRECT`, so the alignment is ours
//!
//! The buffer is aligned to [`ALIGN`] inside a larger allocation (no
//! `unsafe`), every write is a multiple of it, and the image's last block is
//! padded with zeros — inside the drive, which [`crate::Target::fits`]
//! already asked.
//!
//! # Every answer names its write, and the last one is always final
//!
//! One `progress.json` serves every write the machine makes, so each answer
//! about a request carries its [`crate::Request::nonce`]: the queue's job
//! follows its own write and no other. And an answer that says the unit is
//! still at work while it is not would keep the view refusing a new write
//! until the machine restarts — so the unit's `ExecStopPost` runs
//! [`stopped`], which turns a last answer that is not final (the unit was
//! stopped, or killed at its ceiling) into a failure.

use std::cell::Cell;
use std::io::{Read, Seek as _, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use rustix::fs::{Mode, OFlags};

use crate::device::{self, Roots};
use crate::digest::Sha256;
use crate::staging::{self, CANCEL_FILE, Waited};
use crate::{Phase, Request, Why};

/// The alignment `O_DIRECT` wants: a page, which is a multiple of every
/// logical block size a stick reports.
pub const ALIGN: usize = 4096;

/// How much is written per call.
pub const CHUNK_BYTES: usize = 4 << 20;

/// How much of a stick is zeroed when what was written is not the image.
pub const WIPE_BYTES: usize = 1 << 20;

/// How long the unit waits for `ready`: a download of four gibibytes on a
/// bad connection, with room — and never for ever.
pub const WAIT_CEILING: Duration = Duration::from_secs(12 * 3600);

/// How often at most the unit says where it is.
const BEAT: Duration = Duration::from_millis(500);

/// `bytes`, rounded up to [`ALIGN`].
#[must_use]
pub const fn padded(bytes: u64) -> u64 {
    bytes.div_ceil(ALIGN as u64) * ALIGN as u64
}

/// A buffer whose start is aligned to [`ALIGN`].
pub struct Aligned {
    raw: Vec<u8>,
    at: usize,
    len: usize,
}

impl Aligned {
    /// `len` zeroed bytes, aligned.
    #[must_use]
    pub fn new(len: usize) -> Self {
        let raw = vec![0u8; len + ALIGN];
        let at = raw.as_ptr().align_offset(ALIGN).min(ALIGN);
        Self { raw, at, len }
    }

    /// The bytes.
    pub fn bytes(&mut self) -> &mut [u8] {
        &mut self.raw[self.at..self.at + self.len]
    }
}

/// How a copy ended other than whole.
#[derive(Debug)]
pub enum Broke {
    /// The person gave it up.
    Cancelled,
    /// Reading or writing failed.
    Io(std::io::Error),
}

impl From<std::io::Error> for Broke {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Reads until `buf` is full or the reader is done; the bytes read.
fn fill(from: &mut dyn Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut have = 0;
    while have < buf.len() {
        match from.read(&mut buf[have..]) {
            Ok(0) => break,
            Ok(n) => have += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(have)
}

/// Copies `total` bytes of `image` onto `device` from its start, every write
/// a whole number of [`ALIGN`]s (the last padded with zeros), and answers
/// the SHA-256 of the image's bytes. `each` hears the bytes copied so far
/// and answers whether to go on.
///
/// # Errors
///
/// [`Broke::Cancelled`] when `each` said stop; [`Broke::Io`] when reading
/// or writing failed, or the image ended before `total`.
pub fn copy_hashing(
    image: &mut dyn Read,
    device: &mut dyn Write,
    total: u64,
    buf: &mut Aligned,
    each: &mut dyn FnMut(u64) -> bool,
) -> Result<String, Broke> {
    let mut sha = Sha256::new();
    let mut done = 0u64;
    while done < total {
        let bytes = buf.bytes();
        let want = usize::try_from((total - done).min(bytes.len() as u64)).unwrap_or(bytes.len());
        let n = fill(image, &mut bytes[..want])?;
        if n == 0 {
            return Err(Broke::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the image ended early",
            )));
        }
        sha.update(&bytes[..n]);
        let whole = usize::try_from(padded(n as u64)).unwrap_or(n);
        bytes[n..whole].fill(0);
        device.write_all(&bytes[..whole])?;
        done += n as u64;
        if !each(done) {
            return Err(Broke::Cancelled);
        }
    }
    Ok(sha.hex())
}

/// Reads `total` bytes back from the start of `device`, in whole
/// [`ALIGN`]s, and answers their SHA-256 (the padding read past `total` is
/// not hashed). `each` hears the bytes read.
///
/// # Errors
///
/// [`Broke::Cancelled`] when `each` said stop; [`Broke::Io`] when reading
/// failed or the device ended early.
pub fn read_back(
    device: &mut dyn Read,
    total: u64,
    buf: &mut Aligned,
    each: &mut dyn FnMut(u64) -> bool,
) -> Result<String, Broke> {
    let mut sha = Sha256::new();
    let mut done = 0u64;
    while done < total {
        let bytes = buf.bytes();
        let left = total - done;
        let want = usize::try_from(padded(left).min(bytes.len() as u64)).unwrap_or(bytes.len());
        let n = fill(device, &mut bytes[..want])?;
        let useful = usize::try_from(left.min(n as u64)).unwrap_or(n);
        if useful == 0 {
            return Err(Broke::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the drive ended before the image",
            )));
        }
        sha.update(&bytes[..useful]);
        done += useful as u64;
        if !each(done) {
            return Err(Broke::Cancelled);
        }
    }
    Ok(sha.hex())
}

/// How the unit ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// Written and verified.
    Done,
    /// Called off — waiting, or between two blocks: by the person (`None`),
    /// or by the queue, which refused for this reason.
    Cancelled(Option<Why>),
    /// Refused, and why.
    Refused(Why),
}

/// The unit: waits for `ready` in `staging`, writes, reads back — and says
/// where it is in `progress` (the folder beside `staging`'s, root's).
#[must_use]
pub fn unit(staging: &Path, progress: &Path, roots: &Roots) -> Ended {
    let since = SystemTime::now();
    // THE REQUEST'S NUMBER, once it is read: every answer after that names it.
    let nonce: Cell<Option<u64>> = Cell::new(None);
    let say = |phase: Phase, done: u64, total: u64, why: Option<Why>| {
        let said = staging::progress(nonce.get(), phase, done, total, why);
        if let Err(e) = staging::write_progress(progress, &said) {
            eprintln!("<4>the progress could not be written: {e}");
        }
    };
    let mut owner_of = None;
    let ended = (|| {
        let owner = staging::owner_of(staging).map_err(|r| {
            eprintln!("<3>{} is not a folder of a person's own", staging.display());
            r.why
        })?;
        owner_of = Some(owner);
        say(Phase::Waiting, 0, 0, None);
        match staging::wait(staging, owner, since, WAIT_CEILING) {
            Ok(Waited::Ready) => {}
            Ok(Waited::Cancelled) => return Ok(Ended::Cancelled(None)),
            Ok(Waited::TimedOut) => {
                eprintln!(
                    "<4>no `ready` in {} hours; nothing written",
                    WAIT_CEILING.as_secs() / 3600
                );
                return Ok(Ended::Cancelled(None));
            }
            Err(e) => {
                eprintln!("<3>could not wait on {}: {e}", staging.display());
                return Err(Why::Failed);
            }
        }
        let request = staging::read_request(staging, owner).map_err(|r| {
            eprintln!("<3>the staged request is not one");
            r.why
        })?;
        nonce.set(Some(request.nonce));
        write_request(&request, owner, staging, since, roots, &say)
    })();
    let ended = match ended.unwrap_or_else(Ended::Refused) {
        // WHY IT WAS CALLED OFF is the queue's to say, in the `cancel` itself.
        Ended::Cancelled(_) => {
            Ended::Cancelled(owner_of.and_then(|o| staging::cancel_reason(staging, o)))
        }
        other => other,
    };
    match ended {
        Ended::Done => {}
        Ended::Cancelled(why) => {
            if let Some(why) = why {
                eprintln!("<4>the queue called the write off: {}", why.id());
            }
            say(Phase::Failed, 0, 0, why);
        }
        Ended::Refused(why) => {
            eprintln!("<3>refused: {}", why.id());
            say(Phase::Failed, 0, 0, Some(why));
        }
    }
    ended
}

/// What the unit's `ExecStopPost` does: a last answer that is not final —
/// the unit stopped or killed while it waited, wrote or read back — becomes
/// a failure about the same request. A final one is left as it is. Answers
/// whether it wrote.
///
/// # Errors
///
/// The answer could not be written.
pub fn stopped(progress: &Path) -> std::io::Result<bool> {
    match staging::read_progress(progress) {
        Some(p) if !p.phase.is_over() => {
            let failed = staging::progress(
                p.nonce,
                Phase::Failed,
                p.done_bytes,
                p.total_bytes,
                Some(Why::Failed),
            );
            staging::write_progress(progress, &failed).map(|()| true)
        }
        _ => Ok(false),
    }
}

/// Opens the whole disk `name` with `flags`, and makes sure the descriptor
/// is the device that was judged.
fn open_device(name: &str, flags: OFlags, dev: (u32, u32)) -> Result<std::fs::File, Why> {
    let path = Path::new("/dev").join(name);
    let fd = rustix::fs::open(
        &path,
        flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|e| {
        eprintln!("<3>{}: {e}", path.display());
        // `O_EXCL` ON A BLOCK DEVICE: busy is the kernel saying something
        // holds it — mounted, opened by a holder.
        if e == rustix::io::Errno::BUSY {
            Why::NotWritable
        } else {
            Why::Failed
        }
    })?;
    let stat = rustix::fs::fstat(&fd).map_err(|_| Why::Failed)?;
    let block =
        rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::BlockDevice;
    let opened = (
        rustix::fs::major(stat.st_rdev),
        rustix::fs::minor(stat.st_rdev),
    );
    if !block || opened != dev {
        eprintln!("<3>{} is not the device that was judged", path.display());
        return Err(Why::Changed);
    }
    Ok(std::fs::File::from(fd))
}

fn write_request(
    request: &Request,
    owner: u32,
    staging: &Path,
    since: SystemTime,
    roots: &Roots,
    say: &dyn Fn(Phase, u64, u64, Option<Why>),
) -> Result<Ended, Why> {
    // THE IMAGE IS THE PERSON'S, opened as theirs: a regular file they own,
    // never a link — or root would copy any file of the machine onto a
    // stick the person can read.
    let mut image = staging::open_owned(&request.image, owner).map_err(|e| {
        eprintln!("<3>{}: {e}", request.image.display());
        Why::Failed
    })?;
    let total = image.metadata().map_err(|_| Why::Failed)?.len();
    if total == 0 {
        return Err(Why::Failed);
    }
    let name = device::resolve(&request.target.by_id).map_err(|e| {
        eprintln!("<3>{}: {e}", request.target.by_id.display());
        Why::Changed
    })?;
    let probe = device::probe(&name, roots).map_err(|e| {
        eprintln!("<3>{name}: {e}");
        Why::Changed
    })?;
    device::judge(&probe, &request.target)?;
    if !request.target.fits(total) {
        return Err(Why::NotWritable);
    }
    let mut device = open_device(
        &name,
        OFlags::WRONLY | OFlags::DIRECT | OFlags::EXCL,
        probe.dev,
    )?;
    // AND THE NAME STILL LEADS THERE, now that it is held.
    if device::resolve(&request.target.by_id).ok().as_deref() != Some(name.as_str()) {
        return Err(Why::Changed);
    }

    let mut buf = Aligned::new(CHUNK_BYTES);
    let mut said = Instant::now();
    let cancelled = || staging::fresh(staging, CANCEL_FILE, owner, since);
    say(Phase::Writing, 0, total, None);
    let written = copy_hashing(&mut image, &mut device, total, &mut buf, &mut |done| {
        if said.elapsed() >= BEAT {
            say(Phase::Writing, done, total, None);
            said = Instant::now();
        }
        !cancelled()
    });
    let sha = match written {
        Ok(sha) => sha,
        Err(Broke::Cancelled) => return Ok(Ended::Cancelled(None)),
        Err(Broke::Io(e)) => {
            eprintln!("<3>writing {name}: {e}");
            return Err(Why::Failed);
        }
    };
    device.sync_all().map_err(|_| Why::Failed)?;
    if !crate::digest::same(&sha, &request.sha256) {
        eprintln!("<3>what was read from the image is not the image the request names");
        wipe(&mut device);
        return Err(Why::Mismatch);
    }

    // READ BACK WHILE THE WRITE STILL HOLDS THE DRIVE (`O_EXCL`): nothing
    // mounts or probes it in between.
    let mut back = open_device(&name, OFlags::RDONLY | OFlags::DIRECT, probe.dev)?;
    say(Phase::Verifying, 0, total, None);
    let mut said = Instant::now();
    let read = read_back(&mut back, total, &mut buf, &mut |done| {
        if said.elapsed() >= BEAT {
            say(Phase::Verifying, done, total, None);
            said = Instant::now();
        }
        !cancelled()
    });
    match read {
        Ok(back) if crate::digest::same(&back, &sha) => {
            say(Phase::Done, total, total, None);
            Ok(Ended::Done)
        }
        Ok(_) => {
            eprintln!("<3>{name} does not hold what was written to it");
            Err(Why::ReadBackDiffers)
        }
        Err(Broke::Cancelled) => Ok(Ended::Cancelled(None)),
        Err(Broke::Io(e)) => {
            eprintln!("<3>reading {name} back: {e}");
            Err(Why::ReadBackDiffers)
        }
    }
}

/// Zeroes the start of a stick that holds bytes nobody asked for.
fn wipe(device: &mut std::fs::File) {
    let mut zeros = Aligned::new(WIPE_BYTES);
    let wiped = device
        .seek(SeekFrom::Start(0))
        .and_then(|_| device.write_all(zeros.bytes()))
        .and_then(|()| device.sync_all());
    if let Err(e) = wiped {
        eprintln!("<3>the stick could not be wiped: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_is_aligned_and_the_tail_is_padded() {
        let mut b = Aligned::new(3 * ALIGN);
        assert_eq!(b.bytes().as_ptr().align_offset(ALIGN), 0);
        assert_eq!(b.bytes().len(), 3 * ALIGN);
        assert_eq!(padded(1), 4096);
        assert_eq!(padded(4096), 4096);
        assert_eq!(padded(4097), 8192);
        assert_eq!(padded(0), 0);
    }

    /// WHAT GOES TO THE DRIVE IS THE IMAGE, PADDED, and the digest is the
    /// image's: an image that is not a whole number of blocks is written
    /// whole with zeros after it, and read back to the same digest.
    #[test]
    fn an_image_is_written_padded_and_read_back_to_its_own_digest() {
        let image: Vec<u8> = (0..10_000u32).map(|n| (n % 251) as u8).collect();
        let mut device = std::io::Cursor::new(vec![0xAAu8; 16_384]);
        let mut buf = Aligned::new(ALIGN * 2);
        let mut seen = Vec::new();
        let sha = copy_hashing(
            &mut &image[..],
            &mut device,
            image.len() as u64,
            &mut buf,
            &mut |n| {
                seen.push(n);
                true
            },
        )
        .unwrap();
        let mut expected = Sha256::new();
        expected.update(&image);
        assert_eq!(sha, expected.hex());
        assert_eq!(seen, [8192, 10_000], "each write said, the last one whole");
        let written = device.get_ref();
        assert_eq!(&written[..10_000], &image[..]);
        assert!(
            written[10_000..12_288].iter().all(|b| *b == 0),
            "the tail is zeros"
        );
        assert!(
            written[12_288..].iter().all(|b| *b == 0xAA),
            "and nothing past it"
        );

        device.set_position(0);
        let back = read_back(&mut device, image.len() as u64, &mut buf, &mut |_| true).unwrap();
        assert_eq!(back, sha);

        // A DRIVE THAT LOST A BYTE is caught by the read-back.
        device.get_mut()[5000] ^= 1;
        device.set_position(0);
        let lied = read_back(&mut device, image.len() as u64, &mut buf, &mut |_| true).unwrap();
        assert_ne!(lied, sha);
    }

    /// A UNIT STOPPED AT WORK LEAVES A FINAL ANSWER: what `ExecStopPost`
    /// does turns a "writing" nobody will finish into a failure about the
    /// same request, and leaves a final answer alone.
    #[test]
    fn a_stopped_unit_leaves_a_final_answer() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-stick-stopped-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("progress.json");
        assert!(!stopped(&path).unwrap(), "nothing written yet");
        staging::write_progress(
            &path,
            &staging::progress(Some(5), Phase::Writing, 3, 9, None),
        )
        .unwrap();
        assert!(stopped(&path).unwrap());
        assert_eq!(
            staging::read_progress(&path),
            Some(staging::progress(
                Some(5),
                Phase::Failed,
                3,
                9,
                Some(Why::Failed)
            ))
        );
        let done = staging::progress(Some(6), Phase::Done, 9, 9, None);
        staging::write_progress(&path, &done).unwrap();
        assert!(!stopped(&path).unwrap());
        assert_eq!(
            staging::read_progress(&path),
            Some(done),
            "a final answer stays"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE QUEUE'S REASON BECOMES THE UNIT'S ANSWER: a unit waiting for
    /// `ready` that finds a `cancel` holding the queue's refusal answers
    /// that refusal — the view's "the last write" then says why, not
    /// "given up".
    #[test]
    fn a_waiting_unit_answers_the_queues_reason() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = std::env::temp_dir().join(format!("ronoctua-stick-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let staged = root.join("staged");
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o700)).unwrap();
        let progress = root.join("progress.json");
        let canceller = {
            let staged = staged.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                staging::cancel_for(&staged, Some(Why::Mismatch)).unwrap();
            })
        };
        let ended = unit(&staged, &progress, &Roots::system());
        canceller.join().unwrap();
        if rustix::process::geteuid().is_root() {
            // A folder of root's is no person's: refused before waiting.
            assert_eq!(ended, Ended::Refused(Why::Failed));
        } else {
            assert_eq!(ended, Ended::Cancelled(Some(Why::Mismatch)));
            let said = staging::read_progress(&progress).unwrap();
            assert_eq!(
                (said.phase, said.why, said.nonce),
                (Phase::Failed, Some(Why::Mismatch), None)
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_cancel_stops_between_two_blocks_and_a_short_image_fails() {
        let image = vec![1u8; 3 * ALIGN];
        let mut device = std::io::Cursor::new(Vec::new());
        let mut buf = Aligned::new(ALIGN);
        let r = copy_hashing(
            &mut &image[..],
            &mut device,
            image.len() as u64,
            &mut buf,
            &mut |n| n < 2 * ALIGN as u64,
        );
        assert!(matches!(r, Err(Broke::Cancelled)));
        assert_eq!(
            device.get_ref().len(),
            2 * ALIGN,
            "stopped after the second block"
        );
        let mut device = std::io::Cursor::new(Vec::new());
        let r = copy_hashing(
            &mut &image[..],
            &mut device,
            10 * ALIGN as u64,
            &mut buf,
            &mut |_| true,
        );
        assert!(
            matches!(r, Err(Broke::Io(_))),
            "an image shorter than it said"
        );
    }
}
