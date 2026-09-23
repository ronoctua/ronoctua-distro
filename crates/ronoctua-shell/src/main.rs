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
//! - **It does not tile windows.** What does that is the compositor
//!   (invariant 4). One `xdg_toplevel` per native buffer, and the compositor
//!   decides where they go.
//! - **It does not talk to the compositor through anything other than IPC or
//!   layer-shell** (invariant 3). There is no fork and no patch of it, and no
//!   crate of ours but `ronoctua-scroll` knows which compositor it is.
//! - **It does not draw a fullscreen surface reimplementing layout.** That
//!   would throw the compositor away and is forbidden (section 7.5).
//!
//! # This milestone's state
//!
//! Milestone 4: the surface comes up in the right font and the modeline reflects the focused
//! buffer, fed by the compositor's event stream. The leader and which-key arrive in 5;
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
mod audio;
mod audio_window;
mod audio_words;
mod band;
mod bluetooth;
mod bluetooth_window;
mod bluetooth_words;
// THE BRAIN'S LINE, under the minibuffer, and the thread that feeds it.
mod brain;
mod buffer;
mod carry;
mod chrome;
mod clipboard;
mod clipboard_view;
mod daemon;
mod disks;
mod display;
mod display_window;
mod display_words;
mod files;
mod files_window;
mod fold;
mod foreign;
mod geo;
mod geo_window;
mod globe;
mod hidpi;
mod history;
mod history_window;
mod keyboard;
mod keys_link;
mod language;
mod leader;
// THE LINE THAT OUTLIVES THE FOOTER — raised by systemd when it fails.
mod lifeline;
mod locale;
mod locale_window;
mod manual;
// The pure half of the brain bridge — the daemon's records turned into the
// WIT's. Split out so `ronoctua-render-png` can link it without linking the
// footer's line, which is what made the brain photographable at all.
mod camera;
mod camera_settings;
mod camera_settings_window;
mod camera_window;
mod mind_wire;
mod minibuffer;
mod motion;
mod network;
mod network_window;
mod network_words;
mod notifications;
mod notifications_window;
mod paint;
mod pointers;
mod pointers_window;
mod power;
mod power_window;
mod power_words;
mod seats;
// A NAME TOO LONG FOR ITS COLUMN, walking sideways a cell at a time.
mod slide;
mod sound;
mod stage;
mod surface;
mod syncthing;
mod syncthing_window;
mod terminal;
// WHICH THEME THIS PROCESS DRAWS IN, and how the doors hear about a change.
mod theme;
mod theme_chooser;
// THE FILE MANAGER'S THUMBNAILS: the workers that make them and what has been
// made. A pool and not the preview's single thread, because a grid wants every
// answer and the panel wants only the newest.
mod thumbs;
mod toolkits;
mod transfers;
mod world;
// `LEADER s u`, the animated mark on the modeline, and the window that watches
// a run happen. It knows no step: the sequence is `assets/updates.nix`.
mod update_window;

/// The layer-shell surface's namespace.
///
/// It is what the layer surface calls itself on the wire. Nothing in the
/// compositor's configuration matches on it today — the previous one had a
/// `layer-rule` keyed by this string, and this one has no equivalent that we
/// use — so it is an identity rather than a join, and it is the name a
/// `WAYLAND_DEBUG` trace shows when one is looking for our surface.
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

    // THE THEME, before any door: every process of this binary — the footer,
    // the windows, a buffer's host — draws through the same three sources,
    // read once here. The short-lived subcommands pay one file read.
    theme::resolve_at_start();
    // AND THE DATE'S SHAPE, by the same means and at the same moment: it is
    // a process-wide value the way the theme is, so that a date resolves
    // through it wherever one is written. See `locale::prefs::current`.
    locale::prefs::adopt();
    // AND THE SIZE OF THE TEXT, on the same footing and for the same reason.
    // It is not the theme's business — a palette and a size are two choices —
    // but they are read at the same moment because they are both properties
    // of the process rather than of any one door.
    toolkits::resolve_at_start();

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
            return leader::send_it(&line).with_context(|| format!("send `{id}`"));
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
                .context("usage: ronoctua-shell buffer-keymap <pid>")?
                .parse::<u32>()
                .context("the pid has to be a number")?;
            for (keys, id) in leader::buffer_keymap(pid).context("read the buffer's keymap")? {
                println!("{keys} {id}");
            }
            return Ok(());
        }
        Some("buffer-command") => {
            let mut args = std::env::args().skip(2);
            let pid = args
                .next()
                .context("usage: ronoctua-shell buffer-command <pid> <id>")?
                .parse::<u32>()
                .context("the pid has to be a number")?;
            let id = args
                .next()
                .context("usage: ronoctua-shell buffer-command <pid> <id>")?;
            return leader::buffer_command(pid, &id)
                .with_context(|| format!("send `{id}` to buffer {pid}"));
        }
        // It opens a native buffer in a window of its OWN. What decides where it
        // goes is the compositor (invariant 4) — the shell does not tile.
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
            .context("host the buffer");
        }
        // It emits the CHORD binds, for the compositor's config.
        //
        // It exists so the chord does not become a second table: what it does
        // is in `actions.rs`, and the command text comes from there. The
        // prefix is an argument because the leader is `Mod` in production and
        // `Alt` in the nested loop.
        Some("export-binds") => {
            let prefix = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell export-binds <prefix>")?;
            for chord in actions::full_chords() {
                for (key, said) in chord.binds_for(&prefix) {
                    // `Shift+1` becomes `Mod+Shift+1`: the prefix goes in
                    // front, and the Shift already came with the key.
                    println!(
                        "bindsym{}{} {prefix}+{key} {said}",
                        repeat_of(said),
                        region_of(key)
                    );
                }
            }
            // THE EMERGENCY BINDS come out WITHOUT the leader's prefix — and that is what
            // makes them useful when it is the leader that is broken. They come from the
            // same table for the same reason as the chords: written by hand in the
            // two configs, one day they would diverge, and the day one finds out is
            // the day nothing works any more.
            for (key, said, _) in actions::emergency() {
                println!("bindsym{}{} {key} {said}", repeat_of(&said), region_of(key));
            }
            // AND THE MEDIA KEYS, bare for a different reason — see
            // `actions::media`. They come out of the same table for the same
            // reason as everything above: a key written in two configs is a
            // key that one day differs between them.
            for (key, said, _) in actions::media() {
                println!("bindsym{}{} {key} {said}", repeat_of(&said), region_of(key));
            }
            // THE COMPOSITOR'S MODES, from the same table the footer draws
            // their keys from — see `actions::modes`. Inside one the keys are
            // bare, and `Esc` always leaves.
            for mode in actions::modes() {
                println!("mode \"{}\" {{", mode.name);
                for k in &mode.keys {
                    println!("    bindsym {} {}", k.key, k.said);
                }
                println!("    bindsym Escape mode \"default\"");
                println!("}}");
            }
            for line in actions::gestures(&prefix) {
                println!("{line}");
            }
            return Ok(());
        }
        // The window rules, for the same reason as the binds above: the
        // monitor's `app_id` is set in `terminal.rs` and read by the
        // compositor's config, and a string written at both ends drifts. The
        // two configs (`nix/modules/scroll.nix` and `nix/dev.nix`) take this in.
        Some("export-window-rules") => {
            print!("{}", terminal::floating_rules());
            return Ok(());
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
        // `ronoctua-shell power` — the power view in a window of its own. The
        // SAME `power::View` the footer draws from.
        // THE RADIO'S SECOND DOOR, one device to a line and a fact to a
        // row — the footer's capped region is a glance, not a reading.
        Some("bluetooth") => {
            return bluetooth_window::host_it().context("host the bluetooth view");
        }
        // THE SCREENS' SECOND DOOR, one monitor to a sheet of eighteen rows —
        // the footer's capped region is a glance, not a reading.
        Some("display") => {
            return display_window::host_it().context("host the screens view");
        }
        Some("power") => {
            return power_window::host_it().context("host the power view");
        }
        Some("history") => {
            return history_window::host_it();
        }
        Some("syncthing") => {
            return syncthing_window::host_it().context("host the syncthing view");
        }
        Some("audio") => {
            return audio_window::host_it().context("host the sound view");
        }
        // MICE, TOUCHPADS AND EVERY CURSOR, in a window of their own.
        Some("camera") => {
            return camera_window::host_it().context("the camera app");
        }
        Some("camera-settings") => {
            return camera_settings_window::host_it().context("the camera settings window");
        }
        Some("pointers") => {
            return pointers_window::host_it().context("host the pointers view");
        }
        Some("locale") => {
            return locale_window::host_it().context("host the language and region view");
        }
        Some("geo") => {
            return geo_window::host_it().context("host the regions' view");
        }
        // `ronoctua-shell network` — the network in a window of its own. The
        // SAME `network::View` the footer draws from; what it cannot do here
        // is ask for a password, because a window has no minibuffer, and the
        // row says so rather than pretending.
        Some("network") => {
            return network_window::host_it().context("host the network view");
        }
        // `ronoctua-shell network-facts` — what the view would draw, as lines
        // a test can read.
        //
        // IT EXISTS FOR THE VM, and it is honest about that: a virtual machine
        // has no screen worth photographing and no radio at all, so what it
        // can prove about this view is the READINGS — the daemon's answer, the
        // aeroplane, whether the two tunnels are there, how many sockets are
        // listening. Each line is `key=value`, which is what a test asserts on
        // and what a person reads when the screen says something surprising.
        Some("network-facts") => {
            let net = ronoctua_network::Manager::fresh()
                .map(|m| m.look())
                .unwrap_or_default();
            println!("daemon={}", net.daemon);
            println!("way={}", net.way.id());
            println!("networking={}", net.networking);
            println!("airplane={}", net.airplane());
            println!("radio={}", net.radio);
            println!("links={}", net.links.len());
            for l in &net.links {
                println!(
                    "link={} medium={:?} state={:?} address={}",
                    l.interface,
                    l.medium,
                    l.state,
                    l.address.as_deref().unwrap_or("none")
                );
            }
            println!("saved={}", net.saved.len());
            for a in &net.active {
                println!("active={} kind={} tunnel={}", a.id, a.kind, a.tunnel());
            }
            let tunnels = ronoctua_tunnel::look(&ronoctua_tunnel::Places::default());
            match tunnels.tailscale.as_ref() {
                Some(t) => println!("tailscale={} running={}", t.state, t.running),
                None => println!("tailscale=absent running=false"),
            }
            match tunnels.tor.as_ref() {
                Some(t) => println!(
                    "tor=present established={} bootstrap={}",
                    t.established, t.bootstrap
                ),
                None => println!("tor=absent established=false bootstrap=0"),
            }
            let places = ronoctua_memory::Places::default();
            let sockets = ronoctua_memory::net::sockets(&places);
            println!(
                "listening={}",
                sockets.iter().filter(|s| s.listening).count()
            );
            for s in sockets.iter().filter(|s| s.listening) {
                println!(
                    "port={} {} owner={}",
                    s.protocol.label(),
                    s.named(),
                    s.owner.as_ref().map_or("unknown", |o| o.program.as_str())
                );
            }
            println!(
                "neighbours={}",
                ronoctua_memory::net::neighbours(&places).len()
            );
            println!(
                "vendors={}",
                ronoctua_lan::oui::table()
                    .map(|t| ronoctua_lan::oui::Vendors::read(&t).len())
                    .unwrap_or(0)
            );
            return Ok(());
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
            return update_window::host_it().context("host the update");
        }
        Some("terminal") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let (program, rest) = args
                .split_first()
                .map_or((None, &[][..]), |(p, r)| (Some(p.as_str()), r));
            return terminal::host_it(program, rest).context("host the terminal");
        }
        // THE SAME TERMINAL, under the monitor's `app_id`. It is a subcommand and
        // not a flag on the one above because what changes is the window's
        // IDENTITY, which is what the compositor's rule reads to float it — see
        // `terminal::APP_ID_MONITOR`. The program is required: a monitor with no
        // monitor to run is a floating window with a shell in it.
        Some("monitor") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            let (program, rest) = args
                .split_first()
                .context("usage: ronoctua-shell monitor <program> [args]")?;
            return terminal::host_monitor(program, rest).context("host the monitor");
        }
        // THE LINE THAT OUTLIVES THE FOOTER.
        //
        // Raised by `OnFailure=` when `ronoctua-footer.service` fails, and by
        // the nested loop's starter when the child exits badly. It is a
        // subcommand of this binary for the reason the ten window doors are:
        // everything it needs — the renderer, the chrome, the tokens, both
        // languages — is already here, and a crate of its own would be a second
        // closure carrying the same wgpu for a process that lives seconds.
        //
        // It does NOT go through the control socket. That socket belongs to the
        // footer, and this exists precisely when the footer does not answer —
        // which is also why all four emergency binds are dead in that moment.
        Some("lifeline") => {
            let unit = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell lifeline <unit>")?;
            return lifeline::host_it(&unit).context("raise the lifeline");
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
            return terminal::host_rescue(program, rest).context("host the rescue console");
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
            // what is almost never used. It is recognisable by its prefix.
            //
            // AND THE THREE NAMED IN `leader::BY_NAME_ONLY`, which carries
            // the reason for each. It is that constant and not a literal here
            // because the TEST beside it reads the same list, and two literals
            // would be two answers the day one of them moved.
            let by_name_only = leader::BY_NAME_ONLY;
            let orphans: Vec<&str> = registry
                .all_ones()
                .map(|c| c.id)
                .filter(|id| {
                    !id.starts_with("wm-") && !by_name_only.contains(id) && !reachable.contains(id)
                })
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
        size = tokens::text::size(),
        cell = tokens::metrics::cell_width(tokens::text::size()),
        line = tokens::metrics::line_height(tokens::text::size()),
        footer = tokens::metrics::footer_height(tokens::text::size()),
        "bringing the footer up"
    );

    surface::run().context("the footer's surface failed")
}

/// The bind's `repeat` property, when it needs to be stated.
///
/// # Why it is not always `false`
///
/// A bind repeats while the key is held, and that is **desirable** for what
/// the compositor applies: holding `LEADER+←` keeps moving the focus, as
/// holding an arrow in an editor keeps moving. What must not repeat is what
/// **starts a process** — holding `LEADER+RET` would open a queue of
/// terminals, and holding the alternative leader would reopen the sequence on
/// every repeat, which is the defect that was being fixed.
///
/// The rule comes from the command itself, and not from an extra column in
/// the table: if it starts with `exec`, it creates a process.
///
/// THE FLAG GOES BEFORE THE KEY, which is why this returns a prefix rather
/// than a suffix: `bindsym --no-repeat Ctrl+Alt+space exec …`.
fn repeat_of(said: &str) -> &'static str {
    if said.starts_with("exec ") {
        " --no-repeat"
    } else {
        ""
    }
}

/// A MOUSE BIND REACHES THE TITLE BAR AND NOTHING ELSE, unless it says so.
///
/// That is this compositor's default (`scroll.5`: "mouse bindings are only
/// triggered when over the title bar"), and this distro draws NO title bar —
/// `default_border pixel` is the whole decoration, and invariant 15 is why.
/// So the two wheel chords would be bound to a region that does not exist on
/// screen: the config would load clean, the manual would teach them, and
/// scrolling with the leader held would do nothing anywhere.
///
/// `--whole-window` also makes them work over an empty workspace, which is
/// where somebody scrolling between workspaces most often is.
///
/// BY THE KEY AND NOT BY THE COMMAND, unlike `repeat_of` above: what decides
/// is whether the thing pressed is a BUTTON, and the compositor's own spelling
/// for one is `button1`…`button9` or a `BTN_` event name.
fn region_of(key: &str) -> &'static str {
    let button = key
        .rsplit('+')
        .next()
        .is_some_and(|k| k.starts_with("button") || k.starts_with("BTN_"));
    if button { " --whole-window" } else { "" }
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
/// `nix/modules/scroll.nix` in production. A single truth.
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

/// The right-hand side's optional marks, as text — each `None` at rest.
///
/// A struct and not four parameters, because there were four and clippy is
/// right that a call with eight positional arguments is a call somebody
/// will get in the wrong order. Every field is what the segment DRAWS,
/// already resolved by whoever knows the daemon: this function only places.
#[derive(Debug, Clone, Copy, Default)]
pub struct Marks<'a> {
    /// The transfer queue's `glyph NN%`, while something is moving.
    pub transfer: Option<&'a str>,
    /// The update's animated mark, while a rebuild runs.
    pub update: Option<&'a str>,
    /// The battery's glyph (and number), while there is one to show.
    pub battery: Option<&'a str>,
    /// The profile's glyph, off the machine's top profile.
    pub profile: Option<&'a str>,
    /// One mark per CONNECTED bluetooth device, four at most — already
    /// resolved by `bluetooth::modeline`, which is where the owner's rule
    /// about the percentage and the danger colour lives.
    pub devices: &'a [crate::bluetooth::Shown],
    /// The machine is SILENT — the default sink is muted.
    ///
    /// It asks the default and not "any sink": a muted HDMI output nobody is
    /// using is not the machine being silent. At any other volume there is no
    /// mark at all, because a modeline at rest says nothing.
    pub silent: bool,
    /// Something is RECORDING — some program holds a capture stream open.
    ///
    /// A privacy mark before it is an audio one, and derived from PipeWire's
    /// graph rather than guessed: the node exists exactly while the stream
    /// does.
    pub listening: bool,
    /// A CAMERA IS OPEN — any program, found in `/proc`. A privacy mark, the
    /// microphone's twin.
    pub camera: bool,
    /// The studio is recording or streaming.
    pub recording: bool,
    /// The compositor's key mode, by its name, while one is on.
    pub compositor_mode: Option<&'a str>,
    /// SYNCTHING, and it is the only mark whose very existence is a choice.
    ///
    /// `None` draws nothing, which is what an ordinary day looks like: the
    /// default is to say something only when nothing is travelling. `Some(true)`
    /// is "it is syncing" and `Some(false)` is "it is not" — two glyphs, and
    /// only ever one of them, because the person chose which question they
    /// wanted answered. See `ronoctua_syncthing::prefs::Mark`.
    pub syncthing: Option<bool>,
    /// The clipboard's sync, when there is anything to say about it.
    ///
    /// The same three-word ring, and the same silence: a machine that never
    /// told ClipCascade where a server is draws nothing at all. See
    /// `ronoctua_clipcascade::prefs::Mark`.
    pub clip: Option<bool>,
    /// The shell history's sync, likewise — `ronoctua_atuin::prefs::Mark`.
    pub history: Option<bool>,
    /// A link that leads NOWHERE — the portal, the router with no uplink.
    /// Not "there is no internet": nothing connected is not a fault.
    pub adrift: bool,
    /// Networking off, by somebody's own decision.
    pub airplane: bool,
    /// A tunnel is up.
    pub tunnel: bool,
    /// This machine is on its tailnet.
    pub tailscale: bool,
    /// Tor has a circuit.
    pub tor: bool,
}

/// The modeline's segments, with section 4.2's priority order.
///
/// `name`, `mode`, `state` and `unread` are PARAMETERS and not reads of global
/// state on purpose: that way the layout stays a pure function of (width,
/// segments) and every combination can be tested without a compositor.
///
/// `unread` is what decides whether the BELL is there at all, and it is
/// deliberately not the queue's length: a notification a filter suppressed is
/// not shown and is not counted either — a badge that counts what the person
/// asked never to see is a badge that cannot be brought to zero.
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
    marks: &Marks<'_>,
) -> Vec<Segment> {
    let Marks {
        transfer,
        update,
        battery,
        profile,
        devices,
        ..
    } = *marks;
    let mut out = Vec::new();
    // 2. THE BELL, and only while something is UNREAD.
    //
    // It used to be a glyph and a count, on every screen, forever — a `0`
    // beside a bell answering a question nobody had asked. The owner asked for
    // the opposite: nothing at rest, and the dotted bell alone when something
    // is waiting to be read. The NUMBER went with the rest of it: how many is
    // the manager's question, and the pointer answers it on the minibuffer
    // without opening anything (`Footer::segment_hint`).
    //
    // `removable`, now that it can be absent — but at priority 2, so on a line
    // too narrow for everything it is the LAST of the right-hand marks to go.
    // A copy at 62% is on the panel and in the log; a notification nobody was
    // told about is nowhere.
    if unread > 0 {
        out.push(Segment {
            id: "notif",
            text: tokens::icon::NOTIFICATION_PENDING.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // 3. THE TRANSFERS, next to the bell — and it goes away when there is
    // nothing to say.
    //
    // # Why the bell outranks it
    //
    // They answer different questions, and on a screen too narrow for both the
    // bell wins (2 against 3). That is the right way round: a copy at 62% is on
    // the panel and in the log, and a notification nobody was told about is
    // nowhere. With nothing happening this one is not there at all — it would
    // be a column spent on `0%`, on every screen, forever.
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
    // 8. THE BATTERY, and 9. THE PROFILE — both absent at rest by the owner's
    // rule: no glyph at 99% and above, none at the machine's top profile.
    // Below the bell, the transfer and the update in priority: the battery
    // has the catalogue's alarms as a second voice and the profile has the
    // view; the bell has nothing else.
    //
    // `Cut` AND `min_cols` NEVER APPLY TO IT, and this used to say they did
    // ("`Cut` drops the ` NN%` before the glyph"). `layout::resolve` REMOVES a
    // removable segment whole while the line overflows and only shrinks what
    // is left, so a `removable: true` segment is drawn at its natural width or
    // not at all. The two fields are the shape every segment has, and on this
    // one they are inert — which is worth knowing, because it means a narrow
    // line loses the battery ENTIRELY at priority 8, danger colour and all,
    // before it loses the bell, the transfer or the update. That order is the
    // one this file argues for, and it is the order that happens.
    if let Some(b) = battery {
        out.push(Segment {
            id: "battery",
            text: b.to_owned(),
            priority: 8,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // SILENCE, AND THE MICROPHONE. Two marks and no ladder: a volume is
    // drawn nowhere at rest, because a machine that is simply playing at some
    // volume has nothing to report. What IS worth a cell is the machine
    // having gone quiet — a key that seems not to work is usually this — and
    // something holding the microphone open, which is a privacy fact and the
    // only way to see it without opening the view.
    //
    // They sit at priority 2 with the network's two faults, which is the rung
    // for "a thing is not as you would assume".
    if marks.silent {
        out.push(Segment {
            id: "muted",
            text: tokens::icon::VOLUME_MUTE.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE CLIPBOARD'S SYNC AND THE SHELL HISTORY'S, at the same rung and for
    // the same reason as Syncthing: "a thing is not as you would assume".
    // Which of the two glyphs each is was decided where the preference is
    // read; here they are only placed.
    if let Some(syncing) = marks.clip {
        out.push(Segment {
            id: "clip",
            text: if syncing {
                tokens::icon::CLIP_ON.to_string()
            } else {
                tokens::icon::CLIP_OFF.to_string()
            },
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if let Some(syncing) = marks.history {
        out.push(Segment {
            id: "history",
            text: if syncing {
                tokens::icon::SHELL_HISTORY_ON.to_string()
            } else {
                tokens::icon::SHELL_HISTORY_OFF.to_string()
            },
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // SYNCTHING, at the same rung and for the same reason: "a thing is not as
    // you would assume". Which of the two glyphs it is was decided where the
    // preference is read; here it is only placed.
    if let Some(syncing) = marks.syncthing {
        out.push(Segment {
            id: "syncthing",
            text: if syncing {
                tokens::icon::SYNCTHING.to_string()
            } else {
                tokens::icon::SYNCTHING_OFF.to_string()
            },
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if marks.listening {
        out.push(Segment {
            id: "microphone",
            text: tokens::icon::MICROPHONE.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE CAMERA, at the microphone's rung and for its reason: something is
    // WATCHING, which is worth a cell whatever else the line has to say.
    if marks.camera {
        out.push(Segment {
            id: "camera",
            text: tokens::icon::CAMERA_IN_USE.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // A KEY MODE, at the top rung: while it is on every key means something
    // else, and a person who does not know that is a person whose keys have
    // stopped working.
    if let Some(name) = marks.compositor_mode {
        let text = format!("{} {name}", tokens::icon::KEYBOARD);
        out.push(Segment {
            id: "compositor-mode",
            min_cols: ronoctua_protocol::cells(&text),
            text,
            priority: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // AND RECORDING, one rung above: a recording that nobody remembers is
    // running fills a disk and films a room.
    if marks.recording {
        out.push(Segment {
            id: "recording",
            text: tokens::icon::RECORDING.to_string(),
            priority: 1,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE WAY OUT, before everything else on this side: whether anything
    // reaches the world is the first question a modeline answers, and the
    // two faults are the only ones drawn — a machine that is simply not
    // connected shows nothing, because nothing is wrong with it.
    //
    // The aeroplane and the fault cannot both be true (`Snapshot::adrift`
    // asks for networking to be ON), so the two never crowd each other.
    if marks.airplane {
        out.push(Segment {
            id: "airplane",
            text: tokens::icon::AIRPLANE.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if marks.adrift {
        out.push(Segment {
            id: "net-off",
            text: tokens::icon::NO_INTERNET.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if marks.tunnel {
        out.push(Segment {
            id: "vpn",
            text: tokens::icon::VPN.to_string(),
            priority: 5,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // AND THE OTHER TWO TUNNELS, beside the first and at its priority: they
    // answer the same question — is this machine going out through something
    // else — and they can all three be true at once, which is why each has a
    // mark of its own rather than one shared glyph.
    //
    // RUNNING, and not installed: a tailnet nobody has logged into and a Tor
    // that is not started draw nothing at all. Nothing at rest.
    if marks.tailscale {
        out.push(Segment {
            id: "tailscale",
            text: tokens::icon::TAILSCALE.to_string(),
            priority: 5,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if marks.tor {
        out.push(Segment {
            id: "tor",
            text: tokens::icon::TOR.to_string(),
            priority: 5,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE DEVICES, after the battery and the profile: they are about things
    // on the desk, and the machine's own state comes first.
    //
    // THE MARKS DO MOVE, and this used to say they did not ("each one keeps
    // its place in the pool"). The id is `POOL[i]` where `i` is the index in
    // `Scene::connected()`, which sorts ALARMED FIRST and then by kind — so
    // headphones connecting at 12% take `bt-1` and push every other mark
    // along, and a disconnect pulls them back.
    //
    // That is the RIGHT order — the one about to die is the one to see — and
    // the id is a pool slot rather than the device's path for the reason
    // written over `POOL`: `Segment::id` is `&'static str`. What was wrong
    // was the sentence, not the arithmetic.
    for (i, shown) in devices.iter().enumerate() {
        let Some(id) = crate::bluetooth::POOL.get(i) else {
            break;
        };
        out.push(Segment {
            id,
            text: shown.text.clone(),
            priority: 10 + u8::try_from(i).unwrap_or(0),
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    if let Some(p) = profile {
        out.push(Segment {
            id: "profile",
            text: p.to_owned(),
            priority: 9,
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
        // AND THE LINE IS AS FULL AS IT EVER GETS: the queue, an update, the
        // battery with its number, a profile off the top, and four devices —
        // one of them with a percentage of its own. Anything that fits here
        // fits.
        let devices: Vec<crate::bluetooth::Shown> = crate::bluetooth::POOL
            .iter()
            .enumerate()
            .map(|(i, at)| crate::bluetooth::Shown {
                at: (*at).to_owned(),
                text: if i == 0 {
                    format!(
                        "{} 12%",
                        crate::bluetooth::glyph(ronoctua_bluetooth::Kind::Keyboard)
                    )
                } else {
                    crate::bluetooth::glyph(ronoctua_bluetooth::Kind::Headphones).to_string()
                },
                alarmed: i == 0,
            })
            .collect();
        for columns in [20_u16, 40, 80, 200] {
            for c in layout::resolve(
                columns,
                &segments(
                    &name,
                    "ghostty",
                    "search",
                    99,
                    &Marks {
                        // THE TOKENS AND NOT THE CODEPOINTS. Invariant 9
                        // names `codepoint` explicitly, and a test written in
                        // literals stays green on the old drawing the day a
                        // glyph moves in `icons.nix` — which is exactly what
                        // this test is for.
                        transfer: Some(&format!("{} 62%", tokens::icon::TRANSFER)),
                        update: Some(
                            &crate::update_window::frame_of(u64::from(columns)).to_string(),
                        ),
                        battery: Some(&format!("{} 12%", tokens::icon::BATTERY_DANGER)),
                        profile: Some(&tokens::icon::PROFILE_POWER_SAVER.to_string()),
                        devices: &devices,
                        // AND THE TWO SOUND MARKS, both at once: a machine can
                        // be muted while something records, and the narrowest
                        // line has to survive the pair.
                        silent: true,
                        listening: true,
                        camera: true,
                        recording: true,
                        // AND A KEY MODE, the longest of their names: the
                        // fullest line has to survive it beside the rest.
                        compositor_mode: Some("alternar largura"),
                        // AND SYNCTHING'S, which the fullest line has to
                        // carry too: it is a mark somebody chose to have, so
                        // the worst case is a line that has it AND everything
                        // else.
                        syncthing: Some(false),
                        // THE TWO SYNC MARKS, on the fullest line this test
                        // measures: they sit at the same rung as Syncthing's
                        // and have to fit beside it.
                        clip: Some(false),
                        history: Some(false),
                        // AND THE WAY OUT'S TWO THAT CAN SHOW AT ONCE: the
                        // aeroplane and the fault are exclusive by rule, so
                        // the fullest line carries the fault and a tunnel.
                        adrift: true,
                        airplane: false,
                        // AND ALL THREE TUNNELS AT ONCE, which is a real
                        // machine: a VPN inside a tailnet with Tor on top is
                        // three marks, and the narrowest line has to survive
                        // them.
                        tunnel: true,
                        tailscale: true,
                        tor: true,
                    },
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
    fn the_bell_survives_a_deskful_of_devices() {
        // THE POOL IS REMOVABLE AND THE BELL IS NOT ABOUT TO BE: four device
        // marks must not push the one thing on this line that says something
        // was missed. `priority` is what decides it, and this is the test
        // that reads the decision rather than the number.
        let devices: Vec<crate::bluetooth::Shown> = crate::bluetooth::POOL
            .iter()
            .map(|at| crate::bluetooth::Shown {
                at: (*at).to_owned(),
                text: format!(
                    "{} 12%",
                    crate::bluetooth::glyph(ronoctua_bluetooth::Kind::Headphones)
                ),
                alarmed: true,
            })
            .collect();
        for columns in [20_u16, 30, 40, 80] {
            let placed = layout::resolve(
                columns,
                &segments(
                    "buffer",
                    "ghostty",
                    "normal",
                    3,
                    &Marks {
                        devices: &devices,
                        ..Marks::default()
                    },
                ),
            );
            let ids: Vec<&str> = placed.iter().map(|c| c.id).collect();
            assert!(
                ids.contains(&"notif"),
                "at {columns} columns the devices pushed the bell off: {ids:?}"
            );
        }
    }

    #[test]
    fn the_minibuffer_at_rest_is_empty() {
        // The line always exists (the footer has TWO lines, section 4.1), but
        // does not invent content.
        assert!(minibuffer_at_rest().is_empty());
    }

    #[test]
    fn the_bell_is_only_there_while_something_is_unread() {
        // NOTHING AT REST. A bell with a `0` beside it was a mark on every
        // screen answering a question nobody had asked; the owner asked for
        // silence, and for the dotted bell alone when something is waiting.
        let quiet = segments("", "", "", 0, &Marks::default());
        assert!(
            !quiet.iter().any(|s| s.id == "notif"),
            "a bell with nothing unread"
        );
        let with = segments("", "", "", 3, &Marks::default());
        let notif = with.iter().find(|x| x.id == "notif").unwrap();
        // ONE GLYPH AND NO NUMBER: how many is the manager's question, and the
        // pointer answers it on the minibuffer without opening anything.
        assert_eq!(
            layout::width(&notif.text),
            1,
            "{:?} is not one glyph",
            notif.text
        );
        assert_eq!(
            notif.text.chars().next().unwrap(),
            tokens::icon::NOTIFICATION_PENDING
        );
    }

    #[test]
    fn the_update_mark_is_a_column_only_while_something_runs() {
        // IT COSTS NOTHING AT REST: it appears because something is happening,
        // and at rest it would be a column spent on a still glyph on every
        // screen in the session, forever.
        let quiet = segments("a.rs", "rust", "", 0, &Marks::default());
        assert!(!quiet.iter().any(|s| s.id == "update"));
        // ONE UNREAD, so that the bell is on the line and the second half of
        // this test has something to outlive the mark.
        let busy = segments(
            "a.rs",
            "rust",
            "",
            1,
            &Marks {
                update: Some(&crate::update_window::frame_of(0).to_string()),
                ..Marks::default()
            },
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
        // narrow for both keeps the one that says something is waiting to be
        // read — the mark has the brain's own line, a screen and a window; the
        // bell has nothing else.
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
        let s = segments("arq.rs", "rust", "", 1, &Marks::default());
        let notif = s.iter().find(|x| x.id == "notif").unwrap();
        assert!(notif.text.starts_with(tokens::icon::NOTIFICATION_PENDING));
    }
}
