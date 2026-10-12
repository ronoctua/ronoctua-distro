//! `ronoctua-seal` — what the transfer queue runs as a child for `Work::Seal`.
//!
//! Reads one [`ronoctua_pq::Job`] (JSON) on its standard input, seals or
//! opens `from` into `into`, and prints [`ronoctua_pq::Line`]s: `bytes 0/…`
//! the moment `into` is created (the queue's sign that the output is this
//! run's to remove if it must stop it), progress (`bytes <n>/<total>`), then
//! the verdict (`done <n>` or `refused <why>`).
//!
//! - Secrets arrive as the paths of `0600` files and are read, never echoed;
//!   a file somebody else could read is refused.
//! - `into` is created new (`create_new`, `0600`): nothing is ever
//!   overwritten, and the input is never deleted. A failure removes the
//!   half-written output.
//! - A FOLDER is sealed as a tar that `ronoctua-archive` streams (bsdtar),
//!   read as it is written — the folder in clear never becomes a file on a
//!   disk — and refused whole when bsdtar could not read a member of it.
//! - Opening "with the key" opens the belongings' `key.age` with the
//!   passphrase in memory and opens the file with the identity inside: the
//!   identity is never written anywhere, and `key.age` is never removed.
//! - New encryption is post-quantum: a job seals only when it names an
//!   `age1pq1…` recipient, and one with classical `age1…` keys alone is
//!   refused (`corrupt`), never sealed classically. A classical key is read
//!   only so that one beside a PQ one is refused for what it is, `mixed`, and
//!   not as a key that did not parse.

use std::fs::{File, OpenOptions};
use std::io::{BufRead as _, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr as _;

use age::secrecy::SecretString;
use ronoctua_pq::{Action, Identity, Job, Line, Recipient, Refusal, Why};

/// The largest job accepted on standard input.
const JOB_MOST_BYTES: u64 = 64 * 1024;

/// The largest secret file read.
const SECRET_MOST_BYTES: u64 = 64 * 1024;

/// The largest sealed key file read (the belongings' `key.age` is one line
/// of identity, sealed: a few kilobytes).
const KEY_MOST_BYTES: u64 = 64 * 1024;

fn say(line: Line) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", line.text());
    let _ = out.flush();
}

/// The recipients of a job, as written: `age1pq1…`, or classical X25519.
///
/// # Errors
///
/// [`Why::Corrupt`] for one that does not parse, and for classical ones with
/// no PQ one among them: the classical reading exists so that age refuses a
/// mix by its labels (`mixed`), never so that a file is sealed without the
/// post-quantum stanza. An empty list is left to [`ronoctua_pq::seal`], which
/// refuses it as it always has.
fn recipients_of(texts: &[String]) -> Result<Vec<Box<dyn age::Recipient>>, Refusal> {
    let corrupt = Refusal::of(Why::Corrupt);
    let mut pq = 0_usize;
    let mut all: Vec<Box<dyn age::Recipient>> = Vec::with_capacity(texts.len());
    for text in texts.iter().map(|t| t.trim()) {
        if text.starts_with(ronoctua_pq::RECIPIENT_PREFIX) {
            pq += 1;
            all.push(Box::new(Recipient::from_str(text)?));
        } else {
            let classical = age::x25519::Recipient::from_str(text).map_err(|_| corrupt)?;
            all.push(Box::new(classical));
        }
    }
    if pq == 0 && !all.is_empty() {
        return Err(corrupt);
    }
    Ok(all)
}

/// A `0600` file's text; anything another user could read is refused.
fn secret_text(path: &Path) -> Result<String, Refusal> {
    let failed = Refusal::of(Why::Failed);
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nofollow())
        .open(path)
        .map_err(|_| failed)?;
    let meta = file.metadata().map_err(|_| failed)?;
    if !meta.is_file() || meta.mode() & 0o077 != 0 || meta.len() > SECRET_MOST_BYTES {
        return Err(failed);
    }
    // THE WHOLE SIZE AT ONCE: a growing string leaves copies of the secret
    // in the memory it frees on the way.
    let mut text = String::with_capacity(usize::try_from(meta.len()).unwrap_or(0) + 1);
    file.take(SECRET_MOST_BYTES)
        .read_to_string(&mut text)
        .map_err(|_| failed)?;
    Ok(text)
}

/// `O_NOFOLLOW`: a link where a secret should be is refused, not followed.
fn nofollow() -> i32 {
    i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).unwrap_or(0)
}

/// The post-quantum identities in `text`, one per line — the belongings'
/// key holds nothing else. Anything else on a line is ignored, as age's
/// identity files allow comments.
fn identities_of(text: &str) -> Result<Vec<Identity>, Refusal> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with(ronoctua_pq::IDENTITY_PREFIX))
        .map(Identity::from_str)
        .collect()
}

/// Opens `from` into `into` with the identities `text` holds.
fn open_with(
    text: &str,
    from: &mut dyn Read,
    into: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, Refusal> {
    let pq = identities_of(text)?;
    let dyns: Vec<&dyn age::Identity> = pq.iter().map(|i| i as &dyn age::Identity).collect();
    ronoctua_pq::open(&dyns, from, into, progress)
}

/// The identities `key_file` holds, opened with the passphrase of
/// `passphrase_file` — in memory, wiped when dropped, never written.
///
/// # Errors
///
/// [`Why::WrongPassphrase`] when the passphrase does not open it,
/// [`Why::Failed`] for a key file that cannot be read (a link where it should
/// be, a folder, something too big to be one), and what opening refuses.
fn key_text(
    key_file: &Path,
    passphrase_file: &Path,
) -> Result<age::secrecy::SecretString, Refusal> {
    let failed = Refusal::of(Why::Failed);
    let passphrase = SecretString::from(secret_text(passphrase_file)?);
    let key = OpenOptions::new()
        .read(true)
        .custom_flags(nofollow())
        .open(key_file)
        .map_err(|_| failed)?;
    let meta = key.metadata().map_err(|_| failed)?;
    if !meta.is_file() || meta.len() > KEY_MOST_BYTES {
        return Err(failed);
    }
    let mut opened =
        zeroize::Zeroizing::new(Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0)));
    ronoctua_pq::open_passphrase(
        &passphrase,
        &mut key.take(KEY_MOST_BYTES),
        &mut *opened,
        &mut |_| {},
    )?;
    let text = std::str::from_utf8(&opened).map_err(|_| Refusal::of(Why::Corrupt))?;
    Ok(SecretString::from(text.to_owned()))
}

/// What a seal reads: a file, or a folder as the tar `ronoctua-archive`
/// streams of it (the folder in clear never lands on a disk as one file).
enum Source {
    File(BufReader<File>),
    Folder(std::process::Child, std::process::ChildStdout),
}

impl Read for Source {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(f) => f.read(buf),
            Self::Folder(_, out) => out.read(buf),
        }
    }
}

impl Source {
    /// The folder's tar ended well: bsdtar read every member. A folder with
    /// a member it could not read is refused whole, never sealed short.
    fn finish(self) -> Result<(), Refusal> {
        match self {
            Self::File(_) => Ok(()),
            Self::Folder(mut child, out) => {
                drop(out);
                match child.wait() {
                    Ok(status) if status.success() => Ok(()),
                    _ => Err(Refusal::of(Why::Failed)),
                }
            }
        }
    }

    /// Stops a folder's tar that is no longer read.
    fn abandon(self) {
        if let Self::Folder(mut child, out) = self {
            drop(out);
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// `O_NONBLOCK`: a pipe opens at once instead of waiting for a writer, so
/// that what it is can be asked of it — and refused.
fn nonblock() -> i32 {
    i32::try_from(rustix::fs::OFlags::NONBLOCK.bits()).unwrap_or(0)
}

/// The file a job reads, opened — only a REGULAR file. A device behind a
/// link (`/dev/zero` named `photo.jpg`, from a tar or a stick) would be sealed
/// until the disk is full, and a pipe would hold the job at nought for ever.
/// What the path pointed at a moment ago is not trusted: the descriptor
/// itself is asked.
fn regular(path: &Path) -> Result<File, Refusal> {
    let failed = Refusal::of(Why::Failed);
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nonblock())
        .open(path)
        .map_err(|_| failed)?;
    if !file.metadata().map_err(|_| failed)?.is_file() {
        return Err(failed);
    }
    Ok(file)
}

fn run(job: &Job) -> Result<u64, Refusal> {
    let failed = Refusal::of(Why::Failed);
    // FOLLOWING A LINK, as the file manager offers a link into a folder as a
    // folder: what is sealed is what the person sees when they enter it — a
    // folder or a file, and nothing else (a device, a pipe, a socket).
    let meta = std::fs::metadata(&job.from).map_err(|_| failed)?;
    let folder = meta.is_dir();
    if !(folder || meta.is_file()) || (folder && !job.seals()) {
        return Err(failed);
    }
    let file = if folder {
        None
    } else {
        Some(regular(&job.from)?)
    };
    // AN OPEN COUNTS WHAT IT WRITES, which is not what the file weighs: its
    // total is nobody's to know, and a bar would be a guess. Nor does a
    // folder's tar weigh what its files weigh. A file's weight is the opened
    // one's, not the path's of a moment ago.
    let total = match &file {
        Some(f) if job.seals() => Some(f.metadata().map_err(|_| failed)?.len()),
        _ => None,
    };
    let mut into = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&job.into)
        .map_err(|_| failed)?;
    // THE OUTPUT IS OURS from this line on: the queue removes it when it has
    // to stop this run, and only once it heard this — before it, the name may
    // be somebody else's file that `create_new` refused.
    say(Line::Bytes { done: 0, total });
    let source = match file {
        Some(file) => Ok(Source::File(BufReader::new(file))),
        None => std::fs::canonicalize(&job.from)
            .map_err(|_| failed)
            .and_then(|real| {
                ronoctua_archive::Tools::from_env()
                    .tar_stream(&real)
                    .map_err(|_| failed)
            })
            .and_then(|mut child| match child.stdout.take() {
                Some(out) => Ok(Source::Folder(child, out)),
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(failed)
                }
            }),
    };
    let mut progress = |done: u64| say(Line::Bytes { done, total });
    // EVERY REFUSAL INSIDE, so the half-written output is removed whatever
    // refused — a `?` out of `run` would leave it behind.
    let result = source.and_then(|mut from| {
        let sealed = (|| match &job.action {
            Action::SealTo { recipients } => {
                let parsed = recipients_of(recipients)?;
                let dyns: Vec<&dyn age::Recipient> = parsed.iter().map(AsRef::as_ref).collect();
                ronoctua_pq::seal(&dyns, &mut from, &mut into, &mut progress)
            }
            Action::SealPassphrase {
                passphrase_file,
                work,
            } => {
                let passphrase = SecretString::from(secret_text(passphrase_file)?);
                ronoctua_pq::seal_passphrase(
                    &passphrase,
                    *work,
                    &mut from,
                    &mut into,
                    &mut progress,
                )
            }
            Action::Open { identities_file } => {
                let text = SecretString::from(secret_text(identities_file)?);
                open_with(
                    age::secrecy::ExposeSecret::expose_secret(&text),
                    &mut from,
                    &mut into,
                    &mut progress,
                )
            }
            Action::OpenPassphrase { passphrase_file } => {
                let passphrase = SecretString::from(secret_text(passphrase_file)?);
                ronoctua_pq::open_passphrase(&passphrase, &mut from, &mut into, &mut progress)
            }
            Action::OpenWithKey {
                key_file,
                passphrase_file,
            } => {
                let text = key_text(key_file, passphrase_file)?;
                open_with(
                    age::secrecy::ExposeSecret::expose_secret(&text),
                    &mut from,
                    &mut into,
                    &mut progress,
                )
            }
        })();
        match sealed {
            Ok(n) => from.finish().map(|()| n),
            Err(r) => {
                from.abandon();
                Err(r)
            }
        }
    });
    let result = result.and_then(|n| into.sync_all().map(|()| n).map_err(|_| failed));
    if result.is_err() {
        drop(into);
        let _ = std::fs::remove_file(&job.into);
    }
    result
}

fn main() -> ExitCode {
    // Wiped when dropped: a job that carries a secret where none belongs is
    // refused, and its text does not linger either.
    let mut text = zeroize::Zeroizing::new(String::new());
    let read = std::io::stdin()
        .lock()
        .take(JOB_MOST_BYTES)
        .read_line(&mut text);
    let job = match read
        .ok()
        .and_then(|_| serde_json::from_str::<Job>(text.trim()).ok())
    {
        Some(job) => job,
        None => {
            eprintln!("ronoctua-seal: standard input is not one job");
            say(Line::Refused(Why::Failed));
            return ExitCode::FAILURE;
        }
    };
    match run(&job) {
        Ok(bytes) => {
            say(Line::Done { bytes });
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            say(Line::Refused(refusal.why));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::ExposeSecret as _;

    fn sealed(recipients: &[String]) -> Result<Vec<u8>, Refusal> {
        let parsed = recipients_of(recipients)?;
        let dyns: Vec<&dyn age::Recipient> = parsed.iter().map(AsRef::as_ref).collect();
        let mut out = Vec::new();
        ronoctua_pq::seal(&dyns, &mut &b"note"[..], &mut out, &mut |_| {})?;
        Ok(out)
    }

    /// A folder of this test's own, empty.
    fn place(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-seal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A secret the way the queue writes one: `0600`, created new.
    fn secret(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        f.write_all(text.as_bytes()).unwrap();
        path
    }

    fn job(action: Action, from: &Path, into: &Path) -> Job {
        Job {
            action,
            from: from.to_path_buf(),
            into: into.to_path_buf(),
        }
    }

    fn passphrase(file: &Path) -> Action {
        Action::SealPassphrase {
            passphrase_file: file.to_path_buf(),
            work: Some(10),
        }
    }

    /// FAILS IF: a seal or an open writes over a file that is there, or
    /// takes the original away — the two promises the menu row makes.
    #[test]
    fn nothing_is_ever_overwritten_and_the_original_stays() {
        let dir = place("overwrite");
        let note = dir.join("note.txt");
        std::fs::write(&note, b"the note").unwrap();
        let pw = secret(&dir, "pw", "horse battery");
        let taken = dir.join("note.txt.age");
        std::fs::write(&taken, b"somebody's file").unwrap();
        assert_eq!(
            run(&job(passphrase(&pw), &note, &taken)).unwrap_err().why,
            Why::Failed
        );
        assert_eq!(std::fs::read(&taken).unwrap(), b"somebody's file");
        let sealed = dir.join("note (2).txt.age");
        run(&job(passphrase(&pw), &note, &sealed)).unwrap();
        assert_eq!(
            std::fs::read(&note).unwrap(),
            b"the note",
            "the original moved"
        );
        // OPENING over a file that is there: refused, and it is untouched.
        let open = Action::OpenPassphrase {
            passphrase_file: pw,
        };
        assert_eq!(
            run(&job(open.clone(), &sealed, &note)).unwrap_err().why,
            Why::Failed
        );
        assert_eq!(std::fs::read(&note).unwrap(), b"the note");
        let back = dir.join("note (3).txt");
        run(&job(open, &sealed, &back)).unwrap();
        assert_eq!(std::fs::read(&back).unwrap(), b"the note");
        assert!(
            sealed.exists(),
            "the sealed file went away when it was opened"
        );
        // A WRONG PASSPHRASE leaves nothing behind.
        let wrong = secret(&dir, "wrong", "not it");
        let nothing = dir.join("nothing.txt");
        let refused = run(&job(
            Action::OpenPassphrase {
                passphrase_file: wrong,
            },
            &sealed,
            &nothing,
        ));
        assert_eq!(refused.unwrap_err().why, Why::WrongPassphrase);
        assert!(!nothing.exists(), "a refused open left its half behind");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a folder is not sealed whole as the tar of itself — a file
    /// lost, a name changed — or the folder in clear is left as a tar on the
    /// disk beside it.
    #[test]
    fn a_folder_is_sealed_as_its_tar_and_opens_back_to_it() {
        let tools = ronoctua_archive::Tools::from_env();
        assert!(
            tools.bsdtar.is_some(),
            "RONOCTUA_BSDTAR is needed: run inside `nix develop` or the tests derivation"
        );
        let dir = place("folder");
        let photos = dir.join("photos");
        std::fs::create_dir_all(photos.join("2026/june")).unwrap();
        std::fs::write(photos.join("2026/a.jpg"), b"ay").unwrap();
        std::fs::write(photos.join("2026/june/-b.jpg"), b"bee").unwrap();
        let me = Identity::generate();
        let sealed = dir.join("photos.tar.age");
        run(&job(
            Action::SealTo {
                recipients: vec![me.recipient().to_string()],
            },
            &photos,
            &sealed,
        ))
        .unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["photos", "photos.tar.age"], "a clear copy was left");
        assert_eq!(
            ronoctua_pq::sniff(&std::fs::read(&sealed).unwrap()),
            ronoctua_pq::Kind::Pq
        );
        let ids = secret(
            &dir,
            "ids",
            age::secrecy::ExposeSecret::expose_secret(&me.to_secret()),
        );
        let tar = dir.join("photos.tar");
        run(&job(
            Action::Open {
                identities_file: ids,
            },
            &sealed,
            &tar,
        ))
        .unwrap();
        let out = dir.join("out");
        tools.extract(&tar, &out, &[], None, &mut |_| {}).unwrap();
        assert_eq!(std::fs::read(out.join("photos/2026/a.jpg")).unwrap(), b"ay");
        assert_eq!(
            std::fs::read(out.join("photos/2026/june/-b.jpg")).unwrap(),
            b"bee"
        );
        // A FOLDER IS NEVER "OPENED": only a file is.
        let pw = secret(&dir, "pw", "x");
        assert_eq!(
            run(&job(
                Action::OpenPassphrase {
                    passphrase_file: pw
                },
                &photos,
                &dir.join("no")
            ))
            .unwrap_err()
            .why,
            Why::Failed
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a file sealed "to myself" does not open with the
    /// belongings' post-quantum key and its passphrase, a key file holding a
    /// classical identity opens anything, a wrong passphrase is not said as
    /// one, or the key file is touched.
    #[test]
    fn the_belongings_key_opens_what_was_sealed_to_the_person() {
        let dir = place("key");
        let pw = secret(&dir, "pw", "the belongings' passphrase");
        for pq in [true, false] {
            let (identity_line, recipient): (String, Box<dyn age::Recipient>) = if pq {
                let me = Identity::generate();
                (
                    age::secrecy::ExposeSecret::expose_secret(&me.to_secret()).to_owned(),
                    Box::new(me.recipient()),
                )
            } else {
                let me = age::x25519::Identity::generate();
                (
                    age::secrecy::ExposeSecret::expose_secret(&me.to_string()).to_owned(),
                    Box::new(me.to_public()),
                )
            };
            // `key.age` as the belongings write it: the identity, one line,
            // sealed to the passphrase.
            let key = dir.join(format!("key-{pq}.age"));
            let mut sealed_key = Vec::new();
            ronoctua_pq::seal_passphrase(
                &SecretString::from("the belongings' passphrase".to_owned()),
                Some(10),
                &mut format!("{identity_line}\n").as_bytes(),
                &mut sealed_key,
                &mut |_| {},
            )
            .unwrap();
            std::fs::write(&key, &sealed_key).unwrap();
            let note = dir.join(format!("note-{pq}.age"));
            let mut sealed = Vec::new();
            let dyns: Vec<&dyn age::Recipient> = vec![recipient.as_ref()];
            if pq {
                ronoctua_pq::seal(&dyns, &mut &b"mine"[..], &mut sealed, &mut |_| {}).unwrap();
            } else {
                let mut w = age::Encryptor::with_recipients(dyns.into_iter())
                    .unwrap()
                    .wrap_output(&mut sealed)
                    .unwrap();
                w.write_all(b"mine").unwrap();
                w.finish().unwrap();
            }
            std::fs::write(&note, &sealed).unwrap();
            let out = dir.join(format!("note-{pq}"));
            let opened = run(&job(
                Action::OpenWithKey {
                    key_file: key.clone(),
                    passphrase_file: pw.clone(),
                },
                &note,
                &out,
            ));
            if !pq {
                // A CLASSICAL KEY IS NOT THE BELONGINGS' KEY: nothing opens
                // with it, and nothing is left behind.
                assert!(opened.is_err(), "a classical key opened a file");
                assert!(!out.exists(), "a refused open left its output");
                continue;
            }
            opened.unwrap();
            assert_eq!(std::fs::read(&out).unwrap(), b"mine", "pq {pq}");
            assert_eq!(
                std::fs::read(&key).unwrap(),
                sealed_key,
                "the key file changed"
            );
            let wrong = secret(&dir, &format!("wrong-{pq}"), "not it");
            assert_eq!(
                run(&job(
                    Action::OpenWithKey {
                        key_file: key.clone(),
                        passphrase_file: wrong,
                    },
                    &note,
                    &dir.join(format!("never-{pq}")),
                ))
                .unwrap_err()
                .why,
                Why::WrongPassphrase
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: anything but a file or a folder is read — a pipe named like
    /// a file, which would hold the job at nought for ever, or a device
    /// behind a link (`/dev/zero` named `photo.jpg`), which would be sealed
    /// until the disk is full — or an output is left behind for one. Each
    /// runs on a thread, so a regression fails this test by the clock
    /// instead of hanging the whole run.
    #[test]
    fn only_a_file_or_a_folder_is_ever_read() {
        let dir = place("special");
        let pw = secret(&dir, "pw", "horse battery");
        let pipe = dir.join("pipe.txt");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &pipe,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        let device = dir.join("photo.jpg");
        std::os::unix::fs::symlink("/dev/zero", &device).unwrap();
        let sealed_pipe = dir.join("sealed.age");
        std::os::unix::fs::symlink(&pipe, &sealed_pipe).unwrap();
        let in_time = |work: Box<dyn FnOnce() -> Result<(), Why> + Send>| {
            let (said, heard) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = said.send(work());
            });
            heard
                .recv_timeout(std::time::Duration::from_secs(20))
                .expect("a special file held the sealer")
        };
        let jobs = [
            (passphrase(&pw), pipe.clone(), dir.join("pipe.txt.age")),
            (passphrase(&pw), device, dir.join("photo.jpg.age")),
            (
                Action::OpenPassphrase {
                    passphrase_file: pw.clone(),
                },
                sealed_pipe,
                dir.join("sealed"),
            ),
        ];
        for (action, from, into) in jobs {
            let job = job(action, &from, &into);
            let out = into.clone();
            assert_eq!(
                in_time(Box::new(move || run(&job).map(|_| ()).map_err(|r| r.why))),
                Err(Why::Failed),
                "{}",
                from.display()
            );
            assert!(!out.exists(), "{} left an output", from.display());
        }
        // AND THE DESCRIPTOR IS ASKED, not only the path: a pipe swapped in
        // after the look opens at once and is refused for what it is.
        let swapped = pipe;
        assert_eq!(
            in_time(Box::new(move || regular(&swapped)
                .map(|_| ())
                .map_err(|r| r.why))),
            Err(Why::Failed)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_job_with_classical_recipients_alone_seals_nothing() {
        let classical = age::x25519::Identity::generate().to_public().to_string();
        let other = age::x25519::Identity::generate().to_public().to_string();
        // Read, they parse: what refuses them is that none is post-quantum.
        assert!(age::x25519::Recipient::from_str(&classical).is_ok());
        for alone in [vec![classical.clone()], vec![classical, other]] {
            assert_eq!(sealed(&alone).unwrap_err().why, Why::Corrupt);
        }
    }

    #[test]
    fn a_classical_recipient_beside_a_pq_one_is_a_mix() {
        let me = Identity::generate().recipient().to_string();
        let classical = age::x25519::Identity::generate().to_public().to_string();
        for both in [vec![me.clone(), classical.clone()], vec![classical, me]] {
            assert_eq!(sealed(&both).unwrap_err().why, Why::Mixed);
        }
    }

    #[test]
    fn a_pq_recipient_seals_its_stanza() {
        let me = Identity::generate();
        let out = sealed(&[format!(" {} ", me.recipient())]).unwrap();
        assert_eq!(ronoctua_pq::sniff(&out), ronoctua_pq::Kind::Pq);
        // Nothing parses as a recipient that is not one, the identity least.
        let secret = me.to_secret().expose_secret().to_owned();
        for wrong in [secret, "age1".to_owned(), String::new()] {
            assert_eq!(sealed(&[wrong]).unwrap_err().why, Why::Corrupt);
        }
        // No recipient at all is the library's refusal, as before.
        assert_eq!(sealed(&[]).unwrap_err().why, Why::Failed);
    }
}
