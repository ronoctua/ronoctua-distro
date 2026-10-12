//! Which security keys may answer sudo for a person — root's list, not the
//! person's — and registering one with a touch.
//!
//! `pam_u2f` reads a mapping file: one line per user, `name:credential[:…]`,
//! each credential as `pamu2fcfg` writes it. Its default is the person's own
//! `~/.config/Yubico/u2f_keys`, which ANY process of theirs may write — and
//! a key that answers for the password answers for `sudo` too. So the
//! machine's `pam_u2f` reads [`AUTHFILE`] instead: root's, under root's folder,
//! written only here, by the sudo door's `u2f-keys` verb. The person's file
//! ([`person_file`], it travels in the belongings) is where the keys they
//! registered on other machines come from; it is never what PAM trusts.
//!
//! `pam_u2f` is always in sudo's stack, `sufficient`, and only there
//! (`nix/modules/yubikey.nix`): with no line for the person in root's list
//! it answers nothing, so ON AND OFF ARE THE LIST ITSELF, at once, with no
//! rebuild.
//!
//! # One origin for every machine
//!
//! A FIDO credential is bound to the relying party it was made for, and
//! `pam_u2f` and `pamu2fcfg` both default to `pam://<hostname>` — a key
//! registered on one machine would answer on no other. Both are given
//! [`ORIGIN`] instead, as origin and as appid.
//!
//! # Registering a key
//!
//! [`Enrolment`] runs `pamu2fcfg` by store path AS THE PERSON (it needs no
//! root: the key's device is the seat's), and returns the credential once the
//! key is touched. It holds the key's device, so it runs under `setpriv
//! --pdeathsig KILL` and its `Drop` kills it; [`Cancel`] kills it from
//! another thread. What it returns goes to root's list through the door, and
//! to the person's file so it travels.
//!
//! The list is rewritten for the invoking person alone: their line replaced
//! (or taken out, with no credentials), every other user's kept as it was.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};

use crate::{Refusal, Why, refused};

/// Root's list of the keys that may answer for a password — what
/// `nix/modules/yubikey.nix` points `pam_u2f`'s `authfile` at.
pub const AUTHFILE: &str = "/var/lib/ronoctua-security/u2f_keys";

/// The relying party every key is registered for and asked with, on every
/// machine — `pam_u2f`'s `origin` and `appid` in `nix/modules/yubikey.nix`
/// (`pam-u2f-is-only-sufficient` holds the two equal).
pub const ORIGIN: &str = "pam://ronoctua";

/// The person's own list, relative to their home: the one the belongings'
/// `git` target carries to every machine.
pub const PERSON_FILE: &str = ".config/Yubico/u2f_keys";

/// The most a `pamu2fcfg` run may print: one credential, with room.
const SAID_MOST_BYTES: u64 = 16 * 1024;

/// The most keys one person may list: a person with more than a handful of
/// security keys is a list nobody reviews.
pub const MOST_KEYS: usize = 8;

/// The longest credential taken, in bytes: a key handle, a public key, the
/// algorithm and the options, with room to spare.
const CREDENTIAL_MOST_BYTES: usize = 1024;

/// Whether `credential` is one as `pamu2fcfg` writes it after `name:` —
/// base64 and hex, the algorithm and the options, separated by commas — and
/// nothing a mapping line could be split or ended by (`:`, a newline, a
/// space). Pure.
#[must_use]
pub fn credential_is_plain(credential: &str) -> bool {
    !credential.is_empty()
        && credential.len() <= CREDENTIAL_MOST_BYTES
        && credential.contains(',')
        && credential.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=' | b',' | b'-' | b'_')
        })
}

/// The mapping `was` with `user`'s line made of `credentials` — taken out
/// when there are none — and every other line as it was. Pure.
#[must_use]
pub fn keys_with(was: &str, user: &str, credentials: &[String]) -> String {
    let mut out: Vec<String> = was
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter(|line| line.split(':').next() != Some(user))
        .map(str::to_owned)
        .collect();
    if !credentials.is_empty() {
        out.push(format!("{user}:{}", credentials.join(":")));
    }
    let mut text = out.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

/// The credentials `text` (a mapping file) lists for `user`, in order. Pure.
#[must_use]
pub fn credentials_of(text: &str, user: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim_end().split_once(':'))
        .filter(|(name, _)| *name == user)
        .flat_map(|(_, credentials)| credentials.split(':'))
        .filter(|c| !c.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The key handle of a credential — what tells one key from another (the
/// first of its comma-separated parts). Pure.
#[must_use]
pub fn key_handle(credential: &str) -> &str {
    credential.split(',').next().unwrap_or_default()
}

/// `credentials` with `new` at the end — unless a credential of the same
/// key is there already, which stays as it was. Pure.
#[must_use]
pub fn with_key(credentials: &[String], new: &str) -> Vec<String> {
    let mut out = credentials.to_vec();
    if !out.iter().any(|c| key_handle(c) == key_handle(new)) {
        out.push(new.to_owned());
    }
    out
}

/// `credentials` without the key `gone` names (by its key handle). Pure.
#[must_use]
pub fn without_key(credentials: &[String], gone: &str) -> Vec<String> {
    credentials
        .iter()
        .filter(|c| key_handle(c) != key_handle(gone))
        .cloned()
        .collect()
}

/// The keys of `user` in root's list ([`AUTHFILE`]), read as the person
/// (`0644`) — what answers sudo for them on this machine. No list is no
/// keys.
///
/// # Errors
///
/// [`Why::Unreadable`] when the list is there and cannot be read.
pub fn listed(user: &str) -> Result<Vec<String>, Refusal> {
    listed_in(Path::new(AUTHFILE), user)
}

/// [`listed`], of the list at `authfile`.
///
/// # Errors
///
/// As [`listed`].
pub fn listed_in(authfile: &Path, user: &str) -> Result<Vec<String>, Refusal> {
    match std::fs::read_to_string(authfile) {
        Ok(text) => Ok(credentials_of(&text, user)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err(refused(Why::Unreadable)),
    }
}

/// sudo's PAM stack, as the person reads it (`0644`, a link into the store).
pub const SUDO_STACK: &str = "/etc/pam.d/sudo";

/// Whether a PAM stack's text has `pam_u2f` reading root's list: a line that
/// is not a comment, names the module and points its `authfile` at
/// [`AUTHFILE`]. Pure.
#[must_use]
pub fn stack_reads_the_list(text: &str) -> bool {
    let authfile = format!("authfile={AUTHFILE}");
    text.lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with('#'))
        .any(|line| line.contains("pam_u2f.so") && line.contains(&authfile))
}

/// Whether a security key may answer sudo on this machine at all — the
/// module is in sudo's stack, reading root's list (`ronoctua.yubikey.enable`
/// is on). Read as the person; a stack that cannot be read is "no".
#[must_use]
pub fn answers_sudo() -> bool {
    std::fs::read_to_string(SUDO_STACK).is_ok_and(|text| stack_reads_the_list(&text))
}

/// The person's own list, under `home`.
#[must_use]
pub fn person_file(home: &Path) -> PathBuf {
    home.join(PERSON_FILE)
}

/// Writes `user`'s keys into the person's own list at `path` — so a key
/// registered here travels with the belongings — whole and atomically,
/// `0600`, the folders above made when missing. Their other lines stay.
///
/// # Errors
///
/// [`Why::Invalid`] for a name or a credential that is not plain, or more
/// than [`MOST_KEYS`]; [`Why::Unreadable`] when the file is there and
/// cannot be read; [`Why::Failed`] when it cannot be written.
pub fn set_person_keys(path: &Path, user: &str, credentials: &[String]) -> Result<(), Refusal> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|_| refused(Why::Failed))?;
    }
    set_keys_with_mode(path, user, credentials, 0o600)
}

/// Writes `user`'s keys into the list at `authfile` (`0644`: `pam_u2f` runs
/// as whoever is authenticating), whole and atomically.
///
/// # Errors
///
/// [`Why::Invalid`] for a name that is not plain or a credential that is
/// not one ([`credential_is_plain`]), or more than [`MOST_KEYS`];
/// [`Why::Unreadable`] when the list is there and cannot be read;
/// [`Why::Failed`] when it cannot be written.
pub fn set_keys(authfile: &Path, user: &str, credentials: &[String]) -> Result<(), Refusal> {
    set_keys_with_mode(authfile, user, credentials, 0o644)
}

/// [`set_keys`], with the file's `mode`.
fn set_keys_with_mode(
    authfile: &Path,
    user: &str,
    credentials: &[String],
    mode: u32,
) -> Result<(), Refusal> {
    if !crate::users::plain(user)
        || credentials.len() > MOST_KEYS
        || !credentials.iter().all(|c| credential_is_plain(c))
    {
        return Err(refused(Why::Invalid));
    }
    let was = match std::fs::read_to_string(authfile) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(refused(Why::Unreadable)),
    };
    crate::write_whole(
        authfile,
        keys_with(&was, user, credentials).as_bytes(),
        mode,
    )
    .map_err(|e| {
        eprintln!("<3>{} could not be written: {e}", authfile.display());
        refused(Why::Failed)
    })
}

/// `pamu2fcfg`'s arguments: the credential alone, no user name (`-n`), for
/// [`ORIGIN`] as origin and as appid. Pure.
#[must_use]
pub fn enrol_arguments() -> Vec<String> {
    vec![
        "-n".to_owned(),
        "-o".to_owned(),
        ORIGIN.to_owned(),
        "-i".to_owned(),
        ORIGIN.to_owned(),
    ]
}

/// The credential in what `pamu2fcfg -n` printed: one line, a colon before
/// it; `None` when it is not one ([`credential_is_plain`]). Pure.
#[must_use]
pub fn credential_said(stdout: &str) -> Option<String> {
    let line = stdout.trim();
    let credential = line.strip_prefix(':').unwrap_or(line);
    credential_is_plain(credential).then(|| credential.to_owned())
}

/// What a `pamu2fcfg` that failed said went wrong (`LC_ALL=C`): no key
/// plugged in, or something this module does not know. Pure.
fn enrolment_said(said: &str) -> Why {
    if said.contains("No device found") {
        Why::NoKey
    } else {
        Why::Failed
    }
}

/// The programs a registration runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Programs {
    pamu2fcfg: PathBuf,
    setpriv: Option<PathBuf>,
}

impl Programs {
    /// This build's, by store path; a build outside Nix has no `pamu2fcfg`.
    fn baked() -> Result<Self, Refusal> {
        let of = |path: Option<&str>| path.filter(|p| !p.is_empty()).map(PathBuf::from);
        Ok(Self {
            pamu2fcfg: of(option_env!("RONOCTUA_PAMU2FCFG")).ok_or(refused(Why::Unstartable))?,
            setpriv: of(option_env!("RONOCTUA_SETPRIV")),
        })
    }
}

/// A registration waiting for the key's touch. Start it and wait for it on
/// the SAME thread (the death signal is the starting thread's); cancel it
/// from any other with [`Enrolment::cancel`]'s handle. Dropped before it
/// ends, it kills `pamu2fcfg`.
#[derive(Debug)]
pub struct Enrolment {
    child: Arc<Mutex<Child>>,
}

/// Kills a registration from another thread — the person said "cancel".
#[derive(Debug, Clone)]
pub struct Cancel(Arc<Mutex<Child>>);

impl Cancel {
    /// Kills `pamu2fcfg`. Once it has ended (and been waited for) this does
    /// nothing: the process this names is never another one.
    pub fn cancel(&self) {
        let mut child = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
        }
    }
}

impl Enrolment {
    /// Starts a registration as this process's user: `pamu2fcfg` waits for
    /// a key to be touched. Off the event loop: it spawns, and
    /// [`Enrolment::wait`] blocks until the touch.
    ///
    /// # Errors
    ///
    /// [`Why::Unstartable`] when this build has no `pamu2fcfg` or it cannot
    /// be started.
    pub fn start() -> Result<Self, Refusal> {
        Self::start_with(&Programs::baked()?)
    }

    fn start_with(programs: &Programs) -> Result<Self, Refusal> {
        let mut command = match &programs.setpriv {
            Some(setpriv) => {
                let mut c = Command::new(setpriv);
                c.args(["--pdeathsig", "KILL", "--"])
                    .arg(&programs.pamu2fcfg);
                c
            }
            None => Command::new(&programs.pamu2fcfg),
        };
        command
            .args(enrol_arguments())
            .env_clear()
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = crate::tools::spawn(&mut command).map_err(|e| {
            eprintln!("<3>could not start {}: {e}", programs.pamu2fcfg.display());
            refused(Why::Unstartable)
        })?;
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
        })
    }

    /// The handle that cancels it from another thread.
    #[must_use]
    pub fn cancel(&self) -> Cancel {
        Cancel(Arc::clone(&self.child))
    }

    /// Waits for the touch, and returns the credential `pamu2fcfg` made.
    ///
    /// # Errors
    ///
    /// [`Why::Cancelled`] when it was cancelled (killed), [`Why::NoKey`]
    /// when no key is plugged in, [`Why::Failed`] when it failed otherwise
    /// (what it said goes to the journal) or printed no credential.
    pub fn wait(self) -> Result<String, Refusal> {
        let (stdout, stderr) = {
            let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
            (child.stdout.take(), child.stderr.take())
        };
        let stderr = std::thread::spawn(move || {
            let mut said = Vec::new();
            if let Some(stderr) = stderr {
                let _ = stderr.take(SAID_MOST_BYTES).read_to_end(&mut said);
            }
            String::from_utf8_lossy(&said).into_owned()
        });
        // READ TO THE END WITH NO LOCK HELD: a cancel takes the lock to kill
        // it while this waits for the touch.
        let mut out = Vec::new();
        if let Some(stdout) = stdout {
            let _ = stdout.take(SAID_MOST_BYTES).read_to_end(&mut out);
        }
        let status = self
            .child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .wait()
            .map_err(|_| refused(Why::Failed))?;
        let said = stderr.join().unwrap_or_default();
        if status.success() {
            return credential_said(&String::from_utf8_lossy(&out)).ok_or_else(|| {
                eprintln!("<3>pamu2fcfg printed no credential");
                refused(Why::Failed)
            });
        }
        if std::os::unix::process::ExitStatusExt::signal(&status).is_some() {
            return Err(refused(Why::Cancelled));
        }
        let why = enrolment_said(&said);
        if why == Why::Failed {
            for line in said.lines().filter(|l| !l.trim().is_empty()) {
                eprintln!("<3>pamu2fcfg: {}", line.trim_end());
            }
        }
        Err(refused(why))
    }
}

impl Drop for Enrolment {
    fn drop(&mut self) {
        Cancel(Arc::clone(&self.child)).cancel();
        let _ = self
            .child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "owBYtXj1Pq+Zb5c1Kd/AbQ==,04d6f1aa,es256,+presence";
    const OTHER: &str = "Qm9vay1zaGVsZg==,9f00ee12,eddsa,+presence+pin";

    /// FAILS IF: a person's keys land on another user's line, another
    /// user's line is lost, an empty list leaves the person's line behind,
    /// or a credential that could end or split a line is taken.
    #[test]
    fn the_list_changes_the_invoking_persons_line_alone() {
        let was = format!("someone:{OTHER}\nguest:{KEY}\n");
        let mine = vec![KEY.to_owned(), OTHER.to_owned()];
        assert_eq!(
            keys_with(&was, "someone", &mine),
            format!("guest:{KEY}\nsomeone:{KEY}:{OTHER}\n")
        );
        assert_eq!(keys_with(&was, "someone", &[]), format!("guest:{KEY}\n"));
        assert_eq!(keys_with("", "someone", &[]), "");
        assert_eq!(
            keys_with("", "someone", &[KEY.to_owned()]),
            format!("someone:{KEY}\n")
        );
        // A NAME THAT ONLY BEGINS LIKE THE PERSON'S is somebody else.
        assert_eq!(
            keys_with(&format!("someone2:{KEY}\n"), "someone", &[]),
            format!("someone2:{KEY}\n")
        );
        for wrong in ["", "abc", "a:b,c", "a,b\nroot:x,y", "a, b", "a,b;c"] {
            assert!(!credential_is_plain(wrong), "{wrong:?}");
        }
        assert!(credential_is_plain(KEY) && credential_is_plain(OTHER));
    }

    /// FAILS IF: a person's keys are read off another user's line, a key
    /// already listed is added twice (or a new one is not added), removing
    /// one takes another, or the person's own file is not private.
    #[test]
    fn the_keys_are_read_added_and_removed_by_their_handle() {
        let text = format!("someone2:{OTHER}\nsomeone:{KEY}:{OTHER}\n\nbroken line\n");
        assert_eq!(credentials_of(&text, "someone"), [KEY, OTHER]);
        assert_eq!(credentials_of(&text, "someone2"), [OTHER]);
        assert!(credentials_of(&text, "nobody").is_empty());
        let mine = vec![KEY.to_owned()];
        // THE SAME KEY with other options is the same key.
        let again = KEY.replace("+presence", "+presence+pin");
        assert_eq!(with_key(&mine, &again), mine);
        assert_eq!(with_key(&mine, OTHER), [KEY, OTHER]);
        assert_eq!(
            without_key(&[KEY.to_owned(), OTHER.to_owned()], &again),
            [OTHER]
        );
        assert_eq!(key_handle(KEY), "owBYtXj1Pq+Zb5c1Kd/AbQ==");

        use std::os::unix::fs::PermissionsExt as _;
        let root = crate::scratch::Scratch::new("u2f-person");
        let file = person_file(&root.0);
        set_person_keys(&file, "someone", &[KEY.to_owned()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            format!("someone:{KEY}\n")
        );
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(listed_in(&file, "someone").unwrap(), [KEY]);
        assert!(
            listed_in(&root.0.join("absent"), "someone")
                .unwrap()
                .is_empty()
        );
    }

    /// FAILS IF: a registration is not for the one origin every machine
    /// asks with, the credential is not taken from what `pamu2fcfg -n`
    /// prints, or something that is not one is taken.
    #[test]
    fn a_registration_is_for_the_one_origin() {
        assert_eq!(
            enrol_arguments(),
            ["-n", "-o", "pam://ronoctua", "-i", "pam://ronoctua"]
        );
        assert_eq!(credential_said(&format!(":{KEY}\n")).as_deref(), Some(KEY));
        assert_eq!(credential_said(KEY).as_deref(), Some(KEY));
        for wrong in [
            "",
            ":",
            "someone:a,b",
            ":a b,c",
            "\n",
            ":*,pk,es256,+presence",
        ] {
            assert_eq!(credential_said(wrong), None, "{wrong:?}");
        }
        assert_eq!(enrolment_said("No device found. Aborting.\n"), Why::NoKey);
        assert_eq!(
            enrolment_said("error: fido_dev_make_cred (-7)"),
            Why::Failed
        );
    }

    /// FAILS IF: a registration does not return the credential its program
    /// printed, is not run with an empty environment, outlives a cancel, or
    /// says a missing key as anything but that.
    #[test]
    fn a_registration_waits_for_the_touch_and_a_cancel_ends_it() {
        let root = crate::scratch::Scratch::new("u2f-enrol");
        let env = root.0.join("env");
        let touched = Programs {
            pamu2fcfg: root.program(
                "touched",
                &format!(
                    "#!/bin/sh\nexport -p > {}\nprintf '%s\\n' \"$*\" >> {}\nprintf ':{KEY}'\n",
                    env.display(),
                    env.display()
                ),
            ),
            setpriv: None,
        };
        assert_eq!(
            Enrolment::start_with(&touched).unwrap().wait().unwrap(),
            KEY
        );
        let said = std::fs::read_to_string(&env).unwrap();
        assert!(said.contains("LC_ALL") && !said.contains("HOME="), "{said}");
        assert!(
            said.ends_with("-n -o pam://ronoctua -i pam://ronoctua\n"),
            "{said}"
        );

        let absent = Programs {
            pamu2fcfg: root.program(
                "absent",
                "#!/bin/sh\necho 'No device found. Aborting.' >&2\nexit 1\n",
            ),
            setpriv: None,
        };
        assert_eq!(
            Enrolment::start_with(&absent)
                .unwrap()
                .wait()
                .unwrap_err()
                .why,
            Why::NoKey
        );

        // NOBODY TOUCHES IT: a cancel from another thread ends the wait.
        let waiting = Programs {
            // A shell's own loop: the environment is empty, so no `PATH`
            // finds a `sleep`.
            pamu2fcfg: root.program("waiting", "#!/bin/sh\nwhile :; do :; done\n"),
            setpriv: None,
        };
        let enrolment = Enrolment::start_with(&waiting).unwrap();
        let cancel = enrolment.cancel();
        let started = std::time::Instant::now();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            cancel.cancel();
            cancel
        });
        assert_eq!(enrolment.wait().unwrap_err().why, Why::Cancelled);
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        // A CANCEL AFTER THE END names nothing.
        canceller.join().unwrap().cancel();

        assert_eq!(
            Enrolment::start_with(&Programs {
                pamu2fcfg: root.0.join("nowhere"),
                setpriv: None,
            })
            .unwrap_err()
            .why,
            Why::Unstartable
        );
    }

    /// FAILS IF: a stack whose `pam_u2f` reads another list (the person's
    /// own), or a commented line, is taken for one that answers from root's
    /// list — or the stack that does is not.
    #[test]
    fn the_stack_answers_only_from_roots_list() {
        let ours = format!(
            "auth sufficient /nix/store/x-pam/lib/security/pam_u2f.so authfile={AUTHFILE} cue origin=pam://ronoctua\n"
        );
        assert!(stack_reads_the_list(&ours));
        assert!(!stack_reads_the_list(""));
        assert!(!stack_reads_the_list(&format!("# {ours}")));
        assert!(!stack_reads_the_list(
            "auth sufficient pam_u2f.so authfile=/home/p/.config/Yubico/u2f_keys\n"
        ));
        assert!(!stack_reads_the_list(&format!(
            "auth sufficient pam_unix.so authfile={AUTHFILE}\n"
        )));
    }

    /// FAILS IF: the list is written for a name that is not plain or with a
    /// credential that is not one, or a written list is not what
    /// `keys_with` says, or not readable by `pam_u2f` as everyone.
    #[test]
    fn root_writes_the_list_whole() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = crate::scratch::Scratch::new("u2f");
        let file = root.0.join("u2f_keys");
        set_keys(&file, "someone", &[KEY.to_owned()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            format!("someone:{KEY}\n")
        );
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o644
        );
        assert_eq!(
            set_keys(&file, "Some One", &[KEY.to_owned()])
                .unwrap_err()
                .why,
            Why::Invalid
        );
        assert_eq!(
            set_keys(&file, "someone", &["a:b,c".to_owned()])
                .unwrap_err()
                .why,
            Why::Invalid
        );
        set_keys(&file, "someone", &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
    }
}
