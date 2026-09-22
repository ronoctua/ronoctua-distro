//! What each window, workspace and screen command asks of the compositor.
//!
//! # One table, two consumers
//!
//! `surface::run_command` dispatches through it, and the contract test
//! walks all of it against an actual compositor. Two copies would diverge —
//! and the one that diverges silently is always the one nobody runs.
//!
//! # Why the deed's text is declared and not written here
//!
//! This compositor's commands are free text, spread across four tables in C
//! with no enumeration anywhere. So the table is OURS:
//! `assets/compositor-deeds.nix` declares every one, the generator refuses a
//! verb the pinned compositor does not have, and what crosses the wire is
//! built by `ronoctua_scroll::wire::spoken`, which validates the argument. A
//! deed that does not exist is a build failure rather than a key that answers
//! nothing.

/// `(command id, the deed it speaks)`.
///
/// A function and not a `static`: a `serde_json::Value` has to allocate, and there is no
/// allocation at compile time. Building it costs microseconds and happens
/// once per key.
///
/// The ids are the same as the keymap's in `leader::default_of`. The
/// `keymap-consistente` check guarantees that every binding points at a command that
/// exists; the generator refuses at BUILD time a deed the compositor does
/// not have.
#[must_use]
pub fn all_of() -> Vec<(&'static str, &'static str)> {
    vec![
        ("window-left", "wm-focus-left"),
        ("window-right", "wm-focus-right"),
        ("window-up", "wm-focus-up"),
        ("window-down", "wm-focus-down"),
        ("window-move-left", "wm-move-left"),
        ("window-move-right", "wm-move-right"),
        ("window-move-up", "wm-move-up"),
        ("window-move-down", "wm-move-down"),
        ("window-maximize", "wm-maximise"),
        ("window-screen-full", "wm-full-screen"),
        ("window-center", "wm-align-centre"),
        ("window-float", "wm-float"),
        // CONSUME AND EXPEL, in this compositor's words. `nomode` moves the
        // WINDOW and leaves its column alone, so left and right take it out
        // of the column it is in and put it into the one beside it.
        ("window-consume", "wm-move-out-right"),
        ("window-expel", "wm-move-out-left"),
        ("buffer-close", "wm-close"),
        ("buffer-next", "wm-focus-down"),
        ("workspace-next", "wm-workspace-next"),
        ("workspace-overview", "wm-overview"),
        ("workspace-previous", "wm-workspace-previous"),
        ("workspace-move-next", "wm-workspace-move-next"),
        ("workspace-move-previous", "wm-workspace-move-previous"),
        ("workspace-go", "wm-workspace-go"),
        ("workspace-move", "wm-workspace-move"),
    ]
}

/// The deed a keymap command performs, if it performs one.
///
/// It is the join between the two vocabularies: ours, which the keymap and
/// the text keys use, and the compositor's, which the generated table holds.
#[must_use]
pub fn deed_of(id: &str) -> Option<&'static ronoctua_scroll::deeds::Spoken> {
    all_of()
        .into_iter()
        .find(|(ours, _)| *ours == id)
        .and_then(|(_, deed)| ronoctua_scroll::deeds::spoken_of(deed))
}

/// Whether this command needs something typed after it.
///
/// The answer comes from the DEED, which carries its own arity — not from a
/// list here that would age the day a command grew or lost an argument.
#[must_use]
pub fn with_argument(id: &str) -> Option<&'static str> {
    deed_of(id)
        .filter(|s| s.deed.argument.is_some())
        .map(|s| s.deed.id)
}

/// A chord: the leader **held** plus a key.
///
/// # Why it is not in the shell's keymap
///
/// The footer only receives the keyboard after the leader is released — it is the compositor
/// that sees the key and tells the daemon (§7.4). A chord, by definition, never
/// gets there: what resolves it is the compositor, and that is why holding the leader **does
/// not open the list nor take focus from the window**. They are two mechanisms, and it is
/// deliberate that they are.
///
/// # And why that does not recreate two truths
///
/// The compositor's bind is GENERATED from here, and which-key also READS from
/// here. What the chord does and how it appears on screen come from a single
/// place.
pub struct Chord {
    /// How it appears in the list: `←`, `1…9`, `S-f`.
    ///
    /// One line per chord would be illegible with nine workspaces; the range
    /// appears condensed and the binds all come out.
    pub shows: &'static str,
    /// The **command** it runs. Always an id from the registry.
    ///
    /// It is not a label: it is the id. The label comes from it (`cmd-<id>`), so a chord
    /// has no way of showing one command's text and doing another's — which is
    /// exactly what the `Registry` came to refuse.
    pub command: &'static str,
    /// `(key, the command text the compositor is given)`, already expanded.
    pub binds: Vec<(String, String)>,
}

impl Chord {
    /// The label's text key, derived from the command's id.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}{}", ronoctua_cmd::KEY_PREFIX_OF, self.command)
    }
}

/// A chord that reuses a keymap COMMAND's action.
///
/// The bind's text comes from the same deed `run_command` sends over IPC — the
/// two spellings of one name, one derived from the other. It is what keeps
/// `LEADER w h` and `LEADER+←` from diverging.
fn of_command(shows: &'static str, key: &str, id: &'static str) -> Chord {
    let said = deed_of(id)
        .unwrap_or_else(|| panic!("the chord `{key}` points at `{id}`, which does not exist"))
        .text;
    Chord {
        shows,
        command: id,
        binds: vec![(key.to_owned(), said.to_owned())],
    }
}

/// A chord for a command the compositor does **not** run.
///
/// # Why it goes through our binary
///
/// Opening a terminal is not a deed of the compositor's: what does it is `run_command`, which puts the
/// process in a systemd scope of its own so §7.5's PID → cgroup correlation
/// works. The bind sends `ronoctua-shell command <id>`, the same
/// path as the sequence — before it spawned the terminal directly, with no scope, and
/// the same feature had two behaviours depending on which key was used.
fn through_shell(shows: &'static str, key: &str, id: &'static str) -> Chord {
    Chord {
        shows,
        command: id,
        binds: vec![(
            key.to_owned(),
            format!("exec \"{}\" command {id}", shell_path()),
        )],
    }
}

/// A range from 1 to 9: nine binds, one line in the list.
///
/// These are the only chords whose deed requires an **argument**, and that is
/// why the text does not come from the plain table — a number has to go into
/// it. `wire::spoken` is what puts it there, and it refuses anything that is
/// not one; `compositor-contract` walks the result against a real compositor.
fn range_of(shows: &'static str, command: &'static str, prefix: &str) -> Chord {
    // THE TEXT COMES FROM THE TABLE, and the number goes in through the
    // deed's own validation — so a bind cannot carry something the socket
    // would refuse, and a placeholder that moved is a panic here rather than
    // nine keys that do nothing.
    let spoken = deed_of(command).unwrap_or_else(|| {
        panic!("the band `{shows}` points at `{command}`, which does not exist")
    });
    Chord {
        shows,
        command,
        binds: (1..=9)
            .map(|n| {
                let said =
                    ronoctua_scroll::wire::spoken(&spoken.deed, spoken.text, Some(&n.to_string()))
                        .unwrap_or_else(|e| panic!("the band `{shows}` cannot say {n}: {e}"));
                (format!("{prefix}{n}"), said)
            })
            .collect(),
    }
}

/// This binary's path, so the compositor spawns the SAME one that generated the bind.
///
/// A bare name will not do: in the nested loop the shell is not in the
/// compositor's `PATH`, and a systemd unit has a minimal `PATH` — the symptom
/// shows up far from the cause.
fn shell_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "ronoctua-shell".to_owned())
}

/// All the chords.
///
/// The key comes WITHOUT the modifier's prefix: it is `Mod` in production and `Alt`
/// in the nested loop, and whoever knows that is whoever generates the config.
#[must_use]
pub fn chords() -> Vec<Chord> {
    vec![
        // --- focus: arrows AND hjkl, both forms ---
        of_command("←", "Left", "window-left"),
        of_command("→", "Right", "window-right"),
        of_command("↑", "Up", "window-up"),
        of_command("↓", "Down", "window-down"),
        of_command("h", "H", "window-left"),
        of_command("l", "L", "window-right"),
        of_command("k", "K", "window-up"),
        of_command("j", "J", "window-down"),
        // --- moving the window: the same keys with Shift ---
        of_command("S-←", "Shift+Left", "window-move-left"),
        of_command("S-→", "Shift+Right", "window-move-right"),
        of_command("S-↑", "Shift+Up", "window-move-up"),
        of_command("S-↓", "Shift+Down", "window-move-down"),
        // --- window ---
        of_command("q", "Q", "buffer-close"),
        of_command("f", "F", "window-maximize"),
        of_command("S-f", "Shift+F", "window-screen-full"),
        of_command("c", "C", "window-center"),
        of_command("t", "T", "window-float"),
        // --- workspaces ---
        of_command("g", "G", "workspace-overview"),
        range_of("1…9", "workspace-go", ""),
        range_of("S-1…9", "workspace-move", "Shift+"),
        of_command(",", "Comma", "window-consume"),
        of_command(".", "Period", "window-expel"),
        // THE MOUSE WHEEL is a chord too: with the leader held, scrolling
        // switches workspace. It is the only shortcut in the system that is not a key.
        // THE NOTATION AND NOT A WORD. `shows` is drawn on screen by
        // `manual.rs` (`"{chord} + {shows}"`), so these two were a hard-coded
        // UI string (invariant 8) AND a Portuguese one (invariant 18): an
        // English reader opening `LEADER h s` read "SUPER + roda ↓". Every
        // other chord's `shows` is a key notation, which is language-free,
        // and a wheel has one too — the angle brackets this system already
        // uses for `<down>` and `<up>`.
        //
        // `button5` AND `button4`, which is what this compositor calls the
        // wheel: `get_mouse_bindsym` maps `button1..9` onto the X11 numbering,
        // and 4 and 5 are `SWAY_SCROLL_UP` and `SWAY_SCROLL_DOWN`
        // (`sway/input/cursor.c`). The previous compositor's spelling —
        // `WheelScrollDown` — is refused with `Unknown key or button`, one
        // line at a time, which leaves every OTHER bind working and these two
        // silently gone.
        of_command("<wheel-down>", "button5", "workspace-next"),
        of_command("<wheel-up>", "button4", "workspace-previous"),
        // --- opening ---
        through_shell("RET", "Return", "buffer-terminal"),
        // THE MONITOR, held-leader plus `Esc` — and `Ctrl+Alt+Esc` in
        // `emergency()` does the same thing without the leader.
        //
        // NOTHING COMPETES FOR IT any more: we pass `-c`, the compositor's
        // own shipped configuration is never read, and every bind in the
        // session comes from this table. It used to take the key from an
        // upstream default and rely on the deduplication in `full_chords` to
        // drop that one.
        through_shell("ESC", "Escape", "monitor-btop"),
        through_shell("S-ESC", "Shift+Escape", "monitor-bottom"),
    ]
}

/// All the chords, and every one of them is ours.
///
/// # Why there is nothing to merge
///
/// The compositor is started with `-c <path in the store>`, so the
/// configuration it ships is never read and there are no defaults of its own to
/// collide with. This used to fold in a table generated from that shipped file,
/// with ours winning wherever the two met.
///
/// What is left to do here is reserve the emergency binds, which come out
/// WITHOUT the leader's prefix and so are not protected by being in the same
/// list.
/// The key in a form that does not depend on how it was written.
///
/// # Why this exists
///
/// `Alt+Ctrl+L` and `Ctrl+Alt+L` are the SAME key and two different strings, so
/// a deduplication that compares strings lets a collision straight through.
///
/// What a collision costs HERE is quieter than it used to be, and that is the
/// argument for keeping this. The previous compositor refused the whole file
/// over a duplicate keybind, which took every shortcut down at once and was at
/// least loud. This one OVERWRITES: `bindsym` logs "Overwriting binding" at
/// `SWAY_INFO` and carries on, so the first of the two simply stops existing —
/// and with the nag window refused (`scrollnag_command -`,
/// `nix/modules/scroll.nix`) a log line is the only trace there is.
fn canonical_key(key: &str) -> String {
    let mut parts: Vec<String> = key.split('+').map(str::to_lowercase).collect();
    // The LAST one is the key; the rest are modifiers, and their order means
    // nothing.
    if let Some((_, mods)) = parts.split_last_mut() {
        mods.sort_unstable();
    }
    parts.join("+")
}

/// The leader prefixes this system uses.
///
/// The collision depends on the prefix: `Alt+Ctrl+L` only clashes with `Ctrl+Alt+L` in the
/// nested loop. Checking BOTH and dropping in either keeps dev and production
/// with the same keyboard — which is worth more than one extra chord in one of the two.
const PREFIXES: [&str; 2] = ["Mod", "Alt"];

#[must_use]
pub fn full_chords() -> Vec<Chord> {
    let out = chords();
    let mut used: std::collections::BTreeSet<String> = out
        .iter()
        .flat_map(|a| a.binds.iter().map(|(t, _)| canonical_key(t)))
        .collect();

    // THE EMERGENCY BINDS ARE RESERVED.
    //
    // They come out of the config WITHOUT the leader's prefix, so a chord that lands
    // on the same key does not "lose to ours": it breaks the whole file.
    for (key, _, _) in emergency() {
        for p in PREFIXES {
            used.insert(canonical_key(key));
            // And in the form the chord would have with each prefix.
            if let Some(rest) = canonical_key(key)
                .strip_prefix(&format!("{}+", p.to_lowercase()))
                .map(str::to_owned)
            {
                used.insert(rest);
            }
        }
    }

    // THE COMPOSITOR'S OWN DEFAULTS ARE NOT FOLDED IN, and that is a change
    // worth stating rather than leaving as an absence.
    //
    // They used to be, out of a table generated from the compositor's shipped
    // configuration, so that a chord of ours and one of its own could not land
    // on the same key. That is gone because the premise is: we pass `-c` and
    // its shipped configuration is never read, so there are no defaults to
    // collide with. Every bind in this session is ours.
    //
    // What that does NOT change is `canonical_key` above: two lines of OURS on
    // one key would still make the compositor refuse the whole file, and a
    // refused file is a session with no shortcuts at all.
    out
}

/// The keyboard's own top row — the media keys.
///
/// # Why this is a family of its own and not four more emergencies
///
/// [`emergency`] below is four keys that exist for when the leader does NOT
/// respond, and its "why only four" is an argument about RECOVERY. Adding a
/// volume key to that list would make the argument false without changing a
/// word of it.
///
/// These are here for a different and simpler reason: **no buffer receives
/// them.** `XF86AudioRaiseVolume` is not on any keymap a program types into,
/// so the objection §7.4 raises against bare binds — every one is a key the
/// buffers stop receiving — does not apply. What is being taken is a key
/// nobody had.
///
/// # Why they were not here before
///
/// `nix/scripts/gen-default-binds.py` says the `XF86*` keys "wait for the
/// system to handle media keys", and structurally it cannot emit one: its
/// regex only matches lines beginning `Mod+`. The system it was waiting for is
/// this one, and until now this one never handled them — measured, there was
/// not a single `XF86` string in the whole repository, so on this distro the
/// volume keys of every laptop did nothing at all.
///
/// # What each one reaches
///
/// The first four go to `ronoctua-audio` through the footer's socket, which
/// is what makes them agree with the view: one command, one answer, whether
/// it came from a key on the keyboard or a row on a screen.
///
/// The last three go to `playerctl`, and its absence is a SENTENCE rather
/// than silence — see `Footer::media`.
///
/// It returns `(key, the command text, our command's id)`, like [`emergency`].
#[must_use]
pub fn media() -> Vec<(&'static str, String, &'static str)> {
    let shell = shell_path();
    [
        ("XF86AudioRaiseVolume", "volume-up"),
        ("XF86AudioLowerVolume", "volume-down"),
        ("XF86AudioMute", "volume-mute"),
        ("XF86AudioMicMute", "mic-mute"),
        ("XF86AudioPlay", "media-play"),
        ("XF86AudioNext", "media-next"),
        ("XF86AudioPrev", "media-previous"),
    ]
    .into_iter()
    .map(|(key, id)| (key, format!("exec \"{shell}\" command {id}"), id))
    .collect()
}

/// The binds that exist **outside** the leader, on purpose.
///
/// # Why they are not chords
///
/// A chord starts from the leader. These exist exactly for when the leader
/// **does not respond** — if `keyd` falls over, Super goes back to being just a modifier and
/// tapping it no longer emits `F13`. A recovery shortcut that depended on the
/// leader would be the lock inside the safe.
///
/// # Why only four
///
/// Every bind here is a key the buffers stop receiving, and §7.4 wants
/// **one** keymap. Two of them close the loop: one to reach the command
/// list, another to fix the most likely cause of it not opening.
///
/// [`media`] above is bare too and is NOT one of these four, deliberately:
/// this list is about recovery, and a volume key is not. The argument that
/// keeps this list short — a bare bind costs the buffers a key — is also the
/// argument that lets that one exist, because no buffer ever receives an
/// `XF86Audio*`.
///
/// The third is `Ctrl+Alt+Esc`, and it earns its place by the same argument
/// rather than by convenience. What one reaches for when the machine stops
/// answering is the process monitor — and a machine that is thrashing is
/// exactly a machine where the leader may not arrive: it depends on `keyd`
/// telling a tap from a hold, on the daemon being scheduled, and on the footer
/// being alive to expand. This one asks the compositor to spawn, and nothing
/// else. It is the key everyone already presses for this, and it is the
/// companion of `nix/modules/memory.nix`: that file keeps the desktop alive
/// under pressure, and this one is how the person sees WHAT is eating the
/// machine.
///
/// It returns `(key, the command text, our command's id)`.
#[must_use]
pub fn emergency() -> Vec<(&'static str, String, &'static str)> {
    let shell = shell_path();
    vec![
        // THE ALTERNATIVE LEADER. It is not a command: it is the same trigger as the
        // sequence's through another key, and that is why it sends `leader` and not
        // `command`. `Ctrl+Alt+l` is an ordinary chord — the compositor sees it without going
        // through keyd, which is the point.
        //
        // AND THE LETTER IS LOWERCASE, which is not cosmetic. A bind here matches
        // a KEYSYM, and `L` is the keysym Shift produces — so `Ctrl+Alt+L`
        // would need a Shift nobody is holding and would never fire at all.
        // The one below is `Ctrl+Shift+l` for the same reason from the other
        // side: with Shift held the TRANSLATED keysym is `L`, and what a bind
        // is written against is the RAW one (`state_keysyms_raw`,
        // `sway/input/keyboard.c`), which stays `l`.
        (
            "Ctrl+Alt+l",
            format!("exec \"{shell}\" leader"),
            LEADER_ALTERNATE,
        ),
        (
            "Ctrl+Shift+l",
            format!("exec \"{shell}\" command keyboard-restore"),
            "keyboard-restore",
        ),
        // THE PROCESS MONITOR, without the leader. `Escape` and not `Delete`:
        // `Delete` is the rescue console's, one line below, and the two are
        // neighbours on purpose — what one reaches for first is "what is eating
        // the machine", and what one reaches for when that is not enough is the
        // console that can restart or roll back.
        (
            "Ctrl+Alt+Escape",
            format!("exec \"{shell}\" command monitor-btop"),
            "monitor-btop",
        ),
        // THE RESCUE CONSOLE, and the fourth is different from the other three.
        //
        // `Ctrl+Alt+Delete` USED TO BE systemd's reboot on the console, and this
        // file said so where this comment now is. It is ours because we took it:
        // `nix/modules/rescue.nix` redefines `ctrl-alt-del.target`, so one press
        // opens this instead of ending the session mid-sentence — and the reboot
        // it used to do is a row inside the console, which asks first.
        //
        // The hard way out stays: seven presses in two seconds still reboot by
        // force, and that one does not pass through anything of ours.
        //
        // What makes this bind unlike the other three is that it is not the only
        // way in. The other three ask the COMPOSITOR to spawn, so all three are
        // gone exactly when the compositor is what broke. This key has a second
        // door — systemd's — and the two open the same program.
        (
            "Ctrl+Alt+Delete",
            format!("exec \"{shell}\" command rescue-open"),
            "rescue-open",
        ),
    ]
}

/// What the alternative leader fires — the leader, and not a command.
///
/// It exists as a constant so the reachability test can tell the difference: it is the
/// only emergency bind that does **not** point at a registry id.
pub const LEADER_ALTERNATE: &str = "";

/// `xdg-open`, baked at BUILD time.
///
/// It is the same constant `surface.rs` already carries, for the same reason:
/// it is always installed, it is tiny, and a store path that is always wanted
/// belongs in the binary. Two constants would be two answers.
const XDG_OPEN: Option<&str> = option_env!("RONOCTUA_XDG_OPEN");

/// Where the program a notification's action starts lives, if it is installed.
///
/// # Two programs, two sources, and that is the whole point
///
/// `xdg-open` is BAKED: every profile has it, and the rest of this file already
/// reads it that way.
///
/// `swappy` is read from the ENVIRONMENT, and must be: it lives in the `full`
/// profile only, and a store path compiled in is a reference the closure
/// carries whether the switch is on or not — the measured trap `RONOCTUA_ICONS`
/// carries a comment about in `nix/rust.nix`. The
/// `screenshot-notification-contract` check holds that line from the Nix side.
///
/// # The defect this shape came out of
///
/// Both were read from the environment, and `RONOCTUA_XDG_OPEN` only exists at
/// build time — so at runtime every `Run` action was dropped as "not
/// installed", and the screenshot's band came out two lines instead of three,
/// with no actions and nothing failing. It took a screenshot to see.
///
/// `None` makes the action DISAPPEAR from the notification rather than fail
/// when clicked. An offer that does nothing is worse than one never made.
///
/// A PATH and never a name: a systemd unit's `PATH` is minimal, and the failure
/// shows up far from the cause — the trap the `AGENTS.md` names.
#[must_use]
pub fn notification_program(program: ronoctua_notify::Program) -> Option<String> {
    match program {
        ronoctua_notify::Program::Open => XDG_OPEN.map(ToOwned::to_owned),
        ronoctua_notify::Program::Edit => std::env::var("RONOCTUA_SWAPPY").ok(),
    }
    .filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_ids_are_kebab() {
        // `kebab-case`, like every registry id (section 13) — and the dot is
        // REFUSED, because the text key is the id with a prefix. `run_command`
        // consults this table by id, with no fixed prefix: a new id comes
        // in here and starts working, without touching the dispatch.
        for (id, _) in super::all_of() {
            assert!(
                !id.contains('.'),
                "`{id}` still has a dot; a command id is pure kebab"
            );
            assert!(
                !id.is_empty()
                    && !id.starts_with('-')
                    && !id.ends_with('-')
                    && id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "`{id}` is not a kebab-case id"
            );
        }
    }

    #[test]
    fn every_command_points_at_a_deed_that_exists() {
        // A command naming a deed the generated table does not have is a key
        // that does nothing and says nothing. `deed_of` is what every caller
        // goes through, so asking it here asks the same question they do.
        for (id, deed) in super::all_of() {
            assert!(
                super::deed_of(id).is_some(),
                "`{id}` points at `{deed}`, which is not in the table"
            );
        }
    }

    #[test]
    fn every_chord_produces_a_bind_and_a_label() {
        // `of_command` already panics if the id does not exist — building the
        // table is the test. What is asserted here is the rest: no chord
        // comes out without a bind (it would become a line in the list that does nothing) and
        // none comes out without a label (it would become `[key]` on screen).
        for a in super::chords() {
            assert!(
                !a.binds.is_empty(),
                "the chord `{}` generates no bind",
                a.shows
            );
            assert!(!a.shows.is_empty(), "a chord with no key to show");
            assert!(
                a.label().starts_with(ronoctua_cmd::KEY_PREFIX_OF),
                "the label of `{}` is not a command key: {}",
                a.shows,
                a.label()
            );
            for (key, said) in &a.binds {
                assert!(!key.is_empty(), "a bind with no key in `{}`", a.shows);
                assert!(!said.is_empty(), "a bind with no deed in `{}`", a.shows);
            }
        }
    }

    #[test]
    fn a_chord_says_exactly_what_its_command_says() {
        // The bind and the command must not diverge: `LEADER w h` and
        // `LEADER+←` are one feature, and two spellings of it would be two
        // behaviours the day one moved.
        for a in super::chords() {
            let Some(spoken) = super::deed_of(a.command) else {
                continue; // it goes through our binary — see `through_shell`.
            };
            for (key, said) in &a.binds {
                if said.starts_with("exec ") {
                    continue;
                }
                if spoken.deed.argument.is_some() {
                    // A band carries the number, so it is the text with the
                    // placeholder filled in rather than the text itself.
                    let stem = spoken.text.split("{n}").next().unwrap();
                    assert!(
                        said.starts_with(stem),
                        "the chord `{key}` does not say what `{}` says",
                        a.command
                    );
                } else {
                    assert_eq!(
                        said, spoken.text,
                        "the chord `{key}` diverges from the command `{}`",
                        a.command
                    );
                }
            }
        }
    }

    #[test]
    fn every_curated_id_reaches_a_deed() {
        // THE DEFECT: the dispatch asked the GENERATED table with OUR id.
        //
        // There are two vocabularies — `window-left` is the keymap's and
        // `wm-focus-left` is the compositor's — and `deeds::spoken_of` only
        // answers the second. So `LEADER w h` and twelve of its neighbours
        // drew a row in which-key, echoed "nothing to do", and moved nothing.
        // `run-command-handles-every-key` named thirteen at once, which is how
        // it was found.
        //
        // This test is the same question asked at the JOIN rather than at the
        // dispatch, so it fails in a second instead of in a VM.
        for (ours, theirs) in super::all_of() {
            let spoken = super::deed_of(ours)
                .unwrap_or_else(|| panic!("`{ours}` is in the curated table and reaches no deed"));
            assert_eq!(
                spoken.deed.id, theirs,
                "`{ours}` should reach `{theirs}` and reaches `{}`",
                spoken.deed.id
            );
        }
    }

    #[test]
    fn a_deed_id_still_answers_on_its_own() {
        // THE OTHER HALF of the same arm: the compositor's own ids have no key
        // and are reached by NAME from `LEADER :`, so the fallback has to keep
        // answering them. Asking only the curated join would take that away
        // and nothing else would notice — those hundred-odd commands have no
        // keymap entry for a check to count.
        let spoken = ronoctua_scroll::deeds::spoken_of("wm-focus-left")
            .expect("the compositor's own ids answer on their own");
        assert_eq!(spoken.deed.id, "wm-focus-left");
        assert!(
            super::deed_of("wm-focus-left").is_none(),
            "it is not a curated id"
        );
    }

    #[test]
    fn no_chord_repeats_a_key() {
        // Two chords on one key are two `bindsym` lines for it: the compositor
        // overwrites, the last one wins, and the first disappears with nothing
        // but a log line to say so.
        let mut keys: Vec<String> = super::full_chords()
            .into_iter()
            .flat_map(|a| a.binds.into_iter().map(|(t, _)| t))
            .collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), before, "there is a repeated chord key");
    }

    #[test]
    fn every_chord_names_something_that_exists() {
        // A chord that points at nothing is a key that does nothing, and the
        // panic in `of_command` only fires when the chord is BUILT — which
        // this test is what does.
        let all = super::chords();
        assert!(all.len() > 20, "the chord table is there");
    }

    #[test]
    fn there_is_no_repeated_id() {
        // Repeated, the first wins and the second never runs — with no error.
        let mut ids: Vec<_> = super::all_of().into_iter().map(|(k, _)| k).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            before,
            "there is a repeated id in the action table"
        );
    }
}
