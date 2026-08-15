//! What each window, workspace and screen command asks of the compositor.
//!
//! # One table, two consumers
//!
//! `surface::run_command` dispatches through it, and the `ronoctua-niri-actions` binary
//! walks all of it against a actual niri. Two copies would diverge —
//! and the one that diverges silently is always the one nobody runs.
//!
//! # Why the action's name travels as JSON
//!
//! niri has 142 actions. Mirroring them in an enum would give a table that breaks on the
//! first new version of the compositor, and breaking here means a shortcut that
//! disappears with nobody noticing. The name crosses as it is; what guarantees it
//! exists is the `niri-contract` check.

/// `(command id, action in niri's protocol)`.
///
/// A function and not a `static`: a `serde_json::Value` has to allocate, and there is no
/// allocation at compile time. Building it costs microseconds and happens
/// once per key.
///
/// The ids are the same as the keymap's in `leader::default_of`. The
/// `keymap-consistente` check guarantees that every binding points at a command that
/// exists; `niri-contract` guarantees that every command reaches an action the
/// compositor accepts.
#[must_use]
pub fn all_of() -> Vec<(&'static str, serde_json::Value)> {
    use serde_json::json;
    vec![
        ("window-left", json!({ "FocusColumnLeft": {} })),
        ("window-right", json!({ "FocusColumnRight": {} })),
        ("window-up", json!({ "FocusWindowUp": {} })),
        ("window-down", json!({ "FocusWindowDown": {} })),
        ("window-move-left", json!({ "MoveColumnLeft": {} })),
        ("window-move-right", json!({ "MoveColumnRight": {} })),
        ("window-move-up", json!({ "MoveWindowUp": {} })),
        ("window-move-down", json!({ "MoveWindowDown": {} })),
        ("window-maximize", json!({ "MaximizeColumn": {} })),
        ("window-screen-full", json!({ "FullscreenWindow": {} })),
        ("window-center", json!({ "CenterColumn": {} })),
        // TOGGLE FLOATING was only a chord, with the KDL written by hand
        // in a `um(...)` — the only shortcut in the system with no command behind it.
        // It came in here because "nothing key-only" (section 13), and the
        // `todo_acorde_vem_de_um_comando` test stops another from using the same hole.
        ("window-float", json!({ "ToggleWindowFloating": {} })),
        ("window-consume", json!({ "ConsumeOrExpelWindowRight": {} })),
        ("window-expel", json!({ "ConsumeOrExpelWindowLeft": {} })),
        // The two BUFFER ones are here too. They had a method of their own
        // in `ronoctua-niri` and a branch of their own in `run_command`; with the chord
        // needing the same action, keeping both paths would give two
        // truths for the same key.
        ("buffer-close", json!({ "CloseWindow": { "id": null } })),
        ("buffer-next", json!({ "FocusWindowDown": {} })),
        ("workspace-next", json!({ "FocusWorkspaceDown": {} })),
        // THE OVERVIEW: the view of all the workspaces at once. It belongs to the
        // compositor, like everything else here.
        ("workspace-overview", json!({ "ToggleOverview": {} })),
        ("workspace-previous", json!({ "FocusWorkspaceUp": {} })),
        // The ONLY one with an argument. Without `focus`, niri answers "error
        // parsing request" — and a refused action is a shortcut that does
        // nothing.
        (
            "workspace-move-next",
            json!({ "MoveWindowToWorkspaceDown": { "focus": true } }),
        ),
        (
            "workspace-move-previous",
            json!({ "MoveWindowToWorkspaceUp": { "focus": true } }),
        ),
        // `Screenshot` ALSO requires an argument, and `{}` gives "error parsing
        // request" — the same error as a non-existent name. It was the
        // `niri-contract` check that caught it: in the hand probe the answer got lost in a
        // socket timeout, and I had concluded that it passed.
        (
            "screen-capture",
            json!({ "Screenshot": { "show_pointer": true } }),
        ),
    ]
}

/// The commands that require an ARGUMENT, and the compositor action that serves them.
///
/// # Why a separate table
///
/// `all_of()` holds **null** actions, assembled as JSON here. These cannot
/// be: `focus-workspace` takes a workspace reference, and its form in
/// the JSON is niri's — assembling it by hand would bring back the mistake `Screenshot`
/// cost, with the aggravation of the argument varying.
///
/// They go through the compositor's own client (`niri msg action <name> <n>`),
/// which is the path of the 141 mirrored ones and for the same reason: whoever fills in
/// the argument's form is whoever defines it.
#[must_use]
pub fn with_argument(id: &str) -> Option<&'static str> {
    match id {
        "workspace-go" => Some("focus-workspace"),
        "workspace-move" => Some("move-window-to-workspace"),
        _ => None,
    }
}

/// A chord: the leader **held** plus a key.
///
/// # Why it is not in the shell's keymap
///
/// The footer only receives the keyboard after the leader is released — it is the compositor
/// that sees the key and tells the daemon (§7.4). A chord, by definition, never
/// gets there: what resolves it is niri, and that is why holding the leader **does
/// not open the list nor take focus from the window**. They are two mechanisms, and it is
/// deliberate that they are.
///
/// # And why that does not recreate two truths
///
/// niri's bind is GENERATED from here, and which-key also READS from here. What the
/// chord does and how it appears on screen come from a single place.
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
    /// `(key in niri, action in KDL)`, already expanded.
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
/// The KDL comes from the same action `run_command` sends over IPC — the two spellings
/// of the same name, one derived from the other. It is what keeps `LEADER w h` and
/// `LEADER+←` from diverging.
fn of_command(shows: &'static str, key: &str, id: &'static str) -> Chord {
    let kdl = kdl_of(id)
        .as_ref()
        .and_then(config_name)
        .unwrap_or_else(|| panic!("o acorde `{key}` aponta para `{id}`, que nao existe"));
    Chord {
        shows,
        command: id,
        binds: vec![(key.to_owned(), kdl)],
    }
}

/// A chord for a command the compositor does **not** run.
///
/// # Why it goes through our binary
///
/// Opening a terminal is not a niri action: what does it is `run_command`, which puts the
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
            format!("spawn \"{}\" \"command\" \"{id}\"", shell_path()),
        )],
    }
}

/// A range from 1 to 9: nine binds, one line in the list.
///
/// These are the only chords whose action requires an **argument**, and that is why
/// the KDL does not come from `all_of()` — there is no equivalent null action. The action's name is
/// checked against the actual niri by `niri-contract`.
fn range_of(shows: &'static str, command: &'static str, prefix: &str) -> Chord {
    // THE ACTION COMES FROM THE TABLE, and not from a string here. It was written by hand, and
    // it was the last KDL in the system with nothing checking that the action exists.
    let action = with_argument(command).unwrap_or_else(|| {
        panic!("a faixa `{shows}` aponta para `{command}`, que nao leva argumento")
    });
    Chord {
        shows,
        command,
        binds: (1..=9)
            .map(|n| (format!("{prefix}{n}"), format!("{action} {n}")))
            .collect(),
    }
}

/// This binary's path, so niri spawns the SAME one that generated the bind.
///
/// A bare name will not do: in the nested loop the shell is not in niri's `PATH`, and a
/// systemd unit has a minimal `PATH` — the symptom shows up far from the cause.
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
pub fn acordes() -> Vec<Chord> {
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
        of_command("roda ↓", "WheelScrollDown", "workspace-next"),
        of_command("roda ↑", "WheelScrollUp", "workspace-previous"),
        // --- opening ---
        through_shell("RET", "Return", "buffer-terminal"),
        // THE MONITOR, held-leader plus `Esc` — and `Ctrl+Alt+Esc` in
        // `emergency()` does the same thing without the leader.
        //
        // It TAKES the key from niri's own default: upstream binds
        // `Mod+Escape` to `toggle-keyboard-shortcuts-inhibit`, and ours comes
        // first, so the deduplication in `acordes_completos` drops that one. The
        // action is not lost — like the other 140, it is reachable by name
        // (`LEADER :`), which is what that exception exists for.
        through_shell("ESC", "Escape", "monitor-btop"),
        through_shell("S-ESC", "Shift+Escape", "monitor-bottom"),
    ]
}

/// All the chords: ours PLUS the ones niri ships by default.
///
/// # Why both, and in this order
///
/// Ours come first because **they win**: `LEADER+S-←` moves the column
/// here and focuses the left monitor in niri, and whoever decides is whoever wrote the
/// keymap. niri's come in wherever there is room — and that is how `LEADER+=` comes to
/// widen the column without anyone having typed that line.
///
/// The compositor's list is **generated** from the pinned commit's
/// `default-config.kdl`: written by hand, a shortcut upstream adds would never
/// appear, and one it removes would become a key that answers "error
/// parsing request" — a `warn` in the log and nothing on screen.
/// The key in a form that does not depend on how it was written.
///
/// # Why this exists
///
/// `Alt+Ctrl+L` and `Ctrl+Alt+L` are the SAME key and two different strings.
/// The deduplication compared strings, so niri's default `Ctrl+L`
/// sailed right past the emergency `Ctrl+Alt+L` — and niri refused the whole config
/// with "duplicate keybind". A refused config is the compositor falling back to
/// its own default: none of our shortcuts, not even the leader.
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
pub fn acordes_completos() -> Vec<Chord> {
    let mut out = acordes();
    let mut usadas: std::collections::BTreeSet<String> = out
        .iter()
        .flat_map(|a| a.binds.iter().map(|(t, _)| canonical_key(t)))
        .collect();

    // THE EMERGENCY BINDS ARE RESERVED.
    //
    // They come out of the config WITHOUT the leader's prefix, so a chord that lands
    // on the same key does not "lose to ours": it breaks the whole file.
    for (key, _, _) in emergency() {
        for p in PREFIXES {
            usadas.insert(canonical_key(key));
            // And in the form the chord would have with each prefix.
            if let Some(rest) = canonical_key(key)
                .strip_prefix(&format!("{}+", p.to_lowercase()))
                .map(str::to_owned)
            {
                usadas.insert(rest);
            }
        }
    }

    for b in ronoctua_niri::DEFAULT_BINDS {
        // COLLISION: ours stays. Two lines on the same key in the KDL make
        // niri REFUSE the config — and a refused config is the compositor falling back
        // to its own default, with none of our shortcuts.
        if !usadas.insert(canonical_key(b.key)) {
            continue;
        }
        out.push(Chord {
            shows: b.key,
            command: b.command,
            binds: vec![(b.key.to_owned(), b.kdl.to_owned())],
        });
    }
    out
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
/// # Why only three
///
/// Every bind here is a key the buffers stop receiving, and §7.4 wants
/// **one** keymap. Two of them close the loop: one to reach the command
/// list, another to fix the most likely cause of it not opening.
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
/// It returns `(key in niri, action in KDL, command)`.
#[must_use]
pub fn emergency() -> Vec<(&'static str, String, &'static str)> {
    let shell = shell_path();
    vec![
        // THE ALTERNATIVE LEADER. It is not a command: it is the same trigger as the
        // sequence's through another key, and that is why it sends `leader` and not
        // `command`. `Ctrl+Alt+L` is an ordinary chord — niri sees it without going
        // through keyd, which is the point.
        (
            "Ctrl+Alt+L",
            format!("spawn \"{shell}\" \"leader\""),
            LEADER_ALTERNATE,
        ),
        (
            "Ctrl+Shift+L",
            format!("spawn \"{shell}\" \"command\" \"keyboard-restore\""),
            "keyboard-restore",
        ),
        // THE PROCESS MONITOR, without the leader. `Escape` and not `Delete`:
        // `Ctrl+Alt+Delete` is systemd's reboot on the console, and binding the
        // monitor next to it is asking for the wrong finger to end the session.
        (
            "Ctrl+Alt+Escape",
            format!("spawn \"{shell}\" \"command\" \"monitor-btop\""),
            "monitor-btop",
        ),
    ]
}

/// What the alternative leader fires — the leader, and not a command.
///
/// It exists as a constant so the reachability test can tell the difference: it is the
/// only emergency bind that does **not** point at a registry id.
pub const LEADER_ALTERNATE: &str = "";

/// The action's name in niri's CONFIG notation, in `kebab-case`.
///
/// The IPC uses `FocusColumnLeft`; the config file uses `focus-column-left`.
/// They are the same action in two spellings, and deriving one from the other is what stops
/// a third table from existing.
#[must_use]
pub fn config_name(action: &serde_json::Value) -> Option<String> {
    let name = action.as_object()?.keys().next()?;
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// A command's action, if it is a compositor one.
#[must_use]
pub fn kdl_of(id: &str) -> Option<serde_json::Value> {
    all_of().into_iter().find(|(k, _)| *k == id).map(|(_, v)| v)
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
                "`{id}` ainda tem ponto; o id do comando e kebab puro"
            );
            assert!(
                !id.is_empty()
                    && !id.starts_with('-')
                    && !id.ends_with('-')
                    && id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "`{id}` nao e um id em kebab-case"
            );
        }
    }

    #[test]
    fn each_action_is_an_object_with_a_single_name() {
        // niri's protocol is `{"Name": {arguments}}`. Two names in the
        // same object, or none, gives "error parsing request" — which niri
        // answers without saying which it was.
        for (id, action) in super::all_of() {
            let obj = action
                .as_object()
                .unwrap_or_else(|| panic!("{id} nao e objeto"));
            assert_eq!(obj.len(), 1, "{id} tem {} nomes, esperava 1", obj.len());
        }
    }

    #[test]
    fn every_chord_produces_a_bind_and_a_label() {
        // `of_command` already panics if the id does not exist — building the
        // table is the test. What is asserted here is the rest: no chord
        // comes out without a bind (it would become a line in the list that does nothing) and
        // none comes out without a label (it would become `[key]` on screen).
        for a in super::acordes() {
            assert!(!a.binds.is_empty(), "o acorde `{}` nao gera bind", a.shows);
            assert!(!a.shows.is_empty(), "acorde sem tecla para mostrar");
            assert!(
                a.label().starts_with(ronoctua_cmd::KEY_PREFIX_OF),
                "o rotulo de `{}` nao e uma chave de comando: {}",
                a.shows,
                a.label()
            );
            for (key, kdl) in &a.binds {
                assert!(!key.is_empty(), "bind sem tecla em `{}`", a.shows);
                assert!(!kdl.is_empty(), "bind sem acao em `{}`", a.shows);
            }
        }
    }

    #[test]
    fn only_a_chord_with_an_argument_escapes_the_action_table() {
        // `range_of` is the only one that assembles KDL without going through `all_of()`, because
        // `focus-workspace 3` requires an argument and there is no equivalent null action.
        // That exception has to stay restricted to it: a null chord with its own KDL
        // would be the `T` back again, under another name.
        for a in super::acordes() {
            let nulo = super::kdl_of(a.command)
                .as_ref()
                .and_then(super::config_name);
            for (key, kdl) in &a.binds {
                if kdl.starts_with("spawn ") {
                    continue; // it goes through our binary — see `through_shell`.
                }
                match &nulo {
                    Some(n) => assert_eq!(
                        kdl, n,
                        "o acorde `{key}` diverge da acao do comando `{}`",
                        a.command
                    ),
                    None => assert!(
                        kdl.split_whitespace().count() > 1,
                        "o acorde `{key}` tem KDL proprio (`{kdl}`) sem argumento: \
                         ou o comando `{}` entra em `todas()`, ou ele nao devia \
                         existir so como tecla",
                        a.command
                    ),
                }
            }
        }
    }

    #[test]
    fn no_chord_repeats_a_key() {
        // Two chords on the same key give two identical binds in the KDL: niri
        // accepts it, the last one wins, and the first disappears with no warning.
        let mut keys: Vec<String> = super::acordes_completos()
            .into_iter()
            .flat_map(|a| a.binds.into_iter().map(|(t, _)| t))
            .collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), before, "ha tecla de acorde repetida");
    }

    #[test]
    fn the_name_in_the_config_is_kebab_case() {
        // The IPC says `FocusColumnLeft` and the config says `focus-column-left`.
        // Getting it wrong here gives a config niri REFUSES at startup — noisy,
        // unlike the IPC error, which is mute.
        use serde_json::json;
        assert_eq!(
            super::config_name(&json!({ "FocusColumnLeft": {} })).as_deref(),
            Some("focus-column-left")
        );
        assert_eq!(
            super::config_name(&json!({ "ToggleOverview": {} })).as_deref(),
            Some("toggle-overview")
        );
    }

    #[test]
    fn every_curated_action_exists_in_the_compositor() {
        // THE TWO TABLES MUST NOT DIVERGE.
        //
        // This one here is curated by hand: twenty-odd actions with a shortcut, a translated
        // label and a fast path over IPC. The other is GENERATED from the pinned
        // niri, with all 141. If a name from here is not there, it does not
        // exist in the compositor — and the symptom would be `"error parsing request"`
        // in a log `warn`, with nothing on screen.
        //
        // `niri-contract` proves the same against a LIVE niri, and that is why it
        // still exists: this test runs with no compositor and catches the error
        // in `cargo test`; that one catches the case where the generated table is
        // right and the action is refused anyway (a missing argument).
        for (id, action) in super::all_of() {
            let name = super::config_name(&action).expect("acao com nome");
            assert!(
                ronoctua_niri::action_exists(&name),
                "`{id}` manda `{name}`, que nao esta na tabela do niri pinado"
            );
        }
        // And the range chords, which assemble the KDL without going through `all_of()`.
        for a in super::acordes() {
            for (_, kdl) in &a.binds {
                let Some(name) = kdl.split_whitespace().next() else {
                    continue;
                };
                if name == "spawn" {
                    continue;
                }
                assert!(
                    ronoctua_niri::action_exists(name),
                    "o acorde `{}` manda `{name}`, que o niri pinado nao tem",
                    a.shows
                );
            }
        }
    }

    #[test]
    fn there_is_no_repeated_id() {
        // Repeated, the first wins and the second never runs — with no error.
        let mut ids: Vec<_> = super::all_of().into_iter().map(|(k, _)| k).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "ha id repetido na tabela de acoes");
    }
}
