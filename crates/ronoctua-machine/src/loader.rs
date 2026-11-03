//! The boot loader the trial is armed in: systemd-boot's own files on the
//! ESP on a UEFI machine, GRUB's environment block on a BIOS one
//! (`ronoctua.firmware`, `nix/modules/boot-loader.nix`).
//!
//! # One trial, two loaders
//!
//! The trial is two things the loader keeps across a reboot: a DEFAULT, the
//! last good entry, and a ONE-SHOT, the new entry, tried at the next boot and
//! spent by the loader itself. GRUB keeps `next_entry` in its environment
//! block, reads it once and clears it — the one-shot, as NixOS's `grub.cfg`
//! already handles it — and the default is a variable of ours,
//! `ronoctua_default`, read after NixOS's own choice and never on a boot of
//! once (`nix/modules/boot-loader.nix`'s `extraConfig`, which also marks that
//! boot on the kernel's command line for the trial's deadline). An empty entry
//! unsets either, and the menu's own first entry — the profile's newest
//! generation — is the default again.
//!
//! # systemd-boot: files, never EFI variables
//!
//! It used to be `LoaderEntryDefault` and `LoaderEntryOneShot`, written with
//! `bootctl set-default` and `set-oneshot`. Writing an EFI variable while the
//! machine runs is a call into the firmware, and a firmware may answer it
//! badly: on a ThinkPad Z16 Gen 2 every such call — a write or a removal —
//! resets the machine two to five seconds later (2026-11-03). So nothing here
//! writes one, and the machine mounts them read-only
//! (`nix/modules/boot-loader.nix`). The same trial lives in systemd-boot's
//! own files on the ESP, as its Automatic Boot Assessment keeps it:
//!
//! - the DEFAULT is `loader.conf`'s `default` line, the last good entry while
//!   a trial is armed and `nixos-*` — the newest entry that is not bad —
//!   otherwise, which is what NixOS's builder writes with boot counting on;
//! - the ONE-SHOT is `preferred` — the entry systemd-boot starts unless it is
//!   bad — on an entry whose file carries ONE try (`<id>+1.conf`). The boot
//!   that starts it spends the try (`+0-1`), and an entry with no tries left
//!   that nobody blessed is bad: the boot after it takes the default. The
//!   bless takes the counter off the file (`<id>.conf`, [`Loader::bless`]),
//!   as `systemd-bless-boot` would — the machine's own judgement, made after
//!   the default target and a while (`ronoctua-boot-bless.service`), so that
//!   unit is masked.
//!
//! # A trial the loader cannot start
//!
//! Neither the deadline nor the bless sees a kernel that never ran. GRUB
//! falls back to the last good entry in the same boot (its `fallback`,
//! `nix/modules/boot-loader.nix`). systemd-boot restarts when an entry it
//! starts fails if that entry is counted (`reboot-on-error`'s default,
//! `auto`): the trial's try is spent, and the restart takes the default.

use std::path::{Path, PathBuf};

use std::io::Write as _;

use crate::boot::{self, Entry};
use crate::{System, argv, run, say, system};

/// GRUB's variable for the trial's default.
pub const GRUB_DEFAULT: &str = "ronoctua_default";

/// GRUB's variable for the boot of once, which its `grub.cfg` clears.
pub const GRUB_ONESHOT: &str = "next_entry";

/// systemd-boot's configuration, on the ESP — `/boot` (`nix/disko`).
pub const LOADER_CONF: &str = "/boot/loader/loader.conf";

/// What `default` says when no trial is armed: the newest entry that is not
/// bad, as NixOS's builder writes it with boot counting on.
pub const DEFAULT_ANY: &str = "nixos-*";

/// Where a unified kernel image is an entry of its own (type 2), on the ESP:
/// lanzaboote's, on a host with Secure Boot.
pub const UKIS: &str = "/boot/EFI/Linux";

/// The loader, and the tool that writes what it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Loader {
    /// systemd-boot: its menu read through `bootctl`, its trial written in
    /// its own files.
    Systemd {
        /// `bootctl`, which only lists the menu here.
        bootctl: PathBuf,
        /// Its `loader.conf`, on the disk.
        conf: PathBuf,
        /// The folder of its entries, on the disk ([`boot::ENTRIES`]).
        entries: Folders,
    },
    /// GRUB, through `grub-editenv` on the block beside its menu.
    Grub {
        /// `grub-editenv`.
        editenv: PathBuf,
        /// The folder of `grub.cfg` and `grubenv`, on the disk.
        dir: PathBuf,
    },
}

impl Loader {
    /// The loader the system names a tool for; `None` when it names none.
    pub(crate) fn of(system: &System) -> Option<Self> {
        if let Some(bootctl) = &system.bootctl {
            return Some(Self::Systemd {
                bootctl: bootctl.clone(),
                conf: system.real(LOADER_CONF),
                entries: Folders {
                    files: system.real(boot::ENTRIES),
                    ukis: system.real(UKIS),
                },
            });
        }
        system.grub_editenv.as_ref().map(|editenv| Self::Grub {
            editenv: editenv.clone(),
            dir: system.real(system::GRUB_DIR),
        })
    }

    /// What the loader keeps the trial in, for the sentences that say it.
    pub(crate) const fn variables(&self) -> &'static str {
        match self {
            Self::Systemd { .. } => "systemd-boot's loader.conf and entries",
            Self::Grub { .. } => "GRUB's environment block",
        }
    }

    /// The menu's entries; `None`, said, when they cannot be read.
    pub(crate) fn entries(&self) -> Option<Vec<Entry>> {
        match self {
            Self::Systemd { bootctl, .. } => {
                let ran = run::capture(bootctl, &argv::list_entries()).ok()?;
                if !ran.ok() {
                    say(
                        4,
                        &format!("bootctl lists no entries: {}", ran.stderr.trim()),
                    );
                    return None;
                }
                let listed = boot::entries(&ran.stdout);
                if listed.is_none() {
                    say(4, "bootctl's list is not the JSON it should be");
                }
                listed
            }
            Self::Grub { dir, .. } => {
                let cfg = dir.join("grub.cfg");
                let Ok(text) = std::fs::read_to_string(&cfg) else {
                    say(4, &format!("GRUB's menu cannot be read: {}", cfg.display()));
                    return None;
                };
                let listed = boot::grub_entries(&text);
                if listed.is_none() {
                    say(4, &format!("GRUB's menu lists no entry: {}", cfg.display()));
                }
                listed
            }
        }
    }

    /// The default the next boots fall back to; an empty entry unsets it
    /// (`default nixos-*` on systemd-boot, [`DEFAULT_ANY`]).
    pub(crate) fn set_default(&self, entry: &str) -> bool {
        match self {
            Self::Systemd { conf, .. } => {
                let value = if entry.is_empty() { DEFAULT_ANY } else { entry };
                edit_conf(conf, "default", Some(value))
            }
            Self::Grub { editenv, dir } => grub_set(editenv, dir, GRUB_DEFAULT, entry),
        }
    }

    /// The entry the next boot tries once; an empty entry unsets it. On
    /// systemd-boot, `entry` is `preferred` and its file carries one try,
    /// given again when it carried none or had spent it.
    pub(crate) fn set_oneshot(&self, entry: &str) -> bool {
        match self {
            Self::Systemd { conf, entries, .. } => {
                if entry.is_empty() {
                    return edit_conf(conf, "preferred", None);
                }
                let Some(file) = EntryFile::of(entries, entry) else {
                    say(4, &format!("{entry} has no file on the ESP"));
                    return false;
                };
                if !file.untried() && !file.rename_to(&file.counted(1)) {
                    return false;
                }
                edit_conf(conf, "preferred", Some(entry))
            }
            Self::Grub { editenv, dir } => grub_set(editenv, dir, GRUB_ONESHOT, entry),
        }
    }

    /// The default and the one-shot unset, so the menu's own choice is the
    /// loader's. On systemd-boot the one-shot is a `preferred` entry whose
    /// try is unspent; a `preferred` on a good entry is the builder's — the
    /// profile's own generation — and stays.
    pub(crate) fn clear(&self) {
        if !self.oneshot().unwrap_or_default().is_empty() {
            self.set_oneshot("");
        }
        self.set_default("");
    }

    /// `entry` is good: on systemd-boot its file loses the boot counter, as
    /// `systemd-bless-boot` would take it off — an entry that spent its try
    /// and was not blessed is bad, and the boot after it would start the
    /// default. Whether it is good now; GRUB counts nothing.
    pub(crate) fn bless(&self, entry: &str) -> bool {
        match self {
            Self::Systemd { entries, .. } => match EntryFile::of(entries, entry) {
                Some(file) if file.counter.is_none() => true,
                Some(file) => {
                    let blessed = file.rename_to(&file.uncounted());
                    if blessed {
                        say(6, &format!("{entry} is blessed: its boot counter is off"));
                    }
                    blessed
                }
                None => {
                    say(
                        4,
                        &format!("{entry} has no file on the ESP: nothing to bless"),
                    );
                    false
                }
            },
            Self::Grub { .. } => true,
        }
    }

    /// The entry the next boot tries once, as the loader keeps it now: an
    /// empty one when none is set, `None` when it cannot be read. Both
    /// loaders spend it on the boot that starts it, so a one-shot still set
    /// is a boot nobody has tried yet — on systemd-boot, `preferred` while
    /// its file still has its try.
    pub(crate) fn oneshot(&self) -> Option<String> {
        match self {
            Self::Systemd { conf, entries, .. } => {
                let text = std::fs::read_to_string(conf).ok()?;
                let Some(entry) = conf_value(&text, "preferred") else {
                    return Some(String::new());
                };
                let untried = EntryFile::of(entries, entry).is_some_and(|f| f.untried());
                Some(if untried {
                    entry.to_owned()
                } else {
                    String::new()
                })
            }
            Self::Grub { editenv, dir } => {
                let block = dir.join("grubenv");
                if !block.exists() {
                    return Some(String::new());
                }
                let ran = run::capture(editenv, &argv::grub_list(&block)).ok()?;
                if !ran.ok() {
                    return None;
                }
                Some(
                    ran.stdout
                        .lines()
                        .find_map(|l| l.strip_prefix(GRUB_ONESHOT)?.strip_prefix('='))
                        .unwrap_or_default()
                        .to_owned(),
                )
            }
        }
    }
}

/// A boot counter, as systemd-boot writes it into an entry's file name:
/// `+<left>` or `+<left>-<done>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counter {
    left: u32,
    done: u32,
}

/// The two folders systemd-boot finds its entries in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Folders {
    /// Entry files, `<id>.conf` ([`boot::ENTRIES`]).
    pub(crate) files: PathBuf,
    /// Unified kernel images, `<id>.efi` ([`UKIS`]).
    pub(crate) ukis: PathBuf,
}

/// An entry's file, found by its id — `nixos-….conf`, or a UKI's
/// `nixos-….efi`: the name without the counter.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryFile {
    path: PathBuf,
    stem: String,
    suffix: &'static str,
    counter: Option<Counter>,
}

impl EntryFile {
    /// The file of `id`; `None` when there is none, or more than one.
    fn of(folders: &Folders, id: &str) -> Option<Self> {
        let (stem, suffix, dir) = if let Some(stem) = id.strip_suffix(".conf") {
            (stem, ".conf", &folders.files)
        } else {
            (id.strip_suffix(".efi")?, ".efi", &folders.ukis)
        };
        let mut found = std::fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_owned();
                let (file_stem, counter) = split_counter(name.strip_suffix(suffix)?)?;
                (file_stem == stem).then(|| Self {
                    path: e.path(),
                    stem: stem.to_owned(),
                    suffix,
                    counter,
                })
            });
        let one = found.next()?;
        found.next().is_none().then_some(one)
    }

    /// Whether it carries tries and has spent none.
    fn untried(&self) -> bool {
        self.counter.is_some_and(|c| c.left > 0 && c.done == 0)
    }

    /// Its file name with `left` fresh tries.
    fn counted(&self, left: u32) -> String {
        format!("{}+{left}{}", self.stem, self.suffix)
    }

    /// Its file name with no counter: a good entry.
    fn uncounted(&self) -> String {
        format!("{}{}", self.stem, self.suffix)
    }

    /// Renamed to `name` beside it, the folder flushed; whether it was.
    fn rename_to(&self, name: &str) -> bool {
        let Some(folder) = self.path.parent() else {
            return false;
        };
        let to = folder.join(name);
        if to == self.path {
            return true;
        }
        match std::fs::rename(&self.path, &to) {
            Ok(()) => {
                let _ = std::fs::File::open(folder).and_then(|f| f.sync_all());
                true
            }
            Err(e) => {
                say(
                    4,
                    &format!("{} could not become {name}: {e}", self.path.display()),
                );
                false
            }
        }
    }
}

/// A file stem split into the entry's own and its counter: `a+1-2` is `a`
/// with one left and two done; `a` has none; `None` for a counter that is not
/// one.
fn split_counter(stem: &str) -> Option<(&str, Option<Counter>)> {
    let Some((own, counter)) = stem.rsplit_once('+') else {
        return Some((stem, None));
    };
    let (left, done) = counter.split_once('-').unwrap_or((counter, "0"));
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok())?
    };
    Some((
        own,
        Some(Counter {
            left: number(left)?,
            done: number(done)?,
        }),
    ))
}

/// The value of `key`'s line in a `loader.conf`'s text.
fn conf_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|l| {
        let (k, v) = l.trim().split_once(char::is_whitespace)?;
        (k == key).then(|| v.trim())
    })
}

/// `text` with every `key` line replaced by `key value` — appended when there
/// is none — or taken out (`None`); every other line as it was.
fn with_line(text: &str, key: &str, value: Option<&str>) -> String {
    let is_key = |l: &str| {
        l.trim()
            .split_once(char::is_whitespace)
            .map_or(l.trim(), |(k, _)| k)
            == key
    };
    let mut out = String::new();
    let mut put = false;
    for l in text.lines() {
        if !is_key(l) {
            out.push_str(l);
            out.push('\n');
        } else if let (Some(v), false) = (value, put) {
            out.push_str(&format!("{key} {v}\n"));
            put = true;
        }
    }
    if let (Some(v), false) = (value, put) {
        out.push_str(&format!("{key} {v}\n"));
    }
    out
}

/// `conf`'s `key` line set (or taken out) on the disk; whether it is so now.
/// Not there or not readable, it is said and left: a file written from
/// nothing would turn systemd-boot's own defaults on — its editor among them.
fn edit_conf(conf: &Path, key: &str, value: Option<&str>) -> bool {
    let Ok(text) = std::fs::read_to_string(conf) else {
        say(4, &format!("{} cannot be read", conf.display()));
        return false;
    };
    let kept = with_line(&text, key, value);
    if kept == text {
        return true;
    }
    match write_on_fat(conf, kept.as_bytes()) {
        Ok(()) => true,
        Err(e) => {
            say(4, &format!("{}: {e}", conf.display()));
            false
        }
    }
}

/// `bytes` into `path` on the ESP: a sibling written whole and flushed,
/// renamed over it, and the folder flushed — the FAT keeps a rename in its
/// cache, and NixOS's builder syncs after every write for that. No mode:
/// the FAT's is its mount's, and setting one is refused.
fn write_on_fat(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let folder = path
        .parent()
        .ok_or_else(|| std::io::Error::other("not a file's path"))?;
    let draft = folder.join(format!(".ronoctua-{}.new", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&draft)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&draft, path)?;
        // The rename, flushed where the file system flushes a folder; the
        // file is whole either way.
        let _ = std::fs::File::open(folder).and_then(|f| f.sync_all());
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// `name` in GRUB's environment block — set to `value`, or unset when it is
/// empty —, the block made first when there is none (NixOS's `grub.cfg` reads
/// it only when it exists).
fn grub_set(editenv: &Path, dir: &Path, name: &str, value: &str) -> bool {
    let block = dir.join("grubenv");
    let word = |words: &[String]| match run::capture(editenv, words) {
        Ok(ran) if ran.ok() => true,
        Ok(ran) => {
            say(
                4,
                &format!("grub-editenv {}: {}", words.join(" "), ran.stderr.trim()),
            );
            false
        }
        Err(_) => false,
    };
    if !block.exists() && !word(&argv::grub_create(&block)) {
        return false;
    }
    word(&argv::grub_set(&block, name, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Scratch;

    const BUILT: &str =
        "timeout 5\npreferred nixos-b.conf\ndefault nixos-*\neditor 0\nconsole-mode keep\n";

    fn loader(s: &Scratch) -> Loader {
        let entries = s.path("boot/loader/entries");
        std::fs::create_dir_all(&entries).unwrap();
        std::fs::write(s.path("boot/loader/loader.conf"), BUILT).unwrap();
        Loader::Systemd {
            bootctl: s.path("bin/bootctl"),
            conf: s.path("boot/loader/loader.conf"),
            entries: Folders {
                files: entries,
                ukis: s.path("boot/EFI/Linux"),
            },
        }
    }

    fn names(s: &Scratch) -> Vec<String> {
        let mut n: Vec<String> = std::fs::read_dir(s.path("boot/loader/entries"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        n.sort();
        n
    }

    /// FAILS IF: a counter is not read as systemd-boot writes it into a file
    /// name — `+left` or `+left-done` — or something that is no counter is
    /// read as one.
    #[test]
    fn a_counter_is_read_off_the_file_name() {
        let c = |left, done| Some(Counter { left, done });
        assert_eq!(split_counter("nixos-a"), Some(("nixos-a", None)));
        assert_eq!(split_counter("nixos-a+1"), Some(("nixos-a", c(1, 0))));
        assert_eq!(split_counter("nixos-a+0-1"), Some(("nixos-a", c(0, 1))));
        assert_eq!(split_counter("nixos-a+x"), None);
        assert_eq!(split_counter("nixos-a+"), None);
        assert_eq!(split_counter("nixos-a+1-"), None);
    }

    /// FAILS IF: an edit of `loader.conf` touches a line other than its
    /// key's, leaves two of it, or does not append one that was missing.
    #[test]
    fn a_line_of_loader_conf_is_edited_alone() {
        let t = "timeout 5\ndefault nixos-*\neditor 0\n";
        assert_eq!(
            with_line(t, "default", Some("nixos-a.conf")),
            "timeout 5\ndefault nixos-a.conf\neditor 0\n"
        );
        assert_eq!(
            with_line(t, "preferred", Some("nixos-b.conf")),
            format!("{t}preferred nixos-b.conf\n")
        );
        assert_eq!(
            with_line("default a\ndefault b\n", "default", Some("c")),
            "default c\n"
        );
        assert_eq!(
            with_line(BUILT, "preferred", None),
            "timeout 5\ndefault nixos-*\neditor 0\nconsole-mode keep\n"
        );
        assert_eq!(conf_value(BUILT, "preferred"), Some("nixos-b.conf"));
        assert_eq!(conf_value(BUILT, "pref"), None);
    }

    /// FAILS IF: the trial is not the two lines and the one try — the
    /// default on the last good entry, the new one preferred with a fresh
    /// try —, an entry that spent its try is not given it again, or a
    /// one-shot whose try was spent is still read as untried.
    #[test]
    fn the_trial_is_two_lines_and_one_try() {
        let s = Scratch::new("loader-trial");
        let l = loader(&s);
        let dir = s.path("boot/loader/entries");
        std::fs::write(dir.join("nixos-a.conf"), "a").unwrap();
        std::fs::write(dir.join("nixos-b+1.conf"), "b").unwrap();
        assert_eq!(
            l.oneshot().as_deref(),
            Some("nixos-b.conf"),
            "the builder's own preferred, untried"
        );
        assert!(l.set_default("nixos-a.conf"));
        assert!(l.set_oneshot("nixos-b.conf"));
        let conf = std::fs::read_to_string(s.path("boot/loader/loader.conf")).unwrap();
        assert!(
            conf.contains("\ndefault nixos-a.conf\n") && conf.contains("preferred nixos-b.conf\n"),
            "{conf}"
        );
        // THE BOOT SPENDS THE TRY.
        std::fs::rename(dir.join("nixos-b+1.conf"), dir.join("nixos-b+0-1.conf")).unwrap();
        assert_eq!(
            l.oneshot().as_deref(),
            Some(""),
            "a spent try is no one-shot"
        );
        assert!(l.set_oneshot("nixos-b.conf"));
        assert_eq!(
            names(&s),
            ["nixos-a.conf", "nixos-b+1.conf"],
            "armed again: one fresh try"
        );
        // AN ENTRY THAT NEVER HAD ONE gets it.
        assert!(l.set_oneshot("nixos-a.conf"));
        assert_eq!(names(&s), ["nixos-a+1.conf", "nixos-b+1.conf"]);
        assert!(!l.set_oneshot("nixos-z.conf"), "no file, no trial");
    }

    /// FAILS IF: the bless leaves the counter on — the boot after would take
    /// the entry for bad —, or a clear takes `preferred` away from what the
    /// builder wrote (a rollback by hand moved it) or leaves the default on
    /// the last good entry.
    #[test]
    fn the_bless_takes_the_counter_off_and_the_clear_leaves_preferred() {
        let s = Scratch::new("loader-bless");
        let l = loader(&s);
        let dir = s.path("boot/loader/entries");
        std::fs::write(dir.join("nixos-b+0-1.conf"), "b").unwrap();
        assert!(l.set_default("nixos-a.conf"));
        assert!(l.bless("nixos-b.conf"));
        assert_eq!(names(&s), ["nixos-b.conf"]);
        assert!(l.bless("nixos-b.conf"), "a good entry is good");
        l.clear();
        assert_eq!(
            std::fs::read_to_string(s.path("boot/loader/loader.conf")).unwrap(),
            BUILT,
            "a preferred on a good entry is the builder's, and stays"
        );
        // A ONE-SHOT STILL ARMED is taken away by the clear.
        std::fs::rename(dir.join("nixos-b.conf"), dir.join("nixos-b+1.conf")).unwrap();
        l.clear();
        assert_eq!(l.oneshot().as_deref(), Some(""));
        // PUT ASIDE AND BACK, as a hibernation does.
        assert!(l.set_oneshot(""));
        assert_eq!(l.oneshot().as_deref(), Some(""));
        assert!(
            !std::fs::read_to_string(s.path("boot/loader/loader.conf"))
                .unwrap()
                .contains("preferred")
        );
    }

    /// FAILS IF: a unified kernel image — lanzaboote's entry, on a host with
    /// Secure Boot — is not found among the UKIs by its id, or gets a counter
    /// that is not in its own name (`<id>+1.efi`) or loses its suffix.
    #[test]
    fn a_uki_is_counted_in_its_own_name() {
        let s = Scratch::new("loader-uki");
        let l = loader(&s);
        let ukis = s.path("boot/EFI/Linux");
        std::fs::create_dir_all(&ukis).unwrap();
        std::fs::write(ukis.join("nixos-generation-2-ab.efi"), "pe").unwrap();
        assert!(l.set_oneshot("nixos-generation-2-ab.efi"));
        let name = |dir: &Path| -> Vec<String> {
            std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect()
        };
        assert_eq!(name(&ukis), ["nixos-generation-2-ab+1.efi"]);
        assert_eq!(l.oneshot().as_deref(), Some("nixos-generation-2-ab.efi"));
        std::fs::rename(
            ukis.join("nixos-generation-2-ab+1.efi"),
            ukis.join("nixos-generation-2-ab+0-1.efi"),
        )
        .unwrap();
        assert!(l.bless("nixos-generation-2-ab.efi"));
        assert_eq!(name(&ukis), ["nixos-generation-2-ab.efi"]);
    }

    /// FAILS IF: anything about a trial on systemd-boot is still written as
    /// an EFI variable — a firmware may reset the machine on one (the
    /// module's comment) — or the menu is still asked for anything but its
    /// list.
    #[test]
    fn the_trial_writes_no_variable() {
        let words = argv::list_entries();
        assert_eq!(words.iter().filter(|w| w.starts_with("set-")).count(), 0);
        let s = Scratch::new("loader-novars");
        let l = loader(&s);
        let dir = s.path("boot/loader/entries");
        std::fs::write(dir.join("nixos-a.conf"), "a").unwrap();
        std::fs::write(dir.join("nixos-b+1.conf"), "b").unwrap();
        // NO `bootctl` AT ALL in the scratch: every write above succeeds
        // without one, so none of them ran it.
        assert!(!s.path("bin/bootctl").exists());
        assert!(
            l.set_default("nixos-a.conf")
                && l.set_oneshot("nixos-b.conf")
                && l.bless("nixos-b.conf")
        );
        l.clear();
    }
}
