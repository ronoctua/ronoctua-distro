//! `realize ID`: one part built and made its profile's next generation.
//!
//! The lock is held only to read and to write the record, never across the
//! build: a build of a heavy part takes an hour, and the planner and the
//! other realizers must still write theirs meanwhile. Two realizers of ONE
//! part never run at once — systemd starts one instance per name.

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::record::{Record, Target, Why};

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
        let record = record.trying();
        if let Err(e) = disk::write_record(&record) {
            say(3, &format!("{id}: {e}"));
        }
        record
    };

    let nix = program("RONOCTUA_NIX");
    say(6, &format!("{id}: building {}", target.out));
    let built = disk::run(&ronoctua_parts::argv::build(&nix, &target.drv));
    let result = if !built.ok {
        Err((Why::BuildFailed, built.err))
    } else if built
        .out
        .lines()
        .map(str::trim)
        .any(|line| line == target.out)
    {
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
        if set {
            let _ = disk::run_quiet(&ronoctua_parts::argv::trim_profile(
                &crate::nix_env(),
                &profile_text,
            ));
            Ok(())
        } else {
            Err((
                Why::ProfileFailed,
                format!("{profile_text} was not set to {}", target.out),
            ))
        }
    } else {
        Err((
            Why::OutputDiffers,
            format!("built {}, the plan wants {}", built.out.trim(), target.out),
        ))
    };

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
            crate::view::rebuild();
            println!("arrived {id}");
            Ok(exit::DONE)
        }
        Err((why, detail)) => {
            let linked = linked();
            let record = record.failed(why, &detail, disk::now(), linked);
            if let Err(e) = disk::write_record(&record) {
                say(3, &format!("{id}: {e}"));
            }
            say(
                3,
                &format!("{id}: {} ({} tries)", why.id(), record.attempts),
            );
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
