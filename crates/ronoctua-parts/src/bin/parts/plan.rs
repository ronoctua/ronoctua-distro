//! `plan [--start]`: the running system's plan brought into the records.
//!
//! Every wanted part gets a record saying what is wanted; a part the plan
//! dropped is recorded as unwanted (its profile stays until trimmed, so a
//! rollback finds it). The plan's derivations are rooted, the view rebuilt,
//! and with `--start` the parts that are not here go to their realizers.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::record::{Phase, Record, Target};

use crate::disk::{self, LOCK_WAIT, Lock};
use crate::{Usage, plan_path, program, say};

/// How many parts are realized at once unless `--workers` says otherwise:
/// two downloads share a link well, and two builds fit the smallest machine
/// the distro installs on (`nix/build-sizing.nix` sizes each `nix build`).
const WORKERS: usize = 2;

pub fn run(args: &[String]) -> Result<u8, Usage> {
    let start = args.iter().any(|a| a == "--start");
    let rehearse = args.iter().any(|a| a == "--rehearse");
    if start && rehearse {
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
    let plan = match disk::read_plan(&plan_path(args)?) {
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
                let target = Target {
                    drv: wanted.drv.clone(),
                    out: wanted.out.clone(),
                    interface: wanted.interface,
                };
                !known.get(*id).is_some_and(|r| r.is_current(&target))
            })
            .count();
        println!(
            "would plan {} parts, {missing} to realize",
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
            let target = Target {
                drv: wanted.drv.clone(),
                out: wanted.out.clone(),
                interface: wanted.interface,
            };
            let record = known.remove(id).unwrap_or_else(|| Record::new(id));
            write(&record.planned(Some(target)));
        }
        for (_, record) in known {
            if record.wanted.is_some() {
                write(&record.planned(None));
            }
        }
        root_derivations(&plan);
        plan.order()
            .into_iter()
            .filter(|(id, _)| {
                disk::read_record(id).is_ok_and(|r| {
                    matches!(r.phase, Phase::Arriving | Phase::NeedsYou | Phase::Waiting)
                })
            })
            .map(|(id, _)| id.to_owned())
            .collect::<Vec<String>>()
    };
    crate::view::rebuild();

    if !start || to_realize.is_empty() {
        println!(
            "planned {} parts, {} to realize",
            plan.parts.len(),
            to_realize.len()
        );
        return Ok(exit::DONE);
    }

    // THE WORKERS: each takes the next part and waits for its realizer to
    // end (`systemctl start` of a oneshot returns when it is over), so at
    // most `workers` parts build at once. A realizer that fails is the
    // unit's to retry, later each time; the planner does not wait for that.
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
    println!("planned {} parts", plan.parts.len());
    Ok(exit::DONE)
}

fn write(record: &Record) {
    if let Err(e) = disk::write_record(record) {
        say(3, &format!("{}: {e}", record.id));
    }
}

/// Every wanted part's derivation rooted under [`names::PLAN_ROOTS`] — a
/// link per part, replaced when the plan changes, removed when the part is
/// no longer wanted. A link in the collector's roots is all a root is.
fn root_derivations(plan: &ronoctua_parts::plan::Plan) {
    let roots = Path::new(names::PLAN_ROOTS);
    if let Err(e) = disk::make_dir(roots, 0o755) {
        say(4, &format!("the plan's roots: {e}"));
        return;
    }
    if let Ok(entries) = std::fs::read_dir(roots) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let keep = name.to_str().is_some_and(|n| plan.parts.contains_key(n));
            if !keep {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    for (id, wanted) in &plan.parts {
        let link = roots.join(id);
        if std::fs::read_link(&link).ok().as_deref() == Some(Path::new(&wanted.drv)) {
            continue;
        }
        let draft = roots.join(format!(".{id}.{}", std::process::id()));
        let _ = std::fs::remove_file(&draft);
        let made = std::os::unix::fs::symlink(&wanted.drv, &draft)
            .and_then(|()| std::fs::rename(&draft, &link));
        if let Err(e) = made {
            say(4, &format!("{id}: its derivation was not rooted: {e}"));
        }
    }
}
