//! `ronoctua-first-boot`: the person's belongings, finished on the machine
//! once it is up — run as root by its unit (`nix/modules/first-boot.nix`),
//! which tries again ever later until it is done. `--plan RECORD --empty
//! yes|no` prints what it would run for that record, and runs nothing. See
//! `ronoctua_installer::first_boot`.

use std::process::ExitCode;

const USAGE: &str =
    "usage: ronoctua-first-boot\n       ronoctua-first-boot --plan RECORD --empty yes|no";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => ExitCode::from(ronoctua_installer::first_boot::run(&mut |line| {
            eprintln!("ronoctua-first-boot: {line}");
        })),
        [plan, record, empty, answer] if plan == "--plan" && empty == "--empty" => {
            let empty = match answer.as_str() {
                "yes" => true,
                "no" => false,
                _ => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            };
            let read = std::fs::read_to_string(record)
                .map_err(anyhow::Error::from)
                .and_then(|text| ronoctua_installer::first_boot::Record::from_file(&text));
            match read {
                Ok(r) => {
                    print!("{}", ronoctua_installer::first_boot::printed(&r, empty));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("ronoctua-first-boot: {record}: {e:#}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
