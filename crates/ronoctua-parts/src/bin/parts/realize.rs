//! `realize ID`: one part built and made its profile's next generation.
//!
//! The lock is held only to read and to write the record, never across the
//! build: a build of a heavy part takes an hour, and the planner and the
//! other realizers must still write theirs meanwhile. Two realizers of ONE
//! part never run at once — systemd starts one instance per name.

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::record::{Record, Target, Waiting, Why};

use crate::disk::{self, LOCK_WAIT, Lock};
use crate::{Usage, plan_path, program, say};

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
    let target = Target {
        drv: wanted.drv.clone(),
        out: wanted.out.clone(),
        interface: wanted.interface,
    };

    // THE RECORD AS THE TRY BEGINS: planned (a boot or a switch may have
    // changed what is wanted since the planner wrote it) and arriving.
    let begun = {
        let Ok(_lock) = Lock::take(LOCK_WAIT) else {
            println!("refused busy");
            return Ok(exit::BUSY);
        };
        let record = disk::read_record(id).unwrap_or_else(|_| Record::new(id));
        let record = record.planned(Some(target.clone()));
        if record.is_current(&target) {
            let _ = disk::write_record(&record);
            println!("here {id}");
            return Ok(exit::DONE);
        }
        // A METERED LINK downloads nothing by itself: the person's "try
        // now" is the yes for this one try, and it is taken as it is used.
        let yes = took_the_yes(id);
        if ronoctua_parts::record::waits_for_the_meter(metered(), yes) {
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

    let nix = program("RONOCTUA_NIX");
    let mut result = attempt(id, &target, &nix);
    // WHAT THE BUILD COULD NOT FETCH, asked of Nix's plan — and of the
    // person's mirrors, when they named some: what came is built again at
    // once, in this same try.
    let mut missing = Vec::new();
    if matches!(result, Err((Why::BuildFailed, _))) {
        missing = missing_downloads(&nix, &target.drv);
        let mirrors = crate::mirror::named();
        if !missing.is_empty()
            && !mirrors.is_empty()
            && crate::mirror::fetched(&nix, &missing, &mirrors) > 0
        {
            result = attempt(id, &target, &nix);
            missing = if matches!(result, Err((Why::BuildFailed, _))) {
                missing_downloads(&nix, &target.drv)
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

/// One build of the part and, when it gives the plan's output, its profile
/// set to it — or why not.
fn attempt(id: &str, target: &Target, nix: &str) -> Result<(), (Why, String)> {
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

/// The downloads the part still needs: the fixed-output derivations of what
/// a dry run of it says it would build (`ronoctua_parts::frontier`). Asked of
/// Nix's plan and never of the log; an answer it cannot read is no list.
fn missing_downloads(nix: &str, drv: &str) -> Vec<ronoctua_parts::frontier::Download> {
    let dry = disk::run(&[
        nix.to_owned(),
        "build".to_owned(),
        "--dry-run".to_owned(),
        format!("{drv}^out"),
    ]);
    let Some(will) = ronoctua_machine::dry::to_build(&dry.err) else {
        return Vec::new();
    };
    if will.is_empty() {
        return Vec::new();
    }
    let mut show = vec![nix.to_owned(), "derivation".to_owned(), "show".to_owned()];
    show.extend(will.iter().cloned());
    let shown = disk::run(&show);
    let Ok(shown) = serde_json::from_str::<serde_json::Value>(&shown.out) else {
        return Vec::new();
    };
    let nix_store = crate::beside_nix("nix-store");
    let outputs = will
        .iter()
        .filter_map(|d| {
            let said = disk::run(&[
                nix_store.clone(),
                "--query".to_owned(),
                "--outputs".to_owned(),
                d.clone(),
            ]);
            let out = said.out.lines().next()?.trim().to_owned();
            Some((d.clone(), out))
        })
        .collect();
    ronoctua_parts::frontier::downloads(&shown, &will, &outputs)
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
