//! Building the machine's system — its units first, each by its class, then
//! the system, sized to the machine — and keeping the compiled units it was
//! built from, and the sources it was evaluated from.
//!
//! # Sized
//!
//! A system is built from some 800 Rust units, one derivation each, and
//! Nix's own defaults (`max-jobs = auto`, `cores = 0`) would run a compiler
//! per core with a thread per core: the shape that has the kernel or
//! `systemd-oomd` kill a compiler with no message. How many fit is the
//! distro's arithmetic (`nix/build-sizing.nix`), the same the batteries
//! build by: `ronoctua-build-sizing` says how the system itself is built
//! ([`Sizing`]), and `ronoctua-build-units` builds the units. This crate
//! computes none of it: it reads the numbers and hands over the words.
//!
//! # The units, each by its class
//!
//! The distro says how its system is built (`ronoctua.machine.phases`,
//! [`Phases`]): the plan of its release units — one line each, the unit's
//! id, its measured class, how that class was found, its cost, its `.drv`
//! and the lines it is built from —, the system, and the units to keep. One
//! evaluation writes every `.drv`, and every build after it names them.
//! `ronoctua-build-units` builds each unit its own `nix build`, as many at
//! once as the slots their lines give fit the memory, the heavy ones (the
//! shell's program, one thin-LTO link of the whole footer) alone with the
//! cores the memory leaves; then the system is built, sized as everything
//! else.
//!
//! # Across the version boundary
//!
//! The plan is the NEW distro's — evaluated from the machine's flake, which
//! moved its input — and every program that reads it is the RUNNING
//! system's: this one and `ronoctua-build-units`. So a line carries every
//! number the build needs (the slot in MiB, or `alone` with a heavy unit's
//! own), the class is only a name, and neither program holds a table of
//! classes: a class added since, or a slot resized, is built by the line's
//! numbers. Of a line this program reads only the `.drv` (its fifth word)
//! and the numbers after it.
//!
//! The format is what may not cross. A distro in another one — older, or
//! newer than this program — is built in one step, sized, and said: a
//! refusal there would keep the machine on this program for good, since a
//! newer distro is how a newer program arrives. A distro in THIS format
//! that this program cannot read (a line without its `.drv`) is a refusal,
//! with the line and why: the distro is wrong, and the next one mends it.
//!
//! Of the plan only what Nix would build for the system is built — its own
//! plan, a dry run of the system ([`crate::dry::to_build`]), and never the
//! `.drv` closure. Every unit is in that closure, and a store that holds a
//! unit's consumer needs none of it: the ISO's seed carries the shipped
//! programs (the shell in its bundle's copy) and not one unit, nor does a
//! closure from the person's binary cache. Built from the closure, the
//! first update after an install, or any update there, would compile every
//! unit below the shell and link the shell again for a system whose Rust
//! did not change. A dry run that does not say it in words this reads
//! hands every unit of the plan over, and is said. A distro that describes
//! no phases — older than them — is built in one step, sized, and said.
//!
//! # Kept
//!
//! A program references none of the units it was linked from, so after a
//! build they are garbage, and the update's weekly collection takes them:
//! the next update would compile every library again to change one line.
//! After a build that worked, the units of the version just built that the
//! store holds are rooted under the machine's record ([`KEPT_DIR`]), by
//! `ronoctua-keep-units` (`nix/keep-units.nix`), and the last version's are
//! released. A failed build keeps the old ones.
//!
//! And the SOURCES it was evaluated from — the machine's flake, the distro
//! and every input of either — are garbage too, wherever no derivation of
//! the system cites them: an input that is only Nix code (the home manager,
//! the overlays, the distro's own tree) was taken by the collection, and the
//! next update fetched it again, unchanged. (The fonts' and the ambient
//! sounds' archives the system's `.drv`s hold, and Nix keeps the `.drv` of
//! every output still in use.) After a build that worked, the sources the
//! store holds are rooted the same way ([`SOURCES_DIR`]) — named by
//! `nix flake archive --dry-run`, which reads them off the lock and fetches
//! nothing — and the last build's are released.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::argv::{self, Sized};
use crate::system::{self, needed};
use crate::{Machine, Refusal, Why, disk, dry, run, say};

/// Where, under the machine's record, the units of the last build are held.
pub const KEPT_DIR: &str = "kept-units";

/// The name of their set there.
pub const KEPT_SET: &str = "release";

/// Where, under the machine's record, the sources of the last build are held.
pub const SOURCES_DIR: &str = "kept-sources";

/// The name of their set there.
pub const SOURCES_SET: &str = "flake";

/// The format of [`Phases`] this program reads.
pub const PHASES_FORMAT: u32 = 3;

/// How the system itself is built, as `ronoctua-build-sizing` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizing {
    /// Everything that is no unit: the system, and a distro without phases.
    pub rest: Sized,
}

impl Sizing {
    /// The sizing `text` says: `name=value` lines, the counts at least one.
    /// `None` for a line that is not one, or a count missing or not a count
    /// — never a plausible default.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut said = BTreeMap::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (name, value) = line.split_once('=')?;
            said.insert(name.trim(), value.trim());
        }
        let count = |name: &str| {
            said.get(name)
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|n| *n >= 1)
        };
        Some(Self {
            rest: Sized {
                jobs: count("jobs")?,
                cores: count("cores")?,
            },
        })
    }
}

/// How the distro says its system is built (`ronoctua.machine.phases` in
/// `nix/modules/machine.nix`): every unit a line of the plan naming its
/// `.drv`, the system a `.drv`, every kept one a store path.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Phases {
    /// [`PHASES_FORMAT`].
    pub format: u32,
    /// The plan of the units, one line each: `<id> <class> <how> <cost>
    /// <.drv> <the lines it is built from>…` — words `ronoctua-build-units`
    /// reads (`sized_units`, `nix/build-sizing.nix`), its cost a slot in
    /// MiB or `alone:<MiB>+<MiB>`.
    pub units: Vec<String>,
    /// The system.
    pub system: String,
    /// The units to keep after a build that worked (their outputs).
    pub kept: Vec<String>,
}

/// Why the phases a distro describes are not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// No format to read: not JSON, or no `format` in it.
    NoFormat,
    /// Another format than [`PHASES_FORMAT`], older or newer.
    Format(u32),
    /// The format this program reads, and a part of it that is not in it.
    Part(String),
}

/// The first word of a line that would be a unit's `.drv`.
const DRV_WORD: usize = 4;

impl Phases {
    /// The phases `json` describes, in the format this program reads.
    ///
    /// # Errors
    ///
    /// [`Unread`]: no format, another format, or a part of this one that is
    /// not what it says.
    pub fn parse(json: &str) -> Result<Self, Unread> {
        #[derive(Deserialize)]
        struct Format {
            format: u32,
        }
        let Format { format } = serde_json::from_str(json).map_err(|_| Unread::NoFormat)?;
        if format != PHASES_FORMAT {
            return Err(Unread::Format(format));
        }
        let p: Self = serde_json::from_str(json)
            .map_err(|e| Unread::Part(format!("its fields are not the format's ({e})")))?;
        if !(is_store(&p.system) && p.system.ends_with(".drv")) {
            return Err(Unread::Part(format!(
                "its system {:?} is no derivation",
                p.system
            )));
        }
        for (n, line) in p.units.iter().enumerate() {
            unit_line(line, p.units.len())
                .map_err(|why| Unread::Part(format!("line {} of its plan {why}", n + 1)))?;
        }
        if let Some(k) = p.kept.iter().find(|k| !is_store(k)) {
            return Err(Unread::Part(format!(
                "{k:?}, a unit to keep, is no path of the store"
            )));
        }
        Ok(p)
    }

    /// The `.drv` of each unit of the plan.
    fn unit_drvs(&self) -> impl Iterator<Item = &str> {
        self.units
            .iter()
            .filter_map(|l| l.split_whitespace().nth(DRV_WORD))
    }
}

/// What this program reads of a line of the plan: its fifth word, a `.drv`,
/// and after it the numbers of lines (fewer than `lines`) it is built from.
/// The four words before are `ronoctua-build-units`'s to read.
fn unit_line(line: &str, lines: usize) -> Result<(), String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some(drv) = words.get(DRV_WORD) else {
        return Err(format!(
            "has {} words, not <id> <class> <how> <cost> <.drv>…",
            words.len()
        ));
    };
    if !(is_store(drv) && drv.ends_with(".drv")) {
        return Err(format!("names {drv:?} where its .drv is"));
    }
    if let Some(bad) = words[DRV_WORD + 1..]
        .iter()
        .find(|w| w.parse::<usize>().map_or(true, |n| n >= lines))
    {
        return Err(format!("is built from {bad:?}, no line of it"));
    }
    Ok(())
}

/// Whether `path` is one path of the store, in one word.
fn is_store(path: &str) -> bool {
    path.strip_prefix("/nix/store/").is_some_and(|rest| {
        !rest.is_empty()
            && !rest.contains('/')
            && !rest.starts_with('.')
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-._?=".contains(&b))
    })
}

/// The out-links that hold a build's units while it lasts: removed with it.
pub(crate) struct Held(PathBuf);

impl Drop for Held {
    fn drop(&mut self) {
        let _ = disk::remove(&self.0);
    }
}

/// A system built.
pub(crate) struct Built {
    /// The units to keep: `None` when the distro described no phases.
    pub(crate) kept: Option<Vec<String>>,
    /// What holds them meanwhile.
    _held: Held,
}

/// The sizing this machine's program says.
fn sizing(program: &Path) -> Result<Sizing, Refusal> {
    let ran = run::capture(program, &[])?;
    eprint!("{}", ran.stderr);
    if !ran.ok() {
        say(3, "the build sizing did not answer (failed)");
        return Err(Refusal::of(Why::Failed));
    }
    Sizing::parse(&ran.stdout).ok_or_else(|| {
        say(
            3,
            &format!(
                "the build sizing said {:?}, not how many builds fit (failed)",
                ran.stdout.trim()
            ),
        );
        Refusal::of(Why::Failed)
    })
}

/// The refusal of a system that did not build, said.
fn not_built() -> Refusal {
    say(
        3,
        "the new system did not build; nothing changed (build-failed)",
    );
    Refusal::of(Why::BuildFailed)
}

/// `text`'s lines into `path`, each ended.
fn lines_into(path: &Path, lines: &[&str]) -> Result<(), Refusal> {
    let mut text = lines.join("\n");
    text.push('\n');
    disk::write_atomically(path, text.as_bytes(), 0o644).map_err(|e| disk::failed(path, &e))
}

/// The machine flake at `dir`, its system built into `out`: its units first
/// when the distro describes them, else in one step — sized either way.
///
/// # Errors
///
/// [`Why::BuildFailed`]; [`Why::Failed`] when the sizing does not answer or a
/// program does not start.
pub(crate) fn system(
    m: &Machine,
    nix: &Path,
    dir: &Path,
    attr: &str,
    out: &Path,
) -> Result<Built, Refusal> {
    let sizing = sizing(&needed(m.system.sizing.as_ref(), system::BUILD_SIZING)?)?;
    let held = Held(m.places.run.join("units"));
    disk::remove(&held.0).map_err(|e| disk::failed(&held.0, &e))?;
    std::fs::create_dir_all(&held.0).map_err(|e| disk::failed(&held.0, &e))?;

    let ran = run::capture(nix, &argv::phases(dir, attr))?;
    let read = if ran.ok() {
        Phases::parse(&ran.stdout)
    } else {
        Err(Unread::NoFormat)
    };
    let phases = match read {
        Ok(phases) => phases,
        Err(Unread::Part(why)) => {
            say(
                3,
                &format!(
                    "the distro describes its build in format {PHASES_FORMAT}, which this \
                     program reads, and {why}: nothing is built (build-failed)"
                ),
            );
            return Err(Refusal::of(Why::BuildFailed));
        }
        Err(other) => {
            // WHY: another format, in its number; or none, in Nix's own
            // last line — an attribute an older distro does not have, or an
            // evaluation the one build below will say again.
            let why = match other {
                Unread::Format(n) => format!(
                    "it describes them in format {n}, and this program reads {PHASES_FORMAT}"
                ),
                _ => ran
                    .stderr
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map_or_else(|| "no format".to_owned(), |l| l.trim().to_owned()),
            };
            say(
                4,
                &format!(
                    "the distro describes no build phases this program reads ({why}): the \
                     system is built in one step, sized, and its units are not kept"
                ),
            );
            if run::status(nix, &argv::build(dir, attr, out, sizing.rest))? != Some(0) {
                return Err(not_built());
            }
            return Ok(Built {
                kept: None,
                _held: held,
            });
        }
    };

    // WHAT THE SYSTEM'S BUILD WILL BUILD, in Nix's own plan: a unit (or a
    // heavy one) whose consumer the store already holds is not built. A dry
    // run that does not answer, or answers in words this program does not
    // read, hands every unit over, and is said: a unit built for nothing
    // costs time, while a heavy unit left to the system's build would be
    // linked beside everything else.
    let dry = run::capture(
        nix,
        &argv::dry_run_drvs(std::slice::from_ref(&phases.system)),
    )?;
    let will: Option<BTreeSet<String>> = dry
        .ok()
        .then(|| dry::to_build(&dry.stderr))
        .flatten()
        .map(|w| w.into_iter().collect());
    if will.is_none() {
        let why = dry.stderr.lines().rev().find(|l| !l.trim().is_empty());
        say(
            4,
            &format!(
                "the dry run of the system's build did not say what it builds ({}): every unit \
                 is handed over",
                why.map_or("no answer", str::trim)
            ),
        );
    }
    let wanted: Vec<&str> = phases
        .unit_drvs()
        .filter(|d| will.as_ref().is_none_or(|w| w.contains(*d)))
        .collect();
    if !wanted.is_empty() {
        let program = needed(m.system.build_units.as_ref(), system::BUILD_UNITS)?;
        let plan = held.0.join("plan");
        let wanted_file = held.0.join("will");
        let lines: Vec<&str> = phases.units.iter().map(String::as_str).collect();
        lines_into(&plan, &lines)?;
        // WHAT THE UNITS' PROGRAM IS HANDED: Nix's whole plan for the
        // system — its units, and what is no unit, of which the program
        // builds first what the units are built from, sized as what is no
        // unit, and asks Nix nothing again. With no plan, every unit, and
        // the program asks Nix itself what they are built from.
        let handed: Vec<&str> = will.as_ref().map_or_else(
            || wanted.clone(),
            |w| w.iter().map(String::as_str).collect(),
        );
        lines_into(&wanted_file, &handed)?;
        say(
            6,
            &format!(
                "building the {} units the system needs, each by its class",
                wanted.len()
            ),
        );
        let args = argv::build_units(
            nix,
            &plan,
            &wanted_file,
            &held.0.join("built"),
            will.is_none(),
        );
        if run::status(&program, &args)? != Some(0) {
            return Err(not_built());
        }
    }
    if run::status(
        nix,
        &argv::build_drvs(std::slice::from_ref(&phases.system), sizing.rest, out),
    )? != Some(0)
    {
        return Err(not_built());
    }
    Ok(Built {
        kept: Some(phases.kept),
        _held: held,
    })
}

/// Roots the units `kept` names that the store holds, under the machine's
/// record, releasing the last build's. Never fails the apply: a build that
/// worked is not undone because its units could not be kept, and is said.
pub(crate) fn keep(m: &Machine, kept: &[String]) {
    let _ = hold(
        m,
        KEPT_DIR,
        KEPT_SET,
        kept,
        "the units",
        "the next update may compile them again",
    );
}

/// Roots the sources the build just evaluated — the machine's flake, the
/// distro and every input of either, as `nix flake archive --dry-run` names
/// them off the lock, fetching nothing — that the store holds, under the
/// machine's record, releasing the last build's: an update that finds them
/// unchanged fetches none of them again. Whether they were kept: never a
/// refusal, as [`keep`] is none — what could not be done is said.
pub(crate) fn keep_sources(m: &Machine, nix: &Path) -> bool {
    const LOST: &str = "the next update may fetch them again";
    let Ok(ran) = run::capture(nix, &argv::sources(&m.places.dir)) else {
        return false;
    };
    if ran.code != Some(0) {
        say(
            4,
            &format!(
                "the sources of this build were not listed ({}): {LOST}",
                ran.stderr.trim()
            ),
        );
        return false;
    }
    let Some(sources) = sources_of(&ran.stdout) else {
        say(
            4,
            &format!(
                "nix flake archive named its sources in words this program does not read: {LOST}"
            ),
        );
        return false;
    };
    hold(m, SOURCES_DIR, SOURCES_SET, &sources, "the sources", LOST)
}

/// Roots the `paths` the store holds as the set `set` under the machine's
/// record's `dir`, by `ronoctua-keep-units`, which then releases the set's
/// older generation; `what` they are and what is `lost` without them are
/// said when it could not. Whether they are held.
fn hold(m: &Machine, dir: &str, set: &str, paths: &[String], what: &str, lost: &str) -> bool {
    let Some(program) = &m.system.keep_units else {
        say(
            4,
            &format!(
                "{} is not set: {what} of this build are not kept",
                system::KEEP_UNITS
            ),
        );
        return false;
    };
    // The update's folder under `/run`, made here too: `keep-sources` runs
    // at boot, with no apply before it to make it.
    let file = m.places.run.join(format!("{dir}.txt"));
    let mut text = paths.join("\n");
    text.push('\n');
    if let Err(e) = std::fs::create_dir_all(&m.places.run)
        .and_then(|()| disk::write_atomically(&file, text.as_bytes(), 0o644))
    {
        say(4, &format!("{}: {e}: {what} are not kept", file.display()));
        return false;
    }
    let roots = m.places.state.join(dir);
    let held = matches!(
        run::status(program, &argv::keep_units(&roots, set, &file)),
        Ok(Some(0))
    );
    if !held {
        say(4, &format!("{what} of this build were not kept: {lost}"));
    }
    let _ = disk::remove(&file);
    held
}

/// One flake as `nix flake archive --json` names it: its path in the store
/// and its inputs, each one the same.
#[derive(Deserialize)]
struct Archived {
    path: String,
    #[serde(default)]
    inputs: BTreeMap<String, Archived>,
}

/// Every path of the store `nix flake archive --json` names — the flake's
/// own and every input's, an input's inputs included —, once each; `None`
/// for an answer of another shape, or a path that is not one path of the
/// store (a part of one included), never a plausible list.
fn sources_of(json: &str) -> Option<Vec<String>> {
    fn walk(a: &Archived, into: &mut BTreeSet<String>) -> bool {
        if !is_store(&a.path) {
            return false;
        }
        into.insert(a.path.clone());
        a.inputs.values().all(|i| walk(i, into))
    }
    let top: Archived = serde_json::from_str(json).ok()?;
    let mut paths = BTreeSet::new();
    walk(&top, &mut paths).then(|| paths.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a source `nix flake archive` names is left out — an input's
    /// input, or one an input shares through `follows` named twice and kept
    /// twice —, or an answer that is not that shape, or names a path that is
    /// not one path of the store (a file inside one, a path outside it), is
    /// read as a list at all; or a field a newer Nix adds stops the reading.
    #[test]
    fn the_sources_are_every_path_the_archive_names_once() {
        let json = r#"{"path":"/nix/store/mmmm-source","inputs":{
            "ronoctua":{"path":"/nix/store/dddd-source","inputs":{
                "fonts":{"path":"/nix/store/ffff-source"},
                "home-manager":{"path":"/nix/store/hhhh-source","inputs":{
                    "nixpkgs":{"path":"/nix/store/nnnn-source","inputs":{}}}},
                "nixpkgs":{"path":"/nix/store/nnnn-source","inputs":{}}},
              "locked":{"rev":"abc"}}}}"#;
        assert_eq!(
            sources_of(json).unwrap(),
            [
                "/nix/store/dddd-source",
                "/nix/store/ffff-source",
                "/nix/store/hhhh-source",
                "/nix/store/mmmm-source",
                "/nix/store/nnnn-source",
            ]
        );
        for bad in [
            json.replace("/nix/store/ffff-source", "/nix/store/ffff-source/fonts"),
            json.replace("/nix/store/hhhh-source", "/tmp/hhhh-source"),
            json.replace(
                r#""path":"/nix/store/nnnn-source","inputs":{}}}},"#,
                r#""inputs":{}}}},"#,
            ),
            "not json".to_owned(),
            "[]".to_owned(),
        ] {
            assert_eq!(sources_of(&bad), None, "{bad}");
        }
    }

    #[test]
    fn the_sizing_is_read_and_never_guessed() {
        let said = "total_kb=1\nceiling_kb=\nslots=3\njobs=3\ncores=1\nheavy_cores=2\nclass_light_jobs=7\n";
        assert_eq!(
            Sizing::parse(said),
            Some(Sizing {
                rest: Sized { jobs: 3, cores: 1 },
            })
        );
        for bad in [
            "",
            "jobs=3\n",
            &said.replace("cores=1", "cores=0"),
            &said.replace("jobs=3\n", "jobs=many\n"),
            &format!("{said}a line that says nothing\n"),
        ] {
            assert_eq!(Sizing::parse(bad), None, "{bad:?}");
        }
    }

    /// FAILS IF: a plan's line is read by a class this program knows —
    /// a class it never heard of, or a slot of a size it never planned,
    /// refused or read as another — instead of by its `.drv` alone; or
    /// another format is read as this one, or a part of this one that is
    /// not what it says is read at all.
    #[test]
    fn the_phases_are_read_in_their_format_only() {
        let good = r#"{"format":3,
            "units":["release/libs/x colossal default 3000 /nix/store/aaa-rust-lib-x-0.1.0.drv",
                     "release/bins/shell enormous carried alone:9000+700 /nix/store/bbb-ronoctua-shell-0.1.0.drv 0"],
            "system":"/nix/store/ccc-nixos-system.drv","kept":["/nix/store/ddd-rust-lib-x-0.1.0"]}"#;
        let p = Phases::parse(good).unwrap();
        assert_eq!(
            p.unit_drvs().collect::<Vec<_>>(),
            [
                "/nix/store/aaa-rust-lib-x-0.1.0.drv",
                "/nix/store/bbb-ronoctua-shell-0.1.0.drv"
            ]
        );
        assert_eq!(p.kept, ["/nix/store/ddd-rust-lib-x-0.1.0"]);
        for (older_or_newer, n) in [(r#""format":2"#, 2), (r#""format":4"#, 4)] {
            assert_eq!(
                Phases::parse(&good.replace(r#""format":3"#, older_or_newer)),
                Err(Unread::Format(n))
            );
        }
        for none in [r#"{"units":[]}"#, "not json"] {
            assert_eq!(Phases::parse(none), Err(Unread::NoFormat), "{none}");
        }
        for bad in [
            good.replace("ccc-nixos-system.drv", "ccc-nixos-system"),
            good.replace("/nix/store/ddd", "/tmp/ddd"),
            good.replace(
                "release/libs/x colossal default 3000 /nix/store/aaa-rust-lib-x-0.1.0.drv",
                "release/libs/x light /nix/store/aaa-rust-lib-x-0.1.0.drv",
            ),
            good.replace("/nix/store/aaa-rust-lib-x-0.1.0.drv", "/tmp/aaa.drv"),
            good.replace("/nix/store/ddd-rust-lib-x-0.1.0", "/nix/store/../etc"),
            good.replace("ronoctua-shell-0.1.0.drv 0", "ronoctua-shell-0.1.0.drv 2"),
            good.replace(
                "ronoctua-shell-0.1.0.drv 0",
                "ronoctua-shell-0.1.0.drv first",
            ),
            r#"{"format":3}"#.to_owned(),
        ] {
            assert!(
                matches!(Phases::parse(&bad), Err(Unread::Part(_))),
                "{bad}: {:?}",
                Phases::parse(&bad)
            );
        }
    }
}
