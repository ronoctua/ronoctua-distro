//! THE BRAIN'S SOCKET, from a door's side — its mind heard, an ask put and
//! answered, its log followed, an action named — and the glyph and colour a
//! module's mark is drawn in.
//!
//! # Why a module of its own
//!
//! The brain's LINE and marks are the footer's (`ronoctua_shell_footer::brain`);
//! hearing the daemon is every door's that shows it — the buffer host, the
//! update's window, the LLM accounts — and each named the footer's brain line
//! to do it. The socket is a door's plumbing; the line is the footer's drawing.

/// The conversion to the WIT's records lives in [`crate::mind_wire`].
///
/// It moved out so `ronoctua-render-png` could link it without linking the
/// footer's brain line, which reaches for smithay, `ronoctua_shell_kit::band`
/// and `ronoctua_shell_footer::leader`. Re-exported here because every caller
/// in the footer already says `crate::mind_wire::mind_of`, and a move is not a
/// reason to touch them.
pub(crate) use crate::mind_wire::{Answer, answer_from};
use ronoctua_brain::Mind;
use ronoctua_brain::module::Tone;
use ronoctua_tokens as tokens;

/// How many DIFFERENT names of the brain's modules this process will ever
/// make static.
///
/// A segment's id is `&'static str` (`ronoctua_render::layout::Segment`), and
/// so are a question's id (`crate::line::Line`) and an event's tag and names
/// (`ronoctua_shell_footer::events::Event`) — and a module's arrive at run time. So each
/// distinct one is made static once and kept. Bounded: what a module may
/// publish is what its manifest declares, a handful each, and a daemon that
/// sent more than this is refused past it rather than growing this process
/// for ever.
const NAMES_KEPT: usize = 256;

/// A module's glyph token, as a codepoint.
///
/// ONE door to the glyphs the brain's modules declare, measured into the
/// brain's catalogue (`nix/brain-catalogue.nix`, read at run time —
/// `ronoctua_brain::catalogue`): only a module's own token answers, never a
/// chrome token that happens to share its name, and a module the person wrote
/// answers as the distro's do, with no crate compiled for it.
///
/// # Panics
///
/// As `ronoctua_brain::catalogue::catalogue`: when the catalogue cannot be
/// had, which the binary's start has already refused.
#[must_use]
pub fn glyph_of(token: &str) -> Option<char> {
    ronoctua_brain::catalogue::catalogue().glyph(token)
}

/// The names already made static, bounded by [`NAMES_KEPT`].
static INTERNED: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// A module's name as a `&'static str`: the same one every time it is asked
/// for, made once, and none past [`NAMES_KEPT`].
pub fn interned(id: &str) -> Option<&'static str> {
    let mut known = INTERNED.lock().ok()?;
    if let Some(found) = known.iter().find(|k| **k == id) {
        return Some(*found);
    }
    if known.len() >= NAMES_KEPT {
        tracing::warn!(
            id,
            "too many different names from the brain's modules; refused"
        );
        return None;
    }
    let made: &'static str = Box::leak(id.to_owned().into_boxed_str());
    known.push(made);
    Some(made)
}

/// A mark's colour, by the MEANING its module gave it — one token each, and a
/// watch wears the colour the modeline already marks a watch with.
#[must_use]
pub const fn tone_colour(tone: Tone) -> tokens::Color {
    match tone {
        Tone::Quiet => tokens::color::TEXT_SECONDARY,
        Tone::Watching => tokens::color::MODELINE_WATCHING,
        Tone::Warning => tokens::color::STATE_WARNING,
        Tone::Danger => tokens::color::STATE_ERROR,
        Tone::Accent => tokens::color::STATE_ACCENT,
    }
}

/// The `systemctl`, by path and never by name.
///
/// A unit's `PATH` is minimal (`AGENTS.md`), and a bare name fails far from its
/// cause. Outside Nix — `cargo run` — it falls back to the name and the shell's
/// own `PATH` resolves it, which is the shape `ronoctua_shell_kit::keyboard` already uses.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// The unit `nix/modules/brain.nix` defines.
const UNIT: &str = "ronoctua-brain.service";

/// Brings up the thread that watches the brain and pushes every mind into
/// `calloop`.
///
/// It gives back `false` when there is nowhere to put a socket — and **that is
/// not fatal**, for the reason `queue::listen` writes down: without it the
/// footer loses one line and goes on drawing everything else, and taking the
/// shell down over a missing `XDG_RUNTIME_DIR` would trade one feature for all
/// of them.
///
/// # Why a thread and not the socket in `calloop`
///
/// The two reasons the queue's thread already gives, and one of its own. A JSON
/// line can arrive split across two reads, which in the main loop is a state
/// machine and on a thread with a blocking `read_line` is one line of code. The
/// client RECONNECTS, which is a sleep and a retry, and a sleep in the loop that
/// draws is a frozen footer. And `client::watch` is explicitly the SLOW half of
/// that crate's two connects — `connect_quick` is what the drawing loop uses,
/// and it is what [`tell`] below calls.
pub fn listen(channel: smithay_client_toolkit::reexports::calloop::channel::Sender<Mind>) -> bool {
    if let Err(e) = ronoctua_brain::wire::socket_path() {
        tracing::warn!(error = %e, "no brain; the band does not appear");
        return false;
    }
    let built = std::thread::Builder::new()
        .name("brain".into())
        .spawn(move || {
            // THE UNIT FIRST, and on THIS thread — which is the whole reason
            // the start is here and not beside the call. `systemctl start`
            // returns when systemd has taken the job, not when the daemon
            // answers; the connect that follows does have to wait, and waiting
            // is what this thread is for.
            //
            // A failure is not worth a word above `debug`: in the nested loop
            // there is no unit at all, and `client::watch` starts the daemon
            // beside this binary instead. On the installed system it is already
            // running before the shell is.
            wake_the_unit();
            // The closure says "stop" only when the other end has gone, which
            // means the footer is already on its way out.
            let _ = ronoctua_brain::client::watch(|mind| channel.send(mind).is_err());
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the brain's thread");
            false
        }
    }
}

/// Asks systemd for the user unit. Failure is expected off the installed system.
fn wake_the_unit() {
    match std::process::Command::new(SYSTEMCTL)
        .args(["--user", "start", UNIT])
        .output()
    {
        Ok(out) if out.status.success() => tracing::info!(unit = UNIT, "brain asked of systemd"),
        Ok(out) => tracing::debug!(
            unit = UNIT,
            why = %String::from_utf8_lossy(&out.stderr).trim(),
            "systemd did not bring the brain up; carrying on through the socket"
        ),
        Err(e) => tracing::debug!(error = %e, "no systemctl; carrying on through the socket"),
    }
}

/// Sends one ask, and says in the log when there is nobody to send it to.
///
/// `tell` and not `ask`: none of these answer with anything the caller needs,
/// and waiting for a reply inside the loop that draws is the frozen footer this
/// project has paid for twice. What comes back is the whole mind, on the watch
/// stream, which is where the screen reads it anyway.
///
/// # Errors
///
/// Nobody listening.
pub fn tell(what: &ronoctua_brain::Ask) -> Result<(), anyhow::Error> {
    ronoctua_brain::client::tell(what)
}

/// Asks the brain for a change ON A THREAD, and sends what became of it down
/// `back` — the loop that draws never waits on the socket.
///
/// `false` when the thread could not start; the caller says so, since nothing
/// will ever come down `back`.
pub fn put(
    ask: ronoctua_brain::Ask,
    taken: Option<&'static str>,
    back: smithay_client_toolkit::reexports::calloop::channel::Sender<Answer>,
) -> bool {
    let built = std::thread::Builder::new()
        .name("brain-ask".into())
        .spawn(move || {
            let answer = answer_from(ronoctua_brain::client::ask(&ask), taken);
            if answer.taken() {
                tracing::info!(?ask, "the brain took it");
            } else {
                tracing::warn!(?ask, ?answer, "the brain did not take it");
            }
            let _ = back.send(answer);
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the brain's ask");
            false
        }
    }
}

/// The countdown's two answers — the band's buttons, `LEADER :`'s commands
/// and the update's view in either door all come here: sends what answers
/// it, and gives back the sentence to say.
///
/// # Why the refusal is spoken and not swallowed
///
/// Because `brain-abort` with nothing about to happen is a key that appears
/// in which-key and does nothing — indistinguishable from a key that did not
/// register, which is how five mute commands went unnoticed once already.
#[must_use]
pub fn answer(mind: Option<&Mind>, proceeding: bool) -> &'static str {
    let Some(ask) = answer_of(mind, proceeding) else {
        return crate::key::ECHO_BRAIN_IDLE;
    };
    match tell(&ask) {
        Ok(()) => {
            tracing::info!(?ask, "answer to the brain");
            if proceeding {
                crate::key::ECHO_BRAIN_PROCEEDING
            } else {
                crate::key::ECHO_BRAIN_POSTPONED
            }
        }
        // LOUD, and said on screen. A countdown that goes on running after
        // somebody pressed "not now" is the one failure here that must
        // never be silent.
        Err(e) => {
            tracing::error!(error = %e, ?ask, "the brain did not get the answer");
            crate::key::ECHO_BRAIN_UNREACHABLE
        }
    }
}

/// What answers the countdown, or `None` when there is nothing to answer.
///
/// PROCEED IS THE DEADLINE MOVED and not a second way to start something:
/// with nothing announced there is no deadline to move, and starting one here
/// would be a path into a run that the brain's own queue never saw. STOP is
/// [`abort`]'s choice between the two asks that actually stop it.
#[must_use]
pub fn answer_of(mind: Option<&Mind>, proceeding: bool) -> Option<ronoctua_brain::Ask> {
    let m = mind?;
    if proceeding {
        m.announcement
            .as_ref()
            .map(|_| ronoctua_brain::Ask::Proceed)
    } else if m.announcement.is_some() || !m.running.is_empty() {
        Some(abort(m))
    } else {
        None
    }
}

/// What the band's `not now` sends.
///
/// # Why `postpone` and not `abort`, and why the two are one command
///
/// While the countdown is running, nothing is running yet: the task is QUEUED,
/// and `Ask::Abort` on a queued task only takes it out of the queue. The runner
/// enrols whatever is due on the next beat, so it would be back — with a fresh
/// countdown — half a minute later. Its own comment says exactly that about the
/// ordering of `put_off` against `enrol`.
///
/// `Ask::Postpone` is what moves the next moment: the runner writes it down as
/// an event, and the row comes back at its own hour instead of immediately.
///
/// So one command, `brain-abort`, means *stop it* in both situations and picks
/// the ask that actually stops it: postpone while it is only announced, abort
/// once something is running. Two commands for that would make the person
/// choose between two words for one intention, and get it wrong at the only
/// moment it matters.
#[must_use]
pub fn abort(mind: &Mind) -> ronoctua_brain::Ask {
    if mind.running.is_empty() {
        ronoctua_brain::Ask::Postpone
    } else {
        ronoctua_brain::Ask::Abort {
            task: mind.running.clone(),
        }
    }
}

/// One of a module's actions, found: whose, which, and the question it asks
/// first when it asks one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acting {
    /// The module's id.
    pub module: String,
    /// The action's id inside it.
    pub action: String,
    /// The registry's name for it, `<module>-<action>` — what the question on
    /// the line is asked under, and what its answer comes back as.
    pub command: String,
    /// The key of the question it asks first, when it asks one.
    pub asks: Option<String>,
}

/// The ask that does an action, with what was typed for it.
#[must_use]
pub fn act_ask(acting: &Acting, answer: Option<&str>) -> ronoctua_brain::Ask {
    ronoctua_brain::Ask::Act {
        module: acting.module.clone(),
        action: acting.action.clone(),
        row: None,
        answer: answer.map(ToOwned::to_owned),
    }
}

/// A question's id is `&'static str` (`crate::line::Line`), and an action's
/// command arrives at run time: made static once per command — see
/// [`NAMES_KEPT`].
#[must_use]
pub fn question_id(command: &str) -> Option<&'static str> {
    interned(command)
}

/// The question an action asks before it is done: the id it is asked under —
/// its command, which the answer comes back as — and the KEY ITS MODULE
/// DECLARED for the words, which the line draws (`Line::asking`).
///
/// # Why the declared key and not `ask-<command>`
///
/// Because a module names its question as it likes (`isKey`, in the
/// manifest), and a line that asked by convention drew the command's label
/// in place of any question spelt otherwise — a fallback nobody would see
/// until the question was the wrong one.
///
/// `None` for an action that asks nothing, or past [`NAMES_KEPT`].
#[must_use]
pub fn question_of(acting: &Acting) -> Option<(&'static str, &'static str)> {
    let asks = acting.asks.as_deref()?;
    Some((interned(&acting.command)?, interned(asks)?))
}

/// Follows the daemon's own log on a thread of its own, sending every batch
/// of lines down `channel` until the other end goes away.
///
/// `true` when the thread started; the daemon itself is started by the same
/// door [`listen`] uses when nobody is home.
pub fn follow(
    channel: smithay_client_toolkit::reexports::calloop::channel::Sender<
        Vec<ronoctua_brain::tail::Line>,
    >,
) -> bool {
    let built = std::thread::Builder::new()
        .name("brain-log".into())
        .spawn(move || {
            let _ = ronoctua_brain::client::tail(|lines| channel.send(lines).is_err());
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the brain log's thread");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_now_postpones_while_nothing_is_running() {
        // THE DEFECT THIS CATCHES: `Ask::Abort` on a task that is only QUEUED
        // takes it out of the queue, and the runner enrols it again on the next
        // beat. What moves the next moment is the postponement.
        let mut mind = Mind::default();
        assert_eq!(abort(&mind), ronoctua_brain::Ask::Postpone);
        mind.running = "fixture".to_owned();
        assert_eq!(
            abort(&mind),
            ronoctua_brain::Ask::Abort {
                task: "fixture".to_owned()
            }
        );
    }
}
