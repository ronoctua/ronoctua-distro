//! The modeline's segments — what the footer's line draws, in which order
//! they give way on a narrow screen (section 4.2) — and the minibuffer at
//! rest, with the name the footer's surface goes by.
//!
//! # Why here
//!
//! They lived in `main.rs`, the binary's own file: the footer asked the
//! binary how to draw its own line, and a footer that is a crate of its own
//! (shell-data-design §1.6) cannot. What the line SAYS is still resolved by
//! whoever knows each daemon and handed in as [`Marks`]; this module only
//! places it.

use ronoctua_render::layout::{Segment, Shrink, Side};
use ronoctua_tokens as tokens;

/// The layer-shell surface's namespace.
///
/// It is what the layer surface calls itself on the wire. Nothing in the
/// compositor's configuration matches on it today — the previous one had a
/// `layer-rule` keyed by this string, and this one has no equivalent that we
/// use — so it is an identity rather than a join, and it is the name a
/// `WAYLAND_DEBUG` trace shows when one is looking for our surface.
pub const NAMESPACE: &str = "ronoctua-footer";

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
    pub devices: &'a [crate::bluez_words::Shown],
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
    /// Hand gestures are on: the camera reads hands (`LEADER m g`).
    pub gestures: bool,
    /// The screen is cut by the resizer (`LEADER / r`): what everything is
    /// laid out in is not the whole monitor.
    pub resizer: bool,
    /// Which build is running, on a machine that builds from a local checkout:
    /// the machine's own, or a distro checkout's REFERENCE build (`LEADER / b`,
    /// `build_switch`). `None` on an installed machine — nothing to switch to,
    /// and a modeline at rest says nothing.
    pub build: Option<crate::build_switch::Build>,
    /// A build switch is APPLYING: the build glyph becomes the switching mark
    /// (`sync`) until it lands. Takes precedence over [`Marks::build`], which
    /// it hides while on — the two are never drawn at once (`build_switch`).
    pub build_switching: bool,
    /// The player has something loaded: the play/pause glyph of its pair,
    /// which says what a click does — pause or play (`player_mark::mark`). Absent with
    /// nothing loaded — a modeline at rest says nothing.
    pub player: Option<char>,
    /// The numbered workspaces that hold something — empty when fewer than
    /// two do (`workspaces::shown`, where the owner's rule lives).
    pub workspaces: &'a [crate::workspaces::Shown],
    /// How many windows the drawer holds — zero draws nothing.
    pub drawer: usize,
    /// The selection, when something is selected: how many windows, when
    /// that can be counted (`crate::selection`).
    pub selection: Option<Option<usize>>,
    /// The compositor's trails.
    pub trails: ronoctua_compositor::Trails,
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
    /// A tunnel is up — `Some` with where it goes (a country, for a registered
    /// location; empty for an unnamed tunnel), `None` when none is up.
    pub tunnel: Option<&'a str>,
    /// This machine is on its tailnet.
    pub tailscale: bool,
    /// Tor has a circuit.
    pub tor: bool,
    /// What the brain's modules marked, each as it published it — already
    /// drawn by `brain::marks`, and empty at rest (invariant 14).
    pub brain: &'a [crate::brain::Drawn],
    // ── end of brain-ui ──
    /// The passwordless window's end, drawn as it is — the unlocked glyph and
    /// the hour — while one is open (`security::modeline_until`).
    pub security_until: Option<&'a str>,
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
    // THE PLAYER, the FARTHEST of the right-hand marks from the edge — only
    // the workspace numbers sit to its left (the owner's order). The rung is
    // what places a mark on this side, lowest against the border, so it is
    // also the first of them to go on a line too narrow for everything: the
    // sound itself says it is there.
    //
    // A PAIR IN ONE SEGMENT — play/pause, a cell of air, next
    // (`player_mark`) — so the layout keeps both or drops both.
    if let Some(glyph) = marks.player {
        out.push(Segment {
            id: "player",
            text: crate::player_mark::text(glyph),
            priority: 16,
            // BOTH GLYPHS AND THE AIR, or nothing: it is never cut, so its
            // floor is its whole width.
            min_cols: crate::player_mark::CELLS,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE WORKSPACES THAT HOLD SOMETHING, one number each: one segment per
    // number because a segment has one colour, and the current one is drawn
    // brighter.
    //
    // THE FIRST OF THE RIGHT-HAND MARKS, reading left to right, and to the
    // left of the player (the owner's order): the highest rung on this side,
    // since the rung is what places a mark here. Counted from the EDGE, so a
    // click that changes the buffer — and with it the name's length — leaves
    // them where the hand is; on the left, after the name, they slid four
    // columns away from it (measured in the photo battery).
    //
    // PUSHED FROM THE LAST TO THE FIRST: one rung for all of them, and a tie
    // keeps the order of declaration from the edge inwards, so the highest
    // number is the one against the player and they read `1 3 4`. They are
    // the first to go on a narrow screen — each is also `LEADER 1`…`9`.
    // THE DRAWER, while it holds something: the windows put away have no
    // focus and no number, and this is where the pointer finds them. The
    // numbers' rung, pushed before them so it sits between the numbers and
    // the player — it is about windows too.
    if marks.drawer > 0 {
        out.push(Segment {
            id: "drawer",
            text: tokens::icon::DRAWER.to_string(),
            priority: 17,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // PUSHED FROM THE EDGE INWARDS, all on the numbers' rung, so they read
    // left to right: the workspace numbers, the trails, the selection, the
    // drawer — then the player. Each is about windows, as the numbers are.
    //
    // THE SELECTION, while something is selected, in the green of the
    // selected outline, with how many when this footer can count them.
    if let Some(count) = marks.selection {
        out.push(Segment {
            id: "selection",
            text: match count {
                Some(n) => format!("{} {n}", tokens::icon::WINDOWS_SELECTED),
                None => tokens::icon::WINDOWS_SELECTED.to_string(),
            },
            priority: 17,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE TRAILS, while one exists: the glyph with how many windows the one
    // in use holds, then — from two trails on, as the workspaces from two —
    // one number per trail, the one in use drawn brighter. Pushed from the
    // last, so they read `1 2 3` against the glyph.
    if marks.trails.count > 1 {
        for n in (1..=marks.trails.count).rev() {
            let Some(id) = crate::trails::id_of(n) else {
                continue;
            };
            out.push(Segment {
                id,
                text: n.to_string(),
                priority: 17,
                min_cols: 1,
                side: Side::Right,
                shrink: Shrink::Cut,
                removable: true,
            });
        }
    }
    if marks.trails.count > 0 {
        out.push(Segment {
            id: "trail",
            text: format!("{} {}", tokens::icon::TRAIL, marks.trails.marked),
            priority: 17,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    for shown in marks.workspaces.iter().rev() {
        let Some(id) = crate::workspaces::id_of(shown.number) else {
            continue;
        };
        out.push(Segment {
            id,
            text: shown.number.to_string(),
            priority: 17,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE CAMERA, between the player and the devices — the owner's order: the
    // two marks that say the camera is LOOKING (this and the gestures', in
    // their own faded pink) sit together, after the workspace numbers and
    // the player and before the Bluetooth marks. The rung places a mark on
    // this side, so it is the rung that moved: from the microphone's 2 to
    // just above the devices' 10–13.
    if marks.camera {
        out.push(Segment {
            id: "camera",
            text: tokens::icon::CAMERA_IN_USE.to_string(),
            priority: 14,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // HAND GESTURES, beside the camera and to its left: the camera is open
    // FOR THIS, and the mark is also the way to turn it off.
    if marks.gestures {
        out.push(Segment {
            id: "gestures",
            text: tokens::icon::GESTURES.to_string(),
            priority: 15,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // THE RESIZER, at the top rung and beside the gestures, whose colour it
    // wears: the screen is not what it looks like, and the lines the tool
    // itself makes narrow are where that has to stay said. FIRST of its rung,
    // because a line drops the LAST mark of a rung first: at twenty columns
    // the key mode's long name goes and this glyph stays.
    if marks.resizer {
        out.push(Segment {
            id: "resizer",
            text: tokens::icon::RESIZER.to_string(),
            priority: 1,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }
    // WHICH BUILD IS RUNNING, on a machine that builds from a LOCAL checkout.
    // Both marks are this machine's own system, with its host.nix and hardware;
    // they differ only in the distro code underneath, and both wear the dev
    // colour. The committed checkout is a `git-commit` mark; the working tree,
    // with its uncommitted edits, is a flask. Two dev builds, and the way
    // between them is one gesture (`build_switch`, `LEADER / b`). The installed,
    // released system has no checkout, nothing to switch to, and draws no mark
    // here — the `nix` snowflake stays reserved for it, off the two dev marks.
    // While a switch is APPLYING it is the switching mark instead (`sync`): the
    // apply takes minutes, the toggle is refused meanwhile, and this says why on
    // sight rather than leaving the old glyph looking idle.
    if marks.build_switching {
        out.push(Segment {
            id: "build-switching",
            text: tokens::icon::SYNC.to_string(),
            priority: 1,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    } else if let Some(build) = marks.build {
        let (id, glyph) = match build {
            crate::build_switch::Build::Own => ("build-own", tokens::icon::GIT_COMMIT),
            crate::build_switch::Build::Reference => {
                ("build-reference", tokens::icon::SOFTWARE_TRIAL)
            }
        };
        out.push(Segment {
            id,
            text: glyph.to_string(),
            priority: 1,
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
    if let Some(place) = marks.tunnel {
        out.push(Segment {
            id: "vpn",
            // THE COUNTRY BESIDE THE GLYPH, when the tunnel is a registered
            // location (`US · NYC`); `Shrink::Cut` drops back to the glyph alone
            // when the line is tight.
            text: if place.is_empty() {
                tokens::icon::VPN.to_string()
            } else {
                format!("{} {place}", tokens::icon::VPN)
            },
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
        let Some(id) = crate::bluez_words::POOL.get(i) else {
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

    // WHAT THE BRAIN'S MODULES MARKED, `brain:<module>/<mark>` — the disk
    // nearly full, a reminder ringing, a service that keeps failing. Beside the
    // profile and at its rung: each is a fact about the machine that stays
    // until it stops being true, and none of them is louder than the battery.
    // Drawn only while a module publishes one: at rest there is nothing here.
    // A mark's instances in the order they read, `1 2 3` from the left
    // (`brain::from_the_edge`).
    for mark in crate::brain::from_the_edge(marks.brain) {
        out.push(Segment {
            id: mark.id,
            text: mark.text.clone(),
            priority: 9,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
        });
    }

    // THE PASSWORDLESS WINDOW'S END, beside the brain's shield: the HOUR it
    // ends, never a countdown. Absent at rest (invariant 14).
    if let Some(text) = marks.security_until {
        out.push(Segment {
            id: "security-until",
            text: text.to_owned(),
            priority: 8,
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

    /// FAILS IF: a workspace number moves when the focused buffer changes —
    /// clicking one changes the buffer, and a number that slides with the
    /// name's length leaves the hand that clicked it over something else.
    #[test]
    fn the_workspace_numbers_do_not_move_with_the_buffer() {
        let shown = [
            crate::workspaces::Shown {
                number: 1,
                current: true,
            },
            crate::workspaces::Shown {
                number: 3,
                current: false,
            },
        ];
        let marks = Marks {
            workspaces: &shown,
            ..Marks::default()
        };
        let at = |name: &str| {
            layout::resolve(80, &segments(name, "power", "", 0, &marks))
                .into_iter()
                .filter(|c| crate::workspaces::number_of(c.id).is_some())
                .map(|c| c.column)
                .collect::<Vec<_>>()
        };
        let short = at("*Power*");
        assert_eq!(short.len(), 2, "both numbers are drawn");
        assert_eq!(short, at("*a much longer buffer name*"));
        // AND THEY READ IN ORDER, to the left of the player's mark.
        let with_player = Marks {
            workspaces: &shown,
            player: Some(tokens::icon::PLAYER),
            ..Marks::default()
        };
        let placed = layout::resolve(80, &segments("*Power*", "power", "", 0, &with_player));
        let column = |id: &str| {
            placed
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.column)
                .unwrap_or_else(|| panic!("`{id}` was not placed"))
        };
        assert!(column("ws-1") < column("ws-3"), "the numbers read `1 3`");
        assert!(
            column("ws-3") < column("player"),
            "the numbers are left of the player"
        );
    }
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
        let devices: Vec<crate::bluez_words::Shown> = crate::bluez_words::POOL
            .iter()
            .enumerate()
            .map(|(i, at)| crate::bluez_words::Shown {
                at: (*at).to_owned(),
                text: if i == 0 {
                    format!(
                        "{} 12%",
                        crate::bluez_words::glyph(ronoctua_bluetooth::Kind::Keyboard)
                    )
                } else {
                    crate::bluez_words::glyph(ronoctua_bluetooth::Kind::Headphones).to_string()
                },
                alarmed: i == 0,
            })
            .collect();
        // AND TWO MARKS OF THE BRAIN'S MODULES, one with a count: a disk
        // nearly full and a service that keeps failing are on the fullest
        // line as well.
        let brain_marks: Vec<crate::brain::Drawn> = [("full", 0_u32), ("errors", 12)]
            .into_iter()
            .map(|(name, count)| crate::brain::Drawn {
                id: if count > 0 {
                    "brain:fixture/errors"
                } else {
                    "brain:fixture/full"
                },
                module: "fixture".to_owned(),
                text: if count > 0 {
                    format!("{} {count}", tokens::icon::BRAIN)
                } else {
                    tokens::icon::BRAIN.to_string()
                },
                tone: ronoctua_brain::module::Tone::Danger,
                hint: format!("fixture-mark-{name}"),
                count,
                act: None,
                opens: None,
                instance: 0,
                label: None,
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
                        update: Some(&crate::update_mark::frame_of(u64::from(columns)).to_string()),
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
                        gestures: true,
                        // AND THE SCREEN CUT, which is when lines get narrow.
                        resizer: true,
                        // AND A KEY MODE, the longest of their names: the
                        // fullest line has to survive it beside the rest.
                        compositor_mode: Some("alternar largura"),
                        player: Some(tokens::icon::PLAYER),
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
                        tunnel: Some(""),
                        tailscale: true,
                        tor: true,
                        workspaces: &[],
                        // AND THE DRAWER HOLDING SOMETHING, a selection and
                        // three trails: more marks the fullest line carries.
                        drawer: 3,
                        selection: Some(Some(2)),
                        trails: ronoctua_compositor::Trails {
                            count: 3,
                            active: Some(0),
                            marked: 4,
                        },
                        brain: &brain_marks,
                        // AND THE PASSWORDLESS WINDOW'S END, the glyph and
                        // an hour: a mark the fullest line has to carry too.
                        security_until: Some(&format!(
                            "{} 23:45",
                            tokens::icon::SECURITY_SUDO_OPEN
                        )),
                        // AND THE BUILD MARK, on a machine that builds from a
                        // checkout: the fullest line carries it beside the rest.
                        build: Some(crate::build_switch::Build::Reference),
                        // Not switching — the build glyph and the switching mark
                        // are never drawn at once, so the fuller case is one mark.
                        build_switching: false,
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

    /// FAILS IF: the resizer's mark leaves the line the resizer itself made
    /// narrow — twenty columns, every mark on, and the screen is still said
    /// to be cut.
    #[test]
    fn the_resizer_mark_survives_the_narrowest_line() {
        let devices: Vec<crate::bluez_words::Shown> = crate::bluez_words::POOL
            .iter()
            .map(|at| crate::bluez_words::Shown {
                at: (*at).to_owned(),
                text: crate::bluez_words::glyph(ronoctua_bluetooth::Kind::Headphones).to_string(),
                alarmed: false,
            })
            .collect();
        let placed = layout::resolve(
            20,
            &segments(
                "main.rs",
                "ghostty",
                "search",
                9,
                &Marks {
                    transfer: Some(&format!("{} 62%", tokens::icon::TRANSFER)),
                    battery: Some(&format!("{} 12%", tokens::icon::BATTERY_DANGER)),
                    profile: Some(&tokens::icon::PROFILE_POWER_SAVER.to_string()),
                    devices: &devices,
                    silent: true,
                    listening: true,
                    camera: true,
                    recording: true,
                    gestures: true,
                    resizer: true,
                    compositor_mode: Some("alternar largura"),
                    player: Some(tokens::icon::PLAYER),
                    syncthing: Some(false),
                    clip: Some(false),
                    history: Some(false),
                    adrift: true,
                    tunnel: Some(""),
                    tailscale: true,
                    tor: true,
                    drawer: 3,
                    selection: Some(Some(2)),
                    ..Marks::default()
                },
            ),
        );
        let ids: Vec<&str> = placed.iter().map(|c| c.id).collect();
        assert!(ids.contains(&"resizer"), "the cut went unsaid: {ids:?}");
        let mark = placed.iter().find(|c| c.id == "resizer").unwrap();
        assert!(mark.text.starts_with(tokens::icon::RESIZER));
    }

    #[test]
    fn the_bell_survives_a_deskful_of_devices() {
        // THE POOL IS REMOVABLE AND THE BELL IS NOT ABOUT TO BE: four device
        // marks must not push the one thing on this line that says something
        // was missed. `priority` is what decides it, and this is the test
        // that reads the decision rather than the number.
        let devices: Vec<crate::bluez_words::Shown> = crate::bluez_words::POOL
            .iter()
            .map(|at| crate::bluez_words::Shown {
                at: (*at).to_owned(),
                text: format!(
                    "{} 12%",
                    crate::bluez_words::glyph(ronoctua_bluetooth::Kind::Headphones)
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
                update: Some(&crate::update_mark::frame_of(0).to_string()),
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
            crate::update_mark::FRAMES.contains(&mark.text.chars().next().unwrap()),
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
