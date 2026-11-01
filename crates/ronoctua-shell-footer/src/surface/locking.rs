//! THE LOCK SCREEN, from the footer's side: starting it (`LEADER s q l`, and
//! the idle watcher), and the automatic lock armed by the person's choice.
//!
//! # Why the footer holds the watch, and not the lock
//!
//! The footer is the session's one long-lived process with a Wayland
//! connection, and the watch is a notification per seat on it
//! (`crate::idle_notify`). The lock is a process of its OWN — `ronoctua-shell
//! lock`, run by its unit, `ronoctua-lock.service` — so the footer dying, or
//! being restarted by a `switch`, never opens a locked session; and the lock
//! dying is the unit's to answer (it starts another, which takes the
//! compositor's lock over).
//!
//! # The unit, and the child where there is none
//!
//! The installed session asks systemd (`systemctl --user start --no-block`):
//! starting a unit that is active is nothing, which is what a second key or a
//! second idle wants. Where there is no unit — the nested loop, which must
//! NEVER lock the real session, and a test's machine — the lock is this
//! footer's child, watched by a thread: one that died after locking is
//! started again (a few times a minute at most), since a dead locker leaves
//! the session locked with nobody to open it.

use super::Footer;
use crate::idle_notify::{Idler, Verdict, verdict};
use smithay_client_toolkit::reexports::calloop::{self, channel};

/// The lock's unit, as `nix/modules/scroll.nix` declares it.
const UNIT: &str = "ronoctua-lock.service";

/// The `systemctl`, by path — a unit's `PATH` is minimal.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// The status a lock that refused to lock ends with — nothing to start again.
const REFUSED: i32 = 78;

/// How many times a minute a lock that died is started again by this footer.
const AGAIN_A_MINUTE: usize = 5;

/// How long PipeWire is given to say whether sound plays. `pw-dump` answers
/// in milliseconds; one that does not answer at all (a daemon wedged) would
/// otherwise leave the automatic lock waiting on it for good — past this,
/// the sound is unknown, and unknown sound locks (`idle_notify::verdict`).
const SOUND_ANSWERS_IN: std::time::Duration = std::time::Duration::from_secs(10);

/// What the lock's threads hand the loop.
#[derive(Debug)]
pub(crate) enum Heard {
    /// `lock.json` moved.
    ChoiceMoved,
    /// Whether sound plays now — `None`: it could not be asked — for the
    /// question of that number.
    Sound(u64, Option<bool>),
    /// The lock is the unit's now: nothing more to wait for here.
    Started,
    /// The lock this footer started as its child ended, with this status
    /// (`None`: killed by a signal).
    Ended(Option<i32>),
    /// It could not be started at all.
    Failed(String),
}

/// What the footer keeps of the lock and its watch.
#[derive(Debug, Default)]
pub(crate) struct Locking {
    /// Where the threads answer; `None` outside the loop (tests).
    pub(crate) sender: Option<channel::Sender<Heard>>,
    /// A lock this footer started is up (its child, or a start in flight).
    pub(crate) up: bool,
    /// The question about the sound being asked, by its number.
    asking: Option<u64>,
    /// How many questions about the sound were asked — so an answer that
    /// comes after its time is not taken for the next question's.
    asked: u64,
    /// The loop, for the time the sound's answer is given.
    pub(crate) handle: Option<calloop::LoopHandle<'static, Footer>>,
    /// When a dead child lock was started again, for the bound.
    again: Vec<std::time::Instant>,
    /// The watch on `lock.json`.
    pub(crate) _watch: Option<ronoctua_files::notice::Notice>,
}

impl Idler for Footer {
    fn idle(&mut self) -> &mut crate::idle_notify::Idle {
        &mut self.idle
    }

    /// EVERY SEAT IS STILL: is sound playing? Asked off the loop — `pw-dump`
    /// is a program.
    fn idle_everywhere(&mut self) {
        if self.locking.asking.is_some() {
            return;
        }
        let Some(sender) = self.locking.sender.clone() else {
            return;
        };
        self.locking.asked = self.locking.asked.wrapping_add(1);
        let number = self.locking.asked;
        self.locking.asking = Some(number);
        let started = std::thread::Builder::new()
            .name("lock-sound".into())
            .spawn(move || {
                let playing = ronoctua_audio::playing()
                    .map_err(
                        |e| tracing::warn!(error = %e, "whether sound plays could not be asked"),
                    )
                    .ok();
                let _ = sender.send(Heard::Sound(number, playing));
            });
        if let Err(e) = started {
            self.locking.asking = None;
            tracing::warn!(error = %e, "no thread to ask the sound; the screen locks");
            self.lock_the_screen();
            return;
        }
        // AND A TIME FOR THE ANSWER: past it, the sound is unknown.
        if let Some(handle) = self.locking.handle.as_ref() {
            let armed = handle.insert_source(
                calloop::timer::Timer::from_duration(SOUND_ANSWERS_IN),
                move |_, (), state: &mut Self| {
                    if state.locking.asking == Some(number) {
                        tracing::warn!("PipeWire did not say in time whether sound plays");
                        state.sound_answered(number, None);
                    }
                    calloop::timer::TimeoutAction::Drop
                },
            );
            if let Err(e) = armed {
                tracing::warn!(error = %e, "no time set for the sound's answer");
            }
        }
    }
}

impl Footer {
    /// Hangs the lock's channel and the watch on `lock.json`, and arms the
    /// automatic lock as the file says now.
    pub(super) fn hang_the_lock(
        &mut self,
        handle: &calloop::LoopHandle<'static, Self>,
    ) -> anyhow::Result<()> {
        let (sender, heard) = channel::channel::<Heard>();
        handle
            .insert_source(heard, |event, (), state: &mut Self| {
                if let channel::Event::Msg(h) = event {
                    state.lock_heard(h);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the lock's channel: {e}"))?;
        let moved = sender.clone();
        self.locking._watch =
            ronoctua_shell_kit::lock_choice::watch(ronoctua_store::config_dir(), move || {
                moved.send(Heard::ChoiceMoved).is_ok()
            });
        self.locking.sender = Some(sender);
        self.locking.handle = Some(handle.clone());
        // THE SEATS THE COMPOSITOR HAD BEFORE THIS FOOTER CAME: the toolkit
        // says `new_seat` only for one announced later.
        for seat in self.seats.seats() {
            self.idle.seat_came(&seat, &self.queue);
        }
        self.lock_choice_moved();
        Ok(())
    }

    /// The person's choice, read again, and the watch armed by it.
    fn lock_choice_moved(&mut self) {
        use ronoctua_shell_kit::lock_choice::{Armed, armed_by, store};
        let after = match armed_by(&store().reading()) {
            Armed::Off => {
                tracing::info!("the screen does not lock by itself");
                None
            }
            Armed::After(wait) => {
                tracing::info!(
                    minutes = wait.as_secs() / 60,
                    "the screen locks by itself after a time without use"
                );
                Some(wait)
            }
            Armed::Refused => {
                tracing::warn!(
                    "lock.json did not read whole: the screen does not lock by itself until it is chosen again"
                );
                None
            }
        };
        if after != self.idle.armed() {
            self.idle.arm(after, &self.queue);
        }
    }

    /// The answer to the sound's question `number` — PipeWire's, or `None`
    /// when it could not be asked or did not answer in time. An answer to a
    /// question no longer asked is nothing.
    fn sound_answered(&mut self, number: u64, playing: Option<bool>) {
        if self.locking.asking != Some(number) {
            return;
        }
        self.locking.asking = None;
        match verdict(self.idle.everywhere(), playing) {
            Verdict::Nothing => {}
            Verdict::WaitAgain => {
                tracing::info!("idle while sound plays: the lock waits for the next idle");
                self.idle.again(&self.queue);
            }
            Verdict::Lock => {
                tracing::info!(
                    sound_known = playing.is_some(),
                    "idle for the time chosen: the screen locks"
                );
                self.lock_the_screen();
            }
        }
    }

    /// What a thread of the lock handed back.
    fn lock_heard(&mut self, heard: Heard) {
        match heard {
            Heard::ChoiceMoved => self.lock_choice_moved(),
            Heard::Sound(number, playing) => self.sound_answered(number, playing),
            Heard::Started => self.locking.up = false,
            Heard::Ended(status) => {
                self.locking.up = false;
                match status {
                    Some(0) => {}
                    Some(REFUSED) => {
                        self.echo_message(self.texts.t(crate::key::ECHO_LOCK_FAILED));
                        self.draw_and_paint();
                    }
                    // A LOCK THAT DIED after locking leaves the session locked
                    // with nobody to open it: another takes it over.
                    other => {
                        let now = std::time::Instant::now();
                        self.locking.again.retain(|t| {
                            now.duration_since(*t) < std::time::Duration::from_secs(60)
                        });
                        if self.locking.again.len() < AGAIN_A_MINUTE {
                            self.locking.again.push(now);
                            tracing::warn!(status = ?other, "the lock died; another takes the session over");
                            self.lock_the_screen();
                        } else {
                            tracing::error!(status = ?other, "the lock keeps dying; not started again this minute");
                        }
                    }
                }
            }
            Heard::Failed(why) => {
                self.locking.up = false;
                tracing::warn!(why = %why, "the lock could not be started");
                self.echo_message(self.texts.t(crate::key::ECHO_LOCK_FAILED));
                self.draw_and_paint();
            }
        }
    }

    /// LOCKS THE SCREEN — `LEADER s q l`, and an idle the person chose. Off
    /// the loop: `systemctl` and a child's wait are a thread's.
    pub(super) fn lock_the_screen(&mut self) {
        if self.locking.up {
            return;
        }
        let Some(sender) = self.locking.sender.clone() else {
            return;
        };
        // A SECOND COPY OF THE SYSTEM never asks the session's unit: the
        // nested loop's lock is the nested compositor's, its own child.
        let unit = ronoctua_xdg::instance().is_none();
        let me = std::env::current_exe();
        self.locking.up = true;
        let started = std::thread::Builder::new()
            .name("lock".into())
            .spawn(move || {
                if unit {
                    let asked = std::process::Command::new(SYSTEMCTL)
                        .args(["--user", "start", "--no-block", UNIT])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                    match asked {
                        Ok(s) if s.success() => {
                            tracing::info!(unit = UNIT, "the screen is locked by its unit");
                            let _ = sender.send(Heard::Started);
                            return;
                        }
                        other => tracing::info!(
                            answer = ?other,
                            "no lock unit to start; the lock is this footer's child"
                        ),
                    }
                }
                let me = match me {
                    Ok(me) => me,
                    Err(e) => {
                        let _ = sender.send(Heard::Failed(format!("no binary of my own: {e}")));
                        return;
                    }
                };
                let child = crate::watchdog::unshared(&mut std::process::Command::new(me))
                    .arg(crate::session_lock::LOCK_WORD)
                    .stdin(std::process::Stdio::null())
                    .spawn();
                match child {
                    Ok(mut c) => {
                        tracing::info!(pid = c.id(), "the screen is locked by this footer's child");
                        let status = c.wait().ok().and_then(|s| s.code());
                        let _ = sender.send(Heard::Ended(status));
                    }
                    Err(e) => {
                        let _ = sender.send(Heard::Failed(e.to_string()));
                    }
                }
            });
        if let Err(e) = started {
            self.locking.up = false;
            tracing::warn!(error = %e, "no thread to start the lock");
        }
    }
}
