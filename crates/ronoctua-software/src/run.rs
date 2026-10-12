//! Running the programs this crate drives — one way, for every source.
//!
//! Three things every call needs and that each module would otherwise write
//! again, differently:
//!
//! * **by path, never by name.** A program comes from [`crate::Tools`]; a
//!   `None` there is [`Refusal::SourceAbsent`], said before anything runs.
//! * **line by line, from both streams.** Progress arrives on stdout or on
//!   stderr depending on the program, and flatpak rewrites one line with `\r`;
//!   both separators end a line here. The last [`TAIL`] lines are kept, and on
//!   failure they are the job's trouble — the program's own words.
//! * **the rehearsal.** In the `.#dev` instance a call that CHANGES something
//!   prints its argument vector on stderr and runs nothing: the loop runs on
//!   the owner's own machine, and a test click must not install anything
//!   there.
//!
//! Children inherit this process's group: the queue runs the CLI in its own
//! group and cancels by killing that group, so a `nix` or a `flatpak` started
//! here dies with it.

use std::io::{BufRead as _, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::{Context, Failure, Refusal};

/// How many of a program's last lines are kept for the trouble.
pub const TAIL: usize = 20;

/// The program for a tool, or the refusal that says this machine has none.
///
/// # Errors
///
/// [`Refusal::SourceAbsent`].
pub fn tool(path: Option<&PathBuf>) -> Result<&Path, Failure> {
    path.map(PathBuf::as_path)
        .ok_or_else(|| Failure::refused(Refusal::SourceAbsent))
}

/// What a finished program left.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ran {
    /// Its exit code (`None` when killed by a signal).
    pub code: Option<i32>,
    /// Its last lines, both streams, in arrival order.
    pub tail: Vec<String>,
    /// Everything on stdout, when the caller asked to keep it.
    pub stdout: String,
    /// `true` when this was only rehearsed.
    pub rehearsed: bool,
}

impl Ran {
    /// Success, or the failure in the program's own words.
    ///
    /// # Errors
    ///
    /// A non-zero exit: [`Refusal::ToolFailed`] with the tail as trouble.
    pub fn ok(self) -> Result<Self, Failure> {
        if self.code == Some(0) || self.rehearsed {
            Ok(self)
        } else {
            Err(Failure {
                refusal: Some(Refusal::ToolFailed),
                trouble: self.tail.join("\n"),
            })
        }
    }
}

/// One call.
#[derive(Debug, Clone)]
pub struct Call<'a> {
    program: &'a Path,
    args: Vec<String>,
    env: Vec<(String, String)>,
    stdin: Option<Vec<u8>>,
    changes: bool,
    keep_stdout: bool,
}

impl<'a> Call<'a> {
    /// A call of `program` with `args`. It is a READ until [`Call::changing`]
    /// says otherwise: a read runs even in a rehearsal.
    #[must_use]
    pub fn new(program: &'a Path, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program,
            args: args.into_iter().map(Into::into).collect(),
            // Parsed output has to be the same words on every machine: the
            // person's language would translate "Installing" away.
            env: vec![
                ("LC_ALL".into(), "C.UTF-8".into()),
                ("LANGUAGE".into(), String::new()),
            ],
            stdin: None,
            changes: false,
            keep_stdout: false,
        }
    }

    /// This call changes something: rehearsed in `.#dev`.
    #[must_use]
    pub const fn changing(mut self) -> Self {
        self.changes = true;
        self
    }

    /// One more environment variable.
    #[must_use]
    pub fn env(mut self, name: &str, value: impl Into<String>) -> Self {
        self.env.push((name.to_owned(), value.into()));
        self
    }

    /// Bytes for the program's standard input.
    #[must_use]
    pub fn stdin(mut self, bytes: Vec<u8>) -> Self {
        self.stdin = Some(bytes);
        self
    }

    /// Keep all of stdout in [`Ran::stdout`] (for `list --json`).
    #[must_use]
    pub const fn keep_stdout(mut self) -> Self {
        self.keep_stdout = true;
        self
    }

    /// The argument vector, program first — what a rehearsal prints and what a
    /// test reads.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        std::iter::once(self.program.display().to_string())
            .chain(self.args.iter().cloned())
            .collect()
    }

    /// Run it, handing every line to `line` as it arrives.
    ///
    /// # Errors
    ///
    /// The program could not be started ([`Refusal::ToolFailed`] with the
    /// reason). A non-zero exit is NOT an error here: it is in [`Ran::code`],
    /// and [`Ran::ok`] turns it into one.
    pub fn run(self, ctx: &Context, line: &mut dyn FnMut(&str)) -> Result<Ran, Failure> {
        if self.changes && ctx.rehearse {
            let argv = self.argv().join(" ");
            let _ = writeln!(std::io::stderr(), "rehearse: {argv}");
            return Ok(Ran {
                code: Some(0),
                rehearsed: true,
                ..Ran::default()
            });
        }
        let mut command = Command::new(self.program);
        command
            .args(&self.args)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in &self.env {
            command.env(name, value);
        }
        let mut child = command.spawn().map_err(|e| Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("could not start {}: {e}", self.program.display()),
        })?;
        if let (Some(bytes), Some(mut input)) = (self.stdin, child.stdin.take()) {
            // A thread, so a program that answers before it reads everything
            // cannot deadlock against a full pipe.
            std::thread::spawn(move || {
                let _ = input.write_all(&bytes);
            });
        }
        let (tx, rx) = mpsc::channel::<(bool, String)>();
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            let tx = tx.clone();
            readers.push(std::thread::spawn(move || {
                split_lines(out, &|l| tx.send((true, l)).is_ok())
            }));
        }
        if let Some(err) = child.stderr.take() {
            let tx = tx.clone();
            readers.push(std::thread::spawn(move || {
                split_lines(err, &|l| tx.send((false, l)).is_ok())
            }));
        }
        drop(tx);
        let mut ran = Ran::default();
        for (from_stdout, text) in rx {
            if self.keep_stdout && from_stdout {
                ran.stdout.push_str(&text);
                ran.stdout.push('\n');
            }
            line(&text);
            if !text.trim().is_empty() {
                ran.tail.push(text);
                if ran.tail.len() > TAIL {
                    ran.tail.remove(0);
                }
            }
        }
        for r in readers {
            let _ = r.join();
        }
        let status = child.wait().map_err(|e| Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("could not wait for {}: {e}", self.program.display()),
        })?;
        ran.code = status.code();
        Ok(ran)
    }

    /// Run it and ignore the lines.
    ///
    /// # Errors
    ///
    /// As [`Call::run`].
    pub fn quietly(self, ctx: &Context) -> Result<Ran, Failure> {
        self.run(ctx, &mut |_| {})
    }
}

/// Read a stream to its end, calling `each` with every line; `\n`, `\r\n`
/// and a lone `\r` all end one. Bytes that are not UTF-8 are replaced — the
/// lines are for a person and a parser, never written back.
pub fn split_lines(mut stream: impl std::io::Read, each: &dyn Fn(String) -> bool) {
    let mut buffer = [0u8; 8192];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buffer[..n] {
            if b == b'\n' || b == b'\r' {
                if !pending.is_empty() {
                    if !each(String::from_utf8_lossy(&pending).into_owned()) {
                        return;
                    }
                    pending.clear();
                }
            } else {
                pending.push(b);
            }
        }
    }
    if !pending.is_empty() {
        let _ = each(String::from_utf8_lossy(&pending).into_owned());
    }
}

/// A whole file's lines through the same splitter, for a test fixture.
#[must_use]
pub fn lines_of(bytes: &[u8]) -> Vec<String> {
    let out = std::cell::RefCell::new(Vec::new());
    split_lines(bytes, &|l| {
        out.borrow_mut().push(l);
        true
    });
    out.into_inner()
}

/// Read up to `most` bytes of a file (for a head, a magic number).
///
/// # Errors
///
/// When the file cannot be opened.
pub fn head_of(path: &Path, most: usize) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut out = vec![0u8; most];
    let mut filled = 0;
    while filled < most {
        let n = file.read(&mut out[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    out.truncate(filled);
    Ok(out)
}

/// SHA-256 of a file, in lower-case hex, read in pieces.
///
/// # Errors
///
/// When the file cannot be read.
pub fn sha256_of(path: &Path) -> std::io::Result<String> {
    let mut file = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut context = aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256);
    loop {
        let buffer = file.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        context.update(buffer);
        let n = buffer.len();
        file.consume(n);
    }
    Ok(hex(context.finish().as_ref()))
}

/// SHA-256 of bytes, in lower-case hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    hex(aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, bytes).as_ref())
}

/// Lower-case hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(rehearse: bool) -> Context {
        Context {
            tools: crate::Tools::default(),
            roots: crate::Roots::under(&std::env::temp_dir()),
            prefs: crate::prefs::Preferences::default(),
            rehearse,
        }
    }

    #[test]
    fn a_carriage_return_ends_a_line_like_a_newline() {
        assert_eq!(
            lines_of(b"a 10%\rb 20%\r\nc\n\nd"),
            ["a 10%", "b 20%", "c", "d"]
        );
    }

    #[test]
    fn a_rehearsal_runs_nothing_that_changes_something() {
        let never = Path::new("/nonexistent/never-run");
        let ran = Call::new(never, ["install", "x"])
            .changing()
            .quietly(&ctx(true))
            .unwrap();
        assert!(ran.rehearsed);
        // A READ is not rehearsed: it runs (and here fails to start).
        assert!(Call::new(never, ["list"]).quietly(&ctx(true)).is_err());
    }

    #[test]
    fn both_streams_arrive_and_the_tail_is_the_trouble() {
        let sh = Path::new("/bin/sh");
        let ran = Call::new(sh, ["-c", "echo out; echo err >&2; exit 3"])
            .keep_stdout()
            .quietly(&ctx(false))
            .unwrap();
        assert_eq!(ran.code, Some(3));
        assert_eq!(ran.stdout, "out\n");
        assert!(ran.tail.contains(&"out".to_owned()) && ran.tail.contains(&"err".to_owned()));
        let failure = ran.ok().unwrap_err();
        assert_eq!(failure.refusal, Some(Refusal::ToolFailed));
        assert!(failure.trouble.contains("err"));
    }

    #[test]
    fn a_missing_tool_is_a_refusal_before_anything_runs() {
        assert_eq!(tool(None).unwrap_err().refusal, Some(Refusal::SourceAbsent));
    }

    #[test]
    fn the_hash_is_the_hash() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
