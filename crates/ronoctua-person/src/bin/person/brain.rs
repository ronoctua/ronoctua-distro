//! `check-brain`: each brain module of the person's a machine would compile,
//! evaluated ALONE, in a `nix eval` of its own, before any evaluation of the
//! machine reads it — and what it came to written in
//! `local/brain-checked.json` ([`BrainChecked`]).
//!
//! # Why a process per folder
//!
//! A manifest that does not parse, aborts or recurses for ever is an error
//! `tryEval` cannot catch: read by the machine's evaluation, it stops all of
//! it, and the person's data would stop the machine's update. Read here, it
//! stops one `nix eval`, whose failure is the verdict; `lib.machine` reads a
//! folder only when this file says it evaluated at the hash it has now
//! (`nix/machine.nix`, `approvals`), and the report says why the others are
//! out.
//!
//! # Which folders
//!
//! The ones the machine would compile: no NixOS half, or one approved at the
//! hash the folder has now. A folder whose NixOS half waits for an approval
//! is hashed — the hash reads no manifest — and never evaluated, here or
//! there. A folder evaluated before at the same hash keeps its verdict: an
//! apply that changed nothing evaluates nothing.
//!
//! Run by `apply` after it swapped the person's data in, by `approve` after
//! it let a NixOS half in, and by itself for a check (`nix/person-brain-check.nix`
//! is what each `nix eval` asks).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ronoctua_person::local::{Approved, BrainChecked, Checked};
use ronoctua_person::tree::{BRAIN, Tree};
use ronoctua_person::{Document as _, names};

use crate::apply;
use crate::disk;
use crate::nix::Nix;

/// The longest one folder may take to evaluate: a manifest that recurses for
/// ever without an error is stopped, and does not evaluate.
pub const MOST: Duration = Duration::from_secs(120);

/// What runs the evaluations: the machine's `nix` and the bundle's
/// `check.nix` (`RONOCTUA_PERSON_BRAIN_CHECK`), each by store path.
#[derive(Debug, Clone)]
pub struct Tools {
    /// The `nix` that evaluates.
    pub nix: Nix,
    /// `nix/person-brain-check.nix`, at the top of its bundle.
    pub check: String,
}

/// What a check came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Done {
    /// The folders evaluated now (the others kept their verdict, or wait).
    pub evaluated: Vec<String>,
    /// The folders that do not evaluate, with what Nix said.
    pub refused: Vec<(String, String)>,
}

/// The brain modules the machine's copy holds.
fn modules(tree: &Tree) -> BTreeSet<String> {
    tree.dirs
        .iter()
        .filter_map(|d| d.strip_prefix(&format!("{BRAIN}/")).map(str::to_owned))
        .filter(|id| !id.contains('/'))
        .collect()
}

/// Checks the brain modules of the machine directory `machine` (the caller
/// holds root's lock) and writes what it found.
///
/// # Errors
///
/// A sentence: the machine directory or its copy not read, the hashes not
/// computed, or the file not written — and nothing is written then, so the
/// machine keeps leaving out what it left out.
pub fn check(machine: &str, tools: &Tools) -> Result<Done, String> {
    let machine_fd = apply::open_machine(machine).map_err(|r| r.to_string())?;
    let tree = apply::current(&machine_fd).map_err(|r| r.to_string())?;
    let before: BrainChecked = match disk::read_local(&machine_fd, names::BRAIN_CHECKED) {
        Ok(Some(bytes)) => ronoctua_person::parse(&bytes).unwrap_or_default(),
        Ok(None) | Err(_) => BrainChecked::default(),
    };
    let here = modules(&tree);
    let mut after = BrainChecked::default();
    let mut done = Done::default();
    if !here.is_empty() {
        let approved: Approved = match disk::read_local(&machine_fd, names::APPROVED) {
            Ok(Some(bytes)) => ronoctua_person::parse(&bytes).unwrap_or_default(),
            Ok(None) | Err(_) => Approved::default(),
        };
        let folder = std::fs::canonicalize(machine)
            .map_err(|e| format!("{machine}: {e}"))?
            .join(names::MACHINE_PERSON)
            .join(BRAIN);
        let hashes = hashes(tools, &folder)?;
        for id in here {
            // UNHASHABLE: the machine leaves it out, and says so itself.
            let Some(Some(hash)) = hashes.get(&id) else {
                continue;
            };
            // WAITING FOR AN APPROVAL: never evaluated.
            let root_half = tree.files.contains_key(&format!("{BRAIN}/{id}/module.nix"));
            if root_half && approved.modules.get(&id).map(|a| &a.hash) != Some(hash) {
                continue;
            }
            // THE SAME CONTENT, ALREADY EVALUATED: its verdict stands.
            if let Some(was) = before.modules.get(&id).filter(|c| c.hash == *hash) {
                after.modules.insert(id, was.clone());
                continue;
            }
            let checked = match evaluate(tools, &folder, &id) {
                Ok(()) => Checked {
                    hash: hash.clone(),
                    evaluates: true,
                    detail: String::new(),
                },
                Err(said) => {
                    done.refused.push((id.clone(), said.clone()));
                    Checked {
                        hash: hash.clone(),
                        evaluates: false,
                        detail: said,
                    }
                }
            };
            done.evaluated.push(id.clone());
            after.modules.insert(id, checked);
        }
    }
    after
        .validate()
        .map_err(|r| format!("the verdicts do not read back: {r}"))?;
    if after != before {
        disk::write_local(
            &machine_fd,
            names::BRAIN_CHECKED,
            ronoctua_person::to_json(&after).as_bytes(),
        )?;
    }
    Ok(done)
}

/// Every folder's hash (`nix/person-hash.nix`), `None` for one the hash
/// refuses — one `nix eval`, which reads no manifest.
fn hashes(tools: &Tools, folder: &Path) -> Result<BTreeMap<String, Option<String>>, String> {
    let said = run(
        tools,
        "hashes (builtins.getEnv \"RONOCTUA_BRAIN_DIR\")",
        folder,
        "",
    )
    .map_err(|e| format!("the brain modules could not be hashed: {e}"))?;
    serde_json::from_str(&said).map_err(|e| format!("the hashes do not read: {e}"))
}

/// One folder, evaluated alone: `Ok` when it evaluated, else what Nix said.
fn evaluate(tools: &Tools, folder: &Path, id: &str) -> Result<(), String> {
    run(
        tools,
        "verdict (builtins.getEnv \"RONOCTUA_BRAIN_DIR\") (builtins.getEnv \"RONOCTUA_BRAIN_ID\")",
        folder,
        id,
    )
    .map(|_| ())
}

/// `nix eval --json` of `(import <check>).<question>`, the folder and the id
/// in the environment — never spliced into the expression — waited for at
/// most [`MOST`]: what it printed, or its last words.
fn run(tools: &Tools, question: &str, folder: &Path, id: &str) -> Result<String, String> {
    let plain = tools.check.starts_with('/')
        && tools
            .check
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'+' | b'-'));
    if !plain {
        return Err(format!(
            "the check's path {:?} is not a plain path",
            tools.check
        ));
    }
    let expr = format!("(import {}).{question}", tools.check);
    let mut child = Command::new(&tools.nix.program)
        .args([
            "eval",
            "--json",
            "--impure",
            "--expr",
            &expr,
            "--extra-experimental-features",
            "nix-command",
        ])
        .env("RONOCTUA_BRAIN_DIR", folder)
        .env("RONOCTUA_BRAIN_ID", id)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("nix could not be started: {e}"))?;
    // BOTH PIPES DRAINED ON THREADS while this waits: a child that fills one
    // would otherwise wait for us as we wait for it.
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_string(&mut text);
            }
            text
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let until = Instant::now() + MOST;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    match status {
        Some(s) if s.success() => Ok(out),
        Some(_) => Err(last_words(&err)),
        None => Err(format!(
            "it did not finish evaluating in {} seconds",
            MOST.as_secs()
        )),
    }
}

/// The line of what Nix said that says what went wrong: its `error:` line,
/// else the last that says something.
fn last_words(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("warning:"))
        .collect();
    lines
        .iter()
        .rfind(|l| l.starts_with("error:"))
        .or_else(|| lines.last())
        .map_or_else(|| "nix said nothing".to_owned(), |l| (*l).to_owned())
}

/// The tools this environment names — `None` when it names none, and the
/// check is not run: the machine leaves the folders it cannot vouch for out.
pub fn tools_here(nix: &str, check: &str) -> Option<Tools> {
    let named = |v: &str| std::env::var(v).ok().filter(|p| p.starts_with('/'));
    Some(Tools {
        nix: Nix {
            program: PathBuf::from(named(nix)?),
        },
        check: named(check)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Scratch;

    /// A `nix` that answers the two questions as the folders say: each
    /// folder's hash is its file `hash` (none: unhashable), and a folder with
    /// a file `breaks` fails with its text as Nix's last words. Every
    /// evaluation is logged, so a test sees which folders were evaluated.
    fn fake(s: &Scratch) -> Tools {
        let program = s.program(
            "nix",
            &format!(
                "#!/bin/sh\n\
                 dir=\"$RONOCTUA_BRAIN_DIR\"\n\
                 case \"$*\" in\n\
                   *hashes*)\n\
                     printf '{{'; sep=''\n\
                     for f in \"$dir\"/*; do id=$(basename \"$f\")\n\
                       if [ -f \"$f/hash\" ]; then printf '%s\"%s\":\"%s\"' \"$sep\" \"$id\" \"$(cat \"$f/hash\")\"\n\
                       else printf '%s\"%s\":null' \"$sep\" \"$id\"; fi; sep=','\n\
                     done; printf '}}' ;;\n\
                   *verdict*)\n\
                     echo \"$RONOCTUA_BRAIN_ID\" >> {log}\n\
                     if [ -f \"$dir/$RONOCTUA_BRAIN_ID/breaks\" ]; then\n\
                       echo 'warning: something' >&2; cat \"$dir/$RONOCTUA_BRAIN_ID/breaks\" >&2; echo '       at x' >&2; exit 1\n\
                     fi\n\
                     echo '[]' ;;\n\
                   *) exit 2 ;;\n\
                 esac\n",
                log = s.0.join("evaluated").display()
            ),
        );
        Tools {
            nix: Nix { program },
            check: "/nix/store/0000000000000000000000000000000a-check/check.nix".to_owned(),
        }
    }

    fn evaluated(s: &Scratch) -> Vec<String> {
        let log = std::fs::read_to_string(s.0.join("evaluated")).unwrap_or_default();
        let _ = std::fs::remove_file(s.0.join("evaluated"));
        log.lines().map(str::to_owned).collect()
    }

    fn written(s: &Scratch) -> BrainChecked {
        ronoctua_person::parse(
            &std::fs::read(s.0.join("machine/local/brain-checked.json")).unwrap(),
        )
        .unwrap()
    }

    /// FAILS IF: a folder the machine would compile is not evaluated alone
    /// and recorded at its hash — evaluating, or not with Nix's own error
    /// line —; a folder whose NixOS half waits for an approval is evaluated
    /// at all; an unhashable one is recorded; or a folder evaluated before at
    /// the same hash is evaluated again (and one changed since is not).
    #[test]
    fn each_folder_the_machine_would_compile_is_evaluated_alone() {
        let s = Scratch::new("brain-check");
        let tools = fake(&s);
        let module = |id: &str, hash: &str| {
            s.file(&format!("machine/person/brain/{id}/manifest.nix"), b"{ }");
            s.file(&format!("machine/person/brain/{id}/hash"), hash.as_bytes());
        };
        module("my-plain", "sha256-plain");
        module("my-root", "sha256-root");
        s.file("machine/person/brain/my-root/module.nix", b"{ }");
        module("my-waiting", "sha256-waiting");
        s.file("machine/person/brain/my-waiting/module.nix", b"{ }");
        s.file(
            "machine/person/brain/my-waiting/breaks",
            b"error: a waiting module was evaluated\n",
        );
        module("my-unparsed", "sha256-unparsed");
        s.file(
            "machine/person/brain/my-unparsed/breaks",
            b"error: syntax error, unexpected ';'\n",
        );
        s.file("machine/person/brain/my-unhashable/manifest.nix", b"{ }");
        s.file(
            "machine/local/approved.json",
            br#"{"format":1,"modules":{"my-root":{"hash":"sha256-root","approved-at":"2026-11-01T12:00:00Z"},"my-waiting":{"hash":"sha256-older","approved-at":"2026-11-01T12:00:00Z"}}}"#,
        );
        let machine = s.0.join("machine");
        let done = check(machine.to_str().unwrap(), &tools).unwrap();
        let mut ran = evaluated(&s);
        ran.sort();
        assert_eq!(ran, ["my-plain", "my-root", "my-unparsed"], "{done:?}");
        let got = written(&s);
        assert_eq!(
            got.modules.keys().collect::<Vec<_>>(),
            ["my-plain", "my-root", "my-unparsed"]
        );
        assert!(got.modules["my-plain"].evaluates && got.modules["my-root"].evaluates);
        let unparsed = &got.modules["my-unparsed"];
        assert!(!unparsed.evaluates);
        assert_eq!(unparsed.hash, "sha256-unparsed");
        assert_eq!(unparsed.detail, "error: syntax error, unexpected ';'");
        assert_eq!(
            done.refused,
            [(
                "my-unparsed".to_owned(),
                "error: syntax error, unexpected ';'".to_owned()
            )]
        );
        // AGAIN, NOTHING CHANGED: nothing evaluated, nothing written.
        let stamp = std::fs::metadata(s.0.join("machine/local/brain-checked.json"))
            .unwrap()
            .modified()
            .unwrap();
        check(machine.to_str().unwrap(), &tools).unwrap();
        assert!(
            evaluated(&s).is_empty(),
            "an unchanged folder was evaluated again"
        );
        assert_eq!(
            std::fs::metadata(s.0.join("machine/local/brain-checked.json"))
                .unwrap()
                .modified()
                .unwrap(),
            stamp
        );
        // FIXED: a new hash is evaluated again, and the verdict follows it.
        s.file("machine/person/brain/my-unparsed/hash", b"sha256-fixed");
        std::fs::remove_file(s.0.join("machine/person/brain/my-unparsed/breaks")).unwrap();
        check(machine.to_str().unwrap(), &tools).unwrap();
        assert_eq!(evaluated(&s), ["my-unparsed"]);
        let fixed = &written(&s).modules["my-unparsed"];
        assert!(fixed.evaluates && fixed.hash == "sha256-fixed" && fixed.detail.is_empty());
        // AND A FOLDER THAT LEFT takes its verdict with it.
        std::fs::remove_dir_all(s.0.join("machine/person/brain/my-plain")).unwrap();
        check(machine.to_str().unwrap(), &tools).unwrap();
        assert!(!written(&s).modules.contains_key("my-plain"));
    }

    /// FAILS IF: a `nix` that cannot hash the folders leaves anything
    /// written, or one that never finishes is waited for past [`MOST`]
    /// instead of read as not evaluating (proved on its words alone).
    #[test]
    fn a_check_that_cannot_hash_writes_nothing() {
        let s = Scratch::new("brain-check-broken");
        s.file("machine/person/brain/my-plain/manifest.nix", b"{ }");
        let tools = Tools {
            nix: Nix {
                program: s.program("nix", "#!/bin/sh\necho 'error: no store' >&2\nexit 1\n"),
            },
            check: "/nix/store/0000000000000000000000000000000a-check/check.nix".to_owned(),
        };
        let machine = s.0.join("machine");
        let e = check(machine.to_str().unwrap(), &tools).unwrap_err();
        assert!(e.contains("error: no store"), "{e}");
        assert!(!s.0.join("machine/local/brain-checked.json").exists());
        assert_eq!(
            last_words("warning: x\nerror: evaluation aborted\n       at y\n"),
            "error: evaluation aborted"
        );
        let bad = Tools {
            check: "/nix/store/a;b.nix".to_owned(),
            ..tools
        };
        assert!(run(&bad, "hashes", Path::new("/"), "").is_err());
    }
}
