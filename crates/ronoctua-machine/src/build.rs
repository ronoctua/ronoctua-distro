//! Building the machine's system — sized to the machine, in phases — and
//! keeping the compiled units it was built from.
//!
//! # Sized
//!
//! A system is built from some 300 Rust units, one derivation each, and
//! Nix's own defaults (`max-jobs = auto`, `cores = 0`) would run a compiler
//! per core with a thread per core: the shape that has the kernel or
//! `systemd-oomd` kill a compiler with no message. How many fit is
//! `ronoctua-build-sizing`'s answer ([`Sizing`], `nix/build-sizing.nix`) —
//! the same arithmetic the batteries build by — and every build here takes
//! it. This crate computes none of it: it reads the numbers.
//!
//! # In phases
//!
//! The distro says how its system is built (`ronoctua.machine.phases`,
//! [`Phases`]): first what the heavy units are built from, as Rust units
//! (many at once, one core each); then each heavy unit alone — the shell's
//! program, one thin-LTO link of the whole footer — with the cores the
//! memory leaves beside it; then the system, sized as everything else. One
//! evaluation writes every `.drv`, and every build after it names them.
//!
//! Of each phase only what Nix would build for the system is built — its
//! own plan, a dry run of the system ([`crate::dry::to_build`]), and never
//! the `.drv` closure. Every unit is in that closure, and a store that holds
//! a unit's consumer needs none of it: the ISO's seed carries the shipped
//! programs (the shell in its bundle's copy) and not one unit, nor does a
//! closure from the person's binary cache. Built from the closure, the
//! first update after an install, or any update there, would compile every
//! unit below the shell and link the shell again for a system whose Rust
//! did not change. A distro that describes no phases — older than them, or
//! a format this program does not read — is built in one step, sized, and
//! said.
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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::argv::{self, Sized};
use crate::system::{self, needed};
use crate::{Machine, Refusal, Why, disk, dry, run, say};

/// Where, under the machine's record, the units of the last build are held.
pub const KEPT_DIR: &str = "kept-units";

/// The name of their set there.
pub const KEPT_SET: &str = "release";

/// The format of [`Phases`] this program reads.
pub const PHASES_FORMAT: u32 = 1;

/// How many compilations fit at once, as `ronoctua-build-sizing` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizing {
    /// The Rust units: as many at once as fit, one core each.
    pub units: Sized,
    /// A heavy unit, alone: its cores.
    pub heavy_cores: u32,
    /// Everything else.
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
            units: Sized {
                jobs: count("rust_jobs")?,
                cores: count("rust_cores")?,
            },
            heavy_cores: count("heavy_cores")?,
            rest: Sized {
                jobs: count("jobs")?,
                cores: count("cores")?,
            },
        })
    }

    /// One heavy unit, alone.
    #[must_use]
    pub const fn heavy(&self) -> Sized {
        Sized {
            jobs: 1,
            cores: self.heavy_cores,
        }
    }
}

/// How the distro says its system is built (`ronoctua.machine.phases` in
/// `nix/modules/machine.nix`): every name a `.drv`, every kept one a store
/// path.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Phases {
    /// [`PHASES_FORMAT`].
    pub format: u32,
    /// What the heavy units are built from that reaches no heavy unit.
    pub first: Vec<String>,
    /// The heavy units, in the order they are built.
    pub heavy: Vec<Step>,
    /// The system.
    pub system: String,
    /// The units to keep after a build that worked (their outputs).
    pub kept: Vec<String>,
}

/// One heavy unit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Step {
    /// Its name (`rust.heavyUnits`).
    pub name: String,
    /// Its `.drv`.
    pub drv: String,
    /// What it is built from that reaches an earlier heavy unit.
    pub between: Vec<String>,
}

impl Phases {
    /// The phases `json` describes, in the format this program reads;
    /// `None` for anything else.
    #[must_use]
    pub fn parse(json: &str) -> Option<Self> {
        let p: Self = serde_json::from_str(json).ok()?;
        let all_drvs = p
            .first
            .iter()
            .chain(std::iter::once(&p.system))
            .chain(
                p.heavy
                    .iter()
                    .flat_map(|s| std::iter::once(&s.drv).chain(&s.between)),
            )
            .all(|d| is_store(d) && d.ends_with(".drv"));
        (p.format == PHASES_FORMAT && all_drvs && p.kept.iter().all(|k| is_store(k))).then_some(p)
    }
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

/// One sized build of `drvs`; a refusal (`build-failed`) when it fails.
fn build(nix: &Path, drvs: &[String], sized: Sized, link: &Path) -> Result<(), Refusal> {
    if run::status(nix, &argv::build_drvs(drvs, sized, link))? == Some(0) {
        Ok(())
    } else {
        say(
            3,
            "the new system did not build; nothing changed (build-failed)",
        );
        Err(Refusal::of(Why::BuildFailed))
    }
}

/// The machine flake at `dir`, its system built into `out`: in the distro's
/// phases when it describes them, else in one step — sized either way.
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
    let Some(phases) = ran.ok().then(|| Phases::parse(&ran.stdout)).flatten() else {
        // WHY, in Nix's own last line: an attribute an older distro does not
        // have, or an evaluation the one build below will say again.
        let why = ran.stderr.lines().rev().find(|l| !l.trim().is_empty());
        say(
            4,
            &format!(
                "the distro describes no build phases this program reads ({}): the system is \
                 built in one step, sized, and its units are not kept",
                why.map_or("a format it does not read", str::trim)
            ),
        );
        if run::status(nix, &argv::build(dir, attr, out, sizing.rest))? != Some(0) {
            say(
                3,
                "the new system did not build; nothing changed (build-failed)",
            );
            return Err(Refusal::of(Why::BuildFailed));
        }
        return Ok(Built {
            kept: None,
            _held: held,
        });
    };

    // WHAT THE SYSTEM'S BUILD WILL BUILD, in Nix's own plan: a unit (or a
    // heavy one) whose consumer the store already holds is not built. A dry
    // run that does not answer, or answers in words this program does not
    // read, builds every phase, and is said: a phase built for nothing costs
    // time, while a heavy unit left to the system's build would be linked
    // beside everything else.
    let dry = run::capture(
        nix,
        &argv::dry_run_drvs(std::slice::from_ref(&phases.system)),
    )?;
    let will = dry.ok().then(|| dry::to_build(&dry.stderr)).flatten();
    if will.is_none() {
        let why = dry.stderr.lines().rev().find(|l| !l.trim().is_empty());
        say(
            4,
            &format!(
                "the dry run of the system's build did not say what it builds ({}): every phase \
                 is built",
                why.map_or("no answer", str::trim)
            ),
        );
    }
    let needed_now = |drvs: &[String]| -> Vec<String> {
        drvs.iter()
            .filter(|d| will.as_ref().is_none_or(|w| w.contains(*d)))
            .cloned()
            .collect()
    };

    let first = needed_now(&phases.first);
    if !first.is_empty() {
        say(
            6,
            &format!(
                "building the {} units the heavy ones are built from",
                first.len()
            ),
        );
        build(nix, &first, sizing.units, &held.0.join("first"))?;
    }
    for (i, step) in phases.heavy.iter().enumerate() {
        if needed_now(std::slice::from_ref(&step.drv)).is_empty() {
            continue;
        }
        let between = needed_now(&step.between);
        if !between.is_empty() {
            build(
                nix,
                &between,
                sizing.units,
                &held.0.join(format!("between-{i}")),
            )?;
        }
        say(
            6,
            &format!(
                "building {} alone, with {} cores",
                step.name, sizing.heavy_cores
            ),
        );
        build(
            nix,
            std::slice::from_ref(&step.drv),
            sizing.heavy(),
            &held.0.join(format!("heavy-{i}")),
        )?;
    }
    build(nix, std::slice::from_ref(&phases.system), sizing.rest, out)?;
    Ok(Built {
        kept: Some(phases.kept),
        _held: held,
    })
}

/// Roots the units `kept` names that the store holds, under the machine's
/// record, releasing the last build's. Never fails the apply: a build that
/// worked is not undone because its units could not be kept, and is said.
pub(crate) fn keep(m: &Machine, kept: &[String]) {
    let Some(program) = &m.system.keep_units else {
        say(
            4,
            &format!(
                "{} is not set: the units of this build are not kept",
                system::KEEP_UNITS
            ),
        );
        return;
    };
    let file = m.places.run.join("kept-units.txt");
    let mut text = kept.join("\n");
    text.push('\n');
    if let Err(e) = disk::write_atomically(&file, text.as_bytes(), 0o644) {
        say(
            4,
            &format!("{}: {e}: the units are not kept", file.display()),
        );
        return;
    }
    let roots = m.places.state.join(KEPT_DIR);
    match run::status(program, &argv::keep_units(&roots, KEPT_SET, &file)) {
        Ok(Some(0)) => {}
        _ => say(
            4,
            "the units of this build were not kept: the next update may compile them again",
        ),
    }
    let _ = disk::remove(&file);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sizing_is_read_and_never_guessed() {
        let said = "total_kb=1\nceiling_kb=\nslots=3\nrust_jobs=3\nrust_cores=1\njobs=3\ncores=1\nheavy_cores=2\n";
        assert_eq!(
            Sizing::parse(said),
            Some(Sizing {
                units: Sized { jobs: 3, cores: 1 },
                heavy_cores: 2,
                rest: Sized { jobs: 3, cores: 1 },
            })
        );
        assert_eq!(
            Sizing::parse(said).unwrap().heavy(),
            Sized { jobs: 1, cores: 2 }
        );
        for bad in [
            "",
            "rust_jobs=3\nrust_cores=1\njobs=3\ncores=1",
            &said.replace("heavy_cores=2", "heavy_cores=0"),
            &said.replace("jobs=3\ncores", "jobs=many\ncores"),
            &format!("{said}a line that says nothing\n"),
        ] {
            assert_eq!(Sizing::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_phases_are_read_in_their_format_only() {
        let good = r#"{"format":1,"first":["/nix/store/aaa-rust-lib-x-0.1.0.drv"],
            "heavy":[{"name":"shell-release","drv":"/nix/store/bbb-ronoctua-shell-0.1.0.drv","between":[]}],
            "system":"/nix/store/ccc-nixos-system.drv","kept":["/nix/store/ddd-rust-lib-x-0.1.0"]}"#;
        let p = Phases::parse(good).unwrap();
        assert_eq!(p.heavy[0].name, "shell-release");
        assert_eq!(p.kept, ["/nix/store/ddd-rust-lib-x-0.1.0"]);
        for bad in [
            good.replace(r#""format":1"#, r#""format":2"#),
            good.replace("ccc-nixos-system.drv", "ccc-nixos-system"),
            good.replace("/nix/store/ddd", "/tmp/ddd"),
            good.replace("aaa-rust-lib-x-0.1.0.drv", "aaa x.drv"),
            good.replace("/nix/store/ddd-rust-lib-x-0.1.0", "/nix/store/../etc"),
            r#"{"format":1}"#.to_owned(),
            "not json".to_owned(),
        ] {
            assert_eq!(Phases::parse(&bad), None, "{bad}");
        }
    }
}
