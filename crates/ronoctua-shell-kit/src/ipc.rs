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

/// One shortcut a window publishes on its per-PID socket — a line of the
/// `keymap` answer — with what the WINDOW calls it.
///
/// # Why the words and the sentence travel with the keys
///
/// Because the footer draws them twice — the free minibuffer's line and the
/// `LEADER Alt` list — and the window is the one that knows them: a native
/// window's short word is its hint's (`texts.t(id)`), a buffer's is what its
/// component declared, and a sentence a window overrides
/// (`door::Content::explains`) is nowhere the footer could look it up. Read
/// back from the command id, the two places said two different things — the
/// line the short word, the list the id — which is what the owner found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Shortcut {
    /// The sequence: a native window's one key after `LEADER Alt` (`j`), a
    /// buffer's local sequence (`, a`).
    pub keys: String,
    /// The command it runs, in the window's process.
    pub command: String,
    /// The window's own short words for it — what its header says beside
    /// the key. `None` from a window that sends none.
    pub words: Option<String>,
    /// What it does, in a sentence — what the window says of it under the
    /// pointer. `None` likewise.
    pub sentence: Option<String>,
}

/// The verb of the report a window sends the footer when the shortcuts it
/// publishes CHANGED — a page opened, a question took every key, a state
/// teaches others ([`shortcuts_order`]). The footer reads the window's
/// `keymap` again when it is the one in front.
pub const SHORTCUTS_CHANGED: &str = "shortcuts";

/// The report itself: `shortcuts <pid>`. It names the window and nothing
/// else — the set is on the window's socket, where the footer reads it, so
/// the report can never carry a set of its own that disagrees with it.
#[must_use]
pub fn shortcuts_order(pid: u32) -> String {
    format!("{SHORTCUTS_CHANGED} {pid}")
}

/// One line of the `keymap` answer: `<keys> <command>`, then — after a TAB
/// each — the words and the sentence, when the window has them.
///
/// A TAB, and not a space: the sequence carries spaces (`, a`) and the words
/// do too, and a tab is what neither ever holds. Each is put on ONE line, its
/// whitespace collapsed — a sentence a module wrote may carry a line break,
/// and the answer is a line per shortcut. A line with neither is the old
/// one, so a reader of the old shape reads it still.
#[must_use]
pub fn keymap_line(
    keys: &str,
    command: &str,
    words: Option<&str>,
    sentence: Option<&str>,
) -> String {
    let one = |t: Option<&str>| {
        t.map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
    };
    let (words, sentence) = (one(words), one(sentence));
    match (words.is_empty(), sentence.is_empty()) {
        (true, true) => format!("{keys} {command}"),
        (_, true) => format!("{keys} {command}\t{words}"),
        _ => format!("{keys} {command}\t{words}\t{sentence}"),
    }
}

/// A line of the `keymap` answer read back ([`keymap_line`]). `None` for a
/// line with no command.
///
/// `rsplit_once` on the head, and the test is what said so: the SEQUENCE
/// carries spaces (`m f s`) and the id does not, so what separates them is the
/// LAST space. Splitting on the first gave `("m", "f s save-file")` — a
/// plausible pair that would have listed one key and run nothing.
#[must_use]
pub fn shortcut_of_line(line: &str) -> Option<Shortcut> {
    let mut fields = line.trim_end_matches(['\r', '\n']).split('\t');
    let (keys, command) = fields.next()?.trim().rsplit_once(' ')?;
    let said = |f: Option<&str>| {
        f.map(str::trim)
            .filter(|f| !f.is_empty())
            .map(str::to_owned)
    };
    Some(Shortcut {
        keys: keys.to_owned(),
        command: command.to_owned(),
        words: said(fields.next()),
        sentence: said(fields.next()),
    })
}

/// Asks a window for its shortcuts — a buffer, or any window the base door
/// serves: one [`Shortcut`] per entry.
///
/// Empty when there is no window listening on that PID, and that is a normal
/// answer rather than an error: most focused windows are not ours, and
/// `LEADER Alt` has to be ABSENT for those instead of showing a failure.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent.
pub fn buffer_keymap(pid: u32) -> Result<Vec<Shortcut>> {
    buffer_keymap_at(&buffer_socket_path(pid)?)
}

/// The same, at a given path.
///
/// Split off for the same reason as [`listen_on`]: `XDG_RUNTIME_DIR` is
/// process-global, and a test that changes it races every other test in the
/// binary.
pub fn buffer_keymap_at(path: &std::path::Path) -> Result<Vec<Shortcut>> {
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
    Ok(answer.lines().filter_map(shortcut_of_line).collect())
}

/// ONE OF A WINDOW'S TABS — the screens it shows one at a time (the icons'
/// three sets) — as the window publishes them for the modeline to draw while
/// it is in focus (`crate::door::Content::tabs`; the footer's
/// `window_tabs`, owner 2026-11-09).
///
/// A tab is one of the window's own SHORTCUTS, by its command: a click on it
/// runs that command in the window, by the one door `LEADER Alt` runs it
/// through — the mouse reaches nothing the keyboard does not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowTab {
    /// The window's command that shows it.
    pub command: String,
    /// Its name, in the window's language.
    pub name: String,
    /// Whether it is the one shown.
    pub current: bool,
}

/// The verb a window's socket answers its tabs on.
pub const TABS_VERB: &str = "tabs";

/// One line of the `tabs` answer: `<command>`, `1` for the tab shown or `0`,
/// and the name, a TAB between each — the name carries spaces, a tab never.
#[must_use]
pub fn tab_line(tab: &WindowTab) -> String {
    let name = tab.name.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}\t{}\t{name}", tab.command, u8::from(tab.current))
}

/// A line of the `tabs` answer read back ([`tab_line`]); `None` for one that
/// is not a tab — a socket that does not know the verb answers `?`.
#[must_use]
pub fn tab_of_line(line: &str) -> Option<WindowTab> {
    let mut fields = line.trim_end_matches(['\r', '\n']).splitn(3, '\t');
    let command = fields.next()?.trim();
    let current = match fields.next()?.trim() {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    let name = fields.next()?.trim();
    (!command.is_empty() && !command.contains(' ') && !name.is_empty()).then(|| WindowTab {
        command: command.to_owned(),
        name: name.to_owned(),
        current,
    })
}

/// Asks a window for its tabs — none from a window that has none, or that
/// does not answer.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent.
pub fn buffer_tabs(pid: u32) -> Result<Vec<WindowTab>> {
    Ok(buffer_tabs_at(&buffer_socket_path(pid)?))
}

/// The same, at a given path. See [`buffer_keymap_at`].
#[must_use]
pub fn buffer_tabs_at(path: &std::path::Path) -> Vec<WindowTab> {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return Vec::new();
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    if writeln!(stream, "{TABS_VERB}").is_err() {
        return Vec::new();
    }
    let mut answer = String::new();
    if std::io::Read::read_to_string(&mut stream, &mut answer).is_err() {
        return Vec::new();
    }
    answer.lines().filter_map(tab_of_line).collect()
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
    /// A window's tabs cross the socket as they were: the one shown, the
    /// names with their spaces — and an answer that is no tab is none.
    #[test]
    fn a_tab_reads_back_as_it_was_written() {
        let tab = super::WindowTab {
            command: "icons-tab-nerd-fonts".to_owned(),
            name: "Nerd\nFonts".to_owned(),
            current: true,
        };
        let back = super::tab_of_line(&super::tab_line(&tab)).expect("a tab");
        assert_eq!(back.command, tab.command);
        assert_eq!(back.name, "Nerd Fonts");
        assert!(back.current);
        assert_eq!(super::tab_of_line("?"), None);
        assert_eq!(super::tab_of_line("x\t2\tName"), None);
    }

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
                    // THE OLD SHAPE AND THE NEW, side by side: a line with
                    // the words and the sentence after tabs, and one with
                    // neither, which a window that has none still sends.
                    "keymap" => format!(
                        "{}\nm f r reload-file",
                        super::keymap_line("m f s", "save-file", Some("save"), Some("Saves it."))
                    ),
                    order => {
                        seen.lock().expect("lock").push(order.to_owned());
                        "ok".to_owned()
                    }
                };
                writeln!(stream, "{reply}").expect("reply");
            }
        });

        let keymap = super::buffer_keymap_at(&path).expect("read the keymap");
        let pairs: Vec<(&str, &str)> = keymap
            .iter()
            .map(|s| (s.keys.as_str(), s.command.as_str()))
            .collect();
        assert_eq!(
            pairs,
            vec![("m f s", "save-file"), ("m f r", "reload-file")],
            "the sequence carries spaces, so only the LAST one separates the id from it"
        );
        assert_eq!(keymap[0].words.as_deref(), Some("save"));
        assert_eq!(keymap[0].sentence.as_deref(), Some("Saves it."));
        assert_eq!(
            (keymap[1].words.as_ref(), keymap[1].sentence.as_ref()),
            (None, None)
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

    /// FAILS IF: a shortcut's words or sentence do not come back as the
    /// window wrote them — a space in them taken for the seam between the
    /// keys and the command, a sentence of two lines breaking the answer in
    /// two, the words lost when there is no sentence — or a line of the old
    /// shape stops being read.
    #[test]
    fn words_and_sentence_travel_with_the_keys() {
        let line = super::keymap_line(
            ", a",
            "brain-abort",
            Some("not now"),
            Some("Puts it off\nto its next moment."),
        );
        assert_eq!(line.lines().count(), 1, "one line per shortcut: {line:?}");
        assert_eq!(
            super::shortcut_of_line(&line),
            Some(super::Shortcut {
                keys: ", a".to_owned(),
                command: "brain-abort".to_owned(),
                words: Some("not now".to_owned()),
                sentence: Some("Puts it off to its next moment.".to_owned()),
            })
        );
        let words_only = super::keymap_line("u", "update-now", Some("update now"), None);
        let read = super::shortcut_of_line(&words_only).expect("a shortcut");
        assert_eq!(read.words.as_deref(), Some("update now"));
        assert_eq!(read.sentence, None);
        let sentence_only = super::keymap_line("u", "update-now", None, Some("Updates."));
        let read = super::shortcut_of_line(&sentence_only).expect("a shortcut");
        assert_eq!(
            (read.words, read.sentence.as_deref()),
            (None, Some("Updates."))
        );
        // THE OLD SHAPE, which is also what a window with neither sends.
        assert_eq!(super::keymap_line("j", "down", None, None), "j down");
        let old = super::shortcut_of_line("m f s save-file").expect("a shortcut");
        assert_eq!(
            (old.keys.as_str(), old.command.as_str()),
            ("m f s", "save-file")
        );
        assert_eq!(super::shortcut_of_line("nothing"), None);
        // AND THE REPORT names the window and nothing else.
        assert_eq!(super::shortcuts_order(4242), "shortcuts 4242");
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
