//! `ronoctua-machine` — the machine's life, as root.
//!
//! ```text
//! ronoctua-machine migrate
//! ronoctua-machine apply [--rehearse | --next-boot]
//! ronoctua-machine bless
//! ronoctua-machine rollback switch|boot
//! ronoctua-machine disarm
//! ronoctua-machine hibernate pre|post
//! ronoctua-machine hold <rev>
//! ronoctua-machine unhold
//! ronoctua-machine keep-sources
//! ronoctua-machine update-distro
//! ```
//!
//! What it runs and where it looks comes from the environment the units set
//! (`nix/modules/machine.nix`): `RONOCTUA_MACHINE_DIR`, `…_STATE`, `…_RUN`,
//! `…_ATTR`, `…_MENU_LIMIT`, `…_TEMPLATE`, `…_SYSTEM_LOCK`, `RONOCTUA_NIX`,
//! `RONOCTUA_BOOTCTL`. The last line on standard output is the verdict;
//! every step goes to standard error with the journal's priority prefix.
//! Exit 0 is done, 1 refused (`refused <why>`), 2 a command line that is
//! not one of the above.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use ronoctua_machine::{Env, Machine, Places, System, Verb};

const USAGE: &str = "usage: ronoctua-machine migrate\n\
\x20      ronoctua-machine apply [--rehearse | --next-boot]\n\
\x20      ronoctua-machine bless\n\
\x20      ronoctua-machine rollback switch|boot\n\
\x20      ronoctua-machine disarm\n\
\x20      ronoctua-machine hibernate pre|post\n\
\x20      ronoctua-machine hold <rev>      (7 to 64 lower-case hex digits)\n\
\x20      ronoctua-machine unhold\n\
\x20      ronoctua-machine keep-sources\n\
\x20      ronoctua-machine update-distro\n\
\n\
migrate   the old layout (a copy of the distro) becomes the machine's own flake,\n\
\x20         only if the new one builds; the old one is kept in legacy-<stamp>\n\
apply     build the machine directory, switch to it and arm the boot trial;\n\
\x20         --rehearse only measures, on a copy, after moving the distro input;\n\
\x20         --next-boot builds and sets the new system for the next boot,\n\
\x20         activating nothing, with the boot trial armed (so does any apply\n\
\x20         while a system waits for the next boot, or across a moved nixpkgs)\n\
bless     after a boot: the trial that booted is the last good system, and a\n\
\x20         trial that did not puts the machine back on the one that did\n\
rollback  the rescue's way back: to the system that runs when another waits\n\
\x20         for the next boot, else to the generation before; switch now, or\n\
\x20         boot from the next boot; the trial it leaves is disarmed\n\
disarm    after the system profile was rolled back by hand: the trial it left\n\
\x20         is not tried again, and the boot menu alone names the next boot\n\
hibernate around a hibernation (the system-sleep hook): pre puts the boot\n\
\x20         trial's one-shot aside, post puts it back after the resume\n\
hold      stay on the distro revision this machine runs\n\
unhold    stop holding\n\
keep-sources  root the sources the machine's flake was evaluated from (the flake,\n\
\x20         the distro, every input) that the store holds, so an update fetches\n\
\x20         none of them again unchanged; the apply does it after every build\n\
update-distro  move the distro and nixpkgs inputs forward (the update's first\n\
\x20         root step), an aimed machine put back on the release first";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.as_slice(), [a] if a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(verb) = Verb::parse(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let env = Env::of_process();
    let places = match Places::from_env(&env) {
        Ok(places) => places,
        Err(missing) => {
            eprintln!("<3>{missing}: the unit that runs this names it");
            println!("refused failed");
            return ExitCode::from(1);
        }
    };
    let machine = Machine {
        places,
        system: System::from_env(&env),
    };
    match machine.run(&verb) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            println!("refused {refusal}");
            ExitCode::from(1)
        }
    }
}
