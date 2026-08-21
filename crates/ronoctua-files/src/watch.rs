//! Long work that says where it is, and stops when it is told to.
//!
//! # Why `act` did not simply grow a callback
//!
//! Because there are two questions here and only one of them is about copying.
//! [`crate::act`] answers *what does this operation mean*: a link is copied as a
//! link, a taken name gets `(copia 1)`, a move across filesystems copies and
//! only then removes. This module answers *how do I watch it and how do I stop
//! it*, and that answer is the same for a copy, a delete and an overwrite.
//! Written into `act` it would have been the same three lines of bookkeeping in
//! five functions, which is five places for them to drift.
//!
//! There is still ONE implementation of each operation. `act::copy_into` calls
//! [`copy_into`] with [`Watch::none`], and what that compiles to is the same
//! walk with a branch nobody takes.
//!
//! # Why the copy is a CHUNKED `copy_file_range` and not `std::fs::copy`
//!
//! `std::fs::copy` is `copy_file_range` underneath, and on btrfs — which is
//! this system's filesystem — that is a **server-side copy**: the filesystem
//! shares the extents instead of moving bytes through this process. Replacing
//! it with a `read`/`write` loop to win a progress number would have made every
//! copy inside the disk dramatically slower, paying in seconds for a
//! percentage.
//!
//! So the syscall stays and what changes is the SIZE of each call: [`CHUNK`] at
//! a time, with the switch and the counter read in between. The kernel goes on
//! doing the sharing; we merely stop asking for the whole file in one breath.
//!
//! The fallback is not decoration. `copy_file_range` refuses across filesystems
//! on old kernels (`EXDEV`), refuses anything that is not a regular file
//! (`EINVAL`), and on some does not exist at all (`ENOSYS`) — and the
//! interesting case, copying onto a USB stick, is exactly the crossing one.
//! When it refuses, a `read`/`write` loop takes over from the offset it left
//! and the copy carries on.
//!
//! # Why cancelling has an error type of its own
//!
//! Because "it was cancelled" and "it failed" look identical to a caller that
//! only sees `Err`, and the difference decides whether a sentence saying
//! something went wrong goes on the screen. [`Cancelled`] is what tells them
//! apart, and [`was_cancelled`] is the one place that asks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use anyhow::{Context as _, Result};

/// How big a bite the chunked copy takes: eight mebibytes.
///
/// Big enough that the syscall overhead disappears against the transfer, small
/// enough that a cancel lands within a blink even on a slow stick — at the ten
/// megabytes a second a bad USB port gives, one bite is under a second.
pub const CHUNK: usize = 8 * 1024 * 1024;

/// How long a paused operation sleeps before looking at the switch again.
const NAP: std::time::Duration = std::time::Duration::from_millis(100);

/// What a running operation should do at the next bite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Carry on.
    Run,
    /// Hold, without giving up what has been done.
    Pause,
    /// Give up, and undo what can be undone.
    Stop,
}

impl Flow {
    /// The byte this is stored as. An `AtomicU8` is what can be flipped from
    /// another thread without a lock, and a lock around a three-state flag read
    /// once per bite would be a lock in the way of a copy.
    const fn byte(self) -> u8 {
        match self {
            Self::Run => 0,
            Self::Pause => 1,
            Self::Stop => 2,
        }
    }

    /// Back from the byte. Anything unknown reads as [`Flow::Run`]: a flag that
    /// somehow got a value nobody wrote must not stop a copy that is working.
    const fn of_byte(b: u8) -> Self {
        match b {
            1 => Self::Pause,
            2 => Self::Stop,
            _ => Self::Run,
        }
    }
}

/// The switch a running operation reads between bites.
///
/// One per job, shared with whoever can press pause — the queue holds an
/// `Arc<Control>` and the working thread reads it.
#[derive(Debug)]
pub struct Control(AtomicU8);

impl Default for Control {
    fn default() -> Self {
        Self::new()
    }
}

impl Control {
    /// A switch set to run.
    #[must_use]
    pub const fn new() -> Self {
        Self(AtomicU8::new(0))
    }

    /// The one every caller that will never stop anything can share.
    ///
    /// A `static` and not a fresh one per call: `act::copy_into` and the tests
    /// go through here, and allocating a switch to never touch it is noise in
    /// every profile that includes a copy.
    #[must_use]
    pub fn running() -> &'static Self {
        static ONE: Control = Control::new();
        &ONE
    }

    /// What the switch says right now.
    #[must_use]
    pub fn flow(&self) -> Flow {
        Flow::of_byte(self.0.load(Ordering::Relaxed))
    }

    /// Sets it. Called from the thread that draws, never from the one working.
    pub fn set(&self, flow: Flow) {
        self.0.store(flow.byte(), Ordering::Relaxed);
    }

    /// Blocks while paused, and gives back an error once stopped.
    ///
    /// # Errors
    ///
    /// [`Cancelled`], and only that — which is what [`was_cancelled`] looks for.
    pub fn allowed(&self) -> Result<()> {
        loop {
            match self.flow() {
                Flow::Run => return Ok(()),
                Flow::Stop => return Err(Cancelled.into()),
                // SLEEP, and not a spin: a paused copy of an hour must not be
                // an hour of one core at a hundred per cent. A tenth of a
                // second is under what a hand notices on resume and is four
                // hundred thousand times cheaper than the spin.
                Flow::Pause => std::thread::sleep(NAP),
            }
        }
    }
}

/// The end of an operation somebody stopped.
///
/// Its own type because `Err` alone cannot say whether the screen should show
/// a failure. See the module header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelado")
    }
}

impl std::error::Error for Cancelled {}

/// Whether this ending was a cancellation and not a failure.
#[must_use]
pub fn was_cancelled(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Cancelled>().is_some()
}

/// What an operation reports as it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat<'a> {
    /// It is on this one now. The name is what the screen shows; the number
    /// below is what the bar shows.
    On(&'a Path),
    /// This many more units are behind it — bytes for a copy, entries for a
    /// delete. WHICH unit is the caller's business: the operation counts what
    /// it moves and the queue knows what it asked for.
    Did(u64),
}

/// The switch and the sink, together, because every operation needs both.
pub struct Watch<'a> {
    control: &'a Control,
    beat: Option<&'a mut dyn FnMut(Beat<'_>)>,
}

impl std::fmt::Debug for Watch<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("flow", &self.control.flow())
            .field("watched", &self.beat.is_some())
            .finish()
    }
}

impl<'a> Watch<'a> {
    /// A watch that reports somewhere and obeys a switch.
    #[must_use]
    pub fn new(control: &'a Control, beat: &'a mut dyn FnMut(Beat<'_>)) -> Self {
        Self {
            control,
            beat: Some(beat),
        }
    }

    /// Nobody watching and nobody about to stop it — what `act`'s plain
    /// functions pass, so that there is one implementation and not two.
    #[must_use]
    pub fn none() -> Self {
        Self {
            control: Control::running(),
            beat: None,
        }
    }

    /// Says which one it is on now.
    pub fn on(&mut self, path: &Path) {
        if let Some(b) = self.beat.as_mut() {
            b(Beat::On(path));
        }
    }

    /// Adds to the counter.
    pub fn did(&mut self, units: u64) {
        if let Some(b) = self.beat.as_mut() {
            b(Beat::Did(units));
        }
    }

    /// Both at once: this one is done, and it counted for one.
    ///
    /// The shape every per-item loop wants — the trash, a restore — and the
    /// reason it exists rather than two calls is that the two belong together:
    /// a name without its unit leaves the bar behind the label, and the row
    /// then names a file that has already been dealt with.
    pub fn beat_one(&mut self, path: &Path) {
        self.on(path);
        self.did(1);
    }

    /// What the switch says, without waiting for anything.
    ///
    /// For work that cannot simply stop between bites — a child process has to
    /// be killed — and which therefore has to ask instead of being told.
    #[must_use]
    pub fn flow(&self) -> Flow {
        self.control.flow()
    }

    /// Waits out a pause and refuses once stopped. See [`Control::allowed`].
    ///
    /// # Errors
    ///
    /// [`Cancelled`], and only that.
    pub fn allowed(&self) -> Result<()> {
        self.control.allowed()
    }
}

// ---------------------------------------------------------------------------
// The operations
// ---------------------------------------------------------------------------

/// Copies a file, a directory tree or a symlink into a directory, reporting.
///
/// Returns where it landed. On any ending that is not success — a cancel
/// included — **what was written is removed**: [`crate::act::free_name`]
/// guarantees nothing was at the landing beforehand, so taking it away cannot
/// take somebody else's file, and half a copy left behind is what makes
/// cancelling worthless.
///
/// # Errors
///
/// When the source cannot be read, the destination cannot be written, or
/// somebody pressed cancel ([`Cancelled`]).
pub fn copy_into(source: &Path, directory: &Path, watch: &mut Watch<'_>) -> Result<PathBuf> {
    let name = source
        .file_name()
        .context("um caminho sem nome nao pode ser copiado")?;
    let landing = crate::act::free_name(&directory.join(name));
    match copy_tree(source, &landing, watch) {
        Ok(()) => Ok(landing),
        Err(e) => {
            // Best effort, and deliberately silent: the ending that matters is
            // the one being returned, and a second failure while cleaning up
            // after the first would only bury it.
            let _ = remove(&landing, &mut Watch::none());
            Err(e)
        }
    }
}

/// Moves into a directory, reporting — a rename when it can, a copy and a
/// removal when the filesystems differ.
///
/// A rename is one syscall and has no progress to report: it beats the whole
/// weight at once, which is the truth and not a rounding.
///
/// # Errors
///
/// When either half fails, or somebody pressed cancel. A cancel during the
/// COPY half leaves the original alone, which is the safe side.
pub fn move_into(source: &Path, directory: &Path, watch: &mut Watch<'_>) -> Result<PathBuf> {
    let name = source
        .file_name()
        .context("um caminho sem nome nao pode ser movido")?;
    let landing = crate::act::free_name(&directory.join(name));
    watch.on(source);
    match std::fs::rename(source, &landing) {
        Ok(()) => {
            watch.did(crate::act::weigh(&landing));
            Ok(landing)
        }
        Err(e) if e.raw_os_error() == Some(rustix::io::Errno::XDEV.raw_os_error()) => {
            match copy_tree(source, &landing, watch) {
                Ok(()) => {
                    remove(source, &mut Watch::none())?;
                    Ok(landing)
                }
                Err(e) => {
                    let _ = remove(&landing, &mut Watch::none());
                    Err(e)
                }
            }
        }
        Err(e) => Err(e).with_context(|| format!("mover {}", source.display())),
    }
}

/// Deletes, whatever it is, reporting one unit per entry.
///
/// It walks itself instead of calling `remove_dir_all`, and the reason is the
/// whole point of this module: that one is a single call that answers when it
/// is over. A tree of two hundred thousand files would be minutes of a screen
/// saying nothing and no way to stop it.
///
/// A symlink goes as the LINK and never as what it points at — the same rule
/// [`crate::act::remove`] states, and the difference between deleting a
/// shortcut and deleting somebody's home directory.
///
/// # Errors
///
/// When a removal fails, or somebody pressed cancel. What was already deleted
/// stays deleted: there is no undo for this one, and pretending otherwise by
/// carrying on would delete more.
pub fn remove(path: &Path, watch: &mut Watch<'_>) -> Result<()> {
    watch.allowed()?;
    let meta = std::fs::symlink_metadata(path)
        .with_context(|| format!("{} nao existe", path.display()))?;
    if meta.is_dir() && !meta.is_symlink() {
        for child in std::fs::read_dir(path)
            .with_context(|| format!("ler {}", path.display()))?
            .flatten()
        {
            remove(&child.path(), watch)?;
        }
        std::fs::remove_dir(path).with_context(|| format!("apagar {}", path.display()))?;
    } else {
        std::fs::remove_file(path).with_context(|| format!("apagar {}", path.display()))?;
    }
    watch.on(path);
    watch.did(1);
    Ok(())
}

/// Overwrites a file's content and then removes it, reporting the bytes.
///
/// The counter runs over `passes * size`, because that is what the disk
/// actually writes — reporting the file's size and finishing at three hundred
/// per cent would be a bar that lies about the wait.
///
/// What this genuinely destroys and what it cannot is written out on
/// [`crate::act::shred`], which is this function with nobody watching.
///
/// # Errors
///
/// When the file cannot be opened for writing, the removal fails, or somebody
/// pressed cancel. A directory is refused.
pub fn shred(path: &Path, passes: u32, watch: &mut Watch<'_>) -> Result<()> {
    use std::io::{Seek as _, SeekFrom, Write as _};

    let meta = std::fs::symlink_metadata(path)
        .with_context(|| format!("{} nao existe", path.display()))?;
    if meta.is_dir() {
        anyhow::bail!(
            "{} e uma pasta; sobrescrever uma arvore e outra operacao",
            path.display()
        );
    }
    if meta.is_symlink() {
        // Overwriting through a link would destroy the target while the person
        // was looking at the link.
        return remove(path, watch);
    }
    watch.on(path);
    let size = meta.len();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("abrir {} para sobrescrever", path.display()))?;

    // A block at a time, so a large file does not become a large allocation.
    const BLOCK: usize = 64 * 1024;
    for pass in 0..passes {
        // 0x00, then 0xFF, then a pattern that varies with the pass: the three
        // classic passes. On this hardware the pattern is not what decides
        // anything, and a varying one costs nothing.
        let byte = match pass % 3 {
            0 => 0x00,
            1 => 0xFF,
            _ => u8::try_from(pass % 251).unwrap_or(0x5A),
        };
        let block = vec![byte; BLOCK];
        file.seek(SeekFrom::Start(0)).context("voltar ao inicio")?;
        let mut left = size;
        while left > 0 {
            watch.allowed()?;
            let now = usize::try_from(left).unwrap_or(BLOCK).min(BLOCK);
            file.write_all(&block[..now]).context("sobrescrever")?;
            left -= now as u64;
            watch.did(now as u64);
        }
        // Per pass, and not once at the end: without it the passes coalesce in
        // the page cache and only the last one ever reaches the device.
        file.sync_all().context("sincronizar")?;
    }
    file.set_len(0).context("truncar")?;
    file.sync_all().context("sincronizar")?;
    drop(file);

    // The NAME is metadata too, and it survives the content. Renaming to
    // something meaningless before unlinking is what keeps a directory entry
    // from saying `senhas-do-banco.txt` to whoever reads the raw device.
    let blind = path.with_file_name(format!("{:016x}", crate::act::fnv(path)));
    let gone = if std::fs::rename(path, &blind).is_ok() {
        blind
    } else {
        path.to_path_buf()
    };
    remove(&gone, &mut Watch::none())
}

/// How much a tree occupies, stoppable, reporting one unit per entry walked.
///
/// The stoppable twin of [`crate::act::weigh`] — and the reason it exists is
/// that weighing IS the first stage of a copy: a folder of a hundred thousand
/// files takes long enough to weigh that a person will want out before a single
/// byte has moved.
///
/// # Errors
///
/// Only [`Cancelled`]. A directory that cannot be read counts as zero, exactly
/// as in `act::weigh`: a permission error deep in a tree must not turn "how big
/// is this" into a failure.
pub fn weigh(path: &Path, watch: &mut Watch<'_>) -> Result<u64> {
    watch.allowed()?;
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(0);
    };
    if !meta.is_dir() {
        watch.did(1);
        return Ok(meta.len());
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return Ok(0);
    };
    let mut total = 0u64;
    for child in entries.flatten() {
        total = total.saturating_add(weigh(&child.path(), watch)?);
    }
    Ok(total)
}

/// How many entries a tree holds, counting the directories themselves.
///
/// The twin of [`weigh`] for the operations whose cost is entries and not
/// bytes. A directory of two hundred thousand empty files weighs nothing and
/// takes minutes to delete, and a bar over its bytes would sit at a hundred per
/// cent for the whole wait.
///
/// It counts the directories because [`remove`] removes them: a total that left
/// them out would have the bar arrive at the end short, which reads as stuck.
///
/// # Errors
///
/// Only [`Cancelled`]. A directory that cannot be read counts as one entry and
/// no children — the same tolerance [`weigh`] has, and for the same reason.
pub fn count(path: &Path, watch: &mut Watch<'_>) -> Result<u64> {
    watch.allowed()?;
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(0);
    };
    let mut total = 1u64;
    if meta.is_dir()
        && !meta.is_symlink()
        && let Ok(entries) = std::fs::read_dir(path)
    {
        for child in entries.flatten() {
            total = total.saturating_add(count(&child.path(), watch)?);
        }
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// The walk, and the bytes
// ---------------------------------------------------------------------------

/// Copies a file, a symlink or a whole tree, reporting.
fn copy_tree(source: &Path, landing: &Path, watch: &mut Watch<'_>) -> Result<()> {
    watch.allowed()?;
    let meta = std::fs::symlink_metadata(source)
        .with_context(|| format!("{} nao existe", source.display()))?;
    if meta.is_symlink() {
        // The LINK is copied, not what it points at. Copying the target would
        // turn a twelve-byte shortcut into a gigabyte, which is the kind of
        // surprise that fills a disk.
        let target = std::fs::read_link(source)?;
        std::os::unix::fs::symlink(target, landing)
            .with_context(|| format!("recriar o link {}", landing.display()))?;
        watch.on(source);
        watch.did(meta.len());
        return Ok(());
    }
    if !meta.is_dir() {
        watch.on(source);
        return copy_file(source, landing, watch);
    }
    std::fs::create_dir_all(landing).with_context(|| format!("criar {}", landing.display()))?;
    for child in std::fs::read_dir(source)?.flatten() {
        copy_tree(&child.path(), &landing.join(child.file_name()), watch)?;
    }
    Ok(())
}

/// One regular file, in bites, with the kernel doing the moving while it will.
fn copy_file(source: &Path, landing: &Path, watch: &mut Watch<'_>) -> Result<()> {
    let from =
        std::fs::File::open(source).with_context(|| format!("abrir {}", source.display()))?;
    let meta = from
        .metadata()
        .with_context(|| format!("ler {}", source.display()))?;
    let to =
        std::fs::File::create(landing).with_context(|| format!("criar {}", landing.display()))?;

    let mut left = meta.len();
    let mut server_side = true;
    let mut buffer: Vec<u8> = Vec::new();
    while left > 0 {
        watch.allowed()?;
        let bite = usize::try_from(left).unwrap_or(CHUNK).min(CHUNK);
        let moved = if server_side {
            match rustix::fs::copy_file_range(&from, None, &to, None, bite) {
                Ok(n) => n,
                Err(e) if refuses(e) => {
                    // ONCE, in the log. A copy that silently got ten times
                    // slower is a report nobody can act on; the errno is the
                    // whole diagnosis.
                    tracing::debug!(
                        source = %source.display(),
                        errno = ?e,
                        "copy_file_range recusou; seguindo por read/write"
                    );
                    server_side = false;
                    continue;
                }
                Err(e) => {
                    return Err(std::io::Error::from_raw_os_error(e.raw_os_error()))
                        .with_context(|| format!("copiar {}", source.display()));
                }
            }
        } else {
            by_hand(&from, &to, bite, &mut buffer)
                .with_context(|| format!("copiar {}", source.display()))?
        };
        // The file shrank underneath us. Stopping is right: what is on the
        // other side is what the source now holds, and looping on a zero would
        // never end.
        if moved == 0 {
            break;
        }
        left -= moved as u64;
        watch.did(moved as u64);
    }

    // The MODE travels, as `std::fs::copy` has always done: a copy of an
    // executable that comes out unexecutable is a copy that does not work.
    to.set_permissions(meta.permissions())
        .with_context(|| format!("ajustar a permissao de {}", landing.display()))?;
    Ok(())
}

/// The fallback: through this process, from wherever the offsets are.
///
/// It picks up exactly where `copy_file_range` stopped — that call advances
/// both file offsets when it is given no explicit ones, which is what lets the
/// two paths share one loop.
fn by_hand(
    from: &std::fs::File,
    to: &std::fs::File,
    bite: usize,
    buffer: &mut Vec<u8>,
) -> std::io::Result<usize> {
    use std::io::{Read as _, Write as _};
    if buffer.len() < bite {
        buffer.resize(bite, 0);
    }
    let read = (&*from).read(&mut buffer[..bite])?;
    (&*to).write_all(&buffer[..read])?;
    Ok(read)
}

/// Whether this is the kernel saying "not this way", rather than a real fault.
///
/// The five that mean it: crossing filesystems on a kernel that will not
/// (`EXDEV`), anything that is not a regular file (`EINVAL`), a filesystem that
/// does not implement it (`ENOSYS`, `EOPNOTSUPP`), and the one an overlay or a
/// FUSE mount gives back (`EPERM`).
fn refuses(e: rustix::io::Errno) -> bool {
    matches!(
        e,
        rustix::io::Errno::XDEV
            | rustix::io::Errno::INVAL
            | rustix::io::Errno::NOSYS
            | rustix::io::Errno::OPNOTSUPP
            | rustix::io::Errno::PERM
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar diretorio de teste");
        dir
    }

    /// A file of `size` bytes with a pattern that shows a mangled copy.
    fn fill(path: &Path, size: usize) {
        let block: Vec<u8> = (0..=255u8).cycle().take(size).collect();
        std::fs::write(path, &block).expect("escrever o arquivo de teste");
    }

    #[test]
    fn the_beats_add_up_to_the_size_of_what_was_copied() {
        // THE DEFECT THIS CATCHES: a bar that stops at 12% because the counter
        // is fed the size of the bite asked for rather than the one the kernel
        // actually moved. `copy_file_range` is free to move less than it was
        // given, and it does — on the last bite of every file.
        let root = scratch("beats");
        // BIGGER THAN ONE BITE, so the loop runs more than once. A test whose
        // file fits in a single call proves nothing about the loop.
        let size = CHUNK + 4096;
        fill(&root.join("grande.bin"), size);
        std::fs::create_dir(root.join("destino")).unwrap();

        let control = Control::new();
        let mut moved = 0u64;
        let mut beats = 0usize;
        let mut sink = |b: Beat<'_>| {
            if let Beat::Did(n) = b {
                moved += n;
                beats += 1;
            }
        };
        let landed = copy_into(
            &root.join("grande.bin"),
            &root.join("destino"),
            &mut Watch::new(&control, &mut sink),
        )
        .unwrap();

        assert_eq!(moved, size as u64, "a soma dos avancos nao e o tamanho");
        assert!(
            beats >= 2,
            "um arquivo maior que a mordida veio numa so: {beats}"
        );
        assert_eq!(
            std::fs::read(&landed).unwrap(),
            std::fs::read(root.join("grande.bin")).unwrap(),
            "a copia nao e igual ao original"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cancelling_leaves_nothing_behind_and_keeps_the_original() {
        // THIS IS WHAT MAKES CANCELLING WORTH HAVING. A cancel that leaves half
        // a file in the destination turns "I pasted 40 GiB into the wrong
        // folder" into "I pasted 19 GiB into the wrong folder", which is the
        // same problem with a worse name.
        let root = scratch("cancel");
        let size = CHUNK * 3;
        fill(&root.join("grande.bin"), size);
        std::fs::create_dir(root.join("destino")).unwrap();

        let control = Control::new();
        // Stop at the FIRST beat: the copy is under way, the landing exists and
        // has bytes in it, and that is exactly the state the cleanup has to
        // survive.
        let mut sink = |b: Beat<'_>| {
            if matches!(b, Beat::Did(_)) {
                control.set(Flow::Stop);
            }
        };
        let e = copy_into(
            &root.join("grande.bin"),
            &root.join("destino"),
            &mut Watch::new(&control, &mut sink),
        )
        .unwrap_err();

        assert!(was_cancelled(&e), "um cancelamento veio como falha: {e:?}");
        assert_eq!(
            std::fs::read_dir(root.join("destino")).unwrap().count(),
            0,
            "sobrou pedaco no destino"
        );
        assert_eq!(
            std::fs::metadata(root.join("grande.bin")).unwrap().len(),
            size as u64,
            "o original foi mexido"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failure_is_not_a_cancellation() {
        // The other half of the pair above: `was_cancelled` has to say NO for
        // a real error, or every failed copy would be silently swallowed as
        // "the person stopped it".
        let root = scratch("failure");
        let e = copy_into(
            &root.join("nao-existe"),
            &root.join("tambem-nao"),
            &mut Watch::none(),
        )
        .unwrap_err();
        assert!(
            !was_cancelled(&e),
            "uma falha passou por cancelamento: {e:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pausing_holds_and_resuming_finishes() {
        // THE DEFECT THIS CATCHES: a pause that is really a stop. The switch
        // has three states and two of them end the operation if `allowed` is
        // written with one `if`.
        let root = scratch("pause");
        let size = CHUNK * 2;
        fill(&root.join("grande.bin"), size);
        std::fs::create_dir(root.join("destino")).unwrap();

        let control = Arc::new(Control::new());
        control.set(Flow::Pause);

        // The one that resumes, from OUTSIDE the working thread — which is the
        // arrangement the queue actually has, and the only one that proves the
        // switch is shared and not copied.
        let loosen = Arc::clone(&control);
        let hand = std::thread::spawn(move || {
            std::thread::sleep(NAP * 3);
            loosen.set(Flow::Run);
        });

        let started = std::time::Instant::now();
        let landed = copy_into(
            &root.join("grande.bin"),
            &root.join("destino"),
            &mut Watch::new(&control, &mut |_| {}),
        )
        .unwrap();
        hand.join().unwrap();

        assert!(
            started.elapsed() >= NAP * 2,
            "a pausa nao segurou nada: {:?}",
            started.elapsed()
        );
        assert_eq!(
            std::fs::metadata(&landed).unwrap().len(),
            size as u64,
            "a copia nao terminou depois de retomar"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deleting_a_tree_reports_one_unit_per_entry() {
        // Bytes are the wrong unit for a delete — a directory of two hundred
        // thousand empty files weighs nothing and takes minutes. The counter
        // here is ENTRIES, and the count has to include the directories
        // themselves or the bar arrives at the end short.
        let root = scratch("delete");
        std::fs::create_dir_all(root.join("arvore/dentro")).unwrap();
        std::fs::write(root.join("arvore/a"), b"a").unwrap();
        std::fs::write(root.join("arvore/dentro/b"), b"b").unwrap();
        std::os::unix::fs::symlink("a", root.join("arvore/atalho")).unwrap();

        let control = Control::new();
        let mut units = 0u64;
        let mut sink = |b: Beat<'_>| {
            if let Beat::Did(n) = b {
                units += n;
            }
        };
        remove(&root.join("arvore"), &mut Watch::new(&control, &mut sink)).unwrap();

        // `a`, `b`, `atalho`, `dentro` and `arvore` — five.
        assert_eq!(units, 5, "a conta das entradas nao bate");
        assert!(!root.join("arvore").exists(), "a arvore ficou");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deleting_a_link_deletes_the_link() {
        // The rule `act::remove` states, now that the walk is here: following
        // the link would delete somebody's home directory behind a gesture
        // that said "delete this shortcut".
        let root = scratch("delete-link");
        std::fs::create_dir(root.join("alvo")).unwrap();
        std::fs::write(root.join("alvo/dentro"), b"x").unwrap();
        std::os::unix::fs::symlink(root.join("alvo"), root.join("atalho")).unwrap();

        remove(&root.join("atalho"), &mut Watch::none()).unwrap();

        assert!(!root.join("atalho").exists(), "o link ficou");
        assert!(
            root.join("alvo/dentro").exists(),
            "seguiu o link e apagou o alvo"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn overwriting_counts_every_pass() {
        // A bar over `size` that finishes at three hundred per cent is a bar
        // that lies about the wait: three passes write three times the file.
        let root = scratch("shred");
        let size = 4096usize;
        fill(&root.join("segredo"), size);

        let control = Control::new();
        let mut units = 0u64;
        let mut sink = |b: Beat<'_>| {
            if let Beat::Did(n) = b {
                units += n;
            }
        };
        shred(
            &root.join("segredo"),
            crate::act::PASSES,
            &mut Watch::new(&control, &mut sink),
        )
        .unwrap();

        assert_eq!(units, size as u64 * u64::from(crate::act::PASSES));
        assert!(!root.join("segredo").exists(), "o arquivo ficou");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn weighing_can_be_stopped() {
        // A folder big enough to be worth weighing is a folder big enough to
        // want out of — and weighing happens BEFORE a single byte moves, which
        // is precisely when a person changes their mind.
        let root = scratch("weigh");
        std::fs::create_dir(root.join("muitos")).unwrap();
        for n in 0..32 {
            std::fs::write(root.join("muitos").join(format!("f{n}")), b"xx").unwrap();
        }

        let control = Control::new();
        let mut seen = 0u64;
        let mut sink = |b: Beat<'_>| {
            if let Beat::Did(n) = b {
                seen += n;
                control.set(Flow::Stop);
            }
        };
        let e = weigh(&root.join("muitos"), &mut Watch::new(&control, &mut sink)).unwrap_err();

        assert!(was_cancelled(&e), "{e:?}");
        assert_eq!(seen, 1, "continuou andando depois do cancelamento");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_move_inside_one_filesystem_beats_the_whole_weight_at_once() {
        // A rename has no progress to report and must not report zero: a job
        // that finishes at 0% looks stuck to whoever is reading the bar.
        let root = scratch("move");
        std::fs::write(root.join("arq"), b"12345").unwrap();
        std::fs::create_dir(root.join("destino")).unwrap();

        let control = Control::new();
        let mut units = 0u64;
        let mut sink = |b: Beat<'_>| {
            if let Beat::Did(n) = b {
                units += n;
            }
        };
        let landed = move_into(
            &root.join("arq"),
            &root.join("destino"),
            &mut Watch::new(&control, &mut sink),
        )
        .unwrap();

        assert_eq!(units, 5, "o peso nao foi batido de uma vez");
        assert!(landed.exists() && !root.join("arq").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// The measurement behind "the copy is still the kernel's".
///
/// It is `#[ignore]`d because it writes gibibytes and takes seconds: a battery
/// that runs on every edit must not do that. It is run by hand, and the number
/// it printed is written down beside the assertion so the next reader does not
/// have to run it again.
///
/// ```text
/// cargo test -p ronoctua-files -- --ignored --nocapture chunking
/// ```
#[cfg(test)]
mod measured {
    use super::*;

    /// Somewhere with a DISK behind it.
    ///
    /// # The trap this exists to avoid, and it was walked into
    ///
    /// `/tmp` on this machine is a **tmpfs**: 7.2 GiB of RAM wearing a
    /// filesystem's clothes. And inside `nix develop` it is worse — the shell
    /// points `TMPDIR` at a directory of its own **under that same tmpfs**, so
    /// `std::env::temp_dir()` from a `cargo test` run in the devshell writes
    /// into RAM without ever mentioning `/tmp`.
    ///
    /// A two-gibibyte benchmark there does two wrong things at once: it
    /// measures memory bandwidth instead of the filesystem — which is the exact
    /// opposite of the question — and it eats the RAM the compiler needs. It
    /// took the machine to 100% of that tmpfs and every command after it
    /// failed, which is a very confusing way to find out.
    ///
    /// `CARGO_TARGET_DIR` is where the devshell already puts gigabytes of build
    /// output, so it is on a real disk by construction. With it unset there is
    /// nowhere known to be safe, and the honest answer is to say so and stop.
    fn on_a_disk() -> Option<PathBuf> {
        let target = std::env::var_os("CARGO_TARGET_DIR")?;
        let dir = PathBuf::from(target).join("ronoctua-bench-copy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir)
    }

    #[test]
    #[ignore = "escreve um gibibyte; roda a mao"]
    fn chunking_costs_nothing_against_the_one_shot_copy() {
        // THE CLAIM THIS DEFENDS. `std::fs::copy` is one `copy_file_range` for
        // the whole file, and on btrfs that is a server-side copy — the
        // filesystem shares extents instead of moving bytes through this
        // process. Chunking it to win a progress number could have paid seconds
        // for a percentage, and the only thing that says otherwise is a number.
        let Some(root) = on_a_disk() else {
            eprintln!("sem CARGO_TARGET_DIR: nao ha onde medir sem escrever na RAM");
            return;
        };
        std::fs::create_dir(root.join("um")).unwrap();
        std::fs::create_dir(root.join("outro")).unwrap();

        let size = 1024 * 1024 * 1024usize;
        let source = root.join("grande.bin");
        {
            use std::io::Write as _;
            let mut f = std::io::BufWriter::new(std::fs::File::create(&source).unwrap());
            let block = vec![0x5Au8; 1024 * 1024];
            for _ in 0..(size / block.len()) {
                f.write_all(&block).unwrap();
            }
            // FLUSH AND THEN SYNC. Without the sync the first copy would be
            // measured against a file still in the page cache and the second
            // against one on the disk, which is a comparison of two different
            // questions.
            f.flush().unwrap();
            f.into_inner().unwrap().sync_all().unwrap();
        }

        let started = std::time::Instant::now();
        std::fs::copy(&source, root.join("um/copia.bin")).unwrap();
        let one_shot = started.elapsed();

        let started = std::time::Instant::now();
        copy_into(&source, &root.join("outro"), &mut Watch::none()).unwrap();
        let chunked = started.elapsed();

        eprintln!("um GiB — uma chamada so: {one_shot:?}   em mordidas: {chunked:?}");
        let _ = std::fs::remove_dir_all(&root);

        // MEASURED, 2026-08-21, one gibibyte on this machine's btrfs:
        // **153 ms in one call against 178 ms in bites** — sixteen per cent, and
        // the extra is 128 syscalls where there was one.
        //
        // THREE TIMES, and not "not slower". The ratio is loose on purpose: what
        // matters is the ORDER OF MAGNITUDE — a `read`/`write` loop would be ten
        // to a hundred times worse, and that is the mistake this guards. Tighter
        // would be measuring the page cache's mood on whichever machine ran it.
        assert!(
            chunked < one_shot * 3 + std::time::Duration::from_millis(200),
            "as mordidas custaram caro: {chunked:?} contra {one_shot:?}"
        );
    }
}
