//! Installing the person's extensions that are missing here — VS Code's
//! own `code --install-extension`, one id at a time.
//!
//! # Why `code`, and why one at a time
//!
//! The Marketplace, its signatures and the extensions' dependencies are VS
//! Code's to know; `code` is how VS Code installs, and it is told the folder
//! the manifest is read from (`--extensions-dir`), so what this reads after
//! is what it wrote. One id a run: a failure is that id's, and the others
//! still enter — the report says which entered, and which failed with what
//! `code` said of it ([`what_code_said`]). A pre-release is asked as one
//! (`--pre-release`).
//!
//! # Consent, minted here
//!
//! `code` reaches the Marketplace's hosts ([`crate::hosts_of`]). A
//! [`ronoctua_consent::Pass`] cannot cross into it, so the install mints one
//! for every host it would reach, at the moment it asks — the person's
//! `consent.json` (`ronoctua_consent::pass` in the product; a test's own
//! file in the tests) — and with one host not let it asks nothing and says
//! which ([`crate::Why::NotLet`]).

use std::path::Path;
use std::process::{Command, Stdio};

use ronoctua_consent::Pass;

use crate::{EXTENSIONS_IN_HOME, Refusal, Why};

/// One extension that did not enter, and what `code` said of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// Which.
    pub id: String,
    /// What `code` said of it ([`what_code_said`]; data: VS Code's own
    /// words).
    pub said: String,
}

/// What an install did, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// What `code` installed, in the order tried.
    pub entered: Vec<String>,
    /// What `code` could not install, in the order tried.
    pub failed: Vec<Failure>,
}

impl Installed {
    /// Whether every one entered (none missing included).
    #[must_use]
    pub fn whole(&self) -> bool {
        self.failed.is_empty()
    }
}

/// Where an install is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<'a> {
    /// Trying `id`, the `done + 1`-th of `total`.
    Trying {
        /// How many were tried before.
        done: usize,
        /// Of how many.
        total: usize,
        /// Which.
        id: &'a str,
    },
    /// `id` entered.
    Entered(&'a str),
    /// `id` did not, and what `code` said of it.
    Failed(&'a str, &'a str),
}

/// The hosts of `hosts` that `minter` refuses a pass for, in order. Pure
/// but for the minter.
#[must_use]
pub fn not_let(hosts: &[String], minter: &dyn Fn(&str) -> Option<Pass>) -> Vec<String> {
    hosts
        .iter()
        .filter(|h| minter(h).is_none())
        .cloned()
        .collect()
}

/// Installs, with `code`, every extension of the person's list under `home`
/// that is not installed here ([`crate::missing`]), one at a time, saying
/// where it is to `event` — once `minter` gave a pass for every host the
/// install reaches. BLOCKS: each `code` runs to its end (network).
///
/// # Errors
///
/// [`Why::NoCode`] when `code` is not there; [`Why::NotLet`] with the hosts
/// not let (nothing was asked); a list or a manifest that does not read
/// ([`crate::missing`]). A failed id is not an error: it is in
/// [`Installed::failed`].
pub fn install(
    code: &Path,
    home: &Path,
    minter: &dyn Fn(&str) -> Option<Pass>,
    event: &mut dyn FnMut(Event<'_>),
) -> Result<Installed, Refusal> {
    if !code.is_file() {
        return Err(Refusal::about(Why::NoCode, code));
    }
    let missing = crate::missing(home)?;
    if missing.ids.is_empty() {
        return Ok(Installed::default());
    }
    // CONSENT BEFORE ANYTHING IS ASKED: a pass for every host, or nothing.
    let hosts = crate::hosts_of(missing.ids.iter().map(String::as_str));
    let refused = not_let(&hosts, minter);
    if !refused.is_empty() {
        return Err(Refusal {
            why: Why::NotLet,
            at: None,
            hosts: refused,
        });
    }
    let mut out = Installed::default();
    let total = missing.ids.len();
    for (done, id) in missing.ids.iter().enumerate() {
        event(Event::Trying { done, total, id });
        match run(code, home, id, missing.pre_release.contains(id)) {
            Ok(()) => {
                event(Event::Entered(id));
                out.entered.push(id.clone());
            }
            Err(said) => {
                event(Event::Failed(id, &said));
                out.failed.push(Failure {
                    id: id.clone(),
                    said,
                });
            }
        }
    }
    Ok(out)
}

/// `code --extensions-dir <home>/.vscode/extensions --install-extension
/// <id> [--pre-release]`, to its end — what it said when it failed
/// ([`what_code_said`]). Its output is read, never passed on: the
/// belongings' CLI that runs this speaks to the queue on its own standard
/// output.
fn run(code: &Path, home: &Path, id: &str, pre_release: bool) -> Result<(), String> {
    // AN ID IS NEVER AN OPTION NOR A FILE to `code`: the list's reader
    // refused anything else, and this says so again where it matters.
    if !crate::is_extension_id(id) {
        return Err(format!("{id:?} is not an extension's id"));
    }
    let mut command = Command::new(code);
    command
        .arg("--extensions-dir")
        .arg(home.join(EXTENSIONS_IN_HOME))
        .arg("--install-extension")
        .arg(id);
    if pre_release {
        command.arg("--pre-release");
    }
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run {}: {e}", code.display()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(what_code_said(id, &output.stderr, &output.stdout)
        .unwrap_or_else(|| format!("code exited with {}", output.status)))
}

/// The line of a failed `code --install-extension <id>` that says why: the
/// first line of its errors that names the id, else the first line of its
/// errors, else the last line of its output. `None` when it said nothing.
///
/// # Why not the last line
///
/// Measured against the pinned 1.119: `code` says the cause first and ends
/// every failure with a summary that only repeats the id ("Failed Installing
/// Extensions: <id>"), after a hint for a mistyped id. The cause names the
/// id ("Extension '<id>' not found.", "Error while installing extension
/// <id>: Signature verification failed …") — or, when nothing was reached,
/// it is the first error ("Error while installing extensions: connect
/// ECONNREFUSED …"). The id is compared as VS Code compares it, ignoring
/// case.
#[must_use]
pub fn what_code_said(id: &str, stderr: &[u8], stdout: &[u8]) -> Option<String> {
    let lines = |bytes: &[u8]| -> Vec<String> {
        String::from_utf8_lossy(bytes)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let errors = lines(stderr);
    let id = id.to_ascii_lowercase();
    errors
        .iter()
        .find(|l| l.to_ascii_lowercase().contains(&id))
        .or_else(|| errors.first())
        .cloned()
        .or_else(|| lines(stdout).pop())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{manifest, scratch};
    use crate::{Extensions, list_bytes, list_path};

    /// A program of the test's, written by a `/bin/sh` child — never by this
    /// process, whose open file a `fork` on another test's thread would
    /// keep, so running it would be `ETXTBSY` at random.
    fn write_executable(at: &Path, script: &str) {
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg("printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"")
            .arg("sh")
            .arg(at)
            .arg(script)
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// The consent a test hands in: these hosts let, and no other.
    fn letting(dir: &Path, hosts: &[&str]) -> ronoctua_store::Store<ronoctua_consent::Consents> {
        let store = ronoctua_consent::store_at(Some(dir.join("consent")));
        ronoctua_consent::let_hosts_in(&store, hosts, true).unwrap();
        store
    }

    /// FAILS IF: an id already installed is asked of `code` again; the ids
    /// are not asked one at a time into the folder the manifest is read
    /// from, a pre-release as one; a failure stops the others; the report
    /// does not say which entered and which failed with the line `code` said
    /// of it (its cause, never its closing summary); anything is asked with one host not let (and the refusal does
    /// not name it); or an absent `code` is not refused.
    #[test]
    fn each_missing_id_is_asked_once_with_consent_and_reported() {
        let home = scratch("install");
        let list = list_path(&home);
        std::fs::create_dir_all(list.parent().unwrap()).unwrap();
        let ids = ["a.x", "bad.one", "c.z"];
        std::fs::write(
            &list,
            list_bytes(&Extensions::of(
                ids.iter().map(|s| (*s).to_owned()).collect(),
                ["a.x".to_owned()].into_iter().collect(),
            )),
        )
        .unwrap();
        manifest(&home, &[("c.z", false)]);
        let calls = home.join("calls");
        let code = home.join("code");
        write_executable(
            &code,
            &format!(
                "#!/bin/sh\necho \"$*\" >> {}\n[ \"$4\" = bad.one ] && {{ echo 'Installing extensions...'; echo \"Extension 'bad.one' not found.\" >&2; echo 'Make sure you use the full extension ID' >&2; echo 'Failed Installing Extensions: bad.one' >&2; exit 1; }}\nexit 0\n",
                calls.display()
            ),
        );
        // ONE HOST NOT LET: nothing is asked, and the refusal names it.
        let hosts = crate::hosts_of(["a.x", "bad.one"]);
        let all: Vec<&str> = hosts.iter().map(String::as_str).collect();
        let short = letting(&home.join("short"), &all[1..]);
        let refused = install(
            &code,
            &home,
            &|h| ronoctua_consent::pass_in(&short, h).ok(),
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(
            (refused.why, refused.hosts),
            (Why::NotLet, vec![all[0].to_owned()])
        );
        assert!(!calls.exists(), "code was asked with a host not let");

        let every = letting(&home.join("every"), &all);
        let mut said = Vec::new();
        let done = install(
            &code,
            &home,
            &|h| ronoctua_consent::pass_in(&every, h).ok(),
            &mut |e| said.push(format!("{e:?}")),
        )
        .unwrap();
        assert_eq!(
            done,
            Installed {
                entered: vec!["a.x".to_owned()],
                failed: vec![Failure {
                    id: "bad.one".to_owned(),
                    said: "Extension 'bad.one' not found.".to_owned()
                }],
            }
        );
        assert!(!done.whole());
        let dir = home.join(EXTENSIONS_IN_HOME);
        assert_eq!(
            std::fs::read_to_string(&calls).unwrap(),
            format!(
                "--extensions-dir {d} --install-extension a.x --pre-release\n--extensions-dir {d} --install-extension bad.one\n",
                d = dir.display()
            )
        );
        assert!(said.iter().any(|s| s.starts_with("Trying")), "{said:?}");
        assert_eq!(
            install(&home.join("no-code"), &home, &|_| None, &mut |_| {}).map_err(|r| r.why),
            Err(Why::NoCode)
        );
        let _ = std::fs::remove_dir_all(home);
    }

    /// FAILS IF: the line kept of a failure is not the cause `code` said —
    /// as the pinned 1.119 says it, measured: the closing summary, the hint,
    /// an error that does not name the id when one does, or nothing when
    /// only the output said something.
    #[test]
    fn the_cause_is_kept_and_never_the_summary() {
        let summary = "Failed Installing Extensions: nope.missing";
        assert_eq!(
            what_code_said(
                "nope.missing",
                format!("Extension 'nope.missing' not found.\nMake sure you use the full extension ID, including the publisher, e.g.: ms-dotnettools.csharp\n{summary}\n").as_bytes(),
                b"Installing extensions...\n",
            )
            .as_deref(),
            Some("Extension 'nope.missing' not found.")
        );
        assert_eq!(
            what_code_said(
                "rust-lang.rust-analyzer",
                b"Error while installing extension Rust-Lang.Rust-Analyzer: Signature verification failed with 'NotSigned' error.\nFailed Installing Extensions: rust-lang.rust-analyzer\n",
                b"",
            )
            .as_deref(),
            Some(
                "Error while installing extension Rust-Lang.Rust-Analyzer: Signature verification failed with 'NotSigned' error."
            )
        );
        assert_eq!(
            what_code_said(
                "a.b",
                b"\nError while installing extensions: connect ECONNREFUSED 127.0.0.1:443\nconnect ECONNREFUSED 127.0.0.1:443\n",
                b"Installing extensions...\n",
            )
            .as_deref(),
            Some("Error while installing extensions: connect ECONNREFUSED 127.0.0.1:443")
        );
        assert_eq!(
            what_code_said("a.b", b"", b"Installing extensions...\n  gave up\n").as_deref(),
            Some("gave up")
        );
        assert_eq!(what_code_said("a.b", b"\n", b""), None);
    }
}
