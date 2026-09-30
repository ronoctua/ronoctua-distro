//! The player's cache: one directory, a ceiling in bytes, and the least
//! recently READ file leaves first.
//!
//! # Why its own ceiling
//!
//! The sweep of `$XDG_CACHE_HOME` takes whole entries of the ROOT, each as old
//! as its youngest file (`assets/cache.nix`) — and `~/.cache/ronoctua` is
//! touched every day, so nothing under it is ever swept. A directory that
//! grew by a radio list a day would grow for ever. So this one keeps itself
//! under a ceiling the caller passes (the number is `assets/cache.nix`'s),
//! and drops what was read longest ago after every write.
//!
//! # Read time, written down on purpose
//!
//! A machine mounted `noatime` never updates a file's access time by reading
//! it. [`Cache::read`] SETS it, explicitly, so "least recently read" means
//! what it says on any mount.
//!
//! # One writer at a time
//!
//! Writes take an exclusive lock on the directory's `.lock` file, write a
//! scratch (`.<name>.<pid>.new`) and rename it over the name — so a reader
//! sees the old file or the new one, never half — then trim to the ceiling,
//! still under the lock. A scratch found under the lock is a writer's that
//! died, and is removed. [`Cache::hold`] gives a caller the same lock around
//! a whole refresh, so two processes never download the same index at once.

use std::fs::{self, File, FileTimes};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Why the cache did not do what was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trouble {
    /// There is no home to keep a cache in.
    NoHome,
    /// The name is not one this cache keeps (see [`is_name`]).
    NotAName,
    /// The bytes alone are more than the ceiling.
    TooLarge,
    /// The directory or the file could not be written.
    Unwritable,
    /// The file is there and could not be read.
    Unreadable,
}

/// A name this cache keeps a file under: letters, digits, `.`, `_` and `-`,
/// at most 128, not starting with `.` (those are the lock and the scratches).
#[must_use]
pub fn is_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// What a read found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The bytes.
    pub bytes: Vec<u8>,
    /// When they were written.
    pub written: SystemTime,
    /// Whether they are younger than the freshness asked for.
    pub fresh: bool,
}

/// The lock of a cache directory, held until this is dropped.
#[derive(Debug)]
pub struct Held {
    _file: File,
}

/// One cache directory and its ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    dir: PathBuf,
    most_bytes: u64,
}

impl Cache {
    /// A cache in `dir`, kept under `most_bytes`.
    #[must_use]
    pub const fn at(dir: PathBuf, most_bytes: u64) -> Self {
        Self { dir, most_bytes }
    }

    /// The player's: `player` under this instance's cache root.
    ///
    /// # Errors
    ///
    /// [`Trouble::NoHome`] with no cache root.
    pub fn player(most_bytes: u64) -> Result<Self, Trouble> {
        let root = ronoctua_xdg::cache_dir().ok_or(Trouble::NoHome)?;
        Ok(Self::at(root.join("player"), most_bytes))
    }

    /// Where it is.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Its ceiling.
    #[must_use]
    pub const fn most_bytes(&self) -> u64 {
        self.most_bytes
    }

    /// The file under `name`, when there is one, and whether it was written
    /// less than `fresh_for` before `now`. Marks it read at `now`.
    ///
    /// A file written "after" `now` (a clock that moved back) is not fresh:
    /// its age is not known.
    ///
    /// # Errors
    ///
    /// [`Trouble::NotAName`], or [`Trouble::Unreadable`] for a file that is
    /// there and cannot be read.
    pub fn read(
        &self,
        name: &str,
        fresh_for: Duration,
        now: SystemTime,
    ) -> Result<Option<Kept>, Trouble> {
        if !is_name(name) {
            return Err(Trouble::NotAName);
        }
        let path = self.dir.join(name);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Trouble::Unreadable),
        };
        let written = fs::metadata(&path)
            .and_then(|m| m.modified())
            .map_err(|_| Trouble::Unreadable)?;
        // READ NOW, written down: see the module's header. A file removed
        // between the read and this is simply not marked.
        if let Ok(file) = File::options().write(true).open(&path) {
            let _ = file.set_times(FileTimes::new().set_accessed(now));
        }
        let fresh = now.duration_since(written).is_ok_and(|age| age < fresh_for);
        Ok(Some(Kept {
            bytes,
            written,
            fresh,
        }))
    }

    /// The directory's lock, held until the answer is dropped. Blocks while
    /// another process holds it.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unwritable`] when the directory or its lock cannot be made.
    pub fn hold(&self) -> Result<Held, Trouble> {
        fs::create_dir_all(&self.dir).map_err(|_| Trouble::Unwritable)?;
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join(".lock"))
            .map_err(|_| Trouble::Unwritable)?;
        file.lock().map_err(|_| Trouble::Unwritable)?;
        Ok(Held { _file: file })
    }

    /// Keeps `bytes` under `name`, whole or not at all, then trims the
    /// directory to its ceiling — never taking this file.
    ///
    /// Takes the lock, unless the caller holds it already (`held`).
    ///
    /// # Errors
    ///
    /// [`Trouble::NotAName`], [`Trouble::TooLarge`] when the bytes alone are
    /// over the ceiling (nothing is written), [`Trouble::Unwritable`].
    pub fn write(&self, name: &str, bytes: &[u8], held: Option<&Held>) -> Result<(), Trouble> {
        if !is_name(name) {
            return Err(Trouble::NotAName);
        }
        if bytes.len() as u64 > self.most_bytes {
            return Err(Trouble::TooLarge);
        }
        let _mine = match held {
            Some(_) => None,
            None => Some(self.hold()?),
        };
        let scratch = self.dir.join(format!(".{name}.{}.new", std::process::id()));
        let written = (|| -> std::io::Result<()> {
            let mut f = File::create(&scratch)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            fs::rename(&scratch, self.dir.join(name))
        })();
        if written.is_err() {
            let _ = fs::remove_file(&scratch);
            return Err(Trouble::Unwritable);
        }
        self.trim(Some(name));
        Ok(())
    }

    /// Removes the file under `name`, when there is one.
    ///
    /// # Errors
    ///
    /// [`Trouble::NotAName`], or [`Trouble::Unwritable`] when it is there
    /// and cannot be removed.
    pub fn forget(&self, name: &str) -> Result<(), Trouble> {
        if !is_name(name) {
            return Err(Trouble::NotAName);
        }
        match fs::remove_file(self.dir.join(name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Trouble::Unwritable),
            _ => Ok(()),
        }
    }

    /// How many bytes the kept files take.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.files().iter().map(|f| f.bytes).sum()
    }

    /// Every kept file: its name, size and when it was last read.
    fn files(&self) -> Vec<Entry> {
        let Ok(dir) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        dir.filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_owned();
                let meta = e.metadata().ok()?;
                (is_name(&name) && meta.is_file()).then(|| Entry {
                    read: meta
                        .accessed()
                        .or_else(|_| meta.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH),
                    bytes: meta.len(),
                    name,
                })
            })
            .collect()
    }

    /// Drops the least recently read files until the directory is under its
    /// ceiling, never `keep`; and the scratches a dead writer left. Call it
    /// under the lock. Answers how many bytes were freed.
    fn trim(&self, keep: Option<&str>) -> u64 {
        if let Ok(dir) = fs::read_dir(&self.dir) {
            for e in dir.filter_map(Result::ok) {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') && name.ends_with(".new") {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
        let mut files = self.files();
        let mut total: u64 = files.iter().map(|f| f.bytes).sum();
        files.sort_by(|a, b| a.read.cmp(&b.read).then_with(|| a.name.cmp(&b.name)));
        let mut freed = 0;
        for f in files {
            if total <= self.most_bytes {
                break;
            }
            if Some(f.name.as_str()) == keep {
                continue;
            }
            if fs::remove_file(self.dir.join(&f.name)).is_ok() {
                total = total.saturating_sub(f.bytes);
                freed += f.bytes;
            }
        }
        freed
    }
}

/// A kept file, as the trim sees it.
struct Entry {
    name: String,
    bytes: u64,
    read: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-broadcast-cache-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000 + secs)
    }

    #[test]
    fn a_written_file_is_read_back_fresh_then_stale() {
        let dir = scratch("fresh");
        let cache = Cache::at(dir.clone(), 1 << 20);
        cache.write("radio-countries.json", b"[1]", None).unwrap();
        let now = SystemTime::now();
        let kept = cache
            .read("radio-countries.json", Duration::from_secs(60), now)
            .unwrap()
            .unwrap();
        assert_eq!(kept.bytes, b"[1]");
        assert!(kept.fresh);
        let later = now + Duration::from_secs(3600);
        assert!(
            !cache
                .read("radio-countries.json", Duration::from_secs(60), later)
                .unwrap()
                .unwrap()
                .fresh,
            "an hour later, a minute's freshness is gone"
        );
        let before = now - Duration::from_secs(3600);
        assert!(
            !cache
                .read("radio-countries.json", Duration::from_secs(60), before)
                .unwrap()
                .unwrap()
                .fresh,
            "written in the future: its age is not known"
        );
        assert_eq!(cache.read("missing.json", Duration::ZERO, now), Ok(None));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_cache_stays_under_its_ceiling_and_drops_the_least_read() {
        let dir = scratch("ceiling");
        let cache = Cache::at(dir.clone(), 30);
        cache.write("a.json", &[b'a'; 10], None).unwrap();
        cache.write("b.json", &[b'b'; 10], None).unwrap();
        cache.write("c.json", &[b'c'; 10], None).unwrap();
        // READ ORDER, set explicitly: `a` read last, `b` read first.
        cache.read("b.json", Duration::ZERO, at(1)).unwrap();
        cache.read("c.json", Duration::ZERO, at(2)).unwrap();
        cache.read("a.json", Duration::ZERO, at(3)).unwrap();
        cache.write("d.json", &[b'd'; 10], None).unwrap();
        assert!(cache.size() <= 30, "{}", cache.size());
        let left: Vec<bool> = ["a.json", "b.json", "c.json", "d.json"]
            .iter()
            .map(|n| dir.join(n).exists())
            .collect();
        assert_eq!(left, [true, false, true, true], "b was read longest ago");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_file_just_written_is_never_the_one_dropped() {
        let dir = scratch("keep");
        let cache = Cache::at(dir.clone(), 15);
        cache.write("old.json", &[1; 10], None).unwrap();
        // `old` READ TOMORROW: the file written next is the least recently
        // read one — and still the one that stays.
        let tomorrow = SystemTime::now() + Duration::from_secs(86_400);
        cache.read("old.json", Duration::ZERO, tomorrow).unwrap();
        cache.write("new.json", &[2; 10], None).unwrap();
        assert!(dir.join("new.json").exists());
        assert!(!dir.join("old.json").exists());
        assert_eq!(
            cache.write("huge.json", &[0; 16], None),
            Err(Trouble::TooLarge)
        );
        assert!(!dir.join("huge.json").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_dead_writers_scratch_is_removed_and_names_are_checked() {
        let dir = scratch("names");
        let cache = Cache::at(dir.clone(), 1 << 20);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".x.json.99999.new"), b"half").unwrap();
        cache.write("x.json", b"whole", None).unwrap();
        assert!(!dir.join(".x.json.99999.new").exists());
        for bad in ["", ".lock", "../x", "a/b", "x y"] {
            assert_eq!(
                cache.write(bad, b"x", None),
                Err(Trouble::NotAName),
                "{bad:?}"
            );
            assert_eq!(
                cache.read(bad, Duration::ZERO, SystemTime::now()),
                Err(Trouble::NotAName)
            );
        }
        assert!(is_name("archive-HisNewJob_1915.json"));
        cache.forget("x.json").unwrap();
        cache.forget("x.json").unwrap();
        assert_eq!(cache.size(), 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_held_lock_lets_the_holder_write() {
        let dir = scratch("held");
        let cache = Cache::at(dir.clone(), 1 << 20);
        let held = cache.hold().unwrap();
        cache.write("iptv-index.json", b"{}", Some(&held)).unwrap();
        drop(held);
        assert!(dir.join("iptv-index.json").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
