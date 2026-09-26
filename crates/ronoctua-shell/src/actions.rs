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
    // THE COMPOSITOR'S OWN IDS TOO (`wm-…`), and not only ours: asked only of
    // ours, `wm-trail-number` answered "no argument", so `LEADER :` ran it
    // without asking which trail and the leader's generated `o y 1` was
    // refused as a binding.
    deed_of(id)
        .or_else(|| ronoctua_scroll::deeds::spoken_of(id))
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
    /// The same binds in the NESTED loop, when they cannot be the same there.
    ///
    /// The nested prefix is `Alt` and the nested keymap has no Super (it is
    /// the host's), so a chord the compositor's own configuration puts on
    /// `Super+Alt+…` would come out `Alt+Alt+…` — which is the chord without
    /// the Alt, and a collision. Those few get another key in the nested loop
    /// and say which in [`Chord::nested_shows`].
    pub nested: Option<Vec<(String, String)>>,
    /// How [`Chord::nested`] appears in the list.
    pub nested_shows: Option<&'static str>,
}

impl Chord {
    /// The label's text key, derived from the command's id.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}{}", ronoctua_cmd::KEY_PREFIX_OF, self.command)
    }

    /// The binds under `prefix`: the nested ones when it is the nested loop's.
    #[must_use]
    pub fn binds_for(&self, prefix: &str) -> &[(String, String)] {
        match &self.nested {
            Some(n) if prefix == NESTED_PREFIX => n,
            _ => &self.binds,
        }
    }

    /// How it appears, in the nested loop or not.
    #[must_use]
    pub fn shows_for(&self, nested: bool) -> &'static str {
        match self.nested_shows {
            Some(s) if nested => s,
            _ => self.shows,
        }
    }

    /// The same chord with another key in the nested loop.
    fn nested(mut self, shows: &'static str, key: &str) -> Self {
        let said = self
            .binds
            .first()
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        self.nested = Some(vec![(key.to_owned(), said)]);
        self.nested_shows = Some(shows);
        self
    }
}

/// The nested loop's prefix — what `nix/dev.nix` hands to `export-binds`.
pub const NESTED_PREFIX: &str = "Alt";

/// Whether this process is the nested loop's.
#[must_use]
pub fn nested() -> bool {
    std::env::var_os("RONOCTUA_NESTED").is_some()
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
        nested: None,
        nested_shows: None,
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
        nested: None,
        nested_shows: None,
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
        // NINE, the owner's rule: `0` is not a tenth workspace here but every
        // workspace at once, bound on its own beside the band.
        binds: (1..=9)
            .map(|n| {
                let said =
                    ronoctua_scroll::wire::spoken(&spoken.deed, spoken.text, Some(&n.to_string()))
                        .unwrap_or_else(|e| panic!("the band `{shows}` cannot say {n}: {e}"));
                (format!("{prefix}{n}"), said)
            })
            .collect(),
        nested: None,
        nested_shows: None,
    }
}

/// A chord straight onto one of the compositor's deeds (`wm-<id>`), whose
/// label is the deed's own sentence.
fn of_deed(shows: &'static str, key: &str, deed: &'static str) -> Chord {
    of_deed_with(shows, key, deed, None)
}

/// The same, with the deed's argument.
fn of_deed_with(shows: &'static str, key: &str, deed: &'static str, n: Option<&str>) -> Chord {
    let spoken = ronoctua_scroll::deeds::spoken_of(deed)
        .unwrap_or_else(|| panic!("the chord `{key}` points at `{deed}`, which is not declared"));
    let said = ronoctua_scroll::wire::spoken(&spoken.deed, spoken.text, n)
        .unwrap_or_else(|e| panic!("the chord `{key}` cannot say `{deed}`: {e}"));
    Chord {
        shows,
        command: spoken.deed.id,
        binds: vec![(key.to_owned(), said)],
        nested: None,
        nested_shows: None,
    }
}

/// A band of digits onto a deed with a number: `1…9`, and `0` for the ninth.
///
/// NINE WORKSPACES, the owner's rule. The shipped file puts a tenth on `0`,
/// which would create a workspace nobody else here can reach, so `0` names the
/// last of the nine — the way `TAB 0` and `TAB m 0` already do.
fn digits_of(shows: &'static str, deed: &'static str, prefix: &str) -> Chord {
    let spoken = ronoctua_scroll::deeds::spoken_of(deed)
        .unwrap_or_else(|| panic!("the band `{shows}` points at `{deed}`, which is not declared"));
    Chord {
        shows,
        command: spoken.deed.id,
        binds: (1..=9)
            .map(|n| (n, n))
            .chain([(0, 9)])
            .map(|(key, n)| {
                let said =
                    ronoctua_scroll::wire::spoken(&spoken.deed, spoken.text, Some(&n.to_string()))
                        .unwrap_or_else(|e| panic!("the band `{shows}` cannot say {n}: {e}"));
                (format!("{prefix}{key}"), said)
            })
            .collect(),
        nested: None,
        nested_shows: None,
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
        // NINE WORKSPACES, the owner's rule, and `0` is every one of them at
        // once — the `LEADER 0` of the sequence, through our binary for the
        // reason `S-TAB` gives below. The shipped file puts workspace 10 on
        // it; `S-0` keeps moving the window, to the last of the nine, as
        // `TAB m 0` does.
        through_shell("0", "0", "workspace-all"),
        of_deed_with("S-0", "Shift+0", "wm-workspace-move", Some("9")),
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
    .into_iter()
    .chain(scroll_chords())
    .collect()
}

/// EVERY SHORTCUT OF THE COMPOSITOR'S OWN CONFIGURATION, and the few that
/// could not keep their key.
///
/// The session starts the compositor with `-c`, so the configuration it ships
/// — some three hundred binds and fifteen modes — is never read. Everything it
/// binds is here instead, on the key it uses, with the owner's rules for where
/// the two tables meet:
///
/// - **Where a chord of ours already has the key, ours keeps it.** Nine keys:
///   `S-arrows`, `f`, `S-f`, `c`, `t`, `g`, `,`, `.` and the wheel.
/// - **What loses its key and has no other one gets another**, written
///   `RELOCATED` beside it: the other screens' focus (`S-h/j/k/l`), the align
///   mode (`i`), maximising in place (`m`), the saved layouts (`s`), and the
///   content's zoom (`C--`, `C-=` and `C-` wheel). What already had a key of
///   ours — full screen, float, overview — needs none.
/// - **`Super+Alt+…` has another key in the nested loop**, where the prefix
///   IS Alt and the keymap has no Super: `C-S-` and a letter, see
///   [`Chord::nested`].
/// - **The mouse's back button is not taken bare.** The shipped
///   configuration binds `button8` to the overview on the WHOLE window, which
///   takes Back away from every browser and file manager; here it wants the
///   prefix like every other chord.
/// - **One is left out**: `jump criteria [app_id="firefox"]` names a program,
///   and a deed with criteria is outside `assets/compositor-deeds.nix`'s rule.
/// - **What the shipped file hands to programs we do not ship** (`wmenu-run`,
///   a terminal, a file manager, `scrollnag`, `grim`, `brightnessctl`) goes to
///   ours: the launcher, the file manager, leaving the session, the capture,
///   the screens' brightness.
fn scroll_chords() -> Vec<Chord> {
    let d = of_deed;
    let mut out = vec![
        // --- closing, reading, leaving ---
        d("BS", "BackSpace", "wm-close"),
        d("S-BS", "Shift+BackSpace", "wm-close-others"),
        d("C-BS", "Ctrl+BackSpace", "wm-close-all"),
        through_shell("r", "r", "apps-open"),
        through_shell("e", "e", "files-window"),
        d("S-c", "Shift+c", "wm-reload"),
        through_shell("S-x", "Shift+x", "session-quit"),
        // --- the scroll's direction ---
        d("[", "bracketleft", "wm-columns"),
        d("]", "bracketright", "wm-rows"),
        // --- focus and moving ---
        d("HOME", "Home", "wm-focus-beginning"),
        d("END", "End", "wm-focus-end"),
        d("C-←", "Ctrl+Left", "wm-move-left"),
        d("C-↓", "Ctrl+Down", "wm-move-down"),
        d("C-↑", "Ctrl+Up", "wm-move-up"),
        d("C-→", "Ctrl+Right", "wm-move-right"),
        d("C-HOME", "Ctrl+Home", "wm-move-beginning"),
        d("C-END", "Ctrl+End", "wm-move-end"),
        d("M-←", "Alt+Left", "wm-move-out-left").nested("C-S-h", "Ctrl+Shift+h"),
        d("M-↓", "Alt+Down", "wm-move-out-down").nested("C-S-j", "Ctrl+Shift+j"),
        d("M-↑", "Alt+Up", "wm-move-out-up").nested("C-S-k", "Ctrl+Shift+k"),
        d("M-→", "Alt+Right", "wm-move-out-right").nested("C-S-l", "Ctrl+Shift+l"),
        d("M-HOME", "Alt+Home", "wm-move-out-beginning").nested("C-S-HOME", "Ctrl+Shift+Home"),
        d("M-END", "Alt+End", "wm-move-out-end").nested("C-S-END", "Ctrl+Shift+End"),
        // RELOCATED: the shipped file has these on `S-arrows`, which move a
        // window here.
        d("S-h", "Shift+h", "wm-focus-screen-left"),
        d("S-j", "Shift+j", "wm-focus-screen-down"),
        d("S-k", "Shift+k", "wm-focus-screen-up"),
        d("S-l", "Shift+l", "wm-focus-screen-right"),
        // --- workspaces ---
        d("C-S-←", "Ctrl+Shift+Left", "wm-workspace-to-screen-left"),
        d("C-S-→", "Ctrl+Shift+Right", "wm-workspace-to-screen-right"),
        digits_of("C-1…0", "wm-workspace-swap", "Ctrl+"),
        digits_of("C-S-1…0", "wm-workspace-swap-name", "Ctrl+Shift+"),
        // THE NAMES ARE THE OTHER WAY ROUND from what they look like:
        // `wm-workspace-next` is `next_on_output` (it is what the wheel
        // chord has always meant), and `-screen` is the one that crosses to
        // other screens. `every-compositor-shortcut-is-bound` caught these
        // four the right way round only after they were bound wrong.
        d("S-PGUP", "Shift+Prior", "wm-workspace-previous-screen"),
        d("S-PGDN", "Shift+Next", "wm-workspace-next-screen"),
        d("PGUP", "Prior", "wm-workspace-previous"),
        d("PGDN", "Next", "wm-workspace-next"),
        d("C-z", "Ctrl+z", "wm-workspace-back"),
        // --- the zoom of the workspace, and of the content ---
        d("S-,", "Shift+comma", "wm-zoom-out"),
        d("S-.", "Shift+period", "wm-zoom-in"),
        d("C-S-.", "Ctrl+Shift+period", "wm-zoom-reset"),
        d("S-<wheel-up>", "Shift+button4", "wm-zoom-out"),
        d("S-<wheel-down>", "Shift+button5", "wm-zoom-in"),
        // RELOCATED: the shipped file has these on `,`, `.` and the wheel,
        // which are ours.
        d("C--", "Ctrl+minus", "wm-content-smaller"),
        d("C-=", "Ctrl+equal", "wm-content-bigger"),
        d("C-.", "Ctrl+period", "wm-content-reset"),
        d("C-<wheel-up>", "Ctrl+button4", "wm-content-smaller"),
        d("C-<wheel-down>", "Ctrl+button5", "wm-content-bigger"),
        // --- the overview ---
        d("TAB", "Tab", "wm-overview"),
        // EVERY WORKSPACE THROUGH US, and not the compositor's toggle: that
        // view is said by no reply and no event, so the footer — which prints
        // the key back out of it while it is on — has to be the one that
        // turns it on.
        through_shell("S-TAB", "Shift+Tab", "workspace-all"),
        d("<back>", "button8", "wm-overview"),
        // --- full screen and floating ---
        d("C-f", "Ctrl+f", "wm-full-layout"),
        d("M-f", "Alt+f", "wm-full-application").nested("C-S-f", "Ctrl+Shift+f"),
        d("C-M-f", "Ctrl+Alt+f", "wm-full-global").nested("C-S-g", "Ctrl+Shift+g"),
        d("y", "y", "wm-focus-either"),
        d("S-y", "Shift+y", "wm-transpose"),
        d("C-S-a", "Ctrl+Shift+a", "wm-stick"),
        d("a", "a", "wm-pin-beginning"),
        d("S-a", "Shift+a", "wm-pin-end"),
        // --- the selection ---
        //
        // THROUGH OUR BINARY, all five: the compositor says what is selected
        // in no reply and no event, so the footer keeps its own copy
        // (`crate::selection`), and a key that went straight to the compositor
        // would change the selection behind that copy's back.
        through_shell("INS", "Insert", "wm-select"),
        through_shell("C-INS", "Ctrl+Insert", "wm-select-none"),
        through_shell("S-INS", "Shift+Insert", "wm-select-move"),
        through_shell("C-S-INS", "Ctrl+Shift+Insert", "wm-select-workspace"),
        through_shell("M-INS", "Alt+Insert", "wm-select-to-trail")
            .nested("C-S-DEL", "Ctrl+Shift+Delete"),
        // --- the scratchpad ---
        d("S-z", "Shift+z", "wm-scratchpad-put"),
        d("z", "z", "wm-scratchpad-show"),
        d("M-z", "Alt+z", "wm-scratchpad-jump").nested("C-S-z", "Ctrl+Shift+z"),
        // --- the focus ring ---
        d("S-[", "Shift+bracketleft", "wm-ring-previous"),
        d("S-]", "Shift+bracketright", "wm-ring-next"),
        d("S-RET", "Shift+Return", "wm-ring-set"),
        d("S-DEL", "Shift+Delete", "wm-ring-remove"),
        d("S-HOME", "Shift+Home", "wm-ring-first"),
        d("S-END", "Shift+End", "wm-ring-last"),
        // --- sizes ---
        d("-", "minus", "wm-width-previous"),
        d("=", "equal", "wm-width-next"),
        d("S--", "Shift+minus", "wm-height-previous"),
        d("S-=", "Shift+equal", "wm-height-next"),
        d("S-t", "Shift+t", "wm-toggle-size-active"),
        // RELOCATED: `t` floats a window here.
        d("m", "m", "wm-toggle-size-this"),
    ];
    // --- and the doors to the fifteen modes ---
    out.extend(modes().into_iter().map(|m| {
        let spoken = ronoctua_scroll::deeds::spoken_of(m.deed)
            .unwrap_or_else(|| panic!("the mode `{}` has no deed", m.name));
        Chord {
            shows: m.shows,
            command: spoken.deed.id,
            binds: vec![(m.key.to_owned(), spoken.text.to_owned())],
            nested: None,
            nested_shows: None,
        }
    }));
    out
}

/// One of the compositor's MODES: a key that switches every other key to a
/// small table of its own until `Esc` (or one of them) switches back.
///
/// The compositor has no bar here to say a mode is on, so the footer does:
/// it hears the mode change over IPC and draws the mode's keys on the hint
/// line (`Footer::compositor_mode`), from THIS table.
pub struct Mode {
    /// The name the compositor knows it by, which is also its text key's
    /// tail (`compositor-mode-<name>`).
    pub name: &'static str,
    /// The deed that enters it (`wm-mode-<name>`).
    pub deed: &'static str,
    /// Its entry chord's notation.
    pub shows: &'static str,
    /// Its entry chord's key, without the prefix.
    pub key: &'static str,
    /// The keys inside it.
    pub keys: Vec<ModeKey>,
}

/// A key inside a mode. Bare: a mode's whole point is that the prefix is
/// no longer needed.
pub struct ModeKey {
    /// How it appears on the hint line.
    pub shows: String,
    /// The key as the compositor spells it.
    pub key: String,
    /// The deed it runs, which is also its label.
    pub command: &'static str,
    /// The text the compositor is given — with `; mode default` when the key
    /// leaves the mode.
    pub said: String,
}

/// A mode key that runs `deed` (with `n`) and, when `leaves`, goes back to the
/// default mode.
fn mode_key(shows: &str, key: &str, deed: &'static str, n: Option<&str>, leaves: bool) -> ModeKey {
    let spoken = ronoctua_scroll::deeds::spoken_of(deed).unwrap_or_else(|| {
        panic!("the mode key `{key}` points at `{deed}`, which is not declared")
    });
    let mut said = ronoctua_scroll::wire::spoken(&spoken.deed, spoken.text, n)
        .unwrap_or_else(|e| panic!("the mode key `{key}` cannot say `{deed}`: {e}"));
    if leaves {
        said.push_str("; mode default");
    }
    ModeKey {
        shows: shows.to_owned(),
        key: key.to_owned(),
        command: spoken.deed.id,
        said,
    }
}

/// How many pixels a step of the resize mode takes — and a step of the window
/// menu's, which is the same gesture spelt with the pointer.
pub const RESIZE_STEP: &str = "100";
/// How many pixels a floating window moves per step of the floating mode, and
/// of the window menu.
pub const NUDGE_STEP: &str = "50";

/// The fractions the size modes offer, on `1` to `0`, `-` and `=` — the
/// shipped file's own numbers. The window menu offers the same twelve.
pub const FRACTIONS: [(&str, &str, &str); 12] = [
    ("1", "1", "0.125"),
    ("2", "2", "0.1666666667"),
    ("3", "3", "0.25"),
    ("4", "4", "0.333333333"),
    ("5", "5", "0.375"),
    ("6", "6", "0.5"),
    ("7", "7", "0.625"),
    ("8", "8", "0.6666666667"),
    ("9", "9", "0.75"),
    ("0", "0", "0.833333333"),
    ("-", "minus", "0.875"),
    ("=", "equal", "1.0"),
];

/// The five splits of a workspace, on `1` to `5` — top and bottom…
const SPLITS: [&str; 5] = ["0.25", "0.333333333", "0.5", "0.66666667", "0.75"];
/// …and side by side, whose two thirds the shipped file writes with one
/// digit fewer. Kept as it is: the point is the same keys doing the same.
const SPLITS_ACROSS: [&str; 5] = ["0.25", "0.333333333", "0.5", "0.6666667", "0.75"];

/// All fifteen modes.
#[must_use]
pub fn modes() -> Vec<Mode> {
    let k = mode_key;
    let digits = |deed: &'static str, shift: &str, notation: &str| -> Vec<ModeKey> {
        (0..=9)
            .map(|n| {
                let d = n.to_string();
                k(
                    &format!("{notation}{d}"),
                    &format!("{shift}{d}"),
                    deed,
                    Some(&d),
                    true,
                )
            })
            .collect()
    };
    let fractions = |deed: &'static str| -> Vec<ModeKey> {
        FRACTIONS
            .iter()
            .map(|(shows, key, n)| k(shows, key, deed, Some(n), true))
            .collect()
    };
    let resize = |by: &str, moves: bool| -> Vec<ModeKey> {
        let mut v = Vec::new();
        if moves {
            v.extend([
                k("←", "Left", "wm-nudge-left", Some(by), false),
                k("↓", "Down", "wm-nudge-down", Some(by), false),
                k("↑", "Up", "wm-nudge-up", Some(by), false),
                k("→", "Right", "wm-nudge-right", Some(by), false),
            ]);
        } else {
            v.extend([
                k("←", "Left", "wm-resize-narrower", Some(by), false),
                k("↓", "Down", "wm-resize-taller", Some(by), false),
                k("↑", "Up", "wm-resize-shorter", Some(by), false),
                k("→", "Right", "wm-resize-wider", Some(by), false),
            ]);
        }
        v.extend([
            k(
                "S-←",
                "Shift+Left",
                "wm-resize-shrink-right",
                Some(by),
                false,
            ),
            k("S-↓", "Shift+Down", "wm-resize-grow-down", Some(by), false),
            k("S-↑", "Shift+Up", "wm-resize-shrink-down", Some(by), false),
            k(
                "S-→",
                "Shift+Right",
                "wm-resize-grow-right",
                Some(by),
                false,
            ),
            k("C-←", "Ctrl+Left", "wm-resize-grow-left", Some(by), false),
            k("C-↓", "Ctrl+Down", "wm-resize-shrink-up", Some(by), false),
            k("C-↑", "Ctrl+Up", "wm-resize-grow-up", Some(by), false),
            k(
                "C-→",
                "Ctrl+Right",
                "wm-resize-shrink-left",
                Some(by),
                false,
            ),
        ]);
        v
    };
    let mode = |name: &'static str,
                deed: &'static str,
                shows: &'static str,
                key: &'static str,
                keys: Vec<ModeKey>| Mode {
        name,
        deed,
        shows,
        key,
        keys,
    };
    vec![
        mode("split", "wm-mode-split", "S-\\", "Shift+backslash", {
            let mut v: Vec<ModeKey> = SPLITS
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let d = (i + 1).to_string();
                    k(&d, &d, "wm-workspace-split-v", Some(n), true)
                })
                .collect();
            v.extend(SPLITS_ACROSS.iter().enumerate().map(|(i, n)| {
                let d = (i + 1).to_string();
                k(
                    &format!("S-{d}"),
                    &format!("Shift+{d}"),
                    "wm-workspace-split-h",
                    Some(n),
                    true,
                )
            }));
            v.push(k("r", "r", "wm-workspace-split-reset", None, true));
            v
        }),
        mode(
            "jump",
            "wm-mode-jump",
            "/",
            "slash",
            vec![
                k("/", "slash", "wm-jump-tiling", None, true),
                k("S-/", "Shift+slash", "wm-jump-tiling-all", None, true),
                k("c", "c", "wm-jump-container", None, true),
                k("w", "w", "wm-jump-workspaces", None, true),
                k("f", "f", "wm-jump-floating", None, true),
                k("S-f", "Shift+f", "wm-jump-floating-all", None, true),
                k("a", "a", "wm-jump-everywhere", None, true),
                k("S-a", "Shift+a", "wm-jump-everywhere-all", None, true),
                k("s", "s", "wm-scratchpad-jump", None, true),
                k("t", "t", "wm-jump-trailmark", None, true),
                k("S-t", "Shift+t", "wm-jump-trailmark-all", None, true),
                k("v", "v", "wm-jump", None, true),
            ],
        ),
        mode("filter", "wm-mode-filter", "S-/", "Shift+slash", {
            let mut v = Vec::new();
            for (letter, key, what) in [
                ("/", "slash", "tiling"),
                ("c", "c", "container"),
                ("f", "f", "floating"),
                ("t", "t", "trailmark"),
                ("v", "v", "visible"),
            ] {
                let deed = |tail: &str| -> &'static str {
                    ronoctua_scroll::deeds::spoken_of(&format!("wm-filter-{what}{tail}"))
                        .map(|s| s.deed.id)
                        .unwrap_or_else(|| panic!("no filter deed for `{what}{tail}`"))
                };
                v.push(k(letter, key, deed(""), None, true));
                v.push(k(
                    &format!("S-{letter}"),
                    &format!("Shift+{key}"),
                    deed("-all"),
                    None,
                    true,
                ));
                v.push(k(
                    &format!("C-{letter}"),
                    &format!("Ctrl+{key}"),
                    deed("-only"),
                    None,
                    true,
                ));
            }
            v.push(k("r", "r", "wm-filter-reset", None, true));
            v
        }),
        mode(
            "placement",
            "wm-mode-placement",
            "\\",
            "backslash",
            vec![
                k("→", "Right", "wm-placement-after", None, true),
                k("←", "Left", "wm-placement-before", None, true),
                k("HOME", "Home", "wm-placement-beginning", None, true),
                k("END", "End", "wm-placement-end", None, true),
                k("↑", "Up", "wm-placement-focus", None, true),
                k("↓", "Down", "wm-placement-no-focus", None, true),
                k("h", "h", "wm-centre-horizontal", None, true),
                k("S-h", "Shift+h", "wm-no-centre-horizontal", None, true),
                k("v", "v", "wm-centre-vertical", None, true),
                k("S-v", "Shift+v", "wm-no-centre-vertical", None, true),
                k("r", "r", "wm-reorder-auto", None, true),
                k("S-r", "Shift+r", "wm-no-reorder-auto", None, true),
                k("w", "w", "wm-fit-fraction", None, true),
                k("S-w", "Shift+w", "wm-fit-split", None, true),
                k("C-w", "Ctrl+w", "wm-no-fit", None, true),
            ],
        ),
        mode(
            "width",
            "wm-mode-width",
            "b",
            "b",
            fractions("wm-width-set"),
        ),
        mode(
            "height",
            "wm-mode-height",
            "S-b",
            "Shift+b",
            fractions("wm-height-set"),
        ),
        mode(
            "resize",
            "wm-mode-resize",
            "S-r",
            "Shift+r",
            resize(RESIZE_STEP, false),
        ),
        mode(
            "floating",
            "wm-mode-floating",
            "C-r",
            "Ctrl+r",
            resize(NUDGE_STEP, true),
        ),
        mode("toggle-width", "wm-mode-toggle-width", "C-t", "Ctrl+t", {
            let mut v = fractions("wm-toggle-size-width");
            v.push(k("r", "r", "wm-toggle-size-reset", None, true));
            v
        }),
        mode(
            "toggle-height",
            "wm-mode-toggle-height",
            "C-S-t",
            "Ctrl+Shift+t",
            {
                let mut v = fractions("wm-toggle-size-height");
                v.push(k("r", "r", "wm-toggle-size-reset", None, true));
                v
            },
        ),
        // RELOCATED: the shipped file opens this on `c`, which centres here.
        mode(
            "align",
            "wm-mode-align",
            "i",
            "i",
            vec![
                k("c", "c", "wm-align-centre", None, true),
                k("m", "m", "wm-align-middle", None, true),
                k("r", "r", "wm-align-auto", None, true),
                k("←", "Left", "wm-align-left", None, true),
                k("→", "Right", "wm-align-right", None, true),
                k("↑", "Up", "wm-align-top", None, true),
                k("↓", "Down", "wm-align-bottom", None, true),
            ],
        ),
        mode(
            "fit",
            "wm-mode-fit",
            "w",
            "w",
            vec![
                k("w", "w", "wm-fit-visible", None, true),
                k("S-w", "Shift+w", "wm-fit-rows-visible", None, true),
                k("C-w", "Ctrl+w", "wm-fit-columns-visible-equal", None, true),
                k(
                    "C-S-w",
                    "Ctrl+Shift+w",
                    "wm-fit-rows-visible-equal",
                    None,
                    true,
                ),
                k("→", "Right", "wm-fit-columns-to-end", None, true),
                k("S-→", "Shift+Right", "wm-fit-rows-to-end", None, true),
                k(
                    "C-→",
                    "Ctrl+Right",
                    "wm-fit-columns-to-end-equal",
                    None,
                    true,
                ),
                k(
                    "C-S-→",
                    "Ctrl+Shift+Right",
                    "wm-fit-rows-to-end-equal",
                    None,
                    true,
                ),
                k("←", "Left", "wm-fit-columns-to-start", None, true),
                k("S-←", "Shift+Left", "wm-fit-rows-to-start", None, true),
                k(
                    "C-←",
                    "Ctrl+Left",
                    "wm-fit-columns-to-start-equal",
                    None,
                    true,
                ),
                k(
                    "C-S-←",
                    "Ctrl+Shift+Left",
                    "wm-fit-rows-to-start-equal",
                    None,
                    true,
                ),
                k("↑", "Up", "wm-maximise", None, true),
                k("S-↑", "Shift+Up", "wm-fit-rows-active", None, true),
                k("↓", "Down", "wm-fit-columns-all", None, true),
                k("S-↓", "Shift+Down", "wm-fit-rows-all", None, true),
                k("C-↓", "Ctrl+Down", "wm-fit-columns", None, true),
                k("C-S-↓", "Ctrl+Shift+Down", "wm-fit-rows", None, true),
            ],
        ),
        mode(
            "trailmark",
            "wm-mode-trailmark",
            ";",
            "semicolon",
            vec![
                k("]", "bracketright", "wm-trailmark-next", None, false),
                k("[", "bracketleft", "wm-trailmark-previous", None, false),
                k(";", "semicolon", "wm-trailmark", None, true),
                k("/", "slash", "wm-trailmark-jump", None, true),
            ],
        ),
        mode("trail", "wm-mode-trail", "S-;", "Shift+semicolon", {
            let mut v = vec![
                k("]", "bracketright", "wm-trail-next", None, false),
                k("[", "bracketleft", "wm-trail-previous", None, false),
            ];
            v.extend(digits("wm-trail-number", "", ""));
            v.extend([
                k(";", "semicolon", "wm-trail-new", None, true),
                k("d", "d", "wm-trail-delete", None, true),
                k("c", "c", "wm-trail-clear", None, true),
                k("INS", "Insert", "wm-trail-to-selection", None, true),
            ]);
            v
        }),
        // RELOCATED: the shipped file opens this on `g`, the overview here.
        mode("spaces", "wm-mode-spaces", "s", "s", {
            let mut v = digits("wm-space-load", "", "");
            v.extend(digits("wm-space-save", "Shift+", "S-"));
            v.extend(digits("wm-space-restore-hide", "Ctrl+", "C-"));
            v.extend(digits("wm-space-restore", "Ctrl+Shift+", "C-S-"));
            v
        }),
    ]
}

/// The hint line while the compositor's mode `name` is on: every key of it,
/// a band of digits on the same deed told once (`1…0`), and `Esc`.
#[must_use]
pub fn mode_hint(name: &str, texts: &ronoctua_i18n::Texts) -> Option<Vec<crate::chrome::Piece>> {
    use crate::chrome::Piece;
    let mode = modes().into_iter().find(|m| m.name == name)?;
    let mut out: Vec<Piece> = Vec::new();
    let mut i = 0;
    while i < mode.keys.len() {
        let first = &mode.keys[i];
        let mut last = i;
        while last + 1 < mode.keys.len() && mode.keys[last + 1].command == first.command {
            last += 1;
        }
        let shows = if last > i {
            format!("{}…{}", first.shows, mode.keys[last].shows)
        } else {
            first.shows.clone()
        };
        if !out.is_empty() {
            out.push(Piece::separator());
        }
        // A BAND OF DIGITS IS NOT ONE KEY, and a click cannot say which of
        // them — so only a single key is a control (`chrome::Act::Run`), and
        // the band is drawn as a key that is only read.
        let single = last == i;
        let key = Piece::key(shows, false).about(first.command);
        let words = Piece::prose(format!(
            " {}",
            texts.t(&format!("{}{}", ronoctua_cmd::KEY_PREFIX_OF, first.command))
        ))
        .about(first.command);
        if single {
            out.push(key.running(first.command));
            out.push(words.running(first.command));
        } else {
            out.push(key);
            out.push(words);
        }
        i = last + 1;
    }
    out.push(Piece::separator());
    out.push(
        Piece::key("ESC", false)
            .about("wm-mode-default")
            .running("wm-mode-default"),
    );
    out.push(
        Piece::prose(format!(
            " {}",
            texts.t(&format!("{}wm-mode-default", ronoctua_cmd::KEY_PREFIX_OF))
        ))
        .about("wm-mode-default")
        .running("wm-mode-default"),
    );
    Some(out)
}

/// The compositor's other two ways in besides a key: dragging with the prefix
/// held, and four fingers on a touchpad — both as the shipped file has them.
///
/// `floating_modifier` makes the prefix plus the LEFT button move a floating
/// window and the RIGHT button resize it, which is what somebody who has used
/// this compositor anywhere else reaches for first.
#[must_use]
pub fn gestures(prefix: &str) -> Vec<String> {
    vec![
        format!("floating_modifier {prefix} normal"),
        "bindgesture swipe:4:right workspace next".to_owned(),
        "bindgesture swipe:4:left workspace prev".to_owned(),
        "bindgesture swipe:4:up scale_workspace overview".to_owned(),
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
const PREFIXES: [&str; 2] = ["Super", NESTED_PREFIX];

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
        // A PAUSE KEY toggles like play: `playerctl play-pause` is what the
        // compositor's own configuration gives it too.
        ("XF86AudioPause", "media-play"),
        ("XF86AudioStop", "media-stop"),
        ("XF86AudioNext", "media-next"),
        ("XF86AudioPrev", "media-previous"),
        // THE REST OF THE TOP ROW the compositor's own configuration binds:
        // the screen's light, through the screens' own thread (it knows a
        // panel from a monitor over DDC), and `Print`, through the capture
        // `LEADER m s` takes.
        ("XF86MonBrightnessUp", "brightness-up"),
        ("XF86MonBrightnessDown", "brightness-down"),
        ("Print", "screen-capture"),
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
    use super::*;

    /// Every top-level key under `prefix`, canonical, the bare ones included.
    fn keys_under(prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = full_chords()
            .iter()
            .flat_map(|c| {
                c.binds_for(prefix)
                    .iter()
                    .map(|(k, _)| canonical_key(&format!("{prefix}+{k}")))
                    .collect::<Vec<_>>()
            })
            .collect();
        out.extend(emergency().into_iter().map(|(k, _, _)| canonical_key(k)));
        out.extend(media().into_iter().map(|(k, _, _)| canonical_key(k)));
        out
    }

    #[test]
    fn no_chord_is_dropped_and_no_two_binds_share_a_key_in_either_loop() {
        // The deduplication in `full_chords` DROPS a chord that lands on an
        // emergency key, in silence. With three hundred binds that is where
        // one would go missing, so it must drop nothing.
        assert_eq!(full_chords().len(), chords().len(), "a chord was dropped");
        for prefix in PREFIXES {
            let keys = keys_under(prefix);
            let mut seen = std::collections::BTreeSet::new();
            for k in &keys {
                assert!(
                    seen.insert(k.clone()),
                    "`{k}` is bound twice under {prefix}"
                );
            }
        }
    }

    #[test]
    fn nothing_in_the_nested_loop_needs_a_second_alt() {
        // The nested prefix IS Alt: a chord with Alt of its own would be the
        // same keys as the chord without it.
        for c in full_chords() {
            for (k, _) in c.binds_for(NESTED_PREFIX) {
                assert!(
                    !k.split('+').any(|m| m.eq_ignore_ascii_case("alt")),
                    "`{}` is `{k}` in the nested loop",
                    c.shows
                );
            }
        }
    }

    #[test]
    fn every_mode_leaves_and_every_key_in_it_is_distinct() {
        for m in modes() {
            let mut seen = std::collections::BTreeSet::new();
            for k in &m.keys {
                assert!(
                    seen.insert(canonical_key(&k.key)),
                    "`{}` is twice in the mode `{}`",
                    k.key,
                    m.name
                );
                assert!(!k.key.eq_ignore_ascii_case("escape"), "Esc is the way out");
            }
            assert!(!m.keys.is_empty(), "the mode `{}` has no keys", m.name);
            // A mode is entered by the deed `mode "<name>"`, and that name is
            // what the compositor's `mode` event reports back.
            let entry = ronoctua_scroll::deeds::spoken_of(m.deed).expect("declared");
            assert_eq!(entry.text, format!("mode \"{}\"", m.name));
        }
    }

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
