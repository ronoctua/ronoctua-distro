//! THE CONTROL SOCKETS, from the asking side — the footer's
//! (`$XDG_RUNTIME_DIR/ronoctua-<display>.sock`, where an order is one line)
//! and each buffer window's own, by its PID.
//!
//! # Why a module of its own
//!
//! The footer SERVES its socket (`ronoctua_shell_footer::leader::listen`), and everything
//! else only writes a line to it or to a buffer's: a window telling the
//! footer to run a command, a buffer asking its keymap, the modeline asking
//! a buffer which state it is in. The writing half is a handful of paths and
//! one `write_all`, which every door shares — and naming the footer's
//! keymap module to reach them made every door recompile with the leader.

use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::{Context as _, Result};

/// This session's socket path.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent. There is no fallback to `/tmp`: a control
/// socket in a shared directory is attack surface, and section 9
/// does not accept that even in a test.
pub fn socket_path() -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")?;
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    Ok(PathBuf::from(dir).join(format!("ronoctua-{display}.sock")))
}

/// A BUFFER's socket, one per process, named by PID.
///
/// # Why the PID, and not the window id
///
/// It is the only name both sides already know without being told. The footer
/// learns it from `ronoctua_compositor::Window::pid` on the focused window — the same
/// field the modeline's context segment uses to find the git branch — and the
/// buffer's process knows its own. Anything else would need a handshake, and a
/// handshake is a third thing that can be out of date.
///
/// It sits next to the footer's control socket, in `XDG_RUNTIME_DIR`, and speaks
/// the same line protocol. A second grammar in the same directory would be two
/// things to keep in step.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent — same reason as [`socket_path`]: there is no
/// fallback to `/tmp`.
pub fn buffer_socket_path(pid: u32) -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")?;
    Ok(PathBuf::from(dir).join(format!("ronoctua-buffer-{pid}.sock")))
}

/// Asks a buffer for its bindings: `(sequence, command id)` per entry.
///
/// Empty when there is no buffer listening on that PID, and that is a normal
/// answer rather than an error: most focused windows are not our buffers, and
/// `SPC m` has to be ABSENT for those instead of showing a failure.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent.
pub fn buffer_keymap(pid: u32) -> Result<Vec<(String, String)>> {
    buffer_keymap_at(&buffer_socket_path(pid)?)
}

/// The same, at a given path.
///
/// Split off for the same reason as [`listen_on`]: `XDG_RUNTIME_DIR` is
/// process-global, and a test that changes it races every other test in the
/// binary.
pub fn buffer_keymap_at(path: &std::path::Path) -> Result<Vec<(String, String)>> {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return Ok(Vec::new());
    };
    // A buffer that stops answering must not take the footer's frame with it.
    // Everything on the other side is a `HashSet` lookup and a `writeln!`.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    if writeln!(stream, "keymap").is_err() {
        return Ok(Vec::new());
    }
    let mut answer = String::new();
    if std::io::Read::read_to_string(&mut stream, &mut answer).is_err() {
        return Ok(Vec::new());
    }
    // `rsplit_once`, and the test is what said so: the SEQUENCE carries spaces
    // (`m f s`) and the id does not, so what separates them is the LAST space.
    // Splitting on the first gave `("m", "f s save-file")` — a plausible
    // pair that would have listed one key and run nothing.
    Ok(answer
        .lines()
        .filter_map(|l| l.trim().rsplit_once(' '))
        .map(|(keys, command)| (keys.to_owned(), command.to_owned()))
        .collect())
}

/// What a buffer answers about its states: which one is in effect, and the list
/// of `(id, letter)` it declared.
pub type States = (String, ronoctua_protocol::Tone, Vec<(String, char)>);

/// A tone as the control socket and the states socket spell it — one word,
/// after the state's id.
#[must_use]
pub const fn tone_word(tone: ronoctua_protocol::Tone) -> &'static str {
    use ronoctua_protocol::Tone;
    match tone {
        Tone::Standard => "standard",
        Tone::Accent => "accent",
        Tone::Success => "success",
        Tone::Warning => "warning",
        Tone::Error => "error",
    }
}

/// [`tone_word`] read back. AN UNKNOWN WORD — or none, from a buffer older
/// than the tone on the wire — IS THE STANDARD ONE: the host's own rule
/// draws it, which is a colour that is never wrong, where dropping the state
/// would be a segment that vanished.
#[must_use]
pub fn tone_of(word: Option<&str>) -> ronoctua_protocol::Tone {
    use ronoctua_protocol::Tone;
    match word.map(str::trim) {
        Some("accent") => Tone::Accent,
        Some("success") => Tone::Success,
        Some("warning") => Tone::Warning,
        Some("error") => Tone::Error,
        _ => Tone::Standard,
    }
}

/// Asks a buffer which state it is in, and which ones it has.
///
/// Answers `(current, [(id, letter)])`, and `None` when there is no buffer
/// listening on that PID — which is the normal answer for most focused windows,
/// exactly as [`buffer_keymap`] returning empty is.
///
/// # Why the footer asks at all, when the buffer also pushes
///
/// The push covers every change while the window is focused, which is nearly all
/// of them. It does NOT cover a footer that restarted with buffers already open,
/// nor a buffer that changed state while another window had the focus. Asking on
/// each change of focus closes both, and costs one socket round trip per Alt-Tab.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent.
pub fn buffer_states(pid: u32) -> Result<Option<States>> {
    Ok(buffer_states_at(&buffer_socket_path(pid)?))
}

/// The same, at a given path. See [`buffer_keymap_at`].
#[must_use]
pub fn buffer_states_at(path: &std::path::Path) -> Option<States> {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return None;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    if writeln!(stream, "states").is_err() {
        return None;
    }
    let mut answer = String::new();
    if std::io::Read::read_to_string(&mut stream, &mut answer).is_err() {
        return None;
    }
    let mut lines = answer.lines();
    // `<id> [tone]`: the tone is a word after the id, absent from a buffer
    // that predates it.
    let mut first = lines.next()?.split_whitespace();
    let now = first.next()?.to_owned();
    let tone = tone_of(first.next());
    let states = lines
        .filter_map(|l| {
            let (id, letter) = l.trim().split_once(' ')?;
            Some((id.to_owned(), letter.chars().next()?))
        })
        .collect();
    Some((now, tone, states))
}

/// Tells a buffer to go to a state, or to rotate.
///
/// # Errors
///
/// Nobody listening there, or the buffer refused the id.
pub fn buffer_state_at(path: &std::path::Path, order: &str) -> Result<()> {
    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("no buffer listening at {}", path.display()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    writeln!(stream, "{order}").context("could not write to the buffer's socket")?;
    let mut answer = String::new();
    std::io::Read::read_to_string(&mut stream, &mut answer).context("the buffer did not answer")?;
    anyhow::ensure!(
        answer.trim() == "ok",
        "the buffer refused `{order}`: {}",
        answer.trim()
    );
    Ok(())
}

/// [`buffer_state_at`], by PID.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, or nobody listening on that PID.
pub fn buffer_state(pid: u32, order: &str) -> Result<()> {
    buffer_state_at(&buffer_socket_path(pid)?, order)
}

/// Runs one of a buffer's own commands, in the buffer's process.
///
/// This is the half that makes listing honest: without it `SPC m` would be a
/// screen teaching keys that do nothing.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, or nobody listening on that PID.
pub fn buffer_command(pid: u32, id: &str) -> Result<()> {
    buffer_command_at(&buffer_socket_path(pid)?, id)
}

/// The same, at a given path. See [`buffer_keymap_at`].
///
/// # Errors
///
/// Nobody listening there, or the buffer refused the id.
pub fn buffer_command_at(path: &std::path::Path, id: &str) -> Result<()> {
    let mut stream = UnixStream::connect(path)
        .with_context(|| format!("no buffer listening at {}", path.display()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    writeln!(stream, "command {id}").context("could not write to the buffer's socket")?;
    // THE ANSWER IS READ, and not assumed. The buffer refuses an id it did not
    // declare, and a caller that never looked would report success for a key
    // that did nothing — the exact failure this whole path exists to avoid.
    let mut answer = String::new();
    std::io::Read::read_to_string(&mut stream, &mut answer).context("the buffer did not answer")?;
    anyhow::ensure!(
        answer.trim() == "ok",
        "the buffer refused `{id}`: {}",
        answer.trim()
    );
    Ok(())
}

/// Sends an order to the daemon and exits. It is what `ronoctua-shell leader` does.
///
/// # Errors
///
/// There is no daemon listening, or the socket cannot be written to.
pub fn send_it(order: &str) -> Result<()> {
    let path = socket_path()?;
    let mut s = UnixStream::connect(&path).with_context(|| {
        format!(
            "there is no footer listening at {}.\n\
             \n\
             `ronoctua-shell leader` tells a footer that is ALREADY running; it does not start one.",
            path.display()
        )
    })?;
    writeln!(s, "{order}").context("could not write to the control socket")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// THE STATE PROTOCOL, both ways, against a socket that answers like a buffer.
    ///
    /// Two hand-rolled parsers face each other here — the buffer writes the
    /// answer in `publish_keymap` and the footer reads it in `buffer_states_at`
    /// — and nothing but this ties them together. The shape is deliberately
    /// unlike the keymap's: the FIRST line is the current state and the rest are
    /// the list, so a reader that treated every line the same would come back
    /// with a state called `normal n` and a menu missing its first row.
    #[test]
    fn the_footer_reads_a_buffers_states_and_asks_for_one() {
        use std::io::{BufRead as _, Write as _};

        let path =
            std::env::temp_dir().join(format!("ronoctua-states-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");

        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen = std::sync::Arc::clone(&asked);
        let served = std::thread::spawn(move || {
            for _ in 0..3 {
                let Ok(mut stream) = listener.accept().map(|(s, _)| s) else {
                    return;
                };
                let mut line = String::new();
                let clone = stream.try_clone().expect("clone");
                std::io::BufReader::new(clone)
                    .read_line(&mut line)
                    .expect("read");
                let reply = match line.trim() {
                    "states" => "filter warning\nnormal n\nfilter f\nsheet s".to_owned(),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("reply");
            }
        });

        let (now, tone, listed) = super::buffer_states_at(&path).expect("read the states");
        // THE CURRENT ONE IS THE FIRST LINE and is NOT in the list — that is the
        // whole reason the shape differs from the keymap's. Its tone rides on
        // the same line.
        assert_eq!(now, "filter");
        assert_eq!(tone, ronoctua_protocol::Tone::Warning);
        assert_eq!(
            listed,
            vec![
                ("normal".to_owned(), 'n'),
                ("filter".to_owned(), 'f'),
                ("sheet".to_owned(), 's'),
            ]
        );

        super::buffer_state_at(&path, "state sheet").expect("ask for the state");
        super::buffer_state_at(&path, "state-next").expect("cycle");
        served.join().expect("the buffer's thread");
        assert_eq!(
            *asked.lock().expect("lock"),
            vec!["state sheet".to_owned(), "state-next".to_owned()]
        );
        let _ = std::fs::remove_file(&path);
    }

    /// NOBODY LISTENING IS `None`, and not an error.
    ///
    /// Most focused windows are not our buffers, and this runs on every event
    /// the compositor sends. An error here would put a warning in the journal
    /// per window that is not ours, forever — and would teach whoever reads it
    /// to ignore the ones that matter.
    #[test]
    fn a_window_that_is_not_ours_answers_nothing() {
        let path = std::env::temp_dir().join("ronoctua-does-not-exist.sock");
        let _ = std::fs::remove_file(&path);
        assert!(super::buffer_states_at(&path).is_none());
    }

    /// THE TWO DIRECTIONS, against a socket that answers like a buffer.
    ///
    /// It is one test and not two on purpose: listing without running is the
    /// defect this whole path exists to avoid, so the pair is the unit.
    #[test]
    fn the_footer_reads_a_buffer_keymap_and_runs_one_of_its_commands() {
        use std::io::{BufRead as _, Write as _};

        let path = std::env::temp_dir().join(format!("ronoctua-teste-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind");

        // A stand-in for the buffer's own thread, answering the same protocol.
        let ran = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen = std::sync::Arc::clone(&ran);
        let served = std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok(mut stream) = listener.accept().map(|(s, _)| s) else {
                    return;
                };
                let mut line = String::new();
                let clone = stream.try_clone().expect("clone");
                std::io::BufReader::new(clone)
                    .read_line(&mut line)
                    .expect("read");
                let reply = match line.trim() {
                    "keymap" => "m f s save-file\nm f r reload-file".to_owned(),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("reply");
            }
        });

        let keymap = super::buffer_keymap_at(&path).expect("read the keymap");
        assert_eq!(
            keymap,
            vec![
                ("m f s".to_owned(), "save-file".to_owned()),
                ("m f r".to_owned(), "reload-file".to_owned()),
            ],
            "the sequence carries spaces, so only the FIRST one separates the id from it"
        );

        super::buffer_command_at(&path, "save-file").expect("run the command");
        served.join().expect("the fake buffer's thread");
        assert_eq!(
            *ran.lock().expect("lock"),
            vec!["command save-file".to_owned()],
            "the command has to arrive on the buffer's side, not just be sent"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// No buffer listening is a NORMAL answer, and not an error: most focused
    /// windows are not ours, and `SPC m` has to be absent there rather than
    /// showing a failure.
    #[test]
    fn no_buffer_listening_gives_an_empty_keymap() {
        let path = std::env::temp_dir().join("ronoctua-really-does-not-exist.sock");
        let _ = std::fs::remove_file(&path);
        assert!(
            super::buffer_keymap_at(&path)
                .expect("it cannot be an error")
                .is_empty()
        );
    }
}
