//! `realize ID`: one part built and made its profile's next generation.
//!
//! The lock is held only to read and to write the record, never across the
//! build: a build of a heavy part takes an hour, and the planner and the
//! other realizers must still write theirs meanwhile. Two realizers of ONE
//! part never run at once — systemd starts one instance per name.

use std::path::Path;

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::record::{Record, Target, Waiting, Why};

use crate::disk::{self, LOCK_WAIT, Lock};
use crate::{Usage, plan_path, program, say};

/// The machine directory, baked by the build (`nix/machine.nix`'s one name):
/// where the person's mirrors are named.
const MACHINE_DIR: Option<&str> = option_env!("RONOCTUA_MACHINE_DIR");

pub fn run(args: &[String]) -> Result<u8, Usage> {
    let Some((id, flags)) = args.split_first() else {
        return Err(Usage);
    };
    if !ronoctua_parts::id::is_id(id) {
        return Err(Usage);
    }
    let plan = match disk::read_plan(&plan_path(flags)?) {
        Ok(plan) => plan,
        Err(e) => {
            say(3, &format!("{id}: the plan does not read: {e}"));
            println!("refused plan-unreadable");
            return Ok(exit::FAILED);
        }
    };
    let Some(wanted) = plan.parts.get(id.as_str()) else {
        println!("refused unwanted");
        return Ok(exit::UNWANTED);
    };
    let nix = program("RONOCTUA_NIX");
    // A PART OF THE PERSON'S: its target is what this realizer evaluated of
    // the declaration the plan wants — kept in its record while that
    // declaration stays the same, evaluated now, alone, otherwise.
    let target = match (wanted.target(), &wanted.entry) {
        (Some(target), _) => target,
        (None, Some(entry)) => {
            let kept = disk::read_record(id)
                .ok()
                .filter(|r| r.declared.as_deref() == Some(entry.fingerprint.as_str()))
                .and_then(|r| r.wanted);
            match kept {
                Some(target) => target,
                None => match crate::person::evaluate(&nix, entry, wanted.interface) {
                    Ok(target) => {
                        crate::plan::root_one(id, &target.drv);
                        target
                    }
                    Err(said) => return Ok(not_evaluated(id, &entry.fingerprint, &said)),
                },
            }
        }
        (None, None) => {
            println!("refused unwanted");
            return Ok(exit::UNWANTED);
        }
    };
    // What the record says the plan wants of this part.
    let planned = |record: Record| match &wanted.entry {
        Some(entry) => record
            .planned_entry(&entry.fingerprint)
            .resolved(target.clone()),
        None => record.planned(Some(target.clone())),
    };

    // THE RECORD AS THE TRY BEGINS: planned (a boot or a switch may have
    // changed what is wanted since the planner wrote it) and arriving.
    let begun = {
        let Ok(_lock) = Lock::take(LOCK_WAIT) else {
            println!("refused busy");
            return Ok(exit::BUSY);
        };
        let record = planned(disk::read_record(id).unwrap_or_else(|_| Record::new(id)));
        if record.is_current(&target) {
            let _ = disk::write_record(&record);
            println!("here {id}");
            return Ok(exit::DONE);
        }
        // A METERED LINK downloads nothing by itself: the person's "try
        // now" is the yes for this one try, and it is taken as it is used.
        // An output already in the store downloads nothing at all.
        let yes = took_the_yes(id);
        if !in_the_store(&target.out) && ronoctua_parts::record::waits_for_the_meter(metered(), yes)
        {
            let record = record.waits(Waiting::Metered);
            if let Err(e) = disk::write_record(&record) {
                say(3, &format!("{id}: {e}"));
            }
            println!("waiting metered");
            return Ok(exit::METERED);
        }
        let record = record.trying();
        if let Err(e) = disk::write_record(&record) {
            say(3, &format!("{id}: {e}"));
        }
        record
    };

    let mut result = attempt(id, &target, &nix);
    // WHAT THE BUILD COULD NOT FETCH, asked of Nix's plan — and of the
    // person's mirrors, when they named some: what came is built again at
    // once, in this same try.
    let mut missing = Vec::new();
    if matches!(result, Err((Why::BuildFailed, _))) {
        missing = ronoctua_machine::dry::missing(Path::new(&nix), &target.drv);
        let mirrors = MACHINE_DIR.map_or_else(Vec::new, |dir| {
            ronoctua_machine::mirror::named(Path::new(dir))
        });
        if !missing.is_empty()
            && !mirrors.is_empty()
            && ronoctua_machine::mirror::fetched(Path::new(&nix), &missing, &mirrors) > 0
        {
            result = attempt(id, &target, &nix);
            missing = if matches!(result, Err((Why::BuildFailed, _))) {
                ronoctua_machine::dry::missing(Path::new(&nix), &target.drv)
            } else {
                Vec::new()
            };
        }
    }

    // THE RECORD AS THE TRY ENDS. A failure with no link at all is not
    // counted and not retried on a clock: the network coming up starts the
    // planner again (`nix/modules/parts.nix`, the dispatcher).
    let Ok(_lock) = Lock::take(LOCK_WAIT) else {
        println!("refused busy");
        return Ok(exit::BUSY);
    };
    let record = disk::read_record(id).unwrap_or(begun);
    match result {
        Ok(()) => {
            let record = record.arrived(disk::now());
            if let Err(e) = disk::write_record(&record) {
                say(3, &format!("{id}: {e}"));
            }
            drop(_lock);
            crate::keep::downloads(id, &target.drv);
            crate::view::rebuild();
            println!("arrived {id}");
            Ok(exit::DONE)
        }
        Err((why, detail)) => {
            let linked = linked();
            let record = record
                .failed(why, &detail, disk::now(), linked)
                .missing(missing);
            if let Err(e) = disk::write_record(&record) {
                say(3, &format!("{id}: {e}"));
            }
            say(
                3,
                &format!("{id}: {} ({} tries)", why.id(), record.attempts),
            );
            drop(_lock);
            // THE NEXT TRY, on the clock the record now gives — whoever
            // started this one (the planner, the person).
            crate::retry::arm();
            if linked {
                println!("failed {}", why.id());
                Ok(exit::FAILED)
            } else {
                println!("waiting network");
                Ok(exit::OFFLINE)
            }
        }
    }
}

/// The record of a part of the person's whose declaration did not evaluate
/// — `said`, Nix's words or its build's refusal —, a counted try: tried
/// again ever later, and by the person's "try now". The exit code to end
/// with.
fn not_evaluated(id: &str, fingerprint: &str, said: &str) -> u8 {
    let Ok(_lock) = Lock::take(LOCK_WAIT) else {
        println!("refused busy");
        return exit::BUSY;
    };
    let record = disk::read_record(id)
        .unwrap_or_else(|_| Record::new(id))
        .planned_entry(fingerprint)
        .failed(Why::EvalFailed, said, disk::now(), true);
    if let Err(e) = disk::write_record(&record) {
        say(3, &format!("{id}: {e}"));
    }
    say(
        3,
        &format!(
            "{id}: its declaration did not evaluate: {said} ({} tries)",
            record.attempts
        ),
    );
    drop(_lock);
    crate::retry::arm();
    println!("failed {}", Why::EvalFailed.id());
    exit::FAILED
}

/// One build of the part and, when it gives the plan's output, its profile
/// set to it — or why not.
fn attempt(id: &str, target: &Target, nix: &str) -> Result<(), (Why, String)> {
    // AN OUTPUT ALREADY IN THE STORE IS SEEDED, never built: what an ISO or
    // a test machine carries arrives with no derivation and no network —
    // the same path is the same bytes.
    if in_the_store(&target.out) {
        say(6, &format!("{id}: {} is in the store already", target.out));
    } else {
        say(6, &format!("{id}: building {}", target.out));
        let built = disk::run(&ronoctua_parts::argv::build(nix, &target.drv));
        if !built.ok {
            return Err((Why::BuildFailed, built.err));
        }
        if !built
            .out
            .lines()
            .map(str::trim)
            .any(|line| line == target.out)
        {
            return Err((
                Why::OutputDiffers,
                format!("built {}, the plan wants {}", built.out.trim(), target.out),
            ));
        }
    }
    set_the_profile(id, target)
}

/// Whether `out` is a valid path of the store, as Nix says it.
fn in_the_store(out: &str) -> bool {
    ronoctua_parts::plan::is_store_path(out)
        && disk::run_quiet(&[
            crate::beside_nix("nix-store"),
            "--check-validity".to_owned(),
            out.to_owned(),
        ])
}

/// The part's profile set to the plan's output, and trimmed to two
/// generations.
fn set_the_profile(id: &str, target: &Target) -> Result<(), (Why, String)> {
    let profile = names::profile(id, target.interface);
    let profile_text = profile.to_string_lossy().into_owned();
    let parent = profile.parent().map(std::path::Path::to_path_buf);
    let made = parent.map_or(Ok(()), |p| disk::make_dir(&p, 0o755));
    let set = made.is_ok()
        && disk::run_quiet(&ronoctua_parts::argv::set_profile(
            &crate::nix_env(),
            &profile_text,
            &target.out,
        ));
    if !set {
        return Err((
            Why::ProfileFailed,
            format!("{profile_text} was not set to {}", target.out),
        ));
    }
    let _ = disk::run_quiet(&ronoctua_parts::argv::trim_profile(
        &crate::nix_env(),
        &profile_text,
    ));
    Ok(())
}

/// Whether the person said yes to this part's try on a metered link: a plain
/// file `yes/<id>` in their folder (never a link), taken away here, so it
/// lets one try through.
fn took_the_yes(id: &str) -> bool {
    let path = std::path::Path::new(names::YES).join(id);
    let plain = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file());
    let _ = std::fs::remove_file(&path);
    plain
}

/// Whether the link out is metered, as NetworkManager says — `None` when it
/// does not answer, or does not know.
fn metered() -> Option<bool> {
    ronoctua_network::Manager::fresh().ok()?.metered()
}

/// Whether the machine has a link at all. NetworkManager that does not
/// answer is read as linked: a failure is then counted and retried on the
/// clock, which is the safe side — never a part waiting for a network event
/// that will not come. A proxy is made whether the daemon is there or not,
/// so its presence is asked first, of a property every NetworkManager has.
fn linked() -> bool {
    let Ok(manager) = ronoctua_network::Manager::fresh() else {
        return true;
    };
    if manager.radio_on().is_err() {
        return true;
    }
    manager.look().linked()
}
