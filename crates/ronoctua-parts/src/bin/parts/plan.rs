//! `plan [--start | --next]`: the running system's plan brought into the
//! records.
//!
//! Every wanted part gets a record saying what is wanted; a part the plan
//! dropped is recorded as unwanted (its profile stays until trimmed, so a
//! rollback finds it). The plan's derivations are rooted, the view rebuilt,
//! and with `--start` the parts that are not here go to their realizers.
//!
//! # The plan of the next boot
//!
//! The update sets the new system for the NEXT BOOT (`ronoctua-machine
//! apply --next-boot`): its parts are realized by the planner of that boot,
//! and until then nothing holds their derivations — the plan carries them as
//! text — so the update's garbage collection would take them. With `--next`
//! (the update's `parts-plan` step) the plan of the system that waits for the
//! next boot (`<toplevel>/etc/ronoctua/parts-plan.json`) has its derivations
//! rooted under
//! [`names::NEXT_ROOTS`], the set replaced whole; nothing waiting, or the
//! running plan again, empties it. The records and the view are NEVER
//! written from it: a part that plan drops must not leave the session that
//! still runs the old one.
//!
//! A part of the person's names their entry, not a derivation: its record
//! keeps the target its realizer evaluated while the declaration stays the
//! same ([`Record::planned_entry`]), and the realizer evaluates it anew
//! otherwise. The planner itself never evaluates anything.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::plan::Wanted;
use ronoctua_parts::record::{Phase, Record};

use crate::disk::{self, LOCK_WAIT, Lock};
use crate::{Usage, plan_path, program, say};

/// How many parts are realized at once unless `--workers` says otherwise:
/// two downloads share a link well, and two builds fit the smallest machine
/// the distro installs on (`nix/build-sizing.nix` sizes each `nix build`).
const WORKERS: usize = 2;

pub fn run(args: &[String]) -> Result<u8, Usage> {
    let start = args.iter().any(|a| a == "--start");
    let rehearse = args.iter().any(|a| a == "--rehearse");
    let next = args.iter().any(|a| a == "--next");
    if start && (rehearse || next) {
        return Err(Usage);
    }
    let workers = match args.iter().position(|a| a == "--workers") {
        Some(at) => args
            .get(at + 1)
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|n| (1..=8).contains(n))
            .ok_or(Usage)?,
        None => WORKERS,
    };
    let running = plan_path(args)?;
    let plan = match disk::read_plan(&running) {
        Ok(plan) => plan,
        Err(e) => {
            say(3, &format!("the plan does not read: {e}"));
            println!("refused plan-unreadable");
            return Ok(exit::FAILED);
        }
    };

    // A REHEARSAL says what the plan would do, and writes nothing: the
    // update's rehearsal (`parts-plan`) runs it.
    if rehearse {
        let known: BTreeMap<String, Record> = disk::read_records()
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let missing = plan
            .parts
            .iter()
            .filter(|(id, wanted)| {
                let record = known.get(*id).cloned().unwrap_or_else(|| Record::new(id));
                planned(record, wanted).phase != Phase::Here
            })
            .count();
        let kept = if next {
            format!(
                ", {} kept for the next boot",
                next_derivations(&running).len()
            )
        } else {
            String::new()
        };
        println!(
            "would plan {} parts, {missing} to realize{kept}",
            plan.parts.len()
        );
        return Ok(exit::DONE);
    }

    // THE RECORDS, under the lock: each wanted part planned, each dropped one
    // unwanted.
    let to_realize = {
        let Ok(_lock) = Lock::take(LOCK_WAIT) else {
            println!("refused busy");
            return Ok(exit::BUSY);
        };
        let mut known: BTreeMap<String, Record> = disk::read_records()
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        for (id, wanted) in &plan.parts {
            let record = known.remove(id).unwrap_or_else(|| Record::new(id));
            write(&planned(record, wanted));
        }
        for (_, record) in known {
            if record.wanted.is_some() {
                write(&record.planned(None));
            }
        }
        root_derivations(&plan);
        // WHAT IS TRIED NOW: what was never tried, what waits for a network
        // (the planner runs when one comes), and what is due — never a part
        // whose wait is not over, or every run would be a retry storm.
        let now = disk::now().unix_s();
        plan.order()
            .into_iter()
            .filter(|(id, _)| disk::read_record(id).is_ok_and(|r| r.try_now(now)))
            .map(|(id, _)| id.to_owned())
            .collect::<Vec<String>>()
    };
    crate::view::rebuild();

    // THE NEXT BOOT'S, kept (and only kept): the records and the view are
    // the running plan's.
    if next {
        let kept = next_derivations(&running);
        root_set(Path::new(names::NEXT_ROOTS), &kept, |id| {
            kept.contains_key(id)
        });
        println!(
            "planned {} parts, {} to realize, {} kept for the next boot",
            plan.parts.len(),
            to_realize.len(),
            kept.len()
        );
        return Ok(exit::DONE);
    }

    if !start || to_realize.is_empty() {
        if start {
            crate::retry::arm();
        }
        println!(
            "planned {} parts, {} to realize",
            plan.parts.len(),
            to_realize.len()
        );
        return Ok(exit::DONE);
    }

    // THE WORKERS: each takes the next part and waits for its realizer to
    // end (`systemctl start` of a oneshot returns when it is over), so at
    // most `workers` parts build at once. A part that did not arrive is
    // tried again at the time its record gives, by the timer armed below.
    let systemctl = program("RONOCTUA_SYSTEMCTL");
    let queue = Mutex::new(to_realize.into_iter());
    let next = || queue.lock().ok().and_then(|mut q| q.next());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(id) = next() {
                    let argv = ronoctua_parts::argv::start_realizer(&systemctl, &id);
                    if !disk::run_quiet(&argv) {
                        say(4, &format!("{id}: its realizer did not bring it this time"));
                    }
                }
            });
        }
    });
    // THE NEXT TRY of whatever failed now, on the clock its record gives.
    crate::retry::arm();
    println!("planned {} parts", plan.parts.len());
    Ok(exit::DONE)
}

/// `record` after the plan wants `wanted`: the distro's part its target, the
/// person's their declaration.
fn planned(record: Record, wanted: &Wanted) -> Record {
    match (wanted.target(), &wanted.entry) {
        (Some(target), _) => record.planned(Some(target)),
        (None, Some(entry)) => record.planned_entry(&entry.fingerprint),
        (None, None) => record.planned(None),
    }
}

fn write(record: &Record) {
    if let Err(e) = disk::write_record(record) {
        say(3, &format!("{}: {e}", record.id));
    }
}

/// Every wanted part's derivation rooted under [`names::PLAN_ROOTS`] — a
/// link per part, replaced when the plan changes, removed when the part is
/// no longer wanted. A link in the collector's roots is all a root is. A
/// part of the person's is rooted at the derivation its realizer evaluated,
/// once it did ([`root_one`]).
fn root_derivations(plan: &ronoctua_parts::plan::Plan) {
    root_set(Path::new(names::PLAN_ROOTS), &derivations(plan), |id| {
        plan.parts.contains_key(id)
    });
}

/// Each part of `plan` that names its derivation, by id.
fn derivations(plan: &ronoctua_parts::plan::Plan) -> BTreeMap<String, String> {
    plan.parts
        .iter()
        .filter_map(|(id, w)| Some((id.clone(), w.drv.clone()?)))
        .collect()
}

/// The derivations of the plan of the system that waits for the next boot
/// ([`next_plan`]) — none when nothing waits, when its plan does not read,
/// or when it is the running plan (`running`) again.
fn next_derivations(running: &Path) -> BTreeMap<String, String> {
    let state = disk::machine_state().ok();
    let Some(file) = next_plan(Path::new("/"), state.as_ref()) else {
        return BTreeMap::new();
    };
    let Ok(bytes) = std::fs::read(&file) else {
        say(
            4,
            &format!("{}: the next boot's plan does not read", file.display()),
        );
        return BTreeMap::new();
    };
    if std::fs::read(running).ok().as_deref() == Some(bytes.as_slice()) {
        return BTreeMap::new();
    }
    match ronoctua_person::parse::<ronoctua_parts::plan::Plan>(&bytes) {
        Ok(plan) => derivations(&plan),
        Err(r) => {
            say(4, &format!("{}: {r}", file.display()));
            BTreeMap::new()
        }
    }
}

/// Where the plan of the system that waits for the next boot is, when one
/// does: inside that system, where its `/etc` puts the running one's
/// ([`names::PLAN`]). What waits is the machine's answer, read off the
/// system profile and the system that runs under `root` (`/`) — never off
/// the last apply: a refusal or a failed build after a staged one would
/// empty the roots of a system that still waits, and a trial that failed
/// would keep those of one that no longer does.
fn next_plan(
    root: &Path,
    state: Option<&ronoctua_person::state::MachineState>,
) -> Option<std::path::PathBuf> {
    let next = ronoctua_machine::next_boot(root, state)?;
    let top = Some(next.toplevel).filter(|t| ronoctua_parts::plan::is_store_path(t))?;
    Some(
        root.join(top.trim_start_matches('/'))
            .join(names::PLAN.trim_start_matches('/')),
    )
}

/// `links` rooted under `roots`, a link per id at its derivation; every
/// other link there goes but those `kept` says to leave (a part of the
/// person's, rooted by its realizer).
fn root_set(roots: &Path, links: &BTreeMap<String, String>, kept: impl Fn(&str) -> bool) {
    if let Err(e) = disk::make_dir(roots, 0o755) {
        say(4, &format!("{}: {e}", roots.display()));
        return;
    }
    if let Ok(entries) = std::fs::read_dir(roots) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            if !name.to_str().is_some_and(&kept) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    for (id, drv) in links {
        root_in(roots, id, drv);
    }
}

/// The part `id`'s derivation `drv` rooted under [`names::PLAN_ROOTS`].
pub fn root_one(id: &str, drv: &str) {
    root_in(Path::new(names::PLAN_ROOTS), id, drv);
}

/// The part `id`'s derivation `drv` rooted under `roots`.
fn root_in(roots: &Path, id: &str, drv: &str) {
    let link = roots.join(id);
    if std::fs::read_link(&link).ok().as_deref() == Some(Path::new(drv)) {
        return;
    }
    let draft = roots.join(format!(".{id}.{}", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let made = disk::make_dir(roots, 0o755)
        .map_err(std::io::Error::other)
        .and_then(|()| std::os::unix::fs::symlink(drv, &draft))
        .and_then(|()| std::fs::rename(&draft, &link));
    if let Err(e) = made {
        say(4, &format!("{id}: its derivation was not rooted: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ronoctua_person::Stamp;
    use ronoctua_person::state::{ApplyResult, LastApply, MachineState};

    fn with(result: ApplyResult, toplevel: Option<&str>) -> MachineState {
        MachineState {
            last_apply: Some(LastApply {
                at: Stamp::from_unix_s(0),
                result,
                toplevel: toplevel.map(str::to_owned),
                missing: Vec::new(),
            }),
            ..MachineState::default()
        }
    }

    /// FAILS IF: the plan of the next boot is looked for anywhere but inside
    /// the system that waits for it, or read off the last apply rather than
    /// the profile and the system that runs: a refusal after a staged apply
    /// (the profile still names what waits) must keep its plan, and a
    /// staged record whose system the profile no longer names (its trial
    /// failed) must not.
    #[test]
    fn the_next_boots_plan_is_the_waiting_systems() {
        let dir = std::env::temp_dir().join(format!("ronoctua-parts-next-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = |name: &str| {
            let top =
                format!("/nix/store/0123456789abcdfghijklmnpqrsvwx{name}-nixos-system-{name}");
            std::fs::create_dir_all(dir.join(top.trim_start_matches('/'))).unwrap();
            top
        };
        let (a, b) = (store("aa"), store("bb"));
        let link = |at: &str, to: &str| {
            let at = dir.join(at);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&at);
            std::os::unix::fs::symlink(dir.join(to.trim_start_matches('/')), at).unwrap();
        };
        link("run/current-system", &a);
        link("nix/var/nix/profiles/system", &b);
        let plan_of_b = dir.join(format!(
            "{}/etc/ronoctua/parts-plan.json",
            b.trim_start_matches('/')
        ));
        for state in [
            with(ApplyResult::Staged, Some(&b)),
            with(ApplyResult::Refused, None),
            with(ApplyResult::BuildFailed, None),
        ] {
            assert_eq!(next_plan(&dir, Some(&state)), Some(plan_of_b.clone()));
        }
        // THE PROFILE WENT BACK (the trial failed): nothing waits, whatever
        // the record says.
        link("nix/var/nix/profiles/system", &a);
        assert_eq!(
            next_plan(&dir, Some(&with(ApplyResult::Staged, Some(&b)))),
            None
        );
        // IT RUNS: nothing waits.
        link("nix/var/nix/profiles/system", &b);
        link("run/current-system", &b);
        assert_eq!(next_plan(&dir, None), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the roots of two plans one after the other do not end as
    /// the second's — a link per part at its derivation, the one it dropped
    /// gone, a moved derivation followed — or an empty set leaves anything.
    #[test]
    fn a_set_of_roots_is_replaced_whole() {
        let dir = std::env::temp_dir().join(format!("ronoctua-parts-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let set = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect()
        };
        let links = |dir: &Path| -> BTreeMap<String, String> {
            std::fs::read_dir(dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        std::fs::read_link(e.path()).unwrap().display().to_string(),
                    )
                })
                .collect()
        };
        let first = set(&[("a", "/nix/store/a1.drv"), ("b", "/nix/store/b1.drv")]);
        root_set(&dir, &first, |id| first.contains_key(id));
        assert_eq!(links(&dir), first);
        let second = set(&[("b", "/nix/store/b2.drv"), ("c", "/nix/store/c1.drv")]);
        root_set(&dir, &second, |id| second.contains_key(id));
        assert_eq!(links(&dir), second);
        root_set(&dir, &BTreeMap::new(), |_| false);
        assert!(links(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
