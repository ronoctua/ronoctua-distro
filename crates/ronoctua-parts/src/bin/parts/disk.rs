//! What the realizer reads and writes, and the programs it runs.
//!
//! Every file here is ROOT's, in root's folders: the records and the view
//! are written whole — a draft beside the file, synced, renamed over it, the
//! folder synced — so a reader (the shell, watching the folder) never sees
//! half a record, and a power cut leaves the old one or the new one.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ronoctua_parts::names;
use ronoctua_parts::plan::{self, Plan};
use ronoctua_parts::record::Record;
use ronoctua_person::Stamp;
use rustix::fs::FlockOperation;
use rustix::io::Errno;

/// Now, as the records write it.
pub fn now() -> Stamp {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Stamp::from_unix_s(secs)
}

/// The plan at `path`. A system with no plan file wants no part: said, and
/// read as empty.
pub fn read_plan(path: &Path) -> Result<Plan, String> {
    match fs::read(path) {
        Ok(bytes) => ronoctua_person::parse(&bytes).map_err(|r| format!("{}: {r}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            crate::say(
                4,
                &format!("{} is not there: no part is wanted", path.display()),
            );
            Ok(plan::empty())
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// One part's record.
pub fn read_record(id: &str) -> Result<Record, String> {
    let path = names::record(id);
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    ronoctua_person::parse(&bytes).map_err(|r| format!("{}: {r}", path.display()))
}

/// Every record there is; one that does not read is said and skipped.
pub fn read_records() -> Vec<Record> {
    let Ok(entries) = fs::read_dir(names::RECORDS) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".json")
                .map(str::to_owned)
        })
        .filter(|id| ronoctua_parts::id::is_id(id))
        .collect();
    ids.sort();
    ids.iter()
        .filter_map(|id| match read_record(id) {
            Ok(record) => Some(record),
            Err(e) => {
                crate::say(4, &format!("a record was skipped: {e}"));
                None
            }
        })
        .collect()
}

/// Writes `record` whole, `0644`: the broadcast the shell watches.
pub fn write_record(record: &Record) -> Result<(), String> {
    let dir = Path::new(names::RECORDS);
    make_dir(dir, 0o755)?;
    write_whole(
        dir,
        &format!("{}.json", record.id),
        ronoctua_person::to_json(record).as_bytes(),
        0o644,
    )
}

/// The folder at `path`, made with `mode` if it is not there.
pub fn make_dir(path: &Path, mode: u32) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes `bytes` as `name` in `dir`: a draft created with `mode` already
/// on, synced, renamed over the old file, and the folder synced.
pub fn write_whole(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> Result<(), String> {
    let draft = dir.join(format!(".{name}.{}", std::process::id()));
    let target = dir.join(name);
    let written = (|| -> std::io::Result<()> {
        let _ = fs::remove_file(&draft);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc_nofollow())
            .open(&draft)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&draft, &target)?;
        File::open(dir)?.sync_all()
    })();
    written.map_err(|e| {
        let _ = fs::remove_file(&draft);
        format!("{}: {e}", target.display())
    })
}

/// `O_NOFOLLOW`, so a draft name is never a link's.
fn libc_nofollow() -> i32 {
    i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).unwrap_or(0)
}

/// Root's lock of the records, held while this lives.
pub struct Lock {
    _file: File,
}

impl Lock {
    /// Takes [`names::LOCK`], asking again every 200 ms for at most `wait`.
    pub fn take(wait: Duration) -> Result<Self, String> {
        make_dir(Path::new(names::STATE_DIR), 0o755)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc_nofollow())
            .open(names::LOCK)
            .map_err(|e| format!("{}: {e}", names::LOCK))?;
        let deadline = Instant::now() + wait;
        loop {
            match rustix::fs::flock(&file, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => return Ok(Self { _file: file }),
                Err(Errno::WOULDBLOCK | Errno::INTR) => {}
                Err(e) => return Err(format!("{}: {e}", names::LOCK)),
            }
            let now = Instant::now();
            if now >= deadline {
                return Err("busy".to_owned());
            }
            std::thread::sleep((deadline - now).min(Duration::from_millis(200)));
        }
    }
}

/// How long a writer of the records waits for the lock: they hold it for a
/// read and a write, never across a build.
pub const LOCK_WAIT: Duration = Duration::from_secs(30);

/// Runs `argv` and says whether it succeeded; its output goes to the journal.
pub fn run_quiet(argv: &[String]) -> bool {
    let Some((program, args)) = argv.split_first() else {
        return false;
    };
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// What a finished program said.
pub struct Said {
    /// Whether it exited zero.
    pub ok: bool,
    /// Its standard output.
    pub out: String,
    /// The tail of its standard error, which also went to the journal.
    pub err: String,
}

/// Runs `argv`, keeping its standard output and the tail of its standard
/// error — and passing the error on to the journal as it comes, so a long
/// build is seen while it runs.
pub fn run(argv: &[String]) -> Said {
    let Some((program, args)) = argv.split_first() else {
        return Said {
            ok: false,
            out: String::new(),
            err: "no program".to_owned(),
        };
    };
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            return Said {
                ok: false,
                out: String::new(),
                err: format!("{program}: {e}"),
            };
        }
    };
    let err = child.stderr.take().map(|stream| {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let mut tail = String::new();
            for line in std::io::BufReader::new(stream)
                .lines()
                .map_while(Result::ok)
            {
                eprintln!("{line}");
                tail.push_str(&line);
                tail.push('\n');
                if tail.len() > 4 * ronoctua_parts::record::DETAIL_MOST {
                    let cut = tail.len() - 2 * ronoctua_parts::record::DETAIL_MOST;
                    let cut = (cut..tail.len())
                        .find(|&i| tail.is_char_boundary(i))
                        .unwrap_or(0);
                    tail.drain(..cut);
                }
            }
            tail
        })
    });
    let output = child.wait_with_output();
    let err = err.and_then(|t| t.join().ok()).unwrap_or_default();
    match output {
        Ok(output) => Said {
            ok: output.status.success(),
            out: String::from_utf8_lossy(&output.stdout).into_owned(),
            err,
        },
        Err(e) => Said {
            ok: false,
            out: String::new(),
            err: format!("{err}{program}: {e}"),
        },
    }
}
