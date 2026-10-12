//! Running a program: by its path, with no standard input, and with what it
//! prints going to the journal — this CLI's own standard output is its one
//! line of verdict.
//!
//! No standard input because a caller may hand THIS process root's lock as
//! its standard input (`ronoctua-person hold`): a child that inherited it
//! would hold the lock after we are gone.

use std::os::fd::AsFd as _;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Refusal, Why, say};

/// What a captured program said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ran {
    /// Its exit code; `None` when a signal ended it.
    pub code: Option<i32>,
    /// Its standard output.
    pub stdout: String,
    /// Its standard error.
    pub stderr: String,
}

impl Ran {
    /// Whether it exited 0.
    pub(crate) const fn ok(&self) -> bool {
        matches!(self.code, Some(0))
    }
}

fn line(program: &Path, args: &[String]) -> String {
    let mut text = program.display().to_string();
    for a in args {
        text.push(' ');
        if a.is_empty() || a.contains(char::is_whitespace) {
            text.push_str(&format!("{a:?}"));
        } else {
            text.push_str(a);
        }
    }
    text
}

fn spawn_failed(program: &Path, e: &std::io::Error) -> Refusal {
    say(3, &format!("{} did not start: {e}", program.display()));
    Refusal::of(Why::Failed)
}

/// Runs `program` to its end, its output on our standard error; its exit
/// code (`None`: ended by a signal).
pub(crate) fn status(program: &Path, args: &[String]) -> Result<Option<i32>, Refusal> {
    say(6, &format!("running {}", line(program, args)));
    let stderr = std::io::stderr()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|e| spawn_failed(program, &e))?;
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stderr))
        .status()
        .map(|s| s.code())
        .map_err(|e| spawn_failed(program, &e))
}

/// Runs `program` to its end and keeps what it printed.
pub(crate) fn capture(program: &Path, args: &[String]) -> Result<Ran, Refusal> {
    say(6, &format!("running {}", line(program, args)));
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| spawn_failed(program, &e))?;
    Ok(Ran {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_quotes_what_would_read_as_two_words() {
        assert_eq!(
            line(
                Path::new("/b/bootctl"),
                &["set-default".into(), String::new()]
            ),
            "/b/bootctl set-default \"\""
        );
        assert_eq!(line(Path::new("/b/nix"), &["a b".into()]), "/b/nix \"a b\"");
    }

    #[test]
    fn a_program_that_is_not_there_is_a_refusal() {
        assert_eq!(
            status(Path::new("/nonexistent/program"), &[])
                .unwrap_err()
                .why,
            Why::Failed
        );
        let ran = capture(
            Path::new("/bin/sh"),
            &["-c".into(), "echo out; echo err >&2; exit 3".into()],
        )
        .unwrap();
        assert_eq!(
            (ran.code, ran.stdout.as_str(), ran.stderr.as_str()),
            (Some(3), "out\n", "err\n")
        );
        assert!(!ran.ok());
    }
}
