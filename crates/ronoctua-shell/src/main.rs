//! `ronoctua-shell` — the two-line footer.
//!
//! # The system's only chrome (sections 3 and 4.1)
//!
//! **ONE** `wlr-layer-shell` surface, anchored at the bottom, taking the whole
//! width, with an exclusive zone of two lines of text. There is no separate
//! bar, dock, bar clock, taskbar or floating popup — and there is not going
//! to be (invariant 14).
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │              content (buffers)           │
//! ├──────────────────────────────────────────┤
//! │ modeline                  [tray] [notif] │
//! │ minibuffer                               │
//! └──────────────────────────────────────────┘
//! ```
//!
//! # What this binary does NOT do
//!
//! - **It does not tile windows.** What does that is Niri (invariant 4). One
//!   `xdg_toplevel` per native buffer, and the compositor decides where they go.
//! - **It does not talk to Niri through anything other than IPC or layer-shell**
//!   (invariant 3). There is no fork and no patch of the compositor.
//! - **It does not draw a fullscreen surface reimplementing layout.** That
//!   would throw Niri away and is forbidden (section 7.5).
//!
//! # This milestone's state
//!
//! Milestone 4: the surface comes up in the right font and the modeline reflects the focused
//! buffer, fed by Niri's event stream. The leader and which-key arrive in 5;
//! notifications and tray in 6.

// `deny`, not `forbid`: `surface.rs` needs ONE unsafe block to pass
// Wayland's pointers to wgpu, and `forbid` accepts no exception even with the
// contract written out. `deny` requires an explicit, visible `#[allow]`.
#![deny(unsafe_code)]
#![warn(missing_docs)]

use anyhow::{Context as _, Result};
use ronoctua_render::layout::{Segment, Shrink, Side};
use ronoctua_tokens as tokens;

mod actions;
mod apps;
mod band;
// THE BRAIN'S LINE, under the minibuffer, and the thread that feeds it.
mod brain;
mod buffer;
mod chrome;
mod clipboard;
mod daemon;
mod disks;
mod files;
mod files_window;
mod foreign;
mod keyboard;
mod leader;
mod manual;
mod minibuffer;
mod motion;
mod network;
mod notifications;
mod notifications_window;
mod paint;
mod stage;
mod surface;
mod syncthing;
mod terminal;
mod transfers;
// `LEADER s u`, the animated mark on the modeline, and the window that watches
// a run happen. It knows no step: the sequence is `assets/updates.nix`.
mod update_window;

/// The layer-shell surface's namespace.
///
/// `niri` matches layer rules by this name — see the `layer-rule` in
/// `nix/modules/niri.nix`. Changing it here requires changing it there.
pub const NAMESPACE: &str = "ronoctua-footer";

fn main() -> Result<()> {
    // BEFORE the subscriber: building `tracing` already costs time, and it counts
    // towards 5.2's 200 ms target as much as everything else.
    // The value does not matter here; it is the call that ANCHORS the clock.
    let _ = ronoctua_render::start();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_shell=info,ronoctua_render=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    match std::env::args().nth(1).as_deref() {
        // It tells an ALREADY-RUNNING footer. It is the compositor that sees the
        // leader's key — see the top of `leader.rs`.
        Some("leader") => return leader::send_it("leader").context("tell the leader"),
        // RUNNING A COMMAND BY NAME, from outside the process.
        //
        // It is what section 13 asks for — every feature addressable by name
        // — and it is what the emergency binds use: the `Ctrl+Shift+L` that
        // restores the keyboard cannot depend on the leader, precisely because
        // it exists for when the leader does not respond.
        //
        // It does NOT execute here: it sends to the footer that is already running. A
        // process lasting an instant has no surface, does not have the registry
        // loaded and would have nowhere to echo the result.
        Some("command") => {
            let mut args = std::env::args().skip(2);
            let id = args
                .next()
                .context("usage: ronoctua-shell command <id> [argument]")?;
            let rest: Vec<String> = args.collect();
            let line = if rest.is_empty() {
                format!("command {id}")
            } else {
                format!("command {id} {}", rest.join(" "))
            };
            return leader::send_it(&line).with_context(|| format!("mandar `{id}`"));
        }
        // WHAT A BUFFER DECLARES, asked of the buffer's own process.
        //
        // The footer cannot resolve a buffer's keymap: a buffer is a window of
        // its own (invariant 4) and holds the keyboard while focused, so the
        // bindings live over there. These two subcommands are that boundary made
        // reachable by hand — the same shape as `command <id>` above, and the
        // way to tell "the buffer is not publishing" apart from "the footer is
        // not asking" without a screenshot.
        Some("buffer-keymap") => {
            let pid = std::env::args()
                .nth(2)
                .context("uso: ronoctua-shell buffer-keymap <pid>")?
                .parse::<u32>()
                .context("the pid has to be a number")?;
            for (keys, id) in leader::buffer_keymap(pid).context("ler o keymap do buffer")? {
                println!("{keys} {id}");
            }
            return Ok(());
        }
        Some("buffer-command") => {
            let mut args = std::env::args().skip(2);
            let pid = args
                .next()
                .context("uso: ronoctua-shell buffer-command <pid> <id>")?
                .parse::<u32>()
                .context("the pid has to be a number")?;
            let id = args
                .next()
                .context("uso: ronoctua-shell buffer-command <pid> <id>")?;
            return leader::buffer_command(pid, &id)
                .with_context(|| format!("mandar `{id}` ao buffer {pid}"));
        }
        // It opens a native buffer in a window of its OWN. What decides where it
        // goes is niri (invariant 4) — the shell does not tile.
        Some("buffer") => {
            let path = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell buffer <component.wasm> [file]")?;
            // THE FILE IS OPTIONAL, and belongs to the HOST. The component never
            // learns a path: it receives the content through `Input::Paste` and
            // asks to write with `Effect::Save`, which is what lets an editor
            // exist without giving a buffer a filesystem (invariant 11).
            let file = std::env::args().nth(3);
            return buffer::host_it(
                std::path::Path::new(&path),
                file.as_deref().map(std::path::Path::new),
            )
            .context("hospedar o buffer");
        }
        // It emits the CHORD binds in KDL, for niri's config.
        //
        // It exists so the chord does not become a second table: what it
        // does is in `actions.rs`, and the KDL comes from there. The prefix is an argument
        // because the leader is `Mod` in production and `Alt` in the nested loop.
        Some("export-niri-binds") => {
            let prefix = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell export-niri-binds <prefix>")?;
            for chord in actions::full_chords() {
                for (key, kdl) in chord.binds {
                    // `Shift+1` becomes `Mod+Shift+1`: the prefix goes in
                    // front, and the Shift already came with the key.
                    println!("    {prefix}+{key}{} {{ {kdl}; }}", repeat_of(&kdl));
                }
            }
            // THE EMERGENCY BINDS come out WITHOUT the leader's prefix — and that is what
            // makes them useful when it is the leader that is broken. They come from the
            // same table for the same reason as the chords: written by hand in the
            // two configs, one day they would diverge, and the day one finds out is
            // the day nothing works any more.
            for (key, kdl, _) in actions::emergency() {
                println!("    {key}{} {{ {kdl}; }}", repeat_of(&kdl));
            }
            return Ok(());
        }
        // The `window-rule`s, for the same reason as the binds above: the
        // monitor's `app_id` is set in `terminal.rs` and read by the
        // compositor's config, and a string written at both ends drifts. The two
        // configs (`nix/modules/niri.nix` and `nix/dev.nix`) take this in.
        Some("export-niri-window-rules") => {
            print!("{}", terminal::floating_rules());
            return Ok(());
        }
        // The process for an X11 buffer's scope: it starts ITS satellite and
        // runs the app inside. It is not meant to be called by hand — the footer's
        // `run_command` uses it; it exists as a subcommand because the scope needs ONE
        // process that spawns both into the same cgroup (section 9.1).
        Some("satellite") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let (display, rest) = args
                .split_first()
                .context("usage: ronoctua-shell satellite <display> <program> [args]")?;
            let display: u8 = display
                .parse()
                .with_context(|| format!("`{display}` is not a display number"))?;
            let (program, args) = rest
                .split_first()
                .context("usage: ronoctua-shell satellite <display> <program> [args]")?;
            return foreign::satellite_of(display, program, args).context("satellite");
        }
        // A terminal in a window of its own. With no argument it starts `$SHELL`;
        // with one, it runs what is asked — and it is what the VM test uses so as not
        // to depend on some shell's prompt.
        // `ronoctua-shell files [path]` — the manager in a window of its
        // own. The SAME `files::View` the footer draws from: what differs is
        // only the window and the events of it, which is what makes "one
        // implementation, two doors" true rather than claimed.
        // `ronoctua-shell notifications` — the manager in a window of its own.
        // The SAME `notifications::View` the footer draws from: what differs is
        // the window and its events, which is what makes "one implementation,
        // two doors" true rather than claimed.
        Some("notifications") => {
            return notifications_window::host_it().context("host the manager");
        }
        Some("files") => {
            let where_ = std::env::args().nth(2).map(std::path::PathBuf::from);
            return files_window::host_it(where_.as_deref()).context("host the manager");
        }
        // `ronoctua-shell update` — the update, live, in a window of its own.
        //
        // It is the door the modeline's mark opens, and it takes no argument
        // BECAUSE it may name no step: what it draws is the declared table the
        // brain already carries and the two journals the units write. It opens
        // with nothing running too — a run that has finished is exactly when
        // somebody wants to read what it did.
        Some("update") => {
            return update_window::host_it().context("hospedar a atualizacao");
        }
        Some("terminal") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let (program, rest) = args
                .split_first()
                .map_or((None, &[][..]), |(p, r)| (Some(p.as_str()), r));
            return terminal::host_it(program, rest).context("hospedar o terminal");
        }
        // THE SAME TERMINAL, under the monitor's `app_id`. It is a subcommand and
        // not a flag on the one above because what changes is the window's
        // IDENTITY, which is what niri's rule reads to float it — see
        // `terminal::APP_ID_MONITOR`. The program is required: a monitor with no
        // monitor to run is a floating window with a shell in it.
        Some("monitor") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let (program, rest) = args
                .split_first()
                .context("usage: ronoctua-shell monitor <program> [args]")?;
            return terminal::host_monitor(program, rest).context("hospedar o monitor");
        }
        // THE RESCUE CONSOLE, and a third identity for the same terminal.
        //
        // `Ctrl+Alt+Del` reaches this, and so does the VT unit — the difference
        // is that over there nobody goes through the shell at all. That is the
        // whole point of the console being a terminal application: this
        // subcommand is a convenience for the door that has a compositor, not
        // the way in.
        Some("rescue") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let (program, rest) = args
                .split_first()
                .context("usage: ronoctua-shell rescue <program> [args]")?;
            return terminal::host_rescue(program, rest).context("hospedar o resgate");
        }
        // It checks the keymap without bringing anything up. It exists so whoever edits their own
        // keymap gets the same verdict `nix flake check` gives.
        Some("check-keymap") => {
            let (tree, registry) = leader::default_of().context("keymap")?;
            // REACHABILITY, and not only collisions. A command with no key at all does not
            // collide with anything — and that is how it goes unnoticed.
            let reachable = leader::reachable(&tree);
            // The compositor's 141 MIRRORED actions are reachable by NAME
            // and by name only — giving them keys would spend the whole keymap on
            // what is almost never used. It is the only exception, and it is
            // recognisable by its prefix.
            let orphans: Vec<&str> = registry
                .all_ones()
                .map(|c| c.id)
                .filter(|id| !id.starts_with("niri-") && !reachable.contains(id))
                .collect();
            anyhow::ensure!(
                orphans.is_empty(),
                "commands with no key at all (no sequence, no chord, no emergency): {}",
                orphans.join(", ")
            );
            println!(
                "keymap OK: {} bindings, {} chords, {} commands, all reachable",
                tree.amount(),
                actions::chords().len(),
                registry.amount()
            );
            return Ok(());
        }
        _ => {}
    }

    tracing::info!(
        face = tokens::font::FAMILY_MONO,
        size = tokens::font::SIZE_BASE,
        cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE),
        line = tokens::metrics::line_height(tokens::font::SIZE_BASE),
        footer = tokens::metrics::footer_height(tokens::font::SIZE_BASE),
        "bringing the footer up"
    );

    surface::run().context("the footer's surface failed")
}

/// The bind's `repeat` property, when it needs to be stated.
///
/// # Why it is not always `false`
///
/// niri repeats a bind while the key is held, and that is **desirable**
/// for what the compositor applies: holding `LEADER+←` keeps moving the focus,
/// as holding an arrow in an editor keeps moving. What must not repeat
/// is what **spawns a process** — holding `LEADER+RET` would open a queue of
/// terminals, and holding the alternative leader would reopen the sequence on every
/// repeat, which is the defect that was being fixed.
///
/// The rule comes from the action itself, and not from an extra column in the table: if it
/// starts with `spawn`, it creates a process.
fn repeat_of(kdl: &str) -> &'static str {
    if kdl.starts_with("spawn ") {
        " repeat=false"
    } else {
        ""
    }
}

/// How the leader appears on screen.
///
/// # Why it comes from the ENVIRONMENT
///
/// It used to be a fixed `"SPC"`, and that was wrong in both modes: in the nested loop the
/// key is `Ctrl+Alt+Space` (the `Mod` belongs to the host's compositor) and in
/// production it is `Super` ON ITS OWN, tapped. The screen said `SPC` in both — a
/// label that corresponds to no key does not help, it gets in the way.
///
/// Whoever defines the bind defines the label: `nix/dev.nix` in the nested loop,
/// `nix/modules/niri.nix` in production. A single truth.
///
/// Without the variable, it falls back to `SPC` — Doom's notation, which is where the
/// concept comes from, and which serves whoever runs the binary by hand.
#[must_use]
pub fn leader() -> String {
    std::env::var("RONOCTUA_LEADER").unwrap_or_else(|_| "SPC".to_owned())
}

/// How the CHORD's key appears on screen.
///
/// # Why it is another variable
///
/// In production it is the same key as the sequence's — tapping Super opens the list,
/// holding Super is the modifier — and it is that coincidence `keyd` buys.
/// In the nested loop it is **not**: the sequence is `Ctrl+Alt+Space` and the chord is `Alt`
/// alone. Deriving one from the other would make the screen say one holds `CTRL ALT SPC`,
/// and holding Space along with it is exactly what fires the sequence — the
/// opposite of what the chord needs.
///
/// Without the variable, it falls back to the leader's label: outside the two known modes,
/// repeating what one knows is better than inventing a second key.
#[must_use]
pub fn leader_chord() -> String {
    std::env::var("RONOCTUA_LEADER_CHORD").unwrap_or_else(|_| leader())
}

/// The modeline's segments, with section 4.2's priority order.
///
/// `name`, `mode`, `state` and `unread` are PARAMETERS and not reads of global
/// state on purpose: that way the layout stays a pure function of (width,
/// segments) and every combination can be tested without a compositor.
///
/// `unread` is what the BELL counts, and it is deliberately not the queue's
/// length: a notification a filter suppressed is not shown and is not counted
/// either — a badge that counts what the person asked never to see is a badge
/// that cannot be brought to zero.
///
/// # What left here, and why
///
/// **The mode indicator (`NORMAL`).** It came from Doom's evil-mode, and
/// this system has no modal editing: there is no state to indicate. While the
/// leader is active, what there is to show is the SEQUENCE — and its place is
/// the minibuffer, not a second place on screen.
///
/// **The absence label (`fundamental`).** With no focused window there is no mode,
/// and writing an invented mode name is worse than writing nothing: it
/// takes up a column and asserts something false. The segment only exists when there is a buffer.
#[must_use]
pub fn segments(
    name: &str,
    mode: &str,
    state: &str,
    unread: usize,
    transfer: Option<&str>,
    update: Option<&str>,
) -> Vec<Segment> {
    let mut out = vec![
        // 2. Never disappears: the only indicator that something is pending.
        //
        // # One cell between the glyph and the number
        //
        // Glued (`{glyph}{n}`), they were reported from the screen as too close
        // to read — and the measurement says why: a chrome glyph is drawn at
        // `font.size.icon` and its ink leaves the cell by up to 1,2 px, while
        // the digit beside it only begins 1,44 px into its own. They do not
        // touch, and 0,24 px apart is indistinguishable from touching.
        //
        // One cell, not two: two is `space.modeline.segmentGap`, which is what
        // separates one SEGMENT from the next. Spending the same gap inside a
        // segment would say these are two things.
        //
        // # And the bell gains a dot when something is unread
        //
        // `notification-pending` existed in `icons.nix` since the icon layer
        // and had never been drawn. A zero next to a bell answers the question,
        // but only after the number is read; the dot answers it before.
        Segment {
            id: "notif",
            text: format!(
                "{} {unread}",
                if unread > 0 {
                    tokens::icon::NOTIFICATION_PENDING
                } else {
                    tokens::icon::NOTIFICATION
                }
            ),
            priority: 2,
            min_cols: 3,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: false,
        },
    ];
    // 3. THE TRANSFERS, next to the bell — and it goes away when there is
    // nothing to say.
    //
    // # Why `removable`, when the bell is not
    //
    // They answer different questions. The bell is the ONLY indicator that
    // something is pending, so a screen without it has to be able to say
    // "nothing is waiting" — it may shrink, never vanish. This one appears
    // because something is happening; with nothing happening it would be a
    // column spent on `0%`, on every screen, forever.
    //
    // So on a screen too narrow for both, the bell wins. That is the right way
    // round: a copy at 62% is on the panel and in the log, and a notification
    // nobody was told about is nowhere.
    if let Some(t) = transfer {
        out.push(Segment {
            id: "transfer",
            text: t.to_owned(),
            priority: 3,
            // The icon and two digits. Below that there is no segment worth
            // having: `6…` is not a percentage.
            min_cols: 5,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // 5. THE UPDATE, and it is a mark rather than a measurement.
    //
    // One cell, one glyph, and it ANIMATES — see `update_window::FRAMES`. It
    // appears while the machine is bringing its own software forward and goes
    // away the moment it is not, which is why it is `removable` like the
    // transfer and unlike the bell.
    //
    // # Why it gives way BEFORE the transfer and not after
    //
    // Because a rebuild is announced everywhere else too: it has the brain's
    // own line under the minibuffer, its own screen, its own window and its
    // own journal. A copy at 62% has this segment and a panel. On a line too
    // narrow for both, the one with fewer other ways of being seen stays.
    //
    // And it never outranks the BELL, which is the only indicator that
    // something is pending and the one thing on this line that must be the
    // last to go.
    if let Some(u) = update {
        out.push(Segment {
            id: "update",
            text: u.to_owned(),
            priority: 5,
            // One glyph. There is nothing narrower that still says anything,
            // and there is nothing wider worth saying here: a percentage of a
            // rebuild is a number nobody can compute honestly.
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // The name only exists if there is a buffer. `removable: false` still holds
    // for when it does exist: then it elides in the middle, but does not disappear.
    if !name.is_empty() {
        out.push(Segment {
            id: "buffer",
            text: name.to_owned(),
            priority: 4,
            min_cols: 8,
            side: Side::Left,
            shrink: Shrink::InTheMiddle,
            removable: false,
        });
    }
    // The mode only exists if there is a buffer with a mode. See the header.
    if !mode.is_empty() {
        out.push(Segment {
            id: "mode",
            text: mode.to_owned(),
            priority: 6,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // 7. THE STATE — what the buffer is DOING, next to what kind of buffer it is.
    //
    // It only exists for a buffer of ours. A foreign window has no state we can
    // know: the compositor gives a title, an app id and a pid, and none of the
    // three says what `vim` is in the middle of. Writing `normal` for it would
    // be the same false assertion the `fundamental` label used to make.
    //
    // It dies BEFORE the mode (7 against 6) and both die before the buffer's
    // name. On a screen too narrow for all three, what survives is which buffer
    // this is — a person who cannot see the state can press `Esc` and know; a
    // person who cannot see the name has nothing to press.
    if !state.is_empty() {
        out.push(Segment {
            id: "state",
            text: state.to_owned(),
            priority: 7,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    out
}

/// What the minibuffer shows at rest.
///
/// At rest it stays empty: the line exists, but does not invent content. It
/// only gains text when there is a prompt, a message or completion (section 4.1).
#[must_use]
pub fn minibuffer_at_rest() -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_render::layout;

    #[test]
    fn the_segments_fit_on_a_narrow_screen() {
        // 40 columns is a cramped terminal; nothing may spill. The buffer's
        // name now comes from the compositor, so it can be ABSURD — a browser
        // title easily goes past 200 characters.
        let name = "a".repeat(300);
        for columns in [20_u16, 40, 80, 200] {
            for c in layout::resolve(
                columns,
                &segments(
                    &name,
                    "ghostty",
                    "search",
                    99,
                    Some("\u{ebcb} 62%"),
                    Some(&crate::update_window::frame_of(u64::from(columns)).to_string()),
                ),
            ) {
                assert!(
                    c.column + layout::width(&c.text) <= columns,
                    "'{}' leaks at {columns} columns",
                    c.id
                );
            }
        }
    }

    #[test]
    fn the_minibuffer_at_rest_is_empty() {
        // The line always exists (the footer has TWO lines, section 4.1), but
        // does not invent content.
        assert!(minibuffer_at_rest().is_empty());
    }

    #[test]
    fn one_cell_separates_the_bell_from_the_count() {
        // ONE, and not two: two is `segmentGap`, which is what separates one
        // segment from the next — using it inside a segment would say the bell
        // and the number are two different things.
        let s = segments("", "", "", 3, None, None);
        let notif = s.iter().find(|x| x.id == "notif").unwrap();
        let cells: Vec<&str> = notif.text.split(' ').collect();
        assert_eq!(
            cells.len(),
            2,
            "{:?} is not a glyph, a space, a number",
            notif.text
        );
        assert_eq!(cells[1], "3");
    }

    #[test]
    fn the_bell_gains_a_dot_when_something_is_unread() {
        let with = segments("", "", "", 1, None, None);
        let without = segments("", "", "", 0, None, None);
        let of = |v: &[Segment]| {
            v.iter()
                .find(|x| x.id == "notif")
                .unwrap()
                .text
                .chars()
                .next()
                .unwrap()
        };
        assert_eq!(of(&with), tokens::icon::NOTIFICATION_PENDING);
        assert_eq!(of(&without), tokens::icon::NOTIFICATION);
    }

    #[test]
    fn the_update_mark_is_a_column_only_while_something_runs() {
        // IT COSTS NOTHING AT REST. The bell has to be able to say "nothing is
        // waiting" and therefore never disappears; this one appears because
        // something is happening, and at rest it would be a column spent on a
        // still glyph on every screen in the session, forever.
        let quiet = segments("a.rs", "rust", "", 0, None, None);
        assert!(!quiet.iter().any(|s| s.id == "update"));
        let busy = segments(
            "a.rs",
            "rust",
            "",
            0,
            None,
            Some(&crate::update_window::frame_of(0).to_string()),
        );
        let mark = busy
            .iter()
            .find(|s| s.id == "update")
            .expect("the mark has to be there");
        // One glyph, and it is one of the twelve — never the countdown's pie,
        // which answers the opposite question.
        assert_eq!(layout::width(&mark.text), 1);
        assert!(
            crate::update_window::FRAMES.contains(&mark.text.chars().next().unwrap()),
            "the mark did not come from the clock's lap"
        );
        // AND THE BELL OUTLIVES IT on every width worth drawing. A screen too
        // narrow for both keeps the only indicator that something is pending.
        for columns in [20_u16, 30, 40, 80] {
            let placed = layout::resolve(columns, &busy);
            let has = |id: &str| placed.iter().any(|p| p.id == id);
            assert!(has("notif"), "the bell went away at {columns} columns");
            if has("update") {
                assert!(has("notif"), "the mark survived the bell");
            }
        }
    }

    #[test]
    fn the_icons_come_from_a_token() {
        // Invariant 9: no icon codepoint outside the tokens. If
        // someone writes a literal here, this test does not catch it — but the
        // generator has already guaranteed that the token exists in the font.
        let s = segments("arq.rs", "rust", "", 0, None, None);
        let notif = s.iter().find(|x| x.id == "notif").unwrap();
        assert!(notif.text.starts_with(tokens::icon::NOTIFICATION));
    }
}
