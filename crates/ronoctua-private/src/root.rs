//! The root side: what the three units do, and nothing else.
//!
//! # Who runs it
//!
//! `ronoctua-private-root` (`src/bin/root.rs`), as root, from three fixed
//! units (`nix/modules/private.nix`):
//!
//! - [`crate::ROOT_UNIT`] — `run <staging>`: the ONE request the person
//!   staged, read with every check ([`take_staged`]) and done
//!   ([`Machine::run`]). Started only by the sudo door's `private-root`.
//! - [`crate::LOCK_UNIT`] — `lock`, before sleep ([`Machine::lock`]).
//! - [`crate::STATUS_UNIT`] — `status`, at boot and after a switch
//!   ([`Machine::publish`]).
//!
//! # In the machine's own mount namespace
//!
//! `/private` has to appear for everybody, so none of the three may run in a
//! mount namespace of its own: no `ProtectSystem`, `PrivateTmp`,
//! `ProtectHome` or `ReadWritePaths` — and no `PrivateNetwork`, which since
//! systemd 254 brings `PrivateMounts` along — each of which would give it one
//! and keep the mount to itself; the units say `PrivateMounts=no` outright.
//! Its narrowness is here instead: it reads one folder,
//! takes one request, runs the programs below by the paths its unit hands it,
//! and touches the image, its loop, its mapping and `/private` only.
//!
//! # The programs
//!
//! By store path, from the unit's environment ([`Tools::from_env`]) — never
//! by name: a unit's `PATH` is minimal. A program the unit did not hand in is
//! a refusal ([`crate::Why::Failed`]) said in the journal, never a guess.

use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::{
    FORMAT, MAPPER, MOST_REQUEST_BYTES, MOST_SECRET_BYTES, Need, Op, Refusal, Request, Slot, State,
    Status, Verdict, Why, image_of,
};

/// The mount options of the inner btrfs: discards go to the image as holes,
/// in the background; no device nodes, no setuid programs.
pub const MOUNT_OPTIONS: &str = "discard=async,noatime,nodev,nosuid";

/// The cipher, the key size and the key derivation: AES-XTS with two
/// 256-bit keys (a 256-bit security level, which Grover's search halves to
/// 128), and argon2id, which costs memory as well as time.
pub const CIPHER: &str = "aes-xts-plain64";
/// The key size, in bits.
pub const KEY_BITS: &str = "512";
/// The key derivation.
pub const PBKDF: &str = "argon2id";

/// How long the enrolled key is waited for when opening, and the option
/// string `systemd-cryptsetup` takes for it: no question (`headless`), the
/// discards through, and the key found by itself.
pub const FIDO2_OPTIONS: &str = "fido2-device=auto,headless=true,discard,token-timeout=30s";

/// One program the root side runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// `cryptsetup`.
    Cryptsetup,
    /// `losetup`.
    Losetup,
    /// `systemd-cryptenroll`.
    Cryptenroll,
    /// `systemd-cryptsetup`, which opens with the enrolled key.
    Attach,
    /// `mkfs.btrfs`.
    MkfsBtrfs,
    /// `btrfs`.
    Btrfs,
    /// `mount`.
    Mount,
    /// `umount`.
    Umount,
}

impl Tool {
    /// All eight.
    pub const ALL: [Self; 8] = [
        Self::Cryptsetup,
        Self::Losetup,
        Self::Cryptenroll,
        Self::Attach,
        Self::MkfsBtrfs,
        Self::Btrfs,
        Self::Mount,
        Self::Umount,
    ];

    /// The variable its unit hands its store path in.
    #[must_use]
    pub const fn variable(self) -> &'static str {
        match self {
            Self::Cryptsetup => "RONOCTUA_CRYPTSETUP",
            Self::Losetup => "RONOCTUA_LOSETUP",
            Self::Cryptenroll => "RONOCTUA_CRYPTENROLL",
            Self::Attach => "RONOCTUA_CRYPTSETUP_ATTACH",
            Self::MkfsBtrfs => "RONOCTUA_MKFS_BTRFS",
            Self::Btrfs => "RONOCTUA_BTRFS",
            Self::Mount => "RONOCTUA_MOUNT",
            Self::Umount => "RONOCTUA_UMOUNT",
        }
    }
}

/// Where each program is — `None` when its unit did not say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tools {
    paths: Vec<(Tool, PathBuf)>,
}

impl Tools {
    /// The paths the unit's environment names (absolute ones only).
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            paths: Tool::ALL
                .into_iter()
                .filter_map(|t| {
                    std::env::var_os(t.variable())
                        .map(PathBuf::from)
                        .filter(|p| p.is_absolute())
                        .map(|p| (t, p))
                })
                .collect(),
        }
    }

    /// These paths — what a test hands in.
    #[must_use]
    pub const fn of(paths: Vec<(Tool, PathBuf)>) -> Self {
        Self { paths }
    }

    fn path(&self, tool: Tool) -> Result<&Path, Refusal> {
        self.paths
            .iter()
            .find(|(t, _)| *t == tool)
            .map(|(_, p)| p.as_path())
            .ok_or_else(|| {
                eprintln!("<3>the unit did not hand in {}", tool.variable());
                Refusal::of(Why::Failed)
            })
    }
}

/// Where everything is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The image's folder (the `@private` subvolume).
    pub image_dir: PathBuf,
    /// Where the vault is mounted.
    pub mount_point: PathBuf,
    /// The status the doors read.
    pub status: PathBuf,
    /// The last verdict.
    pub verdict: PathBuf,
    /// `/sys/block`: the loops' backing files, the mappings' names.
    pub sys_block: PathBuf,
    /// `/proc/self/mountinfo`.
    pub mountinfo: PathBuf,
    /// `/dev/mapper`.
    pub dev_mapper: PathBuf,
    /// Root's own `0700` folder, for the two programs that take a passphrase
    /// only as a file (the unit's `RuntimeDirectory`, gone when it stops).
    pub scratch: PathBuf,
}

impl Places {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            image_dir: PathBuf::from(crate::IMAGE_DIR),
            mount_point: PathBuf::from(crate::MOUNT_POINT),
            status: PathBuf::from(crate::STATUS_FILE),
            verdict: PathBuf::from(crate::VERDICT_FILE),
            sys_block: PathBuf::from("/sys/block"),
            mountinfo: PathBuf::from("/proc/self/mountinfo"),
            dev_mapper: PathBuf::from("/dev/mapper"),
            scratch: PathBuf::from(SCRATCH),
        }
    }

    /// The image.
    #[must_use]
    pub fn image(&self) -> PathBuf {
        image_of(&self.image_dir)
    }

    /// The open vault's block device.
    #[must_use]
    pub fn mapped(&self) -> PathBuf {
        self.dev_mapper.join(MAPPER)
    }
}

/// Root's scratch folder: the root unit's `RuntimeDirectory`.
pub const SCRATCH: &str = "/run/ronoctua-private-root";

/// A passphrase read by root: wiped when dropped, never printed.
pub struct Key(Zeroizing<Vec<u8>>);

impl Key {
    /// A key of these bytes — what a test hands in.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(Zeroizing::new(bytes.to_vec()))
    }

    fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(<redacted>)")
    }
}

/// A request as root took it: whose it is, and its secrets.
#[derive(Debug)]
pub struct Staged {
    /// What to do.
    pub request: Request,
    /// The person who staged it (the staging folder's owner).
    pub uid: u32,
    /// Their group.
    pub gid: u32,
    /// [`Slot::Passphrase`].
    pub passphrase: Option<Key>,
    /// [`Slot::NewPassphrase`].
    pub new_passphrase: Option<Key>,
}

/// Reads one staged file as root: never through a link, only a plain file
/// the folder's owner owns with nobody else able to read it, one name for
/// it, and no longer than `most`. `Ok(None)` when it is not there.
fn read_staged_file(path: &Path, owner: u32, most: usize) -> Result<Option<Key>, Refusal> {
    let invalid = |what: &str| {
        eprintln!("<3>{}: {what}", path.display());
        Refusal::of(Why::Invalid)
    };
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(invalid("cannot be opened (a link?)")),
    };
    let meta = file.metadata().map_err(|_| invalid("cannot be stat'ed"))?;
    if !meta.is_file() {
        return Err(invalid("is not a plain file"));
    }
    if meta.uid() != owner {
        return Err(invalid("is not the person's"));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(invalid("is readable by others"));
    }
    if meta.nlink() != 1 {
        return Err(invalid("has more than one name"));
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(most as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot be read"))?;
    if bytes.len() > most {
        return Err(invalid("is too long"));
    }
    Ok(Some(Key(bytes)))
}

/// Takes the request the person staged — and wipes the folder, whatever
/// happens: a request is done once.
///
/// The folder must be a real one of a person's (not root's), `0700`; every
/// file in it theirs, `0600` or tighter, with one name, never a link. The
/// request must read and validate, and carry exactly the secrets its
/// operation takes ([`Op::needs`]).
///
/// # Errors
///
/// [`Why::Invalid`], with the operation when the request was read.
pub fn take_staged(staging: &Path) -> Result<Staged, (Refusal, Option<Op>)> {
    let taken = read_all_staged(staging);
    crate::unstage(staging);
    taken
}

fn read_all_staged(staging: &Path) -> Result<Staged, (Refusal, Option<Op>)> {
    let invalid = |what: &str, op: Option<Op>| {
        eprintln!("<3>{}: {what}", staging.display());
        (Refusal::of(Why::Invalid), op)
    };
    let dir = std::fs::symlink_metadata(staging).map_err(|_| invalid("is not there", None))?;
    if !dir.is_dir() {
        return Err(invalid("is not a folder", None));
    }
    if dir.uid() == 0 || dir.mode() & 0o777 != 0o700 {
        return Err(invalid("is not a person's 0700 folder", None));
    }
    let (uid, gid) = (dir.uid(), dir.gid());
    let raw = read_staged_file(&staging.join(crate::REQUEST_FILE), uid, MOST_REQUEST_BYTES)
        .map_err(|r| (r, None))?
        .ok_or_else(|| invalid("holds no request", None))?;
    let request: Request = serde_json::from_slice(raw.bytes())
        .map_err(|_| invalid("the request does not read", None))?;
    let op = Some(request.op);
    request.validate().map_err(|r| (r, op))?;
    let mut secrets = Vec::new();
    for slot in Slot::ALL {
        let secret = read_staged_file(&staging.join(slot.file()), uid, MOST_SECRET_BYTES)
            .map_err(|r| (r, op))?
            .filter(|k| !k.bytes().is_empty());
        match (request.op.needs(slot), &secret) {
            (Need::Must, None) => return Err(invalid("a secret it needs is missing", op)),
            (Need::No, Some(_)) => return Err(invalid("a secret it does not take", op)),
            _ => {}
        }
        secrets.push(secret);
    }
    let new_passphrase = secrets.pop().flatten();
    let passphrase = secrets.pop().flatten();
    Ok(Staged {
        request,
        uid,
        gid,
        passphrase,
        new_passphrase,
    })
}

// ─── reading the machine (pure, over the roots handed in) ──────────────────

/// The loop device whose backing file is `image`, from `/sys/block`. A
/// deleted backing file (`… (deleted)`) is not the image any more.
#[must_use]
pub fn loop_of(sys_block: &Path, image: &Path) -> Option<PathBuf> {
    let wanted = image.as_os_str().as_encoded_bytes();
    std::fs::read_dir(sys_block)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().as_encoded_bytes().starts_with(b"loop"))
        .find(|e| {
            std::fs::read(e.path().join("loop/backing_file"))
                .is_ok_and(|b| b.strip_suffix(b"\n").unwrap_or(&b) == wanted)
        })
        .map(|e| Path::new("/dev").join(e.file_name()))
}

/// The vault's mapping, from `/sys/block`: `Some(suspended)` while it
/// exists.
#[must_use]
pub fn mapping_of(sys_block: &Path, name: &str) -> Option<bool> {
    let entries = std::fs::read_dir(sys_block).ok()?;
    for e in entries.flatten() {
        if !e.file_name().as_encoded_bytes().starts_with(b"dm-") {
            continue;
        }
        let dm = e.path().join("dm");
        let named = std::fs::read_to_string(dm.join("name")).is_ok_and(|n| n.trim_end() == name);
        if named {
            let suspended =
                std::fs::read_to_string(dm.join("suspended")).is_ok_and(|s| s.trim() == "1");
            return Some(suspended);
        }
    }
    None
}

/// `mountinfo`'s octal escapes (`\040` for a space) undone.
fn unescaped(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && bytes
                .get(i + 1..i + 4)
                .is_some_and(|d| d.iter().all(|b| (b'0'..=b'7').contains(b)))
        {
            let n = (bytes[i + 1] - b'0') * 64 + (bytes[i + 2] - b'0') * 8 + (bytes[i + 3] - b'0');
            out.push(n);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether something is mounted at `point`, from `mountinfo`'s text: its
/// type and source when it is. Pure.
#[must_use]
pub fn mounted_in(mountinfo: &str, point: &Path) -> Option<(String, String)> {
    mountinfo.lines().rev().find_map(|line| {
        let (left, right) = line.split_once(" - ")?;
        let at = left.split(' ').nth(4)?;
        if Path::new(&unescaped(at)) != point {
            return None;
        }
        let mut r = right.split(' ');
        Some((r.next()?.to_owned(), unescaped(r.next()?)))
    })
}

/// Whether the header `luksDump --dump-json-metadata` printed has a FIDO2
/// token (`systemd-fido2`). Pure.
#[must_use]
pub fn fido2_in(dump: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(dump)
        .ok()
        .and_then(|v| v.get("tokens").cloned())
        .and_then(|t| t.as_object().cloned())
        .is_some_and(|tokens| {
            tokens
                .values()
                .any(|t| t.get("type").and_then(serde_json::Value::as_str) == Some("systemd-fido2"))
        })
}

/// What `cryptsetup` meant by its exit code: 2 is "no key available with
/// this passphrase", 5 a device that is busy or already there. Pure.
#[must_use]
pub const fn cryptsetup_why(code: Option<i32>) -> Why {
    match code {
        Some(2) => Why::WrongPassphrase,
        Some(5) => Why::Busy,
        _ => Why::Failed,
    }
}

// ─── the argv of each step (pure: what the checks and the tests read) ──────

/// `losetup` attaching the image, direct I/O on: the image's pages are not
/// cached twice, once above the loop and once below.
#[must_use]
pub fn attach_args(image: &Path) -> Vec<String> {
    vec![
        "--find".into(),
        "--show".into(),
        "--direct-io=on".into(),
        image.to_string_lossy().into_owned(),
    ]
}

/// `cryptsetup luksFormat`: LUKS2, AES-XTS 512, argon2id, the passphrase on
/// stdin.
#[must_use]
pub fn format_args(device: &Path) -> Vec<String> {
    vec![
        "luksFormat".into(),
        "--batch-mode".into(),
        "--type".into(),
        "luks2".into(),
        "--cipher".into(),
        CIPHER.into(),
        "--key-size".into(),
        KEY_BITS.into(),
        "--pbkdf".into(),
        PBKDF.into(),
        "--label".into(),
        crate::LABEL.into(),
        "--key-file=-".into(),
        device.to_string_lossy().into_owned(),
    ]
}

/// `cryptsetup open` with the passphrase on stdin, discards let through —
/// kept in the header when `persistent`, so an opening by the enrolled key
/// lets them through too.
#[must_use]
pub fn open_args(device: &Path, persistent: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "open".into(),
        "--type".into(),
        "luks2".into(),
        "--allow-discards".into(),
    ];
    if persistent {
        args.push("--persistent".into());
    }
    args.extend([
        "--key-file=-".into(),
        device.to_string_lossy().into_owned(),
        MAPPER.into(),
    ]);
    args
}

/// `systemd-cryptsetup attach`, opening with the enrolled key and asking
/// nothing.
#[must_use]
pub fn attach_by_key_args(device: &Path) -> Vec<String> {
    vec![
        "attach".into(),
        MAPPER.into(),
        device.to_string_lossy().into_owned(),
        "-".into(),
        FIDO2_OPTIONS.into(),
    ]
}

/// `systemd-cryptenroll` enrolling a FIDO2 key, the passphrase in force as
/// a file: a touch of the key, and no PIN asked (nobody is at a terminal).
#[must_use]
pub fn enroll_args(device: &Path, unlock: &Path) -> Vec<String> {
    vec![
        "--fido2-device=auto".into(),
        "--fido2-with-client-pin=no".into(),
        format!("--unlock-key-file={}", unlock.display()),
        device.to_string_lossy().into_owned(),
    ]
}

/// `mount` of the open vault.
#[must_use]
pub fn mount_args(mapped: &Path, point: &Path) -> Vec<String> {
    vec![
        "-t".into(),
        "btrfs".into(),
        "-o".into(),
        MOUNT_OPTIONS.into(),
        mapped.to_string_lossy().into_owned(),
        point.to_string_lossy().into_owned(),
    ]
}

// ─── running a program ──────────────────────────────────────────────────────

/// How a program ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// Its exit code (`None` when a signal ended it).
    pub code: Option<i32>,
    /// What it printed.
    pub stdout: String,
    /// What it said on standard error.
    pub stderr: String,
}

impl Ran {
    /// Whether it succeeded.
    #[must_use]
    pub const fn ok(&self) -> bool {
        matches!(self.code, Some(0))
    }
}

fn spawn(command: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    let mut tries = 0;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            spawned => return spawned,
        }
    }
}

/// Runs `program` with `args`, an empty environment (`LC_ALL=C`) and
/// `stdin` fed and closed. A program that could not be started is
/// [`Why::Failed`]; one that ran is a [`Ran`], whatever it answered.
fn run_program(program: &Path, args: &[String], stdin: Option<&[u8]>) -> Result<Ran, Refusal> {
    use std::process::{Command, Stdio};
    let mut command = Command::new(program);
    command
        .args(args.iter().map(OsStr::new))
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn(&mut command).map_err(|e| {
        eprintln!("<3>{} could not be started: {e}", program.display());
        Refusal::of(Why::Failed)
    })?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let _ = pipe.write_all(bytes);
    }
    let out = child
        .wait_with_output()
        .map_err(|_| Refusal::of(Why::Failed))?;
    Ok(Ran {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Seconds since the epoch.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Writes `bytes` to `path` whole and atomically, `mode` from its creation:
/// a draft beside it (`create_new`, never through a link) renamed over it.
fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or(std::io::ErrorKind::InvalidInput)?
        .to_string_lossy();
    let draft = parent.join(format!(".{name}.{}", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(&draft)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&draft, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

// ─── the machine ───────────────────────────────────────────────────────────

/// The vault on this machine: its places and its programs.
#[derive(Debug, Clone)]
pub struct Machine {
    tools: Tools,
    places: Places,
}

impl Machine {
    /// The machine's vault, run with these programs.
    #[must_use]
    pub fn system(tools: Tools) -> Self {
        Self::of(tools, Places::system())
    }

    /// A vault at these places — what a test builds.
    #[must_use]
    pub const fn of(tools: Tools, places: Places) -> Self {
        Self { tools, places }
    }

    /// Runs one program by its tool.
    fn run(&self, tool: Tool, args: &[String], stdin: Option<&[u8]>) -> Result<Ran, Refusal> {
        run_program(self.tools.path(tool)?, args, stdin)
    }

    /// Runs one program that must succeed; its failure is `why`, with what
    /// it said in the journal.
    fn must(
        &self,
        tool: Tool,
        args: &[String],
        stdin: Option<&[u8]>,
        why: impl FnOnce(&Ran) -> Why,
    ) -> Result<Ran, Refusal> {
        let ran = self.run(tool, args, stdin)?;
        if ran.ok() {
            return Ok(ran);
        }
        eprintln!(
            "<3>{} {} ended with {:?}: {}",
            tool.variable(),
            args.first().map_or("", String::as_str),
            ran.code,
            ran.stderr.trim()
        );
        Err(Refusal::of(why(&ran)))
    }

    /// Runs one program whose failure is only said.
    fn try_to(&self, tool: Tool, args: &[String]) {
        match self.run(tool, args, None) {
            Ok(ran) if ran.ok() => {}
            Ok(ran) => eprintln!(
                "<4>{} {} ended with {:?}: {}",
                tool.variable(),
                args.join(" "),
                ran.code,
                ran.stderr.trim()
            ),
            Err(_) => {}
        }
    }

    fn image_exists(&self) -> bool {
        std::fs::symlink_metadata(self.places.image()).is_ok_and(|m| m.is_file())
    }

    fn attached(&self) -> Option<PathBuf> {
        loop_of(&self.places.sys_block, &self.places.image())
    }

    fn mapping(&self) -> Option<bool> {
        mapping_of(&self.places.sys_block, MAPPER)
    }

    fn mounted(&self) -> bool {
        std::fs::read_to_string(&self.places.mountinfo)
            .is_ok_and(|text| mounted_in(&text, &self.places.mount_point).is_some())
    }

    /// The image's loop: the one attached, or a new one. `true` when it was
    /// made here (and so is detached again by whoever made it).
    fn attach(&self) -> Result<(PathBuf, bool), Refusal> {
        if let Some(device) = self.attached() {
            return Ok((device, false));
        }
        let ran = self.must(
            Tool::Losetup,
            &attach_args(&self.places.image()),
            None,
            |_| Why::Failed,
        )?;
        let device = PathBuf::from(ran.stdout.trim());
        if !device.is_absolute() {
            eprintln!("<3>losetup named no device: {:?}", ran.stdout);
            return Err(Refusal::of(Why::Failed));
        }
        Ok((device, true))
    }

    fn detach(&self, device: &Path) {
        self.try_to(
            Tool::Losetup,
            &["--detach".into(), device.to_string_lossy().into_owned()],
        );
    }

    /// Whether the passphrase opens the header, without opening anything.
    fn test_passphrase(&self, device: &Path, key: &Key) -> Result<(), Refusal> {
        self.must(
            Tool::Cryptsetup,
            &[
                "open".into(),
                "--test-passphrase".into(),
                "--key-file=-".into(),
                device.to_string_lossy().into_owned(),
            ],
            Some(key.bytes()),
            |r| cryptsetup_why(r.code),
        )
        .map(|_| ())
    }

    /// A passphrase as a `0600` file in root's own `0700` folder, for the
    /// two programs that read one only from a file. Removed (wiped) by
    /// [`Machine::forget_file`].
    fn key_file(&self, name: &str, key: &Key) -> Result<PathBuf, Refusal> {
        let dir = &self.places.scratch;
        let fail = |what: &str| {
            eprintln!("<3>{}: {what}", dir.display());
            Refusal::of(Why::Failed)
        };
        match std::fs::symlink_metadata(dir) {
            Ok(m) if m.is_dir() && m.uid() == rustix::process::geteuid().as_raw() => {}
            Ok(_) => return Err(fail("is not root's own folder")),
            Err(_) => std::fs::create_dir_all(dir).map_err(|_| fail("cannot be made"))?,
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| fail("cannot be closed to others"))?;
        let path = dir.join(name);
        crate::wipe(&path);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
            .open(&path)
            .map_err(|_| fail("the key file cannot be made"))?;
        file.write_all(key.bytes())
            .map_err(|_| fail("the key file cannot be written"))?;
        Ok(path)
    }

    fn forget_file(path: &Path) {
        crate::wipe(path);
    }

    /// Whether the image's folder is a mount of its own (the `@private`
    /// subvolume): never an image on the root, which a snapshot would keep
    /// and the wipe would take.
    fn own_subvolume(&self) -> bool {
        use rustix::fs::{AtFlags, CWD, StatxAttributes, StatxFlags, statx};
        statx(
            CWD,
            &self.places.image_dir,
            AtFlags::SYMLINK_NOFOLLOW,
            StatxFlags::BASIC_STATS,
        )
        .is_ok_and(|s| {
            s.stx_attributes_mask.contains(StatxAttributes::MOUNT_ROOT)
                && s.stx_attributes.contains(StatxAttributes::MOUNT_ROOT)
        })
    }

    /// The image folder as it must be, made so again right before an image
    /// is put in it: `0711` (its names not listable, the image stat-able by
    /// the doors) and NOCOW for what is made in it. The boot lays both too —
    /// the subvolume unit, on the mount's root before it is mounted — but a
    /// line of `tmpfiles` would race a `nofail` mount, and the gesture that
    /// makes the image is the one moment it must be right.
    fn lay_the_image_dir(&self) {
        let dir = &self.places.image_dir;
        if let Err(e) = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o711)) {
            eprintln!("<4>{}: its mode not set: {e}", dir.display());
        }
        if let Ok(folder) = std::fs::File::open(dir)
            && let Ok(flags) = rustix::fs::ioctl_getflags(&folder)
            && !flags.contains(rustix::fs::IFlags::NOCOW)
            && let Err(e) = rustix::fs::ioctl_setflags(&folder, flags | rustix::fs::IFlags::NOCOW)
        {
            eprintln!("<4>{}: NOCOW not set: {e}", dir.display());
        }
    }

    /// How big the disk under the image folder is, and how much is free.
    fn room(&self) -> Option<(u64, u64)> {
        let v = rustix::fs::statvfs(&self.places.image_dir).ok()?;
        Some((
            v.f_blocks.saturating_mul(v.f_frsize),
            v.f_bavail.saturating_mul(v.f_frsize),
        ))
    }

    /// `/private` as it must be underneath: root's, `0000` and immutable, so
    /// nothing lands in it while the vault is not mounted over it. Made here
    /// on a machine that switched to the vault and has not booted since (the
    /// boot's `tmpfiles` makes it otherwise).
    fn ensure_mount_point(&self) {
        use std::os::unix::fs::DirBuilderExt as _;
        let point = &self.places.mount_point;
        if std::fs::symlink_metadata(point).is_ok() {
            return;
        }
        if let Err(e) = std::fs::DirBuilder::new().mode(0o000).create(point) {
            eprintln!("<3>{}: {e}", point.display());
            return;
        }
        if let Ok(dir) = std::fs::File::open(point)
            && let Ok(flags) = rustix::fs::ioctl_getflags(&dir)
            && let Err(e) = rustix::fs::ioctl_setflags(&dir, flags | rustix::fs::IFlags::IMMUTABLE)
        {
            eprintln!("<4>{}: not made immutable: {e}", point.display());
        }
    }

    fn mount(&self) -> Result<(), Refusal> {
        self.ensure_mount_point();
        self.must(
            Tool::Mount,
            &mount_args(&self.places.mapped(), &self.places.mount_point),
            None,
            |_| Why::Failed,
        )
        .map(|_| ())
    }

    /// Unmounts `/private`; anything holding it is [`Why::Busy`].
    fn unmount(&self) -> Result<(), Refusal> {
        self.must(
            Tool::Umount,
            &[self.places.mount_point.to_string_lossy().into_owned()],
            None,
            |r| {
                if r.stderr.contains("busy") {
                    Why::Busy
                } else {
                    Why::Failed
                }
            },
        )
        .map(|_| ())
    }

    fn close_mapping(&self) -> Result<(), Refusal> {
        self.must(
            Tool::Cryptsetup,
            &["close".into(), MAPPER.into()],
            None,
            |r| cryptsetup_why(r.code),
        )
        .map(|_| ())
    }

    /// The inner btrfs as large as its device: after a grow made while the
    /// vault was locked, or while it was open.
    fn fill(&self) {
        self.try_to(
            Tool::Btrfs,
            &[
                "filesystem".into(),
                "resize".into(),
                "max".into(),
                self.places.mount_point.to_string_lossy().into_owned(),
            ],
        );
    }

    // ── the operations ─────────────────────────────────────────────────────

    /// Does the staged request, and keeps its verdict and the new status.
    ///
    /// # Errors
    ///
    /// The operation's refusal.
    pub fn run_request(&self, staged: &Staged) -> Result<(), Refusal> {
        let pass = staged.passphrase.as_ref();
        match staged.request.op {
            Op::Create { cap_bytes } => match pass {
                Some(key) => self.create(cap_bytes, (staged.uid, staged.gid), key),
                None => Err(Refusal::of(Why::Invalid)),
            },
            Op::Open => self.open(pass),
            Op::Close => self.close(),
            Op::EnrollFido2 => match pass {
                Some(key) => self.enroll(key),
                None => Err(Refusal::of(Why::Invalid)),
            },
            Op::ForgetFido2 => self.forget(),
            Op::Passphrase => match (pass, staged.new_passphrase.as_ref()) {
                (Some(old), Some(new)) => self.change(old, new),
                _ => Err(Refusal::of(Why::Invalid)),
            },
            Op::Grow { cap_bytes } => self.grow(cap_bytes, pass),
        }
    }

    /// The root unit's whole run: the staged request taken, done, its
    /// verdict and the status written.
    ///
    /// # Errors
    ///
    /// Why it was refused.
    pub fn run_staged(&self, staging: &Path) -> Result<(), Refusal> {
        let (op, result) = match take_staged(staging) {
            Ok(staged) => (Some(staged.request.op), self.run_request(&staged)),
            Err((refusal, op)) => (op, Err(refusal)),
        };
        self.record(op, result);
        self.publish();
        result
    }

    /// Makes the vault: the sparse image, NOCOW, its loop, LUKS2, the inner
    /// btrfs mounted at `/private` and given to the person. Anything that
    /// fails on the way is undone, the image included.
    fn create(&self, cap: u64, owner: (u32, u32), key: &Key) -> Result<(), Refusal> {
        let image = self.places.image();
        if std::fs::symlink_metadata(&image).is_ok() || self.mapping().is_some() {
            return Err(Refusal::of(Why::Exists));
        }
        if !self.own_subvolume() {
            eprintln!(
                "<3>{} is not a subvolume mounted of its own; the image stays off the root",
                self.places.image_dir.display()
            );
            return Err(Refusal::of(Why::Failed));
        }
        match self.room() {
            Some((total, free)) if cap <= total && free >= crate::MIN_CAP_BYTES => {}
            _ => return Err(Refusal::of(Why::NoRoom)),
        }
        self.lay_the_image_dir();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
            .open(&image)
            .map_err(|e| {
                eprintln!("<3>{}: {e}", image.display());
                Refusal::of(Why::Failed)
            })?;
        // NOCOW BEFORE THE FIRST BYTE: btrfs keeps the attribute only on an
        // empty file. The folder's `+C` gives it too; this is the belt.
        match rustix::fs::ioctl_getflags(&file) {
            Ok(flags) => {
                if let Err(e) = rustix::fs::ioctl_setflags(&file, flags | rustix::fs::IFlags::NOCOW)
                {
                    eprintln!("<4>{}: NOCOW not set: {e}", image.display());
                }
            }
            Err(e) => eprintln!("<4>{}: its attributes do not read: {e}", image.display()),
        }
        let sized = file.set_len(cap);
        drop(file);
        let made = sized
            .map_err(|_| Refusal::of(Why::NoRoom))
            .and_then(|()| self.make(owner, key));
        if made.is_err() {
            self.undo_create();
        }
        made
    }

    fn make(&self, owner: (u32, u32), key: &Key) -> Result<(), Refusal> {
        let (device, _) = self.attach()?;
        self.must(
            Tool::Cryptsetup,
            &format_args(&device),
            Some(key.bytes()),
            |r| cryptsetup_why(r.code),
        )?;
        self.must(
            Tool::Cryptsetup,
            &open_args(&device, true),
            Some(key.bytes()),
            |r| cryptsetup_why(r.code),
        )?;
        self.must(
            Tool::MkfsBtrfs,
            &[
                "--force".into(),
                "--quiet".into(),
                "--nodiscard".into(),
                "--label".into(),
                "private".into(),
                self.places.mapped().to_string_lossy().into_owned(),
            ],
            None,
            |_| Why::Failed,
        )?;
        self.mount()?;
        // THE VAULT IS THE PERSON'S: its root theirs, closed to everybody
        // else. `/private` itself stays root's `0000` underneath.
        let point = &self.places.mount_point;
        std::os::unix::fs::chown(point, Some(owner.0), Some(owner.1))
            .and_then(|()| std::fs::set_permissions(point, std::fs::Permissions::from_mode(0o700)))
            .map_err(|e| {
                eprintln!("<3>{}: {e}", point.display());
                Refusal::of(Why::Failed)
            })
    }

    fn undo_create(&self) {
        if self.mounted() {
            let _ = self.unmount();
        }
        if self.mapping().is_some() {
            let _ = self.close_mapping();
        }
        if let Some(device) = self.attached() {
            self.detach(&device);
        }
        let _ = std::fs::remove_file(self.places.image());
    }

    /// Opens it: resumes a suspended one, mounts one that is mapped, or
    /// attaches, unlocks (the passphrase, or the enrolled key when none was
    /// typed) and mounts. Anything that fails on the way is undone.
    fn open(&self, key: Option<&Key>) -> Result<(), Refusal> {
        if !self.image_exists() {
            return Err(Refusal::of(Why::Absent));
        }
        match self.mapping() {
            Some(true) => {
                let key = key.ok_or(Refusal::of(Why::Invalid))?;
                self.must(
                    Tool::Cryptsetup,
                    &["luksResume".into(), "--key-file=-".into(), MAPPER.into()],
                    Some(key.bytes()),
                    |r| cryptsetup_why(r.code),
                )?;
                return if self.mounted() { Ok(()) } else { self.mount() };
            }
            Some(false) => return if self.mounted() { Ok(()) } else { self.mount() },
            None => {}
        }
        let (device, made) = self.attach()?;
        let unlocked = match key {
            Some(key) => self
                .must(
                    Tool::Cryptsetup,
                    &open_args(&device, false),
                    Some(key.bytes()),
                    |r| cryptsetup_why(r.code),
                )
                .map(|_| ()),
            None if self.fido2(&device) => self
                .must(Tool::Attach, &attach_by_key_args(&device), None, |_| {
                    Why::NoKey
                })
                .map(|_| ()),
            None => Err(Refusal::of(Why::NoKey)),
        };
        if let Err(refusal) = unlocked {
            if made {
                self.detach(&device);
            }
            return Err(refusal);
        }
        if let Err(refusal) = self.mount() {
            let _ = self.close_mapping();
            self.detach(&device);
            return Err(refusal);
        }
        self.fill();
        Ok(())
    }

    /// Closes it: unmounted, unmapped, detached. Something holding
    /// `/private` is [`Why::Busy`], and nothing is taken away from under it.
    fn close(&self) -> Result<(), Refusal> {
        match self.mapping() {
            Some(true) => return Err(Refusal::of(Why::Busy)),
            Some(false) => {
                if self.mounted() {
                    self.unmount()?;
                }
                self.close_mapping()?;
            }
            None => {}
        }
        if let Some(device) = self.attached() {
            self.detach(&device);
        }
        Ok(())
    }

    /// Before sleep: closed — or, when something holds `/private`,
    /// suspended: the key wiped from the kernel and its I/O frozen until the
    /// passphrase resumes it. The status says so BEFORE the suspension, so a
    /// door that reads it stops touching `/private` first.
    ///
    /// # Errors
    ///
    /// A failure that left it neither closed nor suspended.
    pub fn lock(&self) -> Result<State, Refusal> {
        let locked = match self.close() {
            Ok(()) => Ok(if self.image_exists() {
                State::Locked
            } else {
                State::Absent
            }),
            Err(r) if r.why == Why::Busy && self.mapping() == Some(true) => {
                Ok(State::SuspendedInUse)
            }
            Err(r) if r.why == Why::Busy => {
                let mut status = self.status();
                status.state = State::SuspendedInUse;
                self.write_status(&status);
                match self.must(
                    Tool::Cryptsetup,
                    &["luksSuspend".into(), MAPPER.into()],
                    None,
                    |r| cryptsetup_why(r.code),
                ) {
                    Ok(_) => Ok(State::SuspendedInUse),
                    Err(refusal) => Err(refusal),
                }
            }
            Err(refusal) => Err(refusal),
        };
        self.publish();
        locked
    }

    /// Whether the header holds an enrolled key.
    fn fido2(&self, device_or_image: &Path) -> bool {
        self.run(
            Tool::Cryptsetup,
            &[
                "luksDump".into(),
                "--dump-json-metadata".into(),
                device_or_image.to_string_lossy().into_owned(),
            ],
            None,
        )
        .is_ok_and(|r| r.ok() && fido2_in(&r.stdout))
    }

    /// Runs `work` on the image's loop — attached for it when the vault is
    /// locked, and detached again after.
    fn on_the_loop(&self, work: impl FnOnce(&Path) -> Result<(), Refusal>) -> Result<(), Refusal> {
        if !self.image_exists() {
            return Err(Refusal::of(Why::Absent));
        }
        let (device, made) = self.attach()?;
        let result = work(&device);
        if made && self.mapping().is_none() {
            self.detach(&device);
        }
        result
    }

    /// Enrols a FIDO2 key: the passphrase checked first (so a wrong one is
    /// said as that), then a touch of the key. One key at a time.
    fn enroll(&self, key: &Key) -> Result<(), Refusal> {
        self.on_the_loop(|device| {
            if self.fido2(device) {
                return Err(Refusal::of(Why::Exists));
            }
            self.test_passphrase(device, key)?;
            let file = self.key_file("unlock", key)?;
            let enrolled = self.must(Tool::Cryptenroll, &enroll_args(device, &file), None, |_| {
                Why::NoKey
            });
            Self::forget_file(&file);
            enrolled.map(|_| ())
        })
    }

    /// Forgets the enrolled key; the passphrase stays.
    fn forget(&self) -> Result<(), Refusal> {
        self.on_the_loop(|device| {
            if !self.fido2(device) {
                return Ok(());
            }
            self.must(
                Tool::Cryptenroll,
                &[
                    "--wipe-slot=fido2".into(),
                    device.to_string_lossy().into_owned(),
                ],
                None,
                |_| Why::Failed,
            )
            .map(|_| ())
        })
    }

    /// A new passphrase in place of the old: the old checked first.
    fn change(&self, old: &Key, new: &Key) -> Result<(), Refusal> {
        self.on_the_loop(|device| {
            self.test_passphrase(device, old)?;
            let old_file = self.key_file("old", old)?;
            let new_file = match self.key_file("new", new) {
                Ok(f) => f,
                Err(r) => {
                    Self::forget_file(&old_file);
                    return Err(r);
                }
            };
            let changed = self.must(
                Tool::Cryptsetup,
                &[
                    "luksChangeKey".into(),
                    "--batch-mode".into(),
                    "--pbkdf".into(),
                    PBKDF.into(),
                    format!("--key-file={}", old_file.display()),
                    device.to_string_lossy().into_owned(),
                    new_file.to_string_lossy().into_owned(),
                ],
                None,
                |r| cryptsetup_why(r.code),
            );
            Self::forget_file(&old_file);
            Self::forget_file(&new_file);
            changed.map(|_| ())
        })
    }

    /// A larger cap: the image made longer (still sparse) and, while it is
    /// open, the loop, the mapping and the inner btrfs grown with it. A
    /// locked vault grows into it at its next opening.
    fn grow(&self, cap: u64, key: Option<&Key>) -> Result<(), Refusal> {
        let image = self.places.image();
        let len = std::fs::symlink_metadata(&image)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .ok_or(Refusal::of(Why::Absent))?
            .len();
        if cap <= len {
            return Err(Refusal::of(Why::Invalid));
        }
        match self.room() {
            Some((total, _)) if cap <= total => {}
            _ => return Err(Refusal::of(Why::NoRoom)),
        }
        std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
            .open(&image)
            .and_then(|f| f.set_len(cap))
            .map_err(|e| {
                eprintln!("<3>{}: {e}", image.display());
                Refusal::of(Why::Failed)
            })?;
        if let Some(device) = self.attached() {
            self.try_to(
                Tool::Losetup,
                &[
                    "--set-capacity".into(),
                    device.to_string_lossy().into_owned(),
                ],
            );
            // AN OPEN VAULT'S MAPPING grows with the passphrase (LUKS2 asks
            // for the key to reload it); without one it waits for the next
            // opening, which maps the whole image.
            if self.mapping() == Some(false) {
                let resized = match key {
                    Some(key) => self.run(
                        Tool::Cryptsetup,
                        &["resize".into(), "--key-file=-".into(), MAPPER.into()],
                        Some(key.bytes()),
                    ),
                    None => self.run(Tool::Cryptsetup, &["resize".into(), MAPPER.into()], None),
                };
                match resized {
                    Ok(ran) if ran.ok() => {
                        if self.mounted() {
                            self.fill();
                        }
                    }
                    Ok(ran) => eprintln!(
                        "<4>the mapping keeps its size until the next opening: {}",
                        ran.stderr.trim()
                    ),
                    Err(_) => {}
                }
            }
        }
        Ok(())
    }

    // ── what the doors read ────────────────────────────────────────────────

    /// The vault as it is now.
    #[must_use]
    pub fn status(&self) -> Status {
        let image = self.places.image();
        let Ok(meta) = std::fs::symlink_metadata(&image) else {
            return Status::absent();
        };
        if !meta.is_file() {
            return Status::absent();
        }
        let state = match self.mapping() {
            Some(true) => State::SuspendedInUse,
            Some(false) => State::Open,
            None => State::Locked,
        };
        Status {
            format: FORMAT,
            state,
            cap_bytes: meta.len(),
            apparent_bytes: meta.len(),
            real_bytes: meta.blocks().saturating_mul(512),
            fido2: self.fido2(&image),
        }
    }

    fn write_status(&self, status: &Status) {
        let written = serde_json::to_vec(status)
            .map_err(std::io::Error::other)
            .and_then(|bytes| write_atomic(&self.places.status, &bytes, 0o644));
        if let Err(e) = written {
            eprintln!("<3>{}: {e}", self.places.status.display());
        }
    }

    /// Writes the status as it is now, and answers it.
    pub fn publish(&self) -> Status {
        let status = self.status();
        self.write_status(&status);
        status
    }

    /// Keeps the verdict of one request (`op` is `None` for one that did not
    /// read).
    pub fn record(&self, op: Option<Op>, result: Result<(), Refusal>) {
        let verdict = Verdict {
            format: FORMAT,
            op: op.map_or("unread", Op::word).to_owned(),
            why: result.err().map(|r| r.why),
            at_s: now_s(),
        };
        let written = serde_json::to_vec(&verdict)
            .map_err(std::io::Error::other)
            .and_then(|bytes| write_atomic(&self.places.verdict, &bytes, 0o644));
        if let Err(e) = written {
            eprintln!("<3>{}: {e}", self.places.verdict.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-private-root-{name}-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn folder(&self, name: &str, mode: u32) -> PathBuf {
            let p = self.0.join(name);
            std::fs::create_dir_all(&p).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
            p
        }

        /// A fake program: a shell script that logs its arguments and stdin.
        ///
        /// It brings the test's own `PATH`, because root runs every program
        /// with an EMPTY environment (`run_program`): the fake's `cat`, `cp`
        /// and `mkdir` would otherwise be found only through the shell's
        /// built-in default, which names `/usr/bin` — there on a workstation,
        /// absent in the build sandbox, where the fake then logs nothing.
        fn program(&self, name: &str, body: &str) -> PathBuf {
            let p = self.0.join(name);
            let path = std::env::var("PATH")
                .unwrap_or_default()
                .replace('\'', r"'\''");
            std::fs::write(&p, format!("#!/bin/sh\nPATH='{path}'\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        }

        fn places(&self) -> Places {
            Places {
                image_dir: self.folder("images", 0o711),
                mount_point: self.folder("private", 0o755),
                status: self.0.join("run/status.json"),
                verdict: self.0.join("run/verdict.json"),
                sys_block: self.folder("sys/block", 0o755),
                mountinfo: self.0.join("mountinfo"),
                dev_mapper: self.folder("dev/mapper", 0o755),
                scratch: self.0.join("root-scratch"),
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn me() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn staged_folder(s: &Scratch) -> PathBuf {
        s.folder("staged", 0o700)
    }

    /// FAILS IF: root takes a request from a folder that is not a person's
    /// 0700 one, from a file others can read, through a link or a second
    /// name, with a secret its operation does not take or without one it
    /// needs — or leaves anything behind in the folder.
    #[test]
    fn root_takes_only_what_the_person_staged_as_it_must_be() {
        if me() == 0 {
            return; // the checks are about a person's folder
        }
        let s = Scratch::new("take");
        let dir = staged_folder(&s);
        crate::stage(&dir, &Request::of(Op::Open)).unwrap();
        crate::stage_secret(&dir, Slot::Passphrase, b"pass").unwrap();
        let taken = take_staged(&dir).expect("taken");
        assert_eq!(taken.request.op, Op::Open);
        assert_eq!(
            taken.passphrase.as_ref().map(Key::bytes),
            Some(&b"pass"[..])
        );
        assert_eq!(
            (taken.uid, taken.gid),
            (me(), rustix::process::getgid().as_raw())
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a request is done once"
        );
        assert_eq!(format!("{:?}", taken.passphrase), "Some(Key(<redacted>))");

        // OPENING BY THE KEY: no secret at all is a request.
        crate::stage(&dir, &Request::of(Op::Open)).unwrap();
        assert!(take_staged(&dir).unwrap().passphrase.is_none());

        let refused = |dir: &Path| take_staged(dir).map(|_| ()).unwrap_err().0.why;
        // A SECRET ITS OPERATION DOES NOT TAKE.
        crate::stage(&dir, &Request::of(Op::Close)).unwrap();
        crate::stage_secret(&dir, Slot::Passphrase, b"pass").unwrap();
        assert_eq!(refused(&dir), Why::Invalid);
        // ONE IT NEEDS, missing.
        crate::stage(&dir, &Request::of(Op::Passphrase)).unwrap();
        crate::stage_secret(&dir, Slot::Passphrase, b"old").unwrap();
        assert_eq!(
            take_staged(&dir).map(|_| ()).unwrap_err(),
            (Refusal::of(Why::Invalid), Some(Op::Passphrase))
        );
        // A FILE OTHERS CAN READ.
        crate::stage(&dir, &Request::of(Op::Close)).unwrap();
        std::fs::set_permissions(
            dir.join(crate::REQUEST_FILE),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(refused(&dir), Why::Invalid);
        // A LINK where the request goes.
        let elsewhere = s.0.join("elsewhere.json");
        std::fs::write(&elsewhere, br#"{"format":1,"op":{"op":"close"}}"#).unwrap();
        std::os::unix::fs::symlink(&elsewhere, dir.join(crate::REQUEST_FILE)).unwrap();
        assert_eq!(refused(&dir), Why::Invalid);
        assert!(elsewhere.exists(), "never followed");
        // A SECOND NAME for it.
        crate::stage(&dir, &Request::of(Op::Close)).unwrap();
        std::fs::hard_link(dir.join(crate::REQUEST_FILE), s.0.join("second")).unwrap();
        assert_eq!(refused(&dir), Why::Invalid);
        // A FOLDER OTHERS CAN ENTER.
        let open = s.folder("open", 0o755);
        crate::stage(&open, &Request::of(Op::Close)).unwrap();
        assert_eq!(refused(&open), Why::Invalid);
        // NOTHING STAGED.
        assert_eq!(refused(&dir), Why::Invalid);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }

    /// FAILS IF: the machine's facts stop reading from `/sys` and
    /// `mountinfo` as the kernel writes them.
    #[test]
    fn the_machine_is_read_from_the_kernels_files() {
        let s = Scratch::new("read");
        let block = s.folder("sys/block", 0o755);
        let image = Path::new("/var/lib/ronoctua-private/private.img");
        for (name, backing) in [
            ("loop0", "/var/lib/other.img\n"),
            ("loop7", "/var/lib/ronoctua-private/private.img\n"),
        ] {
            std::fs::create_dir_all(block.join(name).join("loop")).unwrap();
            std::fs::write(block.join(name).join("loop/backing_file"), backing).unwrap();
        }
        std::fs::create_dir_all(block.join("loop9/loop")).unwrap();
        std::fs::write(
            block.join("loop9/loop/backing_file"),
            "/var/lib/ronoctua-private/private.img (deleted)\n",
        )
        .unwrap();
        assert_eq!(loop_of(&block, image), Some(PathBuf::from("/dev/loop7")));
        assert_eq!(loop_of(&block, Path::new("/nowhere")), None);
        assert_eq!(mapping_of(&block, MAPPER), None);
        std::fs::create_dir_all(block.join("dm-3/dm")).unwrap();
        std::fs::write(block.join("dm-3/dm/name"), "cryptroot\n").unwrap();
        std::fs::write(block.join("dm-3/dm/suspended"), "0\n").unwrap();
        std::fs::create_dir_all(block.join("dm-4/dm")).unwrap();
        std::fs::write(block.join("dm-4/dm/name"), "ronoctua-private\n").unwrap();
        std::fs::write(block.join("dm-4/dm/suspended"), "0\n").unwrap();
        assert_eq!(mapping_of(&block, MAPPER), Some(false));
        std::fs::write(block.join("dm-4/dm/suspended"), "1\n").unwrap();
        assert_eq!(mapping_of(&block, MAPPER), Some(true));

        let mountinfo = "\
22 1 0:21 /@root / rw,noatime shared:1 - btrfs /dev/mapper/cryptroot rw
41 22 0:21 /@private /var/lib/ronoctua-private rw shared:9 - btrfs /dev/mapper/cryptroot rw
88 22 254:4 / /private rw,nosuid,nodev,noatime shared:40 - btrfs /dev/mapper/ronoctua-private rw,discard=async
90 22 0:50 / /my\\040files rw - tmpfs tmpfs rw
";
        assert_eq!(
            mounted_in(mountinfo, Path::new("/private")),
            Some(("btrfs".into(), "/dev/mapper/ronoctua-private".into()))
        );
        assert_eq!(
            mounted_in(mountinfo, Path::new("/my files")).map(|m| m.0),
            Some("tmpfs".into())
        );
        assert_eq!(mounted_in(mountinfo, Path::new("/priv")), None);

        assert!(fido2_in(
            r#"{"keyslots":{"0":{"type":"luks2"},"1":{"type":"luks2"}},"tokens":{"0":{"type":"systemd-fido2","keyslots":["1"]}}}"#
        ));
        assert!(!fido2_in(
            r#"{"keyslots":{"0":{"type":"luks2"}},"tokens":{}}"#
        ));
        assert!(!fido2_in("not json"));
        assert_eq!(cryptsetup_why(Some(2)), Why::WrongPassphrase);
        assert_eq!(cryptsetup_why(Some(5)), Why::Busy);
        assert_eq!(cryptsetup_why(Some(1)), Why::Failed);
        assert_eq!(cryptsetup_why(None), Why::Failed);
    }

    /// FAILS IF: the vault stops being LUKS2, AES-XTS 512 and argon2id with
    /// the passphrase on stdin, the loop loses direct I/O, discards stop
    /// reaching the image, or a secret ever lands in an argv.
    #[test]
    fn every_step_says_what_the_design_says() {
        let dev = Path::new("/dev/loop7");
        let format = format_args(dev).join(" ");
        for wanted in [
            "luksFormat",
            "--type luks2",
            "--cipher aes-xts-plain64",
            "--key-size 512",
            "--pbkdf argon2id",
            "--key-file=-",
        ] {
            assert!(format.contains(wanted), "{wanted}: {format}");
        }
        assert!(attach_args(Path::new("/i")).contains(&"--direct-io=on".to_owned()));
        let open = open_args(dev, true);
        assert!(
            open.contains(&"--allow-discards".to_owned())
                && open.contains(&"--persistent".to_owned())
        );
        assert!(!open_args(dev, false).contains(&"--persistent".to_owned()));
        assert!(
            mount_args(Path::new("/dev/mapper/x"), Path::new("/private"))
                .contains(&MOUNT_OPTIONS.to_owned())
        );
        assert!(MOUNT_OPTIONS.contains("discard=async"));
        assert!(FIDO2_OPTIONS.contains("headless=true") && FIDO2_OPTIONS.contains("discard"));
        // THE WAIT A DOOR HOLDS "touch your key" FOR is the unit's own.
        assert!(
            FIDO2_OPTIONS
                .split(',')
                .any(|o| o == format!("token-timeout={}s", crate::TOUCH_WAIT.as_secs())),
            "{FIDO2_OPTIONS}"
        );
        let enroll = enroll_args(dev, Path::new("/run/ronoctua-private-root/unlock")).join(" ");
        assert!(enroll.contains("--fido2-with-client-pin=no"), "{enroll}");
        assert!(enroll.contains("--unlock-key-file=/run/ronoctua-private-root/unlock"));
    }

    /// The fake tools of a machine test: each logs `<tool> <args>` and what
    /// it read on stdin into `log`, and answers as `answers` says.
    fn fake_tools(s: &Scratch, log: &Path, mapper_dir: &Path, sys_block: &Path) -> Tools {
        let logged = |name: &str| {
            format!(
                "printf '%s %s\\n' {name} \"$*\" >> {log}\nif [ ! -t 0 ]; then printf 'stdin %s\\n' \"$(cat)\" >> {log}; fi",
                log = log.display()
            )
        };
        let crypt = s.program(
            "cryptsetup",
            &format!(
                "{}\ncase \"$1\" in\n  open) [ \"$2\" = --test-passphrase ] && exit 0; mkdir -p {block}/dm-9/dm; echo ronoctua-private > {block}/dm-9/dm/name; echo 0 > {block}/dm-9/dm/suspended; touch {mapper}/ronoctua-private ;;\n  close) rm -rf {block}/dm-9 ;;\n  luksDump) echo '{{\"tokens\":{{}}}}' ;;\nesac\nexit 0",
                logged("cryptsetup"),
                block = sys_block.display(),
                mapper = mapper_dir.display()
            ),
        );
        let losetup = s.program(
            "losetup",
            &format!(
                "{}\ncase \"$1\" in\n  --find) mkdir -p {block}/loop5/loop; echo \"$4\" > {block}/loop5/loop/backing_file; echo /dev/loop5 ;;\n  --detach) rm -rf {block}/loop5 ;;\nesac\nexit 0",
                logged("losetup"),
                block = sys_block.display()
            ),
        );
        let ok = |name: &str| s.program(name, &format!("{}\nexit 0", logged(name)));
        Tools::of(vec![
            (Tool::Cryptsetup, crypt),
            (Tool::Losetup, losetup),
            (Tool::Cryptenroll, ok("cryptenroll")),
            (Tool::Attach, ok("attach")),
            (Tool::MkfsBtrfs, ok("mkfs")),
            (Tool::Btrfs, ok("btrfs")),
            (Tool::Mount, ok("mount")),
            (Tool::Umount, ok("umount")),
        ])
    }

    /// FAILS IF: opening and closing stop walking loop → mapping → mount and
    /// back, a passphrase stops reaching cryptsetup on its stdin (and only
    /// there), the status stops saying where the vault is, or the verdict
    /// stops naming the operation.
    #[test]
    fn opening_and_closing_walk_the_layers_both_ways() {
        if me() == 0 {
            return;
        }
        let s = Scratch::new("walk");
        let places = s.places();
        let log = s.0.join("log");
        let tools = fake_tools(&s, &log, &places.dev_mapper, &places.sys_block);
        let machine = Machine::of(tools, places.clone());
        std::fs::write(&places.mountinfo, "").unwrap();
        // NO IMAGE: nothing to open.
        let dir = staged_folder(&s);
        crate::stage(&dir, &Request::of(Op::Open)).unwrap();
        crate::stage_secret(&dir, Slot::Passphrase, b"correct horse").unwrap();
        assert_eq!(machine.run_staged(&dir), Err(Refusal::of(Why::Absent)));
        assert_eq!(
            crate::verdict(&places.verdict).map(|v| (v.op, v.why)),
            Some(("open".into(), Some(Why::Absent)))
        );
        assert_eq!(crate::status(&places.status), Ok(Some(Status::absent())));
        // AN IMAGE: opened with the passphrase on stdin.
        std::fs::File::create(places.image())
            .unwrap()
            .set_len(1 << 20)
            .unwrap();
        crate::stage(&dir, &Request::of(Op::Open)).unwrap();
        crate::stage_secret(&dir, Slot::Passphrase, b"correct horse").unwrap();
        machine.run_staged(&dir).expect("opened");
        let said = std::fs::read_to_string(&log).unwrap();
        assert!(
            said.contains("losetup --find --show --direct-io=on"),
            "{said}"
        );
        assert!(said.contains("cryptsetup open --type luks2 --allow-discards --key-file=- /dev/loop5 ronoctua-private"), "{said}");
        assert!(said.contains("stdin correct horse"), "{said}");
        assert!(said.contains("mount -t btrfs -o discard=async"), "{said}");
        assert_eq!(
            said.matches("correct horse").count(),
            1,
            "the passphrase went somewhere else too: {said}"
        );
        let status = crate::status(&places.status).unwrap().unwrap();
        assert_eq!(status.state, State::Open);
        assert_eq!(status.cap_bytes, 1 << 20);
        assert_eq!(crate::verdict(&places.verdict).and_then(|v| v.why), None);
        // CLOSED: unmounted, unmapped, detached.
        std::fs::write(
            &places.mountinfo,
            format!(
                "88 22 254:4 / {} rw - btrfs /dev/mapper/ronoctua-private rw\n",
                places.mount_point.display()
            ),
        )
        .unwrap();
        std::fs::write(&log, "").unwrap();
        crate::stage(&dir, &Request::of(Op::Close)).unwrap();
        machine.run_staged(&dir).expect("closed");
        let said = std::fs::read_to_string(&log).unwrap();
        let umount = said.find("umount").expect("unmounted");
        let close = said.find("cryptsetup close").expect("unmapped");
        let detach = said.find("losetup --detach /dev/loop5").expect("detached");
        assert!(umount < close && close < detach, "{said}");
        std::fs::write(&places.mountinfo, "").unwrap();
        assert_eq!(machine.status().state, State::Locked);
    }

    /// FAILS IF: the sleep hook stops suspending a vault something holds —
    /// and saying so in the status BEFORE it suspends — or stops closing one
    /// nothing holds.
    #[test]
    fn sleep_closes_or_suspends_what_is_held() {
        if me() == 0 {
            return;
        }
        let s = Scratch::new("sleep");
        let places = s.places();
        let log = s.0.join("log");
        let mut tools = fake_tools(&s, &log, &places.dev_mapper, &places.sys_block);
        // `umount` says it is busy, and `luksSuspend` records the status it
        // found at that moment.
        let busy = s.program(
            "umount-busy",
            "echo 'umount: /private: target is busy.' >&2\nexit 32",
        );
        let status_file = places.status.clone();
        let crypt_busy = s.program(
            "cryptsetup-busy",
            &format!(
                "case \"$1\" in\n  luksSuspend) cp {status} {seen} ;;\n  luksDump) echo '{{}}' ;;\nesac\nexit 0",
                status = status_file.display(),
                seen = s.0.join("seen-at-suspend").display()
            ),
        );
        tools
            .paths
            .retain(|(t, _)| !matches!(t, Tool::Umount | Tool::Cryptsetup));
        tools.paths.push((Tool::Umount, busy));
        tools.paths.push((Tool::Cryptsetup, crypt_busy));
        let machine = Machine::of(tools, places.clone());
        std::fs::File::create(places.image())
            .unwrap()
            .set_len(1 << 20)
            .unwrap();
        std::fs::create_dir_all(places.sys_block.join("dm-9/dm")).unwrap();
        std::fs::write(places.sys_block.join("dm-9/dm/name"), "ronoctua-private\n").unwrap();
        std::fs::write(places.sys_block.join("dm-9/dm/suspended"), "0\n").unwrap();
        std::fs::write(
            &places.mountinfo,
            format!(
                "88 22 254:4 / {} rw - btrfs /dev/mapper/ronoctua-private rw\n",
                places.mount_point.display()
            ),
        )
        .unwrap();
        assert_eq!(machine.lock(), Ok(State::SuspendedInUse));
        let seen: Status =
            serde_json::from_slice(&std::fs::read(s.0.join("seen-at-suspend")).unwrap()).unwrap();
        assert_eq!(
            seen.state,
            State::SuspendedInUse,
            "said before it suspended"
        );
        // A MANUAL CLOSE of a held vault is a refusal, nothing taken away.
        let dir = staged_folder(&s);
        std::fs::write(places.sys_block.join("dm-9/dm/suspended"), "0\n").unwrap();
        crate::stage(&dir, &Request::of(Op::Close)).unwrap();
        assert_eq!(machine.run_staged(&dir), Err(Refusal::of(Why::Busy)));
    }

    /// FAILS IF: a cap the image already has is taken as a grow, or a grow
    /// stops making the image longer while it stays sparse.
    #[test]
    fn growing_lengthens_the_sparse_image() {
        if me() == 0 {
            return;
        }
        let s = Scratch::new("grow");
        let places = s.places();
        let log = s.0.join("log");
        let tools = fake_tools(&s, &log, &places.dev_mapper, &places.sys_block);
        let machine = Machine::of(tools, places.clone());
        std::fs::File::create(places.image())
            .unwrap()
            .set_len(crate::MIN_CAP_BYTES)
            .unwrap();
        assert_eq!(
            machine.grow(crate::MIN_CAP_BYTES, None),
            Err(Refusal::of(Why::Invalid))
        );
        let room = machine.room().map_or(0, |r| r.0);
        if room > 2 * crate::MIN_CAP_BYTES {
            machine.grow(2 * crate::MIN_CAP_BYTES, None).expect("grown");
            let meta = std::fs::metadata(places.image()).unwrap();
            assert_eq!(meta.len(), 2 * crate::MIN_CAP_BYTES);
            assert!(meta.blocks() * 512 < crate::MIN_CAP_BYTES, "not sparse");
        }
        assert_eq!(
            machine.grow(crate::MOST_CAP_BYTES, None),
            Err(Refusal::of(Why::NoRoom))
        );
    }
}
